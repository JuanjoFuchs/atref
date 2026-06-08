//! Integration tests for the agent CLI (spec 007), driving the real binary as a
//! subprocess with an isolated `ATREF_DIR`, asserting exit codes + stdout JSON.
//! Runs the debug binary (console subsystem), so stdout is captured normally.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn unique_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("atref_cli_it_{tag}_{}", std::process::id()))
}

fn fresh(tag: &str) -> PathBuf {
    let dir = unique_dir(tag);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str], atref_dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_atref"))
        .args(args)
        .env("ATREF_DIR", atref_dir)
        .output()
        .expect("run atref binary")
}

#[test]
fn describe_exits_zero_with_field_schema() {
    let dir = fresh("describe");
    let out = run(&["describe"], &dir);
    assert!(out.status.success(), "describe exits 0");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("describe is JSON");
    for key in ["folders", "exclude", "chord", "git_aware"] {
        assert!(
            v["fields"][key]["kind"].is_string(),
            "field {key} in schema"
        );
    }
    assert!(
        v["config_path"].is_string(),
        "describe reports the config path"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn add_creates_config_and_reports_change() {
    let dir = fresh("add");
    let target = dir.join("proj");
    fs::create_dir_all(&target).unwrap();
    let out = run(
        &["config", "add", "folders", target.to_str().unwrap()],
        &dir,
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(dir.join("config.json").exists(), "config.json created");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["changed"], serde_json::json!(true));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn unknown_command_is_nonzero() {
    let dir = fresh("unknown");
    let out = run(&["frobnicate"], &dir);
    assert!(!out.status.success(), "unknown command is non-zero");
    assert!(!out.stderr.is_empty(), "an error message on stderr");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn version_exits_zero() {
    let dir = fresh("version");
    let out = run(&["--version"], &dir);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("atref"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn add_succeeds_while_store_is_open() {
    // The CLI never opens the store, so a held index.redb handle (as the resident
    // app holds) does not block a config mutation.
    let dir = fresh("storelock");
    let first = dir.join("a");
    fs::create_dir_all(&first).unwrap();
    assert!(
        run(&["config", "add", "folders", first.to_str().unwrap()], &dir)
            .status
            .success()
    );

    // Hold the store like the running app does.
    let store = atref::store::Store::open_or_reset(&dir.join("index.redb"));

    let second = dir.join("b");
    fs::create_dir_all(&second).unwrap();
    let out = run(
        &["config", "add", "folders", second.to_str().unwrap()],
        &dir,
    );
    assert!(
        out.status.success(),
        "CLI must succeed while the store is held: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    drop(store);
    let _ = fs::remove_dir_all(&dir);
}
