//! atref — a Windows system-tray file-reference picker.
//!
//! With no arguments this launches the resident GUI: a tray icon plus a global
//! chord that summons a borderless WebView2 picker near the cursor; `Enter`
//! inserts `@"<abs path>"` at the caret of the previously-focused app. With
//! arguments it runs the agent CLI (spec 007) against `config.json` and exits.
//! Config is a hand-edited JSON file; the GUI and its acrylic window come from
//! `tauri.conf.json`, the picker view from `ui/`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

fn main() {
    let dirs = directories::BaseDirs::new().expect("resolve base directories");
    // Test seam: `ATREF_DIR` overrides atref's config + data directory so an E2E
    // harness can run against an isolated config.json without ever touching the
    // user's real %APPDATA%\atref. Unset in normal use. The GUI honors the same
    // seam internally (see `atref::run`).
    let atref_dir = std::env::var_os("ATREF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs.config_dir().join("atref"));
    let config_path = atref_dir.join("config.json");
    let home = dirs.home_dir().to_path_buf();

    // CLI mode (spec 007): any arguments run a subcommand and exit — no tray, no
    // GUI. No arguments falls through to launch the resident picker.
    let cli_args: Vec<String> = std::env::args().skip(1).collect();
    if !cli_args.is_empty() {
        attach_parent_console();
        let out = atref::cli::run(&cli_args, &config_path, &home);
        if !out.stdout.is_empty() {
            println!("{}", out.stdout);
        }
        if !out.stderr.is_empty() {
            eprintln!("{}", out.stderr);
        }
        std::process::exit(out.code);
    }

    atref::run();
}

/// Best-effort console attach for CLI mode (spec 007): a release build has no
/// console of its own (`windows_subsystem = "windows"`), so attach to the parent
/// console — when there is one — and point stdout/stderr at it. Only acts when
/// stdout isn't already a valid handle, so piped/redirected output (an agent
/// capturing JSON) is never clobbered. No-op when launched without a parent
/// console (e.g. from Explorer). Debug builds already have a console.
#[cfg(windows)]
fn attach_parent_console() {
    use std::os::windows::io::IntoRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Console::{
        AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE,
        STD_OUTPUT_HANDLE,
    };
    unsafe {
        if matches!(GetStdHandle(STD_OUTPUT_HANDLE), Ok(h) if !h.is_invalid() && !h.0.is_null()) {
            return; // stdout already valid (pipe / redirect) — leave it untouched
        }
        if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
            return; // no parent console
        }
        if let Ok(f) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("CONOUT$")
        {
            let h = HANDLE(f.into_raw_handle());
            let _ = SetStdHandle(STD_OUTPUT_HANDLE, h);
            let _ = SetStdHandle(STD_ERROR_HANDLE, h);
        }
    }
}

#[cfg(not(windows))]
fn attach_parent_console() {}
