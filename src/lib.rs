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
//!
//! Resident state ([`AppState`]) holds the index, the fuzzy matcher, a persistent
//! store + in-memory frecency ledger (spec 005), and the current config + chord.
//! A background watcher-manager thread keeps the index live (spec 002) and
//! hot-reloads `config.json` edits — re-registering the chord and rebuilding the
//! index without a restart (spec 006).

pub mod cli;
pub mod config;
pub mod enrich;
pub mod frecency;
pub mod icon;
pub mod index;
pub mod reference;
pub mod search;
pub mod store;
pub mod watch;
pub mod win32;

use std::any::Any;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nucleo_matcher::{Config as MatcherConfig, Matcher};
use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::config::Config;
use crate::index::Entry;
use crate::store::Store;

/// How many top matches the picker lists (scrollable; spec 003).
const MAX_RESULTS: usize = 50;
/// Picker size (logical px) — kept in sync with tauri.conf.json for clamping.
const PICKER_W: i32 = 720;
const PICKER_H: i32 = 460;
/// Debounce for the live index file-watcher (spec 002 FR6).
const INDEX_DEBOUNCE: Duration = Duration::from_millis(800);
/// Debounce for the config.json hot-reload watcher (spec 006).
const CONFIG_DEBOUNCE: Duration = Duration::from_millis(400);

/// Resident search state. Shared (behind a `Mutex`) by the Tauri commands, the
/// summon handler, and the background watcher-manager thread.
struct AppState {
    /// The current index (cached at startup, refreshed by reconcile / watcher).
    entries: Vec<Entry>,
    matcher: Matcher,
    /// The window focused when the picker was summoned — the insertion target.
    target: isize,
    /// Persistent index cache + frecency ledger (spec 005).
    store: Store,
    /// In-memory frecency ledger: `path -> (pick_count, last_picked)`. Loaded
    /// from the store at startup, bumped on each pick, fed into `search::rank`.
    frecency: HashMap<PathBuf, (u32, SystemTime)>,
    /// The live config — folders/exclude/git_aware drive reconcile; chord is the
    /// registered hotkey (kept so a hot-reload can re-register only on change).
    config: Config,
    /// The currently-registered chord, so hot-reload can unregister it.
    current_chord: Option<Shortcut>,
}

#[derive(Serialize)]
struct Row {
    name: String,
    location: String,
    abs: String,
    /// Matched-char positions (code-point indices) in `name` / `location`,
    /// from the ranker's own match (spec 009). Empty on an empty query.
    name_hl: Vec<u32>,
    loc_hl: Vec<u32>,
    /// Byte size from index metadata — renders immediately, no content read
    /// (spec 010 FR1).
    size: u64,
}

#[derive(Serialize)]
struct SearchOut {
    rows: Vec<Row>,
    matches: usize,
    total: usize,
}

/// Fuzzy-search the index for `query` and return the top rows for the UI,
/// frecency-aware (spec 005 FR6): an empty query leads with the most-frecent
/// files, and near-equal fuzzy matches are broken by frecency.
#[tauri::command]
fn search_files(query: String, state: State<'_, Mutex<AppState>>) -> SearchOut {
    let mut st = state.lock().unwrap();
    let now = SystemTime::now();
    let AppState {
        entries,
        matcher,
        frecency,
        ..
    } = &mut *st;
    let total = entries.len();
    // Per-entry frecency scores (parallel to `entries`), computed here so
    // `search::rank` stays pure of `now()` (spec 005 FR6).
    let scores: Vec<f64> = entries
        .iter()
        .map(|e| match frecency.get(&e.abs) {
            Some(&(count, last)) => {
                crate::frecency::score(count, now.duration_since(last).unwrap_or_default())
            }
            None => 0.0,
        })
        .collect();
    let (idx, matches) = search::rank(&query, entries, &scores, matcher, MAX_RESULTS);
    let rows = idx
        .iter()
        .map(|&i| {
            let e = &entries[i];
            // Highlight positions only for the returned page (spec 009 NFR1).
            let indices = search::match_indices(&e.rel, &query, matcher);
            let (name_hl, loc_hl) = search::split_highlights(&e.rel, e.root_name(), &indices);
            Row {
                name: e.name().to_string(),
                location: e.location(),
                abs: e.abs.to_string_lossy().into_owned(),
                name_hl,
                loc_hl,
                size: e.size,
            }
        })
        .collect();
    SearchOut {
        rows,
        matches,
        total,
    }
}

