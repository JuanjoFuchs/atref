//! Build script: Tauri's build step. It embeds the frontend assets (`ui/`), the
//! Windows `.exe` icon, and validates `tauri.conf.json` against the schema.
fn main() {
    tauri_build::build();
}
