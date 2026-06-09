//! In-memory file index built by enumerating the configured folders (FR5),
//! optionally respecting `.gitignore` (spec 002 FR1–FR4).

use std::collections::HashSet;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use ignore::{DirEntry, WalkBuilder};

/// One indexed file.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Absolute path (inserted as `@"<abs>"`).
    pub abs: PathBuf,
    /// The configured root this file was found under.
    pub root: PathBuf,
    /// Path relative to `root`, used for matching and display.
    pub rel: String,
    /// Index of `root` in the configured `folders` list. Lower wins ranking
    /// ties (spec 002 FR5 — folder priority).
    pub root_rank: usize,
    /// Byte size at index time, shown on result rows without a content read
    /// (spec 010 FR1). `0` when metadata was unavailable.
    pub size: u64,
    /// Last-modified (unix secs) at index time; keys the enrichment cache
    /// (spec 010 TC3). `0` when metadata was unavailable.
    pub mtime: u64,
}

impl Entry {
    /// File name, for the primary (normal-weight) display text.
    pub fn name(&self) -> &str {
        self.abs
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(self.rel.as_str())
    }

    /// Parent directory relative to root, for the dim secondary text.
    pub fn parent_rel(&self) -> &str {
        match self.rel.rfind(['\\', '/']) {
            Some(i) => &self.rel[..i],
            None => "",
        }
    }

    /// Name of the configured root folder this file came from.
    pub fn root_name(&self) -> &str {
        self.root.file_name().and_then(|n| n.to_str()).unwrap_or("")
    }

    /// Dim secondary text for a result row: the source folder, plus the parent
    /// directory within it, so results from different configured folders are
    /// distinguishable (roadmap #30).
    pub fn location(&self) -> String {
        let folder = self.root_name();
        let parent = self.parent_rel().replace('\\', "/");
        if parent.is_empty() {
            folder.to_string()
        } else {
            format!("{folder}/{parent}")
        }
    }
}

/// Build the index over `folders`. Always skips hidden files and prunes
/// `exclude`d directory names. When `git_aware` and a folder is inside a Git
/// working tree, paths ignored by Git (`.gitignore`, `.git/info/exclude`, the
/// global gitignore) are skipped — but untracked, non-ignored files are still
/// indexed. Symlinks are not followed; a given absolute path appears once,
/// under its earliest-listed root.
pub fn build(folders: &[PathBuf], exclude: &[String], git_aware: bool) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    for (root_rank, root) in folders.iter().enumerate() {
        let exclude_owned = exclude.to_vec();
        let walker = WalkBuilder::new(root)
            .follow_links(false)
            .hidden(true)
            .parents(git_aware)
            .git_ignore(git_aware)
            .git_global(git_aware)
            .git_exclude(git_aware)
            .ignore(false)
            .filter_entry(move |e| !is_excluded_dir(e, &exclude_owned))
            .build();
        for entry in walker.flatten() {
            let path = entry.path();
            let is_file = entry.file_type().map(|ft| ft.is_file()).unwrap_or(false);
            if !is_file {
                continue;
            }
            // One metadata read serves the hidden check and size/mtime capture.
            let md = entry.metadata().ok();
            if is_hidden(path, md.as_ref()) {
                continue;
            }
            let abs = path.to_path_buf();
            if !seen.insert(abs.clone()) {
                continue;
            }
            let rel = abs
                .strip_prefix(root)
                .unwrap_or(&abs)
                .to_string_lossy()
                .into_owned();
            let (size, mtime) = md.map(|m| (m.len(), mtime_secs(&m))).unwrap_or((0, 0));
            entries.push(Entry {
                abs,
                root: root.clone(),
                rel,
                root_rank,
                size,
                mtime,
            });
        }
    }
    entries
}

/// Prune a directory whose name matches an `exclude` entry.
fn is_excluded_dir(entry: &DirEntry, exclude: &[String]) -> bool {
    entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
        && entry
            .file_name()
            .to_str()
            .map(|name| exclude.iter().any(|ex| ex == name))
            .unwrap_or(false)
}

