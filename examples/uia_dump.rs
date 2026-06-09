//! Probe: can UIA read the live Tauri/WebView2 picker's DOM by name/role?
//!
//! WebView2 (Chromium) exposes its DOM as a Windows UIA tree on-demand when a
//! UIA client attaches. This dumps the named elements of the running window
//! titled "atref" to confirm we can observe the search box, rows, and counter by
//! meaning — the basis for the agentic-GUI harness.
//!
//! Run while the picker is open: `cargo run --example uia_dump`.

use uiautomation::controls::ControlType;
use uiautomation::{UIAutomation, UIElement, UITreeWalker};

fn walk(walker: &UITreeWalker, el: &UIElement, depth: usize, out: &mut String, count: &mut usize) {
    if depth > 25 || *count > 4000 {
        return;
    }
    *count += 1;
    let name = el.get_name().unwrap_or_default();
    let ct = el
        .get_control_type()
        .map(|c| format!("{c:?}"))
        .unwrap_or_else(|_| "?".into());
    // Keep it readable: print shallow levels and any element that carries text.
    if depth <= 1 || !name.is_empty() {
        out.push_str(&format!("{}{ct} {name:?}\n", "  ".repeat(depth)));
    }
    let mut child = walker.get_first_child(el).ok();
    let mut n = 0;
    while let Some(c) = child {
        walk(walker, &c, depth + 1, out, count);
        child = walker.get_next_sibling(&c).ok();
        n += 1;
        if n > 1000 {
            break;
        }
    }
}

fn main() {
    let automation = UIAutomation::new().expect("init UIAutomation");
    let walker = automation
        .get_control_view_walker()
        .expect("control view walker");
    let win = automation
        .create_matcher()
        .control_type(ControlType::Window)
        .contains_name("atref")
        .timeout(4000)
        .find_first()
        .expect("could not find the 'atref' window — is the picker open?");

    let mut out = String::new();
    let mut count = 0usize;
    walk(&walker, &win, 0, &mut out, &mut count);
    println!("--- atref UIA subtree ({count} nodes walked; named/shallow shown) ---");
    print!("{out}");
    println!("--- end ---");
}
