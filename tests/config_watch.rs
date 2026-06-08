//! Integration tests for the config-file watcher (spec 006). Headless: spawn
//! `watch::spawn_config` over a temp dir and assert the callback fires on
//! `config.json` edits/replaces but not on sibling-file writes (e.g. the store).

use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use atref::watch;

fn unique_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("atref_cfgwatch_{tag}_{}", std::process::id()))
}

const CFG_A: &str = r#"{"folders":["C:\\x"],"chord":"Control+Space"}"#;
const CFG_B: &str = r#"{"folders":["C:\\y"],"chord":"Control+Space"}"#;

fn fired(rx: &Receiver<()>, within: Duration) -> bool {
    rx.recv_timeout(within).is_ok()
}

#[test]
fn fires_on_config_edit_but_not_on_sibling_writes() {
    // An edit to config.json triggers the callback; a write to a sibling file in
    // the same dir (mimicking the frequent index.redb writes) does not — proving
    // the file-name filter that keeps store writes from triggering reloads.
    let dir = unique_dir("edit");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let cfg = dir.join("config.json");
    fs::write(&cfg, CFG_A).unwrap();

    let (tx, rx) = channel::<()>();
    let guard = watch::spawn_config(cfg.clone(), Duration::from_millis(200), move || {
        let _ = tx.send(());
    })
    .expect("config watch spawns");
    std::thread::sleep(Duration::from_millis(150)); // let the watch establish

    fs::write(dir.join("index.redb"), b"not a real db").unwrap();
    assert!(
        !fired(&rx, Duration::from_millis(900)),
        "a sibling-file write must not trigger a reload"
    );

    fs::write(&cfg, CFG_B).unwrap();
    assert!(
        fired(&rx, Duration::from_secs(3)),
        "editing config.json triggers the callback"
    );

    drop(guard);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fires_on_atomic_replace_on_save() {
    // Atomic replace-on-save (write temp + rename over config.json) fires too,
    // proving the parent-dir + filename watch rather than a held file handle.
    let dir = unique_dir("replace");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let cfg = dir.join("config.json");
    fs::write(&cfg, CFG_A).unwrap();

    let (tx, rx) = channel::<()>();
    let guard = watch::spawn_config(cfg.clone(), Duration::from_millis(200), move || {
        let _ = tx.send(());
    })
    .expect("config watch spawns");
    std::thread::sleep(Duration::from_millis(150));

    let tmp = dir.join("config.json.tmp");
    fs::write(&tmp, CFG_B).unwrap();
    fs::rename(&tmp, &cfg).unwrap();
    assert!(
        fired(&rx, Duration::from_secs(3)),
        "atomic replace-on-save triggers the callback"
    );

    drop(guard);
    let _ = fs::remove_dir_all(&dir);
}
