//! atref — a Windows system-tray file-reference picker.
//!
//! Launch adds a tray icon (no console window in release). A global chord shows
//! a borderless fuzzy picker near the cursor; `Enter` inserts `@"<abs path>"` at
//! the caret of the previously-focused app. Config is a hand-edited JSON file.
//!
//! Architecture note (validated by the TC7 spike): `eframe` owns the event
//! loop; tray + hotkey events are delivered on global channels and a background
//! watcher thread wakes the loop via `request_repaint()`. The window is never
//! *hidden* — a hidden eframe window stops being serviced by winit — it is kept
//! visible but parked off-screen, then moved on-screen to "show".
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, SystemTime};

use eframe::egui;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use nucleo_matcher::{Config as MatcherConfig, Matcher};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use windows::Win32::Foundation::{BOOL, HWND, POINT};
use windows::Win32::Graphics::Gdi::{
    CreateRoundRectRgn, GetMonitorInfoW, MonitorFromPoint, SetWindowRgn, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetForegroundWindow, SetForegroundWindow,
};

use atref::config::Config;
use atref::frecency;
use atref::index::{self, Entry};
use atref::picker;
use atref::reference;
use atref::search;
use atref::store;
use atref::watch;

/// Off-screen parking spot for the "hidden" state (see the TC7 note above).
const OFFSCREEN: f32 = -32000.0;
/// How many top matches the picker lists (scrollable; spec 003).
const MAX_RESULTS: usize = 50;
/// Picker window size in logical points — kept in sync with the NativeOptions
/// inner size so on-screen clamping uses the right dimensions.
const PICKER_W: f32 = 720.0;
const PICKER_H: f32 = 460.0;
/// Flag to suppress the brief console window when shelling out to open config.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Messages from the background watcher thread to the egui update loop.
enum Msg {
    /// Chord fired: show the picker. Carries the foreground window + cursor
    /// captured at chord time, before atref steals focus.
    Show {
        hwnd: isize,
        x: i32,
        y: i32,
    },
    OpenConfig,
    Reload,
    Quit,
    /// A background rebuild finished; swap it in if still the current generation.
    IndexReady {
        generation: u64,
        index: Vec<Entry>,
    },
}

/// Tray menu item ids, used to route `MenuEvent`s.
#[derive(Clone)]
struct MenuIds {
    open: MenuId,
    reload: MenuId,
    quit: MenuId,
}

struct App {
    config_path: PathBuf,
    home: PathBuf,
    config: Config,
    index: Vec<Entry>,
    matcher: Matcher,
    /// Persistent index cache + frecency ledger (spec 005).
    store: store::Store,
    /// In-memory frecency ledger: `path -> (pick_count, last_picked)`. Loaded
    /// from the store at startup, bumped on each pick, fed into `search::rank`.
    frecency: HashMap<PathBuf, (u32, SystemTime)>,

    // Kept alive for the lifetime of the app.
    hotkeys: GlobalHotKeyManager,
    current_chord: Option<HotKey>,
    tray: TrayIcon,

    rx: Receiver<Msg>,
    tx: Sender<Msg>,
    ctx: egui::Context,
    // Live file-watcher (spec 002 FR6). Holding the guard keeps the watch alive;
    // `watch_generation` discards rebuilds superseded by a Reload.
    debouncer: Option<Box<dyn std::any::Any>>,
    watch_generation: u64,

    // Picker state.
    visible: bool,
    query: String,
    last_query: Option<String>,
    selected: usize,
    results: Vec<usize>,
    match_total: usize,
    target_hwnd: isize,
    /// True once the picker has held OS focus since showing — so hide-on-blur
    /// only fires after it actually had focus (spec 003).
    focus_armed: bool,
    /// True once the window has been clipped to rounded corners (spec 004).
    region_applied: bool,
}

