//! Integration tests for the live file-watcher (spec 002 AC6–AC9). Uses the
//! real `notify` watcher over temp dirs and polls with a hard 2 s deadline so a
//! failure fails fast rather than hanging.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

use atref::index::Entry;
use atref::watch;

fn unique_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("atref_it_{tag}_{}", std::process::id()))
}

fn touch(dir: &Path, name: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join(name), b"x").unwrap();
}

fn has(idx: &[Entry], name: &str) -> bool {
    idx.iter().any(|e| e.name() == name)
}

/// Wait up to 2 s for an index snapshot satisfying `pred`; return it, or `None`.
fn wait_for_snapshot(
    rx: &Receiver<Vec<Entry>>,
    pred: impl Fn(&[Entry]) -> bool,
) -> Option<Vec<Entry>> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let now = Instant::now();
        if now >= deadline {
            return None;
        }
        match rx.recv_timeout(deadline - now) {
            Ok(index) if pred(&index) => return Some(index),
            Ok(_) => continue,
            Err(_) => return None,
        }
    }
}

fn spawn(
    dir: &Path,
    exclude: Vec<String>,
    tx: std::sync::mpsc::Sender<Vec<Entry>>,
) -> Box<dyn std::any::Any> {
    watch::spawn(
        vec![dir.to_path_buf()],
        exclude,
        false, // git_aware off — these temp dirs are not repos
        Duration::from_millis(200),
        move |index| {
            let _ = tx.send(index);
        },
    )
    .expect("watcher spawns")
}

#[test]
fn picks_up_create_and_delete() {
    let tmp = unique_dir("watch_cd");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    touch(&tmp, "existing.md");

    let (tx, rx) = channel::<Vec<Entry>>();
    let guard = spawn(&tmp, vec![], tx);

    // AC6: create → appears within 2 s, no Reload.
    touch(&tmp, "fresh.md");
    assert!(
        wait_for_snapshot(&rx, |idx| has(idx, "fresh.md")).is_some(),
        "new file should appear within 2s"
    );

    // AC7: delete → gone within 2 s.
    fs::remove_file(tmp.join("fresh.md")).unwrap();
    assert!(
        wait_for_snapshot(&rx, |idx| !has(idx, "fresh.md")).is_some(),
        "deleted file should disappear within 2s"
    );

    drop(guard);
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn honors_filters_on_watched_changes() {
    // AC8: a newly-created hidden/excluded file is never added (the rebuild
    // re-applies the filters). The gitignored case is covered transitively —
    // the watcher rebuilds via the same `index::build` proven git-aware in
    // tests/git_aware.rs.
    let tmp = unique_dir("watch_filter");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let (tx, rx) = channel::<Vec<Entry>>();
    let guard = spawn(&tmp, vec!["node_modules".to_string()], tx);

    fs::write(tmp.join(".hidden_new"), b"x").unwrap();
    touch(&tmp.join("node_modules"), "lib.js");
    touch(&tmp, "visible.md");

    let snap = wait_for_snapshot(&rx, |idx| has(idx, "visible.md"))
        .expect("the visible file should appear");
    assert!(!has(&snap, ".hidden_new"), "hidden file not added (AC8)");
    assert!(!has(&snap, "lib.js"), "excluded-dir file not added (AC8)");

    drop(guard);
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn survives_a_burst() {
    // AC9: a burst of changes neither panics nor hangs; the index converges.
    let tmp = unique_dir("watch_burst");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let (tx, rx) = channel::<Vec<Entry>>();
    let guard = spawn(&tmp, vec![], tx);

    for i in 0..50 {
        fs::write(tmp.join(format!("b{i}.md")), b"x").unwrap();
    }
    assert!(
        wait_for_snapshot(&rx, |idx| has(idx, "b49.md")).is_some(),
        "burst should converge without panic"
    );

    drop(guard);
    let _ = fs::remove_dir_all(&tmp);
}
