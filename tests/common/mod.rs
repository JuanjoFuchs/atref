//! Shared harness for driving atref's *live* GUI from the outside on Windows.
//!
//! Used by two callers:
//!   - `tests/e2e.rs` — the deterministic gate (`cargo test`).
//!   - `examples/drive.rs` — the ad-hoc "eyes" (`cargo run --example drive`).
//!
//! The approach: launch the real binary against an isolated config (`ATREF_DIR`
//! seam), inject OS-level input with `enigo`, and read the running picker
//! through the Windows UI Automation tree that AccessKit exposes for egui
//! widgets (so we assert by element name/role, not pixels).
#![allow(dead_code)] // not every helper is used by every caller

use std::path::PathBuf;
use std::process::{Child, Command};
use std::thread::sleep;
use std::time::{Duration, Instant};

use enigo::{Direction, Enigo, Key, Keyboard};
use uiautomation::{UIAutomation, UIElement, UITreeWalker};

/// Known files placed in the isolated index folder. Querying `gamma` should
/// surface `gamma_widget.rs`.
pub const FILES: &[(&str, &str)] = &[
    ("alpha_notes.md", "# alpha\n"),
    ("beta_config.json", "{}\n"),
    ("gamma_widget.rs", "// gamma widget\n"),
];

/// Resolve the atref binary. Integration tests get `CARGO_BIN_EXE_atref` for
/// free; examples fall back to the debug build path (run `cargo build` first).
pub fn atref_exe() -> PathBuf {
    if let Some(p) = option_env!("CARGO_BIN_EXE_atref") {
        return PathBuf::from(p);
    }
    let exe = if cfg!(windows) { "atref.exe" } else { "atref" };
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("debug")
        .join(exe)
}

/// A launched atref process with its isolated home + index dirs. Killed and
/// cleaned up on drop.
pub struct Atref {
    pub child: Child,
    base: PathBuf,
}

impl Drop for Atref {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// Launch atref against a fresh, isolated config + index folder so the run never
/// touches the user's real `%APPDATA%\atref`.
pub fn launch_isolated_atref() -> Atref {
    let base = std::env::temp_dir().join(format!("atref-e2e-{}", std::process::id()));
    let home = base.join("home");
    let files = base.join("files");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&files).unwrap();
    for (name, body) in FILES {
        std::fs::write(files.join(name), body).unwrap();
    }
    let cfg = serde_json::json!({
        "folders": [files.to_string_lossy()],
        "exclude": [],
        "chord": "Control+Space",
        "git_aware": false,
    });
    std::fs::write(
        home.join("config.json"),
        serde_json::to_vec_pretty(&cfg).unwrap(),
    )
    .unwrap();

    let child = Command::new(atref_exe())
        .env("ATREF_DIR", &home)
        .spawn()
        .expect("launch atref");
    Atref { child, base }
}

/// Poll `f` until it yields `Some` or the timeout elapses.
pub fn poll<T>(timeout: Duration, mut f: impl FnMut() -> Option<T>) -> Option<T> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(v) = f() {
            return Some(v);
        }
        if Instant::now() >= deadline {
            return None;
        }
        sleep(Duration::from_millis(250));
    }
}

/// Find atref's top-level window by process id (robust to the window having no
/// UIA Name and being parked off-screen / off-taskbar).
pub fn find_window_by_pid(
    automation: &UIAutomation,
    walker: &UITreeWalker,
    pid: u32,
) -> Option<UIElement> {
    let root = automation.get_root_element().ok()?;
    let mut child = walker.get_first_child(&root).ok();
    while let Some(c) = child {
        if c.get_process_id().ok() == Some(pid) {
            return Some(c);
        }
        child = walker.get_next_sibling(&c).ok();
    }
    None
}

/// Render a UIA subtree (control type + name) as evidence of what AccessKit
/// exposes for the egui picker.
pub fn dump_to_string(walker: &UITreeWalker, el: &UIElement) -> String {
    fn walk(walker: &UITreeWalker, el: &UIElement, depth: usize, out: &mut String) {
        let name = el.get_name().unwrap_or_default();
        let ct = el
            .get_control_type()
            .map(|c| format!("{c:?}"))
            .unwrap_or_else(|_| "?".into());
        out.push_str(&format!("{}{ct} {name:?}\n", "  ".repeat(depth)));
        let mut child = walker.get_first_child(el).ok();
        while let Some(c) = child {
            walk(walker, &c, depth + 1, out);
            child = walker.get_next_sibling(&c).ok();
        }
    }
    let mut out = String::new();
    walk(walker, el, 0, &mut out);
    out
}

/// Fire the default global chord (Ctrl+Space) to show the picker.
pub fn fire_chord(enigo: &mut Enigo) {
    enigo.key(Key::Control, Direction::Press).unwrap();
    enigo.key(Key::Space, Direction::Click).unwrap();
    enigo.key(Key::Control, Direction::Release).unwrap();
}

/// Capture the primary monitor to `target/e2e-artifacts/<tag>.png` and return
/// the path.
pub fn screenshot(tag: &str) -> PathBuf {
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("e2e-artifacts");
    std::fs::create_dir_all(&out_dir).ok();
    let path = out_dir.join(format!("{tag}.png"));
    if let Ok(monitors) = xcap::Monitor::all() {
        if let Some(m) = monitors.into_iter().next() {
            if let Ok(img) = m.capture_image() {
                let _ = img.save(&path);
            }
        }
    }
    path
}
