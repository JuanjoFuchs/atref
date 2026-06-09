//! Deterministic live-GUI gates for the Tauri picker (WebView2 UIA tree). Each
//! launches the real binary isolated, drives it with synthetic OS input, and
//! asserts the running picker's state through UIA. `#[ignore]`d (they fire global
//! chords + type, so they take focus) — run them deliberately:
//!
//!     cargo test --test e2e -- --ignored --nocapture

mod common;

use std::thread::sleep;
use std::time::Duration;

use common::{
    automation, find_window, fire_chord, fire_chord_f9, launch_isolated, poll, press_enter,
    press_escape, texts, type_text,
};

#[test]
#[ignore = "fires a global chord + types — takes focus; run deliberately"]
fn summon_search_and_hide() {
    let _app = launch_isolated();
    let (a, walker) = automation();

    // Let setup register the chord + build the index, then summon.
    sleep(Duration::from_secs(2));
    fire_chord();
    poll(Duration::from_secs(15), || find_window(&a)).expect("picker did not summon on the chord");
    sleep(Duration::from_millis(1000));

    type_text("gamma");
    sleep(Duration::from_millis(700));

    let win = find_window(&a).expect("picker window disappeared mid-search");
    let labels = texts(&walker, &win);
    assert!(
        labels.iter().any(|t| t == "1 / 3"),
        "counter should read '1 / 3', got {labels:?}"
    );
    assert!(
        labels.iter().any(|t| t == "gamma_widget.rs"),
        "result row 'gamma_widget.rs' missing, got {labels:?}"
    );

    // Exercise dismissal (it must not crash). The window's *visible* hide is the
    // doc's manual sliver: UIA can't reliably read a hidden WebView2 window's
    // visibility (it returns a stale rect), while `hide()` returns Ok with the
    // rect collapsed — so visible-hide is eyeballed, not asserted here.
    press_escape();
    sleep(Duration::from_millis(500));
}

#[test]
#[ignore = "fires a global chord + types — takes focus; run deliberately"]
fn pick_floats_to_top_via_frecency() {
    // Spec 005: accepting a row records the pick, so the next empty-query summon
    // leads with it. Summon → search "gamma" → Enter (records gamma_widget.rs) →
    // re-summon → on the now-empty query, gamma_widget.rs ranks above the others.
    let _app = launch_isolated();
    let (a, walker) = automation();

    sleep(Duration::from_secs(2));
    fire_chord();
    poll(Duration::from_secs(15), || find_window(&a)).expect("picker did not summon on the chord");
    sleep(Duration::from_millis(1000));

    type_text("gamma");
    sleep(Duration::from_millis(700));
    press_enter(); // accept → records the pick, hides, inserts into the prior app
    sleep(Duration::from_millis(1200));

    // Re-summon: the summon event clears the query, so we see the empty-query
    // (frecency) order.
    fire_chord();
    poll(Duration::from_secs(15), || find_window(&a)).expect("picker did not re-summon");
    sleep(Duration::from_millis(1000));

    let win = find_window(&a).expect("picker window disappeared");
    let labels = texts(&walker, &win);
    let pos = |name: &str| labels.iter().position(|t| t == name);
    let Some(gamma) = pos("gamma_widget.rs") else {
        panic!("gamma_widget.rs should be listed on the empty query, got {labels:?}");
    };
    // The picked file must rank ahead of the never-picked ones (frecency DESC).
    for other in ["alpha_notes.md", "beta_config.json"] {
        if let Some(p) = pos(other) {
            assert!(
                gamma < p,
                "picked gamma_widget.rs should rank above {other}, got {labels:?}"
            );
        }
    }
    press_escape();
    sleep(Duration::from_millis(500));
}

#[test]
#[ignore = "fires a global chord + edits config — takes focus; run deliberately"]
fn config_chord_hot_reloads() {
    // Spec 006: editing config.json re-registers the chord without a restart.
    // Launch with Ctrl+Alt+F8, rewrite the chord to Ctrl+Alt+F9, then prove the
    // NEW chord summons (the watcher re-registered it live).
    let app = launch_isolated();
    let (a, _walker) = automation();

    sleep(Duration::from_secs(2)); // initial setup + F8 registered
    app.set_chord("Control+Alt+F9");
    sleep(Duration::from_millis(1500)); // config debounce (400ms) + reload margin

    fire_chord_f9();
    poll(Duration::from_secs(15), || find_window(&a))
        .expect("hot-reloaded chord (Ctrl+Alt+F9) did not summon the picker");
    press_escape();
    sleep(Duration::from_millis(500));
}
