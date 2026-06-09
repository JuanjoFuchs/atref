//! Shared agentic-GUI harness for the Tauri picker (WebView2 UIA tree). WebView2
//! exposes its DOM as a Windows UIA tree, so the approach mirrors the egui app's:
//! launch the real binary against an isolated config (`ATREF_DIR`), inject OS
//! input with `enigo`, and read the running picker through UIA (assert by element
//! name/role, not pixels). See ai-docs/agentic-gui-testing.md.
//!
//! Used by two callers:
//!   - `tests/e2e.rs` — the deterministic gate (`cargo test --test e2e -- --ignored`).
//!   - `examples/drive.rs` — the ad-hoc "eyes" (`cargo run --example drive`).
#![allow(dead_code)]

use std::path::PathBuf;
use std::process::{Child, Command};
use std::thread::sleep;
use std::time::{Duration, Instant};

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use uiautomation::controls::ControlType;
use uiautomation::{UIAutomation, UIElement, UITreeWalker};

/// Files placed in the isolated index folder. Querying `gamma` should surface
/// `gamma_widget.rs`.
pub const FILES: &[(&str, &str)] = &[
    ("alpha_notes.md", "# alpha\n"),
    ("beta_config.json", "{}\n"),
    ("gamma_widget.rs", "// gamma widget\n"),
];

/// The built atref binary (run `cargo build` first for examples).
pub fn exe() -> PathBuf {
    let name = if cfg!(windows) { "atref.exe" } else { "atref" };
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("debug")
        .join(name)
}

/// A launched picker with its isolated config dir; killed + cleaned on drop.
pub struct App {
    pub child: Child,
    base: PathBuf,
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

impl App {
    /// Rewrite `config.json` with a new chord (keeping the isolated folder) to
    /// exercise config hot-reload (spec 006). The running app's config watcher
    /// picks the edit up and re-registers the chord without a restart.
    pub fn set_chord(&self, chord: &str) {
        let files = self.base.join("files");
        let cfg = serde_json::json!({
            "folders": [files.to_string_lossy()],
            "exclude": [],
            "chord": chord,
            "git_aware": false,
        });
        std::fs::write(
            self.base.join("home").join("config.json"),
            serde_json::to_vec_pretty(&cfg).unwrap(),
        )
        .unwrap();
    }
}

/// Launch the picker against a fresh, isolated config + index folder so a run
/// never touches the user's real `%APPDATA%\atref`.
pub fn launch_isolated() -> App {
    let base = std::env::temp_dir().join(format!("atref-e2e-{}", std::process::id()));
    let home = base.join("home");
    let files = base.join("files");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&files).unwrap();
    for (name, body) in FILES {
        std::fs::write(files.join(name), body).unwrap();
    }
    // Use Ctrl+Alt+F8, not the user-facing default (Control+Space), for two
    // reasons: (1) a global hotkey can only be owned by one process, and an IME /
    // another app often already holds Ctrl+Space on a real desktop, so
    // registration would silently fail and the picker never summon; (2) the
    // trigger key is non-printable, so if the synthetic keypress leaks into the
    // freshly-focused search box before `summon` clears it, it inserts no text
    // (a printable trigger like `j` would corrupt the query → `jgamma`). The gate
    // verifies the summon→search→insert flow, not which chord triggers it.
    let cfg = serde_json::json!({
        "folders": [files.to_string_lossy()],
        "exclude": [],
        "chord": "Control+Alt+F8",
        "git_aware": false,
    });
    std::fs::write(
        home.join("config.json"),
        serde_json::to_vec_pretty(&cfg).unwrap(),
    )
    .unwrap();
    let child = Command::new(exe())
        .env("ATREF_DIR", &home)
        .spawn()
        .expect("launch atref");
    App { child, base }
}

