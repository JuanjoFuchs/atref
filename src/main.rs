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

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use eframe::egui;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config as MatcherConfig, Matcher, Utf32Str};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetForegroundWindow, SetForegroundWindow,
};

use atref::config::Config;
use atref::index::{self, Entry};
use atref::reference;

/// Off-screen parking spot for the "hidden" state (see the TC7 note above).
const OFFSCREEN: f32 = -32000.0;
/// How many results the picker shows at once (FR8).
const MAX_RESULTS: usize = 10;
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

    // Kept alive for the lifetime of the app.
    hotkeys: GlobalHotKeyManager,
    current_chord: Option<HotKey>,
    tray: TrayIcon,

    rx: Receiver<Msg>,

    // Picker state.
    visible: bool,
    query: String,
    last_query: Option<String>,
    selected: usize,
    results: Vec<usize>,
    target_hwnd: isize,
}

impl App {
    fn new(
        cc: &eframe::CreationContext<'_>,
        config_path: PathBuf,
        home: PathBuf,
        config: Config,
        index: Vec<Entry>,
    ) -> Self {
        let matcher = Matcher::new(MatcherConfig::DEFAULT.match_paths());

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

        // --- watcher thread: wake the loop on tray/hotkey events (TC7) ---
        let (tx, rx) = channel::<Msg>();
        let ctx = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            let hotkey_rx = GlobalHotKeyEvent::receiver();
            let menu_rx = MenuEvent::receiver();
            loop {
                while let Ok(ev) = hotkey_rx.try_recv() {
                    if ev.state == HotKeyState::Pressed {
                        let (hwnd, x, y) = capture_foreground_and_cursor();
                        let _ = tx.send(Msg::Show { hwnd, x, y });
                        ctx.request_repaint();
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
                        let _ = tx.send(m);
                        ctx.request_repaint();
                    }
                }
                std::thread::sleep(Duration::from_millis(30));
            }
        });

        Self {
            config_path,
            home,
            config,
            index,
            matcher,
            hotkeys,
            current_chord,
            tray,
            rx,
            visible: false,
            query: String::new(),
            last_query: None,
            selected: 0,
            results: Vec::new(),
            target_hwnd: 0,
        }
    }

    /// Move the window on-screen near the cursor and take focus (FR7).
    fn show_at(&mut self, ctx: &egui::Context, hwnd: isize, x: i32, y: i32) {
        self.target_hwnd = hwnd;
        self.query.clear();
        self.last_query = None;
        self.selected = 0;
        self.visible = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
            (x + 12) as f32,
            (y + 24) as f32,
        )));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    /// Park the window off-screen (FR12 / our "hidden" state).
    fn hide(&mut self, ctx: &egui::Context) {
        self.visible = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
            OFFSCREEN, OFFSCREEN,
        )));
    }

    /// Accept the selected row: hide, then insert `@"<abs>"` at the caret (FR11).
    fn accept(&mut self, ctx: &egui::Context) {
        let Some(&idx) = self.results.get(self.selected) else {
            return;
        };
        let text = reference::at_quoted(&self.index[idx].abs);
        let target = self.target_hwnd;
        self.hide(ctx);
        // Off the UI thread so the event loop keeps spinning during the
        // clipboard save / paste / restore dance.
        std::thread::spawn(move || insert_reference(&text, target));
    }

    /// Recompute the result list when the query changed (FR9).
    fn recompute(&mut self) {
        if self.last_query.as_deref() == Some(self.query.as_str()) {
            return;
        }
        self.last_query = Some(self.query.clone());
        self.selected = 0;
        self.results.clear();

        if self.index.is_empty() {
            return;
        }
        if self.query.is_empty() {
            self.results.extend(0..self.index.len().min(MAX_RESULTS));
            return;
        }

        let pattern = Pattern::parse(&self.query, CaseMatching::Smart, Normalization::Smart);
        let mut buf = Vec::new();
        let mut scored: Vec<(u32, usize)> = Vec::new();
        for (i, entry) in self.index.iter().enumerate() {
            let haystack = Utf32Str::new(&entry.rel, &mut buf);
            if let Some(score) = pattern.score(haystack, &mut self.matcher) {
                scored.push((score, i));
            }
        }
        scored.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        self.results
            .extend(scored.into_iter().take(MAX_RESULTS).map(|(_, i)| i));
    }

    /// Re-read config, rebuild the index, re-register the chord (FR2 Reload).
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
                self.index = index::build(&cfg.folders, &cfg.exclude);
                self.last_query = None;
                self.config = cfg;
            }
            Err(e) => error_dialog(&format!(
                "Reload failed:\n{e}\n\nKeeping the previous configuration."
            )),
        }
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

        egui::CentralPanel::default().show(ctx, |ui| {
            if !self.visible {
                return;
            }
            ui.add_space(4.0);
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("type to filter…")
                    .desired_width(f32::INFINITY),
            );
            resp.request_focus();

            self.recompute();
            let n = self.results.len();
            if n > 0 {
                if down {
                    self.selected = (self.selected + 1) % n;
                }
                if up {
                    self.selected = (self.selected + n - 1) % n;
                }
                if self.selected >= n {
                    self.selected = n - 1;
                }
            }

            ui.separator();
            if self.index.is_empty() {
                ui.weak("no files indexed");
            } else if n == 0 {
                ui.weak("no matches");
            } else {
                let mut clicked: Option<usize> = None;
                let normal = ui.visuals().text_color();
                let dim = ui.visuals().weak_text_color();
                for (row, &idx) in self.results.iter().enumerate() {
                    let entry = &self.index[idx];
                    let mut job = egui::text::LayoutJob::default();
                    job.append(entry.name(), 0.0, fmt(normal));
                    let parent = entry.parent_rel();
                    if !parent.is_empty() {
                        job.append(&format!("    {parent}"), 0.0, fmt(dim));
                    }
                    if ui.selectable_label(row == self.selected, job).clicked() {
                        clicked = Some(row);
                    }
                }
                if let Some(row) = clicked {
                    self.selected = row;
                    self.accept(ctx);
                }
            }

            // Enter accepts the selection (captured at frame top — see above).
            if enter {
                self.accept(ctx);
            }
        });

        // Keep the loop ticking as a backstop to the watcher's wakes.
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}

/// A `TextFormat` of the given color at the default font.
fn fmt(color: egui::Color32) -> egui::TextFormat {
    egui::TextFormat {
        color,
        ..Default::default()
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

/// A 16×16 solid teal tray icon (placeholder art for v0.1).
fn make_icon() -> Icon {
    let (w, h) = (16u32, 16u32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for _ in 0..(w * h) {
        rgba.extend_from_slice(&[0x16, 0xA3, 0x8A, 0xFF]);
    }
    Icon::from_rgba(rgba, w, h).expect("valid icon")
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
    let index = index::build(&config.folders, &config.exclude);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 360.0])
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_position([OFFSCREEN, OFFSCREEN]),
        ..Default::default()
    };

    eframe::run_native(
        "atref",
        native_options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, config_path, home, config, index)))),
    )
}
