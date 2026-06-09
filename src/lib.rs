//! atref core + GUI shell.
//!
//! The pure, unit-testable logic (config, index, search, frecency, reference,
//! store, watch, cli) lives in the child modules; everything that can be tested
//! without a screen lives there. The GUI is a Tauri/WebView2 picker — a
//! transparent borderless window with native DWM acrylic (tauri.conf.json
//! windowEffects) and a Raycast-style UI in `ui/` — wired here in [`run`]:
//! a resident tray app whose global chord summons the picker over the focused
//! app, with `Enter`/click inserting `@"<abs>"` at that app's caret (Win32) and
//! `Esc`/blur hiding. `main.rs` is a thin shell over [`run`] (plus the agent
//! CLI); `win32` lives here so `tests/` can reach it.

pub mod cli;
pub mod config;
pub mod frecency;
pub mod icon;
pub mod index;
pub mod reference;
pub mod search;
pub mod store;
pub mod watch;
pub mod win32;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use nucleo_matcher::{Config as MatcherConfig, Matcher};
use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::config::Config;
use crate::index::Entry;

/// How many top matches the picker lists (scrollable; spec 003).
const MAX_RESULTS: usize = 50;
/// Picker size (logical px) — kept in sync with tauri.conf.json for clamping.
const PICKER_W: i32 = 720;
const PICKER_H: i32 = 460;

/// Resident search state: the index, the fuzzy matcher, and the window that was
/// focused when the picker was summoned (the insertion target).
struct AppState {
    entries: Vec<Entry>,
    matcher: Matcher,
    target: isize,
}

#[derive(Serialize)]
struct Row {
    name: String,
    location: String,
    abs: String,
}

#[derive(Serialize)]
struct SearchOut {
    rows: Vec<Row>,
    matches: usize,
    total: usize,
}

/// Fuzzy-search the index for `query` and return the top rows for the UI.
#[tauri::command]
fn search_files(query: String, state: State<'_, Mutex<AppState>>) -> SearchOut {
    let mut st = state.lock().unwrap();
    let total = st.entries.len();
    // Flat frecency scores for now — the persistent ledger (spec 005) is wired
    // into the resident state as a follow-up.
    let scores = vec![0.0_f64; total];
    let AppState {
        entries, matcher, ..
    } = &mut *st;
    let (idx, matches) = search::rank(&query, entries, &scores, matcher, MAX_RESULTS);
    let rows = idx
        .iter()
        .map(|&i| {
            let e = &entries[i];
            Row {
                name: e.name().to_string(),
                location: e.location(),
                abs: e.abs.to_string_lossy().into_owned(),
            }
        })
        .collect();
    SearchOut {
        rows,
        matches,
        total,
    }
}

/// Accept a row: hide the picker, then insert `@"<abs>"` at the caret of the
/// previously-focused app (off-thread so the UI stays responsive).
#[tauri::command]
fn accept(abs: String, app: AppHandle, state: State<'_, Mutex<AppState>>) {
    let target = state.lock().unwrap().target;
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
    let text = reference::at_quoted(Path::new(&abs));
    std::thread::spawn(move || win32::insert_reference(&text, target));
}

/// Dismiss the picker and return focus to the app that was focused on summon.
#[tauri::command]
fn hide(app: AppHandle, state: State<'_, Mutex<AppState>>) {
    let target = state.lock().unwrap().target;
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
    win32::set_foreground(target);
}

/// Show the picker near the cursor, capturing the foreground window first so a
/// later accept can insert into it.
fn summon(app: &AppHandle) {
    let (hwnd, x, y) = win32::capture_foreground_and_cursor();
    {
        let st = app.state::<Mutex<AppState>>();
        st.lock().unwrap().target = hwnd;
    }
    if let Some(w) = app.get_webview_window("main") {
        let work = win32::work_area_at(x, y).unwrap_or((0, 0, i32::MAX, i32::MAX));
        let (px, py) = win32::clamp_to_work_area(x + 12, y + 24, PICKER_W, PICKER_H, work);
        let _ = w.set_position(PhysicalPosition::new(px, py));
        let _ = w.show();
        let _ = w.set_focus();
        // Tell the UI to clear the query, refocus the box, and re-search.
        let _ = w.emit("summon", ());
    }
}

/// atref's config dir (honoring the `ATREF_DIR` test seam).
fn config_dir() -> Option<PathBuf> {
    std::env::var_os("ATREF_DIR")
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|d| d.config_dir().join("atref")))
}

fn load_config() -> Option<Config> {
    config_dir()
        .map(|d| d.join("config.json"))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| Config::from_json(&t).ok())
}

fn load_index(cfg: &Option<Config>) -> Vec<Entry> {
    match cfg {
        Some(c) => index::build(&c.folders, &c.exclude, c.git_aware),
        None => Vec::new(),
    }
}

/// Build and run the Tauri picker (the resident tray app).
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        summon(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let cfg = load_config();
            let entries = load_index(&cfg);
            let matcher = Matcher::new(MatcherConfig::DEFAULT.match_paths());
            app.manage(Mutex::new(AppState {
                entries,
                matcher,
                target: 0,
            }));

            // Tray icon with a Quit item (resident app).
            let quit = MenuItem::with_id(app, "quit", "Quit atref", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit])?;
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("atref")
                .menu(&menu)
                .on_menu_event(|app, event| {
                    if event.id() == "quit" {
                        app.exit(0);
                    }
                })
                .build(app)?;

            // Register the configured chord (default Ctrl+Space).
            let chord = cfg
                .as_ref()
                .map(|c| c.chord.clone())
                .unwrap_or_else(|| "Control+Space".to_string());
            if let Ok(shortcut) = chord.parse::<Shortcut>() {
                let _ = app.global_shortcut().register(shortcut);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![search_files, accept, hide])
        .run(tauri::generate_context!())
        .expect("error while running atref");
}
