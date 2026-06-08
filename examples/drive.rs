//! The "eyes": launch atref isolated, optionally drive it with a query, then
//! screenshot + dump the live UIA tree so an agent can *see* what it built —
//! ad-hoc, not a pass/fail test. See `ai-docs/agentic-gui-testing.md`.
//!
//!     cargo build                          # ensure atref.exe exists
//!     cargo run --example drive            # just launch + screenshot + dump
//!     cargo run --example drive -- gamma   # also fire the chord and type "gamma"
//!
//! When a query is given it injects OS input (global chord + typing), so it
//! takes keyboard focus for a moment — don't type elsewhere while it runs.

#[path = "../tests/common/mod.rs"]
mod common;

use std::thread::sleep;
use std::time::Duration;

use enigo::{Enigo, Keyboard, Settings};
use uiautomation::UIAutomation;

use common::{
    dump_to_string, find_window_by_pid, fire_chord, launch_isolated_atref, poll, screenshot,
};

fn main() {
    let query = std::env::args().nth(1);

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
    eprintln!("[ok] launched atref (pid {pid})");
    sleep(Duration::from_millis(1500));

    if let Some(q) = &query {
        let mut enigo = Enigo::new(&Settings::default()).expect("enigo");
        fire_chord(&mut enigo);
        sleep(Duration::from_millis(600));
        enigo.text(q).expect("type query");
        sleep(Duration::from_millis(800));
        eprintln!("[ok] fired chord + typed {q:?}");
    }

    let window = find_window_by_pid(&automation, &walker, pid).expect("atref window disappeared");
    println!("--- atref UIA subtree ---");
    print!("{}", dump_to_string(&walker, &window));
    println!("-------------------------");

    let shot = screenshot("drive");
    println!("screenshot -> {}", shot.display());
    // atref is killed + its temp dirs removed when `atref` drops here.
}
