//! The "eyes" for the Tauri picker: launch an isolated instance, optionally type
//! a query, then dump the live UIA tree + screenshot — so the agent can see and
//! assert what it built without a human at the screen. See atref's
//! `ai-docs/agentic-gui-testing.md`.
//!
//!   cargo build                          # build atref.exe first
//!   cargo run --example drive            # launch + dump UIA + screenshot
//!   cargo run --example drive -- gamma   # also type "gamma" and filter
//!
//! Takes keyboard focus for a moment (synthetic typing) — don't type elsewhere.

#[path = "../tests/common/mod.rs"]
mod common;

use std::thread::sleep;
use std::time::Duration;

use common::{
    automation, dump, find_window, fire_chord, launch_isolated, poll, screenshot_window, type_text,
};

fn main() {
    let query = std::env::args().nth(1);

    let app = launch_isolated();
    let (a, walker) = automation();
    // Let setup register the chord + build the index, then summon the hidden window.
    sleep(Duration::from_secs(2));
    fire_chord();
    poll(Duration::from_secs(15), || find_window(&a))
        .expect("atref window never appeared after chord");
    eprintln!("[ok] launched + summoned atref (pid {})", app.child.id());
    sleep(Duration::from_millis(1200));

    if let Some(q) = &query {
        type_text(q);
        sleep(Duration::from_millis(700));
        eprintln!("[ok] typed {q:?}");
    }

    let win = find_window(&a).expect("atref window disappeared");
    println!("--- atref UIA subtree ---");
    print!("{}", dump(&walker, &win));
    println!("--- end ---");
    if let Some(p) = screenshot_window("drive", &win) {
        println!("screenshot -> {}", p.display());
    }
    // atref is killed + its temp config removed when `app` drops here.
}
