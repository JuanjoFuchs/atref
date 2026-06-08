//! Agentic GUI E2E gate (branch `spike/agentic-e2e`).
//!
//! Drives atref's *live* GUI end-to-end: launch the real binary isolated, fire
//! the global chord + type with `enigo`, then assert the running picker's state
//! through the Windows UIA tree AccessKit exposes for egui widgets, and
//! screenshot it. See `ai-docs/agentic-gui-testing.md`.
//!
//! This drives the REAL desktop (global hotkey + synthetic keystrokes), so it is
//! `#[ignore]`d and must be run deliberately:
//!
//!     cargo test --test e2e -- --ignored --nocapture
//!
//! Don't type elsewhere while it runs — focus is taken for a few seconds.

mod common;

use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use uiautomation::controls::ControlType;
use uiautomation::patterns::UIValuePattern;
use uiautomation::{UIAutomation, UITreeWalker};

use common::{
    atref_exe, dump_to_string, e2e_lock, find_window_by_pid, fire_chord, launch_isolated_atref,
    poll, screenshot,
};

#[test]
#[ignore = "live desktop E2E; run: cargo test --test e2e -- --ignored --nocapture"]
fn picker_shows_results_via_uia() {
    let _serial = e2e_lock();
    let atref = launch_isolated_atref();
    let pid = atref.child.id();

    let automation = UIAutomation::new().expect("init UIAutomation");
    let walker = automation
        .get_control_view_walker()
        .expect("control view walker");

    // 1) Wait for atref's window (created at launch, parked off-screen).
    poll(Duration::from_secs(15), || {
        find_window_by_pid(&automation, &walker, pid)
    })
    .expect("atref window never appeared");
    eprintln!("[ok] found atref window (pid {pid})");

    // Let the background reconcile walk + index the 3 files.
    sleep(Duration::from_millis(1500));

    // 2) Fire the global chord to show the picker.
    let mut enigo = Enigo::new(&Settings::default()).expect("enigo");
    fire_chord(&mut enigo);
    sleep(Duration::from_millis(600));

    // 3) Type a query that should match gamma_widget.rs.
    enigo.text("gamma").expect("type query");
    sleep(Duration::from_millis(800));

    // 4) Read the running picker via UIA (re-find the window post-show).
    let window = find_window_by_pid(&automation, &walker, pid).expect("atref window disappeared");
    eprintln!(
        "--- atref UIA subtree (evidence) ---\n{}------------------------------------",
        dump_to_string(&walker, &window)
    );

    // 5) Screenshot the result.
    let shot = screenshot("picker_gamma");
    eprintln!("[ok] screenshot -> {}", shot.display());

    // 6a) The query Edit reflects what we typed (ValuePattern).
    let edit = automation
        .create_matcher()
        .from(window.clone())
        .control_type(ControlType::Edit)
        .depth(50)
        .timeout(2000)
        .find_first();
    match edit
        .as_ref()
        .ok()
        .and_then(|e| e.get_pattern::<UIValuePattern>().ok())
        .and_then(|p| p.get_value().ok())
    {
        Some(v) => eprintln!("[info] query Edit value = {v:?}"),
        None => eprintln!("[info] query Edit / ValuePattern not exposed"),
    }

    // 6b) The result row for gamma_widget.rs is a named Button.
    let buttons = automation
        .create_matcher()
        .from(window.clone())
        .control_type(ControlType::Button)
        .depth(50)
        .timeout(2000)
        .find_all()
        .unwrap_or_default();
    let names: Vec<String> = buttons
        .iter()
        .map(|b| b.get_name().unwrap_or_default())
        .collect();
    eprintln!("[info] buttons: {names:?}");

    // 6c) The counter Text renders "<matches> / <total>".
    let texts = automation
        .create_matcher()
        .from(window.clone())
        .control_type(ControlType::Text)
        .depth(50)
        .timeout(1000)
        .find_all()
        .unwrap_or_default();
    let counter = texts
        .iter()
        .filter_map(|t| t.get_name().ok())
        .find(|n| n.contains('/'));
    eprintln!("[info] counter text = {counter:?}");

    // Dismiss the picker so the desktop is left clean.
    enigo.key(Key::Escape, Direction::Click).ok();

    let has_result = names.iter().any(|n| n.contains("gamma_widget.rs"));
    assert!(
        has_result,
        "expected a result Button containing 'gamma_widget.rs' in the live UIA tree; saw {names:?}"
    );
}