/// A file is hidden if its name is dot-prefixed or it carries the Windows
/// hidden attribute. (`ignore`'s `hidden(true)` covers dot-prefixed names but
/// not the NTFS hidden attribute, so this overlay preserves FR4 parity.)
/// Takes the walker's already-fetched metadata to avoid a second stat.
fn is_hidden(path: &Path, md: Option<&Metadata>) -> bool {
    let dot_prefixed = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with('.'))
        .unwrap_or(false);
    if dot_prefixed {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        if let Some(md) = md {
            return md.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0;
        }
    }
    #[cfg(not(windows))]
    let _ = md;
    false
}

/// Last-modified as unix seconds, best-effort (`0` if unavailable).
pub(crate) fn mtime_secs(md: &Metadata) -> u64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(dir: &Path, name: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(name), b"x").unwrap();
    }

    #[test]
    fn excludes_named_dirs_and_hidden_files() {
        // AC9: hidden files and .git / node_modules / target are excluded.
        // git_aware: false so this exercises the plain-walk + exclude + hidden path
        // without requiring a Git repo (git-aware filtering is covered in tests/).
        let tmp = std::env::temp_dir().join("atref_test_idx_excludes");
        let _ = fs::remove_dir_all(&tmp);
        touch(&tmp, "keep.md");
        touch(&tmp.join("sub"), "nested.txt");
        touch(&tmp.join(".git"), "config");
        touch(&tmp.join("node_modules"), "lib.js");
        touch(&tmp.join("target"), "out.bin");
        fs::write(tmp.join(".hidden"), b"x").unwrap();

        let exclude = vec![
            ".git".to_string(),
            "node_modules".to_string(),
            "target".to_string(),
        ];
        let names: Vec<String> = build(std::slice::from_ref(&tmp), &exclude, false)
            .iter()
            .map(|e| e.name().to_string())
            .collect();

        assert!(names.contains(&"keep.md".to_string()));
        assert!(names.contains(&"nested.txt".to_string()));
        assert!(!names.contains(&"config".to_string()));
        assert!(!names.contains(&"lib.js".to_string()));
        assert!(!names.contains(&"out.bin".to_string()));
        assert!(!names.contains(&".hidden".to_string()));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn indexes_multiple_folders_dedupes_and_ranks() {
        // AC10: files from every configured folder are searchable; no dupes;
        // each entry records its folder's rank (FR5).
        let base = std::env::temp_dir().join("atref_test_idx_multi");
        let _ = fs::remove_dir_all(&base);
        let a = base.join("a");
        let b = base.join("b");
        touch(&a, "alpha.md");
        touch(&b, "beta.md");

        let idx = build(&[a.clone(), b.clone()], &[], false);
        let by_name = |n: &str| idx.iter().find(|e| e.name() == n).unwrap();
        assert_eq!(by_name("alpha.md").root_rank, 0);
        assert_eq!(by_name("beta.md").root_rank, 1);

        // The same root listed twice still yields each file once.
        let twice = build(&[a.clone(), a.clone()], &[], false);
        assert_eq!(twice.iter().filter(|e| e.name() == "alpha.md").count(), 1);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn location_shows_source_folder() {
        // Roadmap #30: the dim secondary text names the configured folder so
        // results from different folders are distinguishable.
        let top = Entry {
            abs: PathBuf::from(r"D:\dev\second-brain\note.md"),
            root: PathBuf::from(r"D:\dev\second-brain"),
            rel: "note.md".to_string(),
            root_rank: 0,
            size: 0,
            mtime: 0,
        };
        assert_eq!(top.location(), "second-brain");

        let nested = Entry {
            abs: PathBuf::from(r"D:\dev\atref\specs\001.md"),
            root: PathBuf::from(r"D:\dev\atref"),
            rel: r"specs\001.md".to_string(),
            root_rank: 1,
            size: 0,
            mtime: 0,
        };
        assert_eq!(nested.location(), "atref/specs");
    }
}
