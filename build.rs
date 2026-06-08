//! Build script: embed the application icon (`assets/icon.ico`) into the Windows
//! `.exe` so Explorer, the taskbar, and Alt-Tab show atref's `@` mark. The tray
//! icon is separate (embedded raw RGBA, loaded at runtime — see `src/icon.rs`);
//! this is only the executable's resource icon.
//!
//! No-op on non-Windows targets. If the Windows resource compiler is unavailable
//! the build still succeeds (the icon just isn't embedded) — atref is otherwise
//! fully functional, so a missing icon shouldn't break `cargo install`.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=assets/icon.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        if let Err(e) = res.compile() {
            println!(
                "cargo:warning=could not embed .exe icon (resource compiler unavailable?): {e}"
            );
        }
    }
}