/// Show the picker, clear any retained text, type `query`, and return the result
/// rows' Button names. Leaves the picker dismissed.
fn show_and_query(
    automation: &UIAutomation,
    walker: &UITreeWalker,
    enigo: &mut Enigo,
    pid: u32,
    query: &str,
) -> Vec<String> {
    fire_chord(enigo);
    sleep(Duration::from_millis(600));
    // Clear any text retained from a prior phase (select-all + delete).
    enigo.key(Key::Control, Direction::Press).ok();
    enigo.key(Key::Unicode('a'), Direction::Click).ok();
    enigo.key(Key::Control, Direction::Release).ok();
    enigo.key(Key::Backspace, Direction::Click).ok();
    sleep(Duration::from_millis(150));
    enigo.text(query).expect("type query");
    sleep(Duration::from_millis(800));

    let window = find_window_by_pid(automation, walker, pid).expect("atref window");
    let names: Vec<String> = automation
        .create_matcher()
        .from(window)
        .control_type(ControlType::Button)
        .depth(50)
        .timeout(2000)
        .find_all()
        .unwrap_or_default()
        .iter()
        .map(|b| b.get_name().unwrap_or_default())
        .collect();
    enigo.key(Key::Escape, Direction::Click).ok();
    sleep(Duration::from_millis(300));
    names
}

#[test]
#[ignore = "live desktop E2E; run: cargo test --test e2e -- --ignored --test-threads=1 --nocapture"]
fn config_change_reflows_index_live() {
    // Specs 006 + 007 end-to-end: a folder added to config.json — by a direct
    // edit (006 hot-reload) and by the CLI (`atref config add`, 007) — is picked
    // up by the *running* app with no manual Reload, so a file under it becomes
    // findable in the live picker.
    let _serial = e2e_lock();
    let atref = launch_isolated_atref();
    let pid = atref.child.id();

    let automation = UIAutomation::new().expect("init UIAutomation");
    let walker = automation
        .get_control_view_walker()
        .expect("control view walker");
    poll(Duration::from_secs(15), || {
        find_window_by_pid(&automation, &walker, pid)
    })
    .expect("atref window never appeared");
    let mut enigo = Enigo::new(&Settings::default()).expect("enigo");
    sleep(Duration::from_millis(1500));

    // --- Phase 006: add a folder by editing config.json directly ---
    let extra = atref.make_extra_folder("extra_edit", "zeta_hotreload.rs", "// zeta\n");
    let cfg_path = atref.home().join("config.json");
    let mut cfg: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&cfg_path).unwrap()).unwrap();
    cfg["folders"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!(extra.to_string_lossy()));
    std::fs::write(&cfg_path, serde_json::to_vec_pretty(&cfg).unwrap()).unwrap();
    sleep(Duration::from_secs(3)); // config-watch debounce + background reconcile

    let names = show_and_query(&automation, &walker, &mut enigo, pid, "zeta");
    eprintln!("[006] buttons after config edit: {names:?}");
    eprintln!(
        "[006] screenshot -> {}",
        screenshot("config_hotreload_zeta").display()
    );
    assert!(
        names.iter().any(|n| n.contains("zeta_hotreload.rs")),
        "spec 006: a folder added by editing config.json should hot-reload into the index; saw {names:?}"
    );

    // --- Phase 007: add another folder via the CLI ---
    let extra2 = atref.make_extra_folder("extra_cli", "omega_cli.rs", "// omega\n");
    let out = Command::new(atref_exe())
        .args(["config", "add", "folders", extra2.to_str().unwrap()])
        .env("ATREF_DIR", atref.home())
        .output()
        .expect("run atref config add");
    assert!(
        out.status.success(),
        "CLI add failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    sleep(Duration::from_secs(3));

    let names2 = show_and_query(&automation, &walker, &mut enigo, pid, "omega");
    eprintln!("[007] buttons after CLI add: {names2:?}");
    eprintln!(
        "[007] screenshot -> {}",
        screenshot("config_cli_omega").display()
    );
    assert!(
        names2.iter().any(|n| n.contains("omega_cli.rs")),
        "spec 007: a folder added via `atref config add` should hot-reload into the index; saw {names2:?}"
    );
}
