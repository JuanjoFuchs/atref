//! In-memory file index built by enumerating the configured folders (FR5).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use walkdir::{DirEntry, WalkDir};

/// One indexed file.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Absolute path (inserted as `@"<abs>"`).
    pub abs: PathBuf,
    /// The configured root this file was found under.
    pub root: PathBuf,
    /// Path relative to `root`, used for matching and display.
    pub rel: String,
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
}

/// Build the index over `folders`, pruning `exclude`d directories and skipping
/// hidden files. Symlinks are not followed; a given absolute path appears once.
pub fn build(folders: &[PathBuf], exclude: &[String]) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    for root in folders {
        let walker = WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !is_excluded_dir(e, exclude));
        for entry in walker.flatten() {
            if !entry.file_type().is_file() || is_hidden(entry.path()) {
                continue;
            }
            let abs = entry.path().to_path_buf();
            if !seen.insert(abs.clone()) {
                continue;
            }
            let rel = abs
                .strip_prefix(root)
                .unwrap_or(&abs)
                .to_string_lossy()
                .into_owned();
            entries.push(Entry {
                abs,
                root: root.clone(),
                rel,
            });
        }
    }
    entries
}

/// Prune a directory whose name matches an `exclude` entry.
fn is_excluded_dir(entry: &DirEntry, exclude: &[String]) -> bool {
    entry.file_type().is_dir()
        && entry
            .file_name()
            .to_str()
            .map(|name| exclude.iter().any(|ex| ex == name))
            .unwrap_or(false)
}

/// A file is hidden if its name is dot-prefixed or it carries the Windows
/// hidden attribute.
fn is_hidden(path: &Path) -> bool {
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
        if let Ok(md) = path.metadata() {
            return md.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0;
        }
    }
    false
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
        let names: Vec<String> = build(std::slice::from_ref(&tmp), &exclude)
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
    fn indexes_multiple_folders_and_dedupes() {
        // AC10: files from every configured folder are searchable; no dupes.
        let base = std::env::temp_dir().join("atref_test_idx_multi");
        let _ = fs::remove_dir_all(&base);
        let a = base.join("a");
        let b = base.join("b");
        touch(&a, "alpha.md");
        touch(&b, "beta.md");

        let names: Vec<String> = build(&[a.clone(), b.clone()], &[])
            .iter()
            .map(|e| e.name().to_string())
            .collect();
        assert!(names.contains(&"alpha.md".to_string()));
        assert!(names.contains(&"beta.md".to_string()));

        // The same root listed twice still yields each file once.
        let twice = build(&[a.clone(), a.clone()], &[]);
        assert_eq!(twice.iter().filter(|e| e.name() == "alpha.md").count(), 1);

        let _ = fs::remove_dir_all(&base);
    }
}
