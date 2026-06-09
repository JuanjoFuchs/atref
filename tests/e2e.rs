//! Deterministic live-GUI gate for the Tauri picker (WebView2 UIA tree). Launch
//! isolated → fire the chord → assert it summoned and searched → Esc. `#[ignore]`d
//! (fires a global chord + types, so it takes focus) — run it deliberately:
//!
//!     cargo test --test e2e -- --ignored --nocapture

mod common;

use std::thread::sleep;
use std::time::Duration;

use common::{
    automation, find_window, fire_chord, launch_isolated, poll, press_escape, texts, type_text,
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