impl App {
    fn new(
        cc: &eframe::CreationContext<'_>,
        config_path: PathBuf,
        home: PathBuf,
        config: Config,
        store: store::Store,
        index: Vec<Entry>,
        frecency: HashMap<PathBuf, (u32, SystemTime)>,
    ) -> Self {
        let matcher = Matcher::new(MatcherConfig::DEFAULT.match_paths());
        picker::install_theme(&cc.egui_ctx);

        // --- global hotkey (registered here, on the event-loop thread) ---
        let hotkeys = GlobalHotKeyManager::new().expect("create hotkey manager");
        let current_chord = match register_chord(&hotkeys, &config.chord) {
            Ok(hk) => Some(hk),
            Err(e) => {
                error_dialog(&format!("Could not register chord: {e}"));
                std::process::exit(1);
            }
        };

        // --- tray icon + menu (FR2) ---
        let menu = Menu::new();
        let version = MenuItem::new(format!("atref v{}", env!("CARGO_PKG_VERSION")), false, None);
        let open = MenuItem::new("Open config file", true, None);
        let reload = MenuItem::new("Reload config", true, None);
        let quit = MenuItem::new("Quit", true, None);
        menu.append(&version).expect("menu");
        menu.append(&PredefinedMenuItem::separator()).expect("menu");
        menu.append(&open).expect("menu");
        menu.append(&reload).expect("menu");
        menu.append(&PredefinedMenuItem::separator()).expect("menu");
        menu.append(&quit).expect("menu");
        let ids = MenuIds {
            open: open.id().clone(),
            reload: reload.id().clone(),
            quit: quit.id().clone(),
        };
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("atref")
            .with_icon(make_icon())
            .build()
            .expect("build tray icon");

        // --- hotkey/menu thread: wake the loop on tray + chord events (TC7) ---
        let (tx, rx) = channel::<Msg>();
        let ctx = cc.egui_ctx.clone();
        let hk_tx = tx.clone();
        let hk_ctx = ctx.clone();
        std::thread::spawn(move || {
            let hotkey_rx = GlobalHotKeyEvent::receiver();
            let menu_rx = MenuEvent::receiver();
            loop {
                while let Ok(ev) = hotkey_rx.try_recv() {
                    if ev.state == HotKeyState::Pressed {
                        let (hwnd, x, y) = capture_foreground_and_cursor();
                        let _ = hk_tx.send(Msg::Show { hwnd, x, y });
                        hk_ctx.request_repaint();
                    }
                }
                while let Ok(ev) = menu_rx.try_recv() {
                    let msg = if ev.id == ids.open {
                        Some(Msg::OpenConfig)
                    } else if ev.id == ids.reload {
                        Some(Msg::Reload)
                    } else if ev.id == ids.quit {
                        Some(Msg::Quit)
                    } else {
                        None
                    };
                    if let Some(m) = msg {
                        let _ = hk_tx.send(m);
                        hk_ctx.request_repaint();
                    }
                }
                std::thread::sleep(Duration::from_millis(30));
            }
        });

        let mut app = Self {
            config_path,
            home,
            config,
            index,
            matcher,
            store,
            frecency,
            hotkeys,
            current_chord,
            tray,
            rx,
            tx,
            ctx,
            debouncer: None,
            watch_generation: 0,
            visible: false,
            query: String::new(),
            last_query: None,
            selected: 0,
            results: Vec::new(),
            match_total: 0,
            target_hwnd: 0,
            focus_armed: false,
            region_applied: false,
        };
        // Show the cached index immediately; reconcile against the filesystem in
        // the background (spec 005 load-then-reconcile). The watcher must be
        // armed first so both share the current generation.
        app.start_watcher();
        app.start_reconcile();
        app
    }

