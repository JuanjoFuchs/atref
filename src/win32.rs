//! Win32 OS integration: capture the foreground window + cursor at summon time,
//! position the picker near the cursor, and insert `@"<abs>"` at the caret of the
//! previously-focused app via clipboard-paste-with-restore.

use std::ffi::c_void;
use std::time::Duration;

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetForegroundWindow, SetForegroundWindow,
};

/// Snapshot the foreground window + cursor position (physical px) at chord time,
/// before the picker steals focus.
pub fn capture_foreground_and_cursor() -> (isize, i32, i32) {
    unsafe {
        let hwnd = GetForegroundWindow();
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        (hwnd.0 as isize, pt.x, pt.y)
    }
}

/// Restore focus to a previously-captured window.
pub fn set_foreground(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    unsafe {
        let _ = SetForegroundWindow(HWND(hwnd as *mut c_void));
    }
}

/// Work area (left, top, right, bottom physical px) of the monitor containing
/// `(x, y)`, or `None` if it can't be determined.
pub fn work_area_at(x: i32, y: i32) -> Option<(i32, i32, i32, i32)> {
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
pub fn clamp_to_work_area(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    work: (i32, i32, i32, i32),
) -> (i32, i32) {
    let (left, top, right, bottom) = work;
    let cx = x.min(right - w).max(left);
    let cy = y.min(bottom - h).max(top);
    (cx, cy)
}

/// Insert `text` at the caret of the previously-focused window `target`:
/// save clipboard → write text → restore focus → Ctrl+V → wait → restore.
pub fn insert_reference(text: &str, target: isize) {
    let mut clipboard = arboard::Clipboard::new();
    let saved = clipboard.as_mut().ok().and_then(|c| c.get_text().ok());

    if let Ok(c) = clipboard.as_mut() {
        let _ = c.set_text(text.to_owned());
    }

    set_foreground(target);
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

#[cfg(test)]
mod tests {
    use super::clamp_to_work_area;

    #[test]
    fn clamps_window_to_work_area() {
        let work = (0, 0, 1920, 1080);
        // Fits → unchanged.
        assert_eq!(clamp_to_work_area(100, 100, 720, 460, work), (100, 100));
        // Near the right edge → pushed left to fit.
        assert_eq!(clamp_to_work_area(1900, 500, 720, 460, work), (1200, 500));
        // Near the bottom edge → pushed up to fit.
        assert_eq!(clamp_to_work_area(100, 1000, 720, 460, work), (100, 620));
        // A left-of-primary monitor with negative coords is honored.
        let left_monitor = (-1920, 0, 0, 1080);
        assert_eq!(
            clamp_to_work_area(-50, 100, 720, 460, left_monitor),
            (-720, 100)
        );
    }
}