/// Init UIA + a control-view walker.
pub fn automation() -> (UIAutomation, UITreeWalker) {
    let a = UIAutomation::new().expect("init UIAutomation");
    let w = a.get_control_view_walker().expect("control view walker");
    (a, w)
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

/// Find the picker's top-level window. Matches the title *exactly* (`"atref"`),
/// not a substring: a browser tab on the atref repo (e.g. "… JuanjoFuchs/atref -
/// Google Chrome") also contains "atref", and a substring match would grab it
/// instead of the picker.
pub fn find_window(a: &UIAutomation) -> Option<UIElement> {
    a.create_matcher()
        .control_type(ControlType::Window)
        .name("atref")
        .timeout(800)
        .find_first()
        .ok()
}

/// Inject text into the focused picker (the search box).
pub fn type_text(text: &str) {
    let mut e = Enigo::new(&Settings::default()).expect("enigo");
    e.text(text).expect("type query");
}

/// Fire the test chord (Ctrl+Alt+F8 — see `launch_isolated`) to summon the
/// hidden picker.
pub fn fire_chord() {
    let mut e = Enigo::new(&Settings::default()).expect("enigo");
    e.key(Key::Control, Direction::Press).unwrap();
    e.key(Key::Alt, Direction::Press).unwrap();
    e.key(Key::F8, Direction::Click).unwrap();
    e.key(Key::Alt, Direction::Release).unwrap();
    e.key(Key::Control, Direction::Release).unwrap();
}

/// Fire Ctrl+Alt+F9 — the post-reload chord used by the hot-reload gate (see
/// `App::set_chord`). Non-printable trigger, for the same reason as `fire_chord`.
pub fn fire_chord_f9() {
    let mut e = Enigo::new(&Settings::default()).expect("enigo");
    e.key(Key::Control, Direction::Press).unwrap();
    e.key(Key::Alt, Direction::Press).unwrap();
    e.key(Key::F9, Direction::Click).unwrap();
    e.key(Key::Alt, Direction::Release).unwrap();
    e.key(Key::Control, Direction::Release).unwrap();
}

/// Press Enter (accepts the selected row — records the pick + inserts).
pub fn press_enter() {
    let mut e = Enigo::new(&Settings::default()).expect("enigo");
    e.key(Key::Return, Direction::Click).unwrap();
}

/// Press Escape (dismisses the picker).
pub fn press_escape() {
    let mut e = Enigo::new(&Settings::default()).expect("enigo");
    e.key(Key::Escape, Direction::Click).unwrap();
}

/// Render a UIA subtree (control type + name) as evidence of the picker's DOM.
/// Shallow levels and any named element are shown.
pub fn dump(walker: &UITreeWalker, el: &UIElement) -> String {
    fn go(walker: &UITreeWalker, el: &UIElement, depth: usize, out: &mut String, n: &mut usize) {
        if depth > 25 || *n > 4000 {
            return;
        }
        *n += 1;
        let name = el.get_name().unwrap_or_default();
        let ct = el
            .get_control_type()
            .map(|c| format!("{c:?}"))
            .unwrap_or_else(|_| "?".into());
        if depth <= 1 || !name.is_empty() {
            out.push_str(&format!("{}{ct} {name:?}\n", "  ".repeat(depth)));
        }
        let mut child = walker.get_first_child(el).ok();
        let mut k = 0;
        while let Some(c) = child {
            go(walker, &c, depth + 1, out, n);
            child = walker.get_next_sibling(&c).ok();
            k += 1;
            if k > 1000 {
                break;
            }
        }
    }
    let mut out = String::new();
    let mut n = 0;
    go(walker, el, 0, &mut out, &mut n);
    out
}

/// All text labels under `el` (row names/locations, counter, brand) — the basis
/// for assertions by meaning.
pub fn texts(walker: &UITreeWalker, el: &UIElement) -> Vec<String> {
    fn go(walker: &UITreeWalker, el: &UIElement, out: &mut Vec<String>, n: &mut usize) {
        if *n > 4000 {
            return;
        }
        *n += 1;
        if let Ok(name) = el.get_name() {
            if !name.is_empty() {
                out.push(name);
            }
        }
        let mut child = walker.get_first_child(el).ok();
        let mut k = 0;
        while let Some(c) = child {
            go(walker, &c, out, n);
            child = walker.get_next_sibling(&c).ok();
            k += 1;
            if k > 1000 {
                break;
            }
        }
    }
    let mut out = Vec::new();
    let mut n = 0;
    go(walker, el, &mut out, &mut n);
    out
}

/// Capture the primary monitor and crop to the window (plus a margin for the
/// shadow/acrylic edge), saving to `target/e2e-artifacts/<tag>.png`.
pub fn screenshot_window(tag: &str, win: &UIElement) -> Option<PathBuf> {
    let r = win.get_bounding_rectangle().ok()?;
    let m = xcap::Monitor::all().ok()?.into_iter().next()?;
    let img = m.capture_image().ok()?;
    let (mw, mh) = (img.width() as i32, img.height() as i32);
    let pad = 56;
    let x0 = (r.get_left() - pad).clamp(0, mw);
    let y0 = (r.get_top() - pad).clamp(0, mh);
    let x1 = (r.get_right() + pad).clamp(0, mw);
    let y1 = (r.get_bottom() + pad).clamp(0, mh);
    let (w, h) = ((x1 - x0).max(1) as u32, (y1 - y0).max(1) as u32);
    let crop = image::imageops::crop_imm(&img, x0 as u32, y0 as u32, w, h).to_image();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("e2e-artifacts");
    std::fs::create_dir_all(&dir).ok();
    let path = dir.join(format!("{tag}.png"));
    crop.save(&path).ok()?;
    Some(path)
}
