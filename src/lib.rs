//! atref core — the pure, unit-testable logic shared by the binary.
//!
//! The GUI, tray, hotkey, and OS-injection wiring live in `main.rs`; everything
//! that can be tested without a screen lives here.

pub mod config;
pub mod frecency;
pub mod icon;
pub mod index;
pub mod picker;
pub mod reference;
pub mod search;
pub mod store;
pub mod watch;
