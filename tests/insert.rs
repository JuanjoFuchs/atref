//! Integration test for the insertion mechanism — the "Win32 EDIT fixture" the
//! agentic-GUI doc reserves for insertion paste. Proves the @-quoted reference
//! (`@"<abs>"`) round-trips through the clipboard and pastes verbatim into a
//! standard Windows text field. Deterministic: it sets the clipboard and sends
//! `WM_PASTE` (no synthetic input, no focus, no timing). `insert_reference`'s
//! synthetic-Ctrl+V + foreground-restore is the per-app manual spot-check.

use std::path::Path;

use windows::core::w;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GetWindowTextLengthW, GetWindowTextW, SendMessageW,
    WINDOW_EX_STYLE, WM_PASTE, WS_POPUP,
};

use atref::reference;

unsafe fn window_text(hwnd: HWND) -> String {
    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (len + 1) as usize];
    let n = GetWindowTextW(hwnd, &mut buf);
    String::from_utf16_lossy(&buf[..n as usize])
}

#[test]
fn at_quoted_reference_pastes_into_a_text_field() {
    unsafe {
        // A standalone EDIT control — the text-field fixture.
        let edit = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("EDIT"),
            w!(""),
            WS_POPUP,
            0,
            0,
            400,
            40,
            None,
            None,
            HINSTANCE(std::ptr::null_mut()),
            None,
        )
        .expect("create EDIT fixture");

        // Exactly what `insert_reference` puts on the clipboard for this file.
        let text = reference::at_quoted(Path::new(r"C:\notes\Weekly Review.md"));
        {
            let mut cb = arboard::Clipboard::new().expect("clipboard");
            cb.set_text(text.clone()).expect("set clipboard");
        }

        // Paste it into the field (WM_PASTE is synchronous on this thread).
        let _ = SendMessageW(edit, WM_PASTE, WPARAM(0), LPARAM(0));

        let got = window_text(edit);
        let _ = DestroyWindow(edit);

        assert_eq!(got, text, "the @-quoted reference should paste verbatim");
    }
}