    /// Move the window on-screen near the cursor and take focus (FR7), clamped
    /// to the monitor work area so it stays fully visible near screen edges.
    fn show_at(&mut self, ctx: &egui::Context, hwnd: isize, x: i32, y: i32) {
        self.target_hwnd = hwnd;
        self.query.clear();
        self.last_query = None;
        self.selected = 0;
        self.visible = true;
        self.focus_armed = false;

        let ppp = ctx.pixels_per_point();
        let w = (PICKER_W * ppp) as i32;
        let h = (PICKER_H * ppp) as i32;
        let work = work_area_at(x, y).unwrap_or((0, 0, i32::MAX, i32::MAX));
        let (px, py) = clamp_to_work_area(x + 12, y + 24, w, h, work);

        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
            px as f32, py as f32,
        )));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    /// Park the window off-screen and return focus to the app that was focused
    /// when the picker opened (FR12).
    fn hide(&mut self, ctx: &egui::Context) {
        self.visible = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
            OFFSCREEN, OFFSCREEN,
        )));
        if self.target_hwnd != 0 {
            unsafe {
                let _ = SetForegroundWindow(HWND(self.target_hwnd as *mut c_void));
            }
        }
    }

    /// Accept the selected row: record the pick, hide, then insert `@"<abs>"` at
    /// the caret (FR11; spec 005 FR4 pick recording).
    fn accept(&mut self, ctx: &egui::Context) {
        let Some(&idx) = self.results.get(self.selected) else {
            return;
        };
        let abs = self.index[idx].abs.clone();
        let text = reference::at_quoted(&abs);
        let target = self.target_hwnd;

        // Record the pick: bump the in-memory ledger now so the next empty query
        // reflects it immediately, and persist off the UI thread (FR4/AC5).
        let entry = self
            .frecency
            .entry(abs.clone())
            .or_insert((0, SystemTime::UNIX_EPOCH));
        entry.0 = entry.0.saturating_add(1);
        entry.1 = SystemTime::now();
        let store = self.store.clone();
        std::thread::spawn(move || store.record_pick(&abs));

        self.hide(ctx);
        // Off the UI thread so the event loop keeps spinning during the
        // clipboard save / paste / restore dance.
        std::thread::spawn(move || insert_reference(&text, target));
    }

    /// Recompute the result list when the query changed (FR9), via the pure
    /// ranker (folder-priority tiebreak + CamelHumps — spec 002 FR5/FR9).
    fn recompute(&mut self) {
        if self.last_query.as_deref() == Some(self.query.as_str()) {
            return;
        }
        self.last_query = Some(self.query.clone());
        self.selected = 0;
        // Per-entry frecency scores (parallel to `index`), computed here so
        // `search::rank` stays pure of `now()` (spec 005 FR6).
        let now = SystemTime::now();
        let scores: Vec<f64> = self
            .index
            .iter()
            .map(|e| match self.frecency.get(&e.abs) {
                Some(&(count, last)) => {
                    frecency::score(count, now.duration_since(last).unwrap_or_default())
                }
                None => 0.0,
            })
            .collect();
        let (results, total) = search::rank(
            &self.query,
            &self.index,
            &scores,
            &mut self.matcher,
            MAX_RESULTS,
        );
        self.results = results;
        self.match_total = total;
    }

    /// Re-read config, re-register the chord, and reconcile the index + store
    /// against the (possibly new) folder set (FR2 Reload; spec 005 FR8).
    fn reload(&mut self) {
        match load_or_init(&self.config_path, &self.home) {
            Ok(cfg) => {
                if let Some(old) = self.current_chord.take() {
                    let _ = self.hotkeys.unregister(old);
                }
                match register_chord(&self.hotkeys, &cfg.chord) {
                    Ok(hk) => self.current_chord = Some(hk),
                    Err(e) => error_dialog(&format!("Could not register chord: {e}")),
                }
                self.config = cfg;
                self.last_query = None;
                // Same load-then-reconcile path as startup: the background walk
                // rebuilds the index, persists it, and prunes the store for the
                // new folder set (FR8). The cached index stays usable meanwhile.
                self.start_watcher();
                self.start_reconcile();
            }
            Err(e) => error_dialog(&format!(
                "Reload failed:\n{e}\n\nKeeping the previous configuration."
            )),
        }
    }

    /// (Re)start the live file-watcher against the current folder set (FR6/FR8).
    /// Each rebuild writes through to the store before swapping in memory (spec
    /// 005 FR3).
    fn start_watcher(&mut self) {
        self.watch_generation += 1;
        let generation = self.watch_generation;
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        let store = self.store.clone();
        self.debouncer = watch::spawn(
            self.config.folders.clone(),
            self.config.exclude.clone(),
            self.config.git_aware,
            Duration::from_millis(800),
            move |index| {
                // Persist before sending so the on-disk cache is never behind
                // the in-memory index (the closure runs on the watcher thread).
                store.persist(&index);
                let _ = tx.send(Msg::IndexReady { generation, index });
                ctx.request_repaint();
            },
        );
    }

    /// Walk the configured folders in the background, persist the result to the
    /// store, and swap it into memory — without blocking startup on the walk
    /// (spec 005 load-then-reconcile, FR2). Tagged with the current generation
    /// so a later Reload discards a stale in-flight result.
    fn start_reconcile(&mut self) {
        let generation = self.watch_generation;
        let folders = self.config.folders.clone();
        let exclude = self.config.exclude.clone();
        let git_aware = self.config.git_aware;
        let store = self.store.clone();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let index = index::build(&folders, &exclude, git_aware);
            store.persist(&index);
            let _ = tx.send(Msg::IndexReady { generation, index });
            ctx.request_repaint();
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Show { hwnd, x, y } => self.show_at(ctx, hwnd, x, y),
                Msg::OpenConfig => open_in_editor(&self.config_path),
                Msg::Reload => self.reload(),
                Msg::Quit => {
                    let _ = self.tray.set_visible(false);
                    std::process::exit(0);
                }
                Msg::IndexReady { generation, index } => {
                    if generation == self.watch_generation {
                        self.index = index;
                        self.last_query = None;
                    }
                }
            }
        }

        // Read navigation/accept keys at frame top, before the focused text
        // field can swallow them (a focused TextEdit consumes Enter, and since
        // we re-`request_focus()` every frame it never reports `lost_focus`).
        let (up, down, esc, enter) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::Enter) && !i.modifiers.any(),
            )
        });
        if self.visible && esc {
            self.hide(ctx);
        }

        // Hide when the window loses focus (e.g. a click outside), but only once
        // it has actually held focus since showing (spec 003). On first focus,
        // round the (now-foreground) window's corners (spec 004).
        if self.visible {
            if ctx.input(|i| i.focused) {
                self.focus_armed = true;
                if !self.region_applied {
                    round_window(ctx.pixels_per_point());
                    self.region_applied = true;
                }
            } else if self.focus_armed {
                self.hide(ctx);
            }
        }

        if self.visible {
            self.recompute();
            let n = self.results.len();
            if n > 0 {
                // Clamp at the ends (don't wrap) so the arrows scroll to and
                // stay at the last/first row.
                if down {
                    self.selected = (self.selected + 1).min(n - 1);
                }
                if up {
                    self.selected = self.selected.saturating_sub(1);
                }
                if self.selected >= n {
                    self.selected = n - 1;
                }
            }

            let rows: Vec<picker::Row> = self
                .results
                .iter()
                .map(|&idx| {
                    let entry = &self.index[idx];
                    picker::Row {
                        name: entry.name().to_string(),
                        location: entry.location(),
                    }
                })
                .collect();
            let total = self.index.len();
            match picker::render(
                ctx,
                &mut self.query,
                &rows,
                self.selected,
                up || down,
                self.match_total,
                total,
            ) {
                picker::Action::Accept(row) => {
                    self.selected = row;
                    self.accept(ctx);
                }
                picker::Action::Close => self.hide(ctx),
                picker::Action::None => {}
            }
            // Enter accepts the selection (captured at frame top — see above).
            if enter {
                self.accept(ctx);
            }
        }

        // Keep the loop ticking as a backstop to the watcher's wakes.
        ctx.request_repaint_after(Duration::from_millis(200));
    }

    /// Opaque panel color (window transparency wasn't compositing on Windows);
    /// the panel fills the window and `SetWindowRgn` rounds the corners.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Color32::from_rgb(0x1E, 0x1E, 0x20).to_normalized_gamma_f32()
    }
}

