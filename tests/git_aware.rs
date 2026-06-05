//! Integration tests for git-aware indexing (spec 002 AC1–AC3). Creates a real
//! temp Git repo, because `require_git` means `.gitignore` is honored only
//! inside one.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use atref::index::{self, Entry};

fn unique_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("atref_it_{tag}_{}", std::process::id()))
}

fn touch(dir: &Path, name: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join(name), b"x").unwrap();
}

fn git_init(dir: &Path) {
    let mut cmd = Command::new("git");
    cmd.arg("init").arg(dir);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let status = cmd.status().expect("git must be on PATH for this test");
    assert!(status.success(), "git init failed");
}

fn names(entries: &[Entry]) -> Vec<String> {
    entries.iter().map(|e| e.name().to_string()).collect()
}

#[test]
fn git_aware_excludes_ignored_keeps_new_and_honors_overlays() {
    let tmp = unique_dir("gitaware");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    git_init(&tmp);

    fs::write(tmp.join(".gitignore"), "ignored/\n*.log\n").unwrap();
    touch(&tmp, "keep.md"); // not ignored, not committed → should show (FR1)
    touch(&tmp.join("ignored"), "secret.txt"); // gitignored dir
    fs::write(tmp.join("debug.log"), b"x").unwrap(); // gitignored pattern
    fs::write(tmp.join(".hidden"), b"x").unwrap(); // hidden overlay
    touch(&tmp.join("node_modules"), "lib.js"); // manual exclude

    let exclude = vec!["node_modules".to_string()];

    // AC1: git_aware=true → ignored excluded, new non-ignored present.
    let on = names(&index::build(std::slice::from_ref(&tmp), &exclude, true));
    assert!(
        on.contains(&"keep.md".to_string()),
        "new non-ignored file shown"
    );
    assert!(
        !on.contains(&"secret.txt".to_string()),
        "gitignored dir excluded"
    );
    assert!(
        !on.contains(&"debug.log".to_string()),
        "gitignored pattern excluded"
    );
    // AC3: the exclude list + hidden skipping still apply in git-aware mode.
    assert!(!on.contains(&".hidden".to_string()), "hidden file excluded");
    assert!(
        !on.contains(&"lib.js".to_string()),
        "manual exclude applies"
    );

    // AC2: git_aware=false → gitignored files reappear (plain walk); the
    // exclude list + hidden skipping still apply.
    let off = names(&index::build(std::slice::from_ref(&tmp), &exclude, false));
    assert!(
        off.contains(&"secret.txt".to_string()),
        "ignored reappears when off"
    );
    assert!(
        off.contains(&"debug.log".to_string()),
        "ignored reappears when off"
    );
    assert!(
        !off.contains(&".hidden".to_string()),
        "hidden still excluded"
    );
    assert!(
        !off.contains(&"lib.js".to_string()),
        "exclude still applies"
    );

    let _ = fs::remove_dir_all(&tmp);
}