/// Lazily enrich one result with line/token metrics (spec 010; spec 011 adds
/// thumbnails to the same payload). Async so the stat + content read run off
/// the UI thread (NFR1); results are cached by `(path, mtime)` (TC3). The
/// frontend requests this only for visible rows after input settles (FR3).
#[tauri::command]
async fn enrich(
    abs: String,
    cache: State<'_, Arc<Mutex<enrich::EnrichCache>>>,
) -> Result<enrich::Enrichment, String> {
    let cache = Arc::clone(&cache);
    tauri::async_runtime::spawn_blocking(move || {
        let path = PathBuf::from(&abs);
        let md = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        let mtime = index::mtime_secs(&md);
        if let Some(hit) = cache.lock().unwrap().get(&path, mtime) {
            return Ok(hit);
        }
        let out = enrich::enrich_file(&path, &md, &enrich::DEFAULT_CAPS);
        cache.lock().unwrap().put(path, mtime, out.clone());
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Accept a row: record the pick (spec 005 FR4), hide the picker, then insert
/// `@"<abs>"` at the caret of the previously-focused app (both off the UI thread).
#[tauri::command]
fn accept(abs: String, app: AppHandle, state: State<'_, Mutex<AppState>>) {
    let path = PathBuf::from(&abs);
    let (target, store) = {
        let mut st = state.lock().unwrap();
        // Bump the in-memory ledger now so the next empty query reflects it
        // immediately; persist off the UI thread below (FR4/AC5).
        let entry = st.frecency.entry(path.clone()).or_insert((0, UNIX_EPOCH));
        entry.0 = entry.0.saturating_add(1);
        entry.1 = SystemTime::now();
        (st.target, st.store.clone())
    };
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
    let text = reference::at_quoted(&path);
    std::thread::spawn(move || store.record_pick(&path));
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

// --- config + paths ---------------------------------------------------------

/// atref's config dir (honoring the `ATREF_DIR` test seam).
fn config_dir() -> PathBuf {
    std::env::var_os("ATREF_DIR")
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|d| d.config_dir().join("atref")))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn home_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Read + validate `config.json`, or `None` if missing / invalid (a hot-reload
/// then keeps the previous config rather than breaking on a half-saved edit).
fn read_config(config_path: &Path) -> Option<Config> {
    std::fs::read_to_string(config_path)
        .ok()
        .and_then(|t| Config::from_json(&t).ok())
}

/// Load `config.json`, writing the default on first launch (FR3) so the picker
/// works out of the box and the hot-reload watcher has a real file to track.
fn ensure_config(config_path: &Path, home: &Path) -> Config {
    if let Some(cfg) = read_config(config_path) {
        return cfg;
    }
    let cfg = Config::default_with_home(home.to_path_buf());
    if let Some(parent) = config_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(config_path, cfg.to_json());
    cfg
}

// --- chord registration -----------------------------------------------------

/// Parse + register a chord, returning the registered `Shortcut` (None if it
/// can't parse or the OS rejects it — e.g. already held by another app).
fn register_chord(app: &AppHandle, chord: &str) -> Option<Shortcut> {
    let shortcut = chord.parse::<Shortcut>().ok()?;
    app.global_shortcut().register(shortcut).ok()?;
    Some(shortcut)
}

// --- index reconcile + live watch (spec 002 / 005) --------------------------

/// Snapshot the current walk parameters + store handle from the shared state.
fn walk_params(app: &AppHandle) -> (Vec<PathBuf>, Vec<String>, bool, Store) {
    let st = app.state::<Mutex<AppState>>();
    let g = st.lock().unwrap();
    (
        g.config.folders.clone(),
        g.config.exclude.clone(),
        g.config.git_aware,
        g.store.clone(),
    )
}

/// Rebuild the index against the current folder set, persist it, and swap it
/// into the shared state (spec 005 load-then-reconcile, FR2).
fn reconcile(app: &AppHandle) {
    let (folders, exclude, git_aware, store) = walk_params(app);
    let index = index::build(&folders, &exclude, git_aware);
    store.persist(&index);
    let st = app.state::<Mutex<AppState>>();
    st.lock().unwrap().entries = index;
}

/// Start the live file-watcher against the current folder set (spec 002 FR6).
/// Each debounced rebuild writes through to the store before swapping in memory
/// (spec 005 FR3). Returns the watcher guard — dropping it stops the watch.
fn start_index_watch(app: &AppHandle) -> Option<Box<dyn Any>> {
    let (folders, exclude, git_aware, store) = walk_params(app);
    let app = app.clone();
    watch::spawn(folders, exclude, git_aware, INDEX_DEBOUNCE, move |index| {
        store.persist(&index);
        let st = app.state::<Mutex<AppState>>();
        st.lock().unwrap().entries = index;
    })
}

/// Apply a `config.json` edit (spec 006): re-read + validate it, re-register the
/// chord if it changed, and swap in the new config. Invalid edits are ignored so
/// a half-saved file never breaks the running app. The index reconcile + watcher
/// re-spawn are handled by the caller (it owns the watch guard).
fn apply_reload(app: &AppHandle, config_path: &Path) {
    let Some(new_cfg) = read_config(config_path) else {
        return;
    };
    let st = app.state::<Mutex<AppState>>();
    let mut g = st.lock().unwrap();
    if g.config.chord != new_cfg.chord {
        if let Some(old) = g.current_chord.take() {
            let _ = app.global_shortcut().unregister(old);
        }
        g.current_chord = register_chord(app, &new_cfg.chord);
    }
    g.config = new_cfg;
}

/// The background watcher-manager: owns the config + index watch guards (they
/// live on this thread's stack for the process lifetime, sidestepping `Send`),
/// runs the initial reconcile, and serializes hot-reloads. On each `config.json`
/// edit it applies the reload, reconciles the index, and re-spawns the index
/// watcher for the (possibly new) folder set.
fn watcher_manager(app: AppHandle, config_path: PathBuf) {
    let (tx, rx) = channel::<()>();
    // Watch config.json (spec 006). Kept alive for the process lifetime.
    let _config_guard = watch::spawn_config(config_path.clone(), CONFIG_DEBOUNCE, move || {
        let _ = tx.send(());
    });

    // Initial live watch + reconcile (refresh the cached index against the FS).
    let mut _index_guard = start_index_watch(&app);
    reconcile(&app);

    while rx.recv().is_ok() {
        apply_reload(&app, &config_path);
        // Rebuild for the new folder set, then re-point the live watcher at it
        // (dropping the old guard stops the stale watch).
        reconcile(&app);
        _index_guard = start_index_watch(&app);
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
            let dir = config_dir();
            let config_path = dir.join("config.json");
            let store_path = dir.join("index.redb");
            let cfg = ensure_config(&config_path, &home_dir());

            // Persistent store (spec 005): load the cached index + frecency
            // instantly so the picker is usable at once; the watcher-manager
            // reconciles against the filesystem in the background.
            let store = Store::open_or_reset(&store_path);
            let entries = store.load_entries();
            let frecency = store.load_frecency();
            let matcher = Matcher::new(MatcherConfig::DEFAULT.match_paths());

            let current_chord = register_chord(app.handle(), &cfg.chord);
            app.manage(Mutex::new(AppState {
                entries,
                matcher,
                target: 0,
                store,
                frecency,
                config: cfg,
                current_chord,
            }));
            // Enrichment cache (spec 010 NFR2), shared with the async command.
            app.manage(Arc::new(Mutex::new(enrich::EnrichCache::new(
                enrich::CACHE_CAP,
            ))));

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

            // Background watcher-manager: live index watch + config hot-reload.
            let handle = app.handle().clone();
            std::thread::spawn(move || watcher_manager(handle, config_path));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![search_files, accept, hide, enrich])
        .run(tauri::generate_context!())
        .expect("error while running atref");
}