/// Parse + register a chord string, returning the registered `HotKey` (FR6).
fn register_chord(manager: &GlobalHotKeyManager, chord: &str) -> Result<HotKey, String> {
    let hotkey: HotKey = chord
        .parse()
        .map_err(|e| format!("invalid chord '{chord}': {e}"))?;
    manager
        .register(hotkey)
        .map_err(|e| format!("could not register '{chord}': {e}"))?;
    Ok(hotkey)
}

/// Snapshot the foreground window + cursor position (physical px) at chord time.
fn capture_foreground_and_cursor() -> (isize, i32, i32) {
    unsafe {
        let hwnd = GetForegroundWindow();
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        (hwnd.0 as isize, pt.x, pt.y)
    }
}

/// The work area (left, top, right, bottom in physical px) of the monitor
/// containing `(x, y)`, or `None` if it can't be determined.
fn work_area_at(x: i32, y: i32) -> Option<(i32, i32, i32, i32)> {
    unsafe {
        let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            let r = info.rcWork;
            Some((r.left, r.top, r.right, r.bottom))
        } else {
            None
        }
    }
}

/// Clamp a top-left window position so a `w`×`h` window stays inside the work
/// area `(left, top, right, bottom)`. Pure — unit-tested.
fn clamp_to_work_area(x: i32, y: i32, w: i32, h: i32, work: (i32, i32, i32, i32)) -> (i32, i32) {
    let (left, top, right, bottom) = work;
    let cx = x.min(right - w).max(left);
    let cy = y.min(bottom - h).max(top);
    (cx, cy)
}

