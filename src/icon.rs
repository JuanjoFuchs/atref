//! Tray-icon asset — the designed `@` mark as raw RGBA8 bytes, embedded directly
//! so the binary needs no image-decoding dependency (keeping `cargo install`
//! lean, the same reason redb was chosen over bundled SQLite).
//!
//! The bytes are the 32x32, straight (non-premultiplied) RGBA decode of the
//! icon. Regenerate from `assets/icon.svg` via `tools/` (render -> downscale to
//! 32 -> extract raw RGBA). See `ai-docs/icon-design.md`.

/// Tray icon dimensions in pixels.
pub const TRAY_W: u32 = 32;
pub const TRAY_H: u32 = 32;

/// Straight (non-premultiplied) RGBA8 pixels for the tray icon, row-major,
/// top-to-bottom — ready for `tray_icon::Icon::from_rgba`.
pub const TRAY_RGBA: &[u8] = include_bytes!("../assets/icon-32.rgba");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_icon_asset_is_well_formed() {
        // Guards the embedded asset: right size, and not fully transparent (so a
        // truncated/empty .rgba fails the test instead of shipping a blank tray).
        assert_eq!(
            TRAY_RGBA.len(),
            (TRAY_W * TRAY_H * 4) as usize,
            "RGBA buffer is W*H*4"
        );
        assert!(
            TRAY_RGBA.chunks_exact(4).any(|p| p[3] > 0),
            "icon has opaque pixels (the teal tile)"
        );
    }
}