/// Clip our window to a rounded rectangle so it has rounded corners without a
/// transparent window (which wasn't compositing on Windows). Called once, the
/// first time the picker holds focus, on its (now-foreground) HWND.
fn round_window(ppp: f32) {
    let w = (PICKER_W * ppp).round() as i32;
    let h = (PICKER_H * ppp).round() as i32;
    let ellipse = (28.0 * ppp).round() as i32; // ~14 px corner radius
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return;
        }
        let rgn = CreateRoundRectRgn(0, 0, w + 1, h + 1, ellipse, ellipse);
        let _ = SetWindowRgn(hwnd, rgn, BOOL(1));
    }
}

/// Insert `text` at the caret of the previously-focused window (FR11):
/// save clipboard → write text → restore focus → Ctrl+V → wait → restore.
fn insert_reference(text: &str, target: isize) {
    let mut clipboard = arboard::Clipboard::new();
    let saved = clipboard.as_mut().ok().and_then(|c| c.get_text().ok());

    if let Ok(c) = clipboard.as_mut() {
        let _ = c.set_text(text.to_owned());
    }

    unsafe {
        let _ = SetForegroundWindow(HWND(target as *mut c_void));
    }
    std::thread::sleep(Duration::from_millis(40));

    if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        let _ = enigo.key(Key::Control, Direction::Press);
        let _ = enigo.key(Key::Unicode('v'), Direction::Click);
        let _ = enigo.key(Key::Control, Direction::Release);
    }

    std::thread::sleep(Duration::from_millis(150));
    if let (Ok(c), Some(prev)) = (clipboard.as_mut(), saved) {
        let _ = c.set_text(prev);
    }
}

/// Open the config file with its default handler (FR2 Open config file).
fn open_in_editor(path: &Path) {
    use std::os::windows::process::CommandExt;
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", ""])
        .arg(path)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

/// The atref tray icon — the designed `@` mark, embedded as raw RGBA (generated
/// from `assets/icon.svg` via `tools/`; see `ai-docs/icon-design.md`).
fn make_icon() -> Icon {
    Icon::from_rgba(
        atref::icon::TRAY_RGBA.to_vec(),
        atref::icon::TRAY_W,
        atref::icon::TRAY_H,
    )
    .expect("valid tray icon")
}

/// Show a native error dialog (there is no console in release — FR4).
fn error_dialog(msg: &str) {
    rfd::MessageDialog::new()
        .set_title("atref")
        .set_description(msg)
        .set_level(rfd::MessageLevel::Error)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

/// Load config, writing the default on first launch (FR3).
fn load_or_init(path: &Path, home: &Path) -> Result<Config, String> {
    if !path.exists() {
        let cfg = Config::default_with_home(home.to_path_buf());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        std::fs::write(path, cfg.to_json())
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        return Ok(cfg);
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Config::from_json(&text)
}

fn main() -> eframe::Result {
    let dirs = directories::BaseDirs::new().expect("resolve base directories");
    let config_path = dirs.config_dir().join("atref").join("config.json");
    let home = dirs.home_dir().to_path_buf();

    // Load config before the GUI starts so errors surface in a dialog (FR4).
    let config = match load_or_init(&config_path, &home) {
        Ok(c) => c,
        Err(e) => {
            error_dialog(&format!("Configuration error:\n{e}"));
            std::process::exit(1);
        }
    };
    // Persistent store (spec 005): load the cached index + frecency instantly so
    // the picker is usable at once; `App::start_reconcile` refreshes them from
    // the filesystem in the background.
    let store_path = dirs.config_dir().join("atref").join("index.redb");
    let store = store::Store::open_or_reset(&store_path);
    let index = store.load_entries();
    let frecency = store.load_frecency();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([PICKER_W, PICKER_H])
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_position([OFFSCREEN, OFFSCREEN]),
        ..Default::default()
    };

    eframe::run_native(
        "atref",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(App::new(
                cc,
                config_path,
                home,
                config,
                store,
                index,
                frecency,
            )))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::clamp_to_work_area;

    #[test]
    fn clamps_window_to_work_area() {
        let work = (0, 0, 1920, 1080);
        // Fits → unchanged.
        assert_eq!(clamp_to_work_area(100, 100, 560, 360, work), (100, 100));
        // Near the right edge → pushed left to fit.
        assert_eq!(clamp_to_work_area(1900, 500, 560, 360, work), (1360, 500));
        // Near the bottom edge → pushed up to fit.
        assert_eq!(clamp_to_work_area(100, 1000, 560, 360, work), (100, 720));
        // A left-of-primary monitor with negative coords is honored.
        let left_monitor = (-1920, 0, 0, 1080);
        assert_eq!(
            clamp_to_work_area(-50, 100, 560, 360, left_monitor),
            (-560, 100)
        );
    }
}
