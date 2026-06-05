//! Snapshot tests for the picker's look & feel (spec 004 AC1, AC3–AC7). The
//! rendered picker is diffed against an approved baseline PNG under
//! `tests/snapshots/`. Create or refresh baselines with:
//!     UPDATE_SNAPSHOTS=1 cargo test --test picker
//! The baselines are committed; JJ approves them once by eye.
//!
//! Each `egui_kittest` snapshot spins up a wgpu device; creating several
//! concurrently crashes the GPU driver (STATUS_ACCESS_VIOLATION). A global lock
//! serializes the device work so the default parallel test runner is safe, while
//! each test keeps its own single `SnapshotResults`.

use std::sync::Mutex;

use atref::picker::{self, Row};
use egui_kittest::Harness;

static SNAPSHOT_LOCK: Mutex<()> = Mutex::new(());

fn rows(items: &[(&str, &str)]) -> Vec<Row> {
    items
        .iter()
        .map(|(name, location)| Row {
            name: name.to_string(),
            location: location.to_string(),
        })
        .collect()
}

fn snapshot(
    name: &str,
    mut query: String,
    rows: Vec<Row>,
    selected: usize,
    matches: usize,
    total: usize,
) {
    // Serialize wgpu device creation across tests (concurrent devices crash).
    let _guard = SNAPSHOT_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    // Fixed window size so the pinned footer + fill behave like the real app.
    let mut harness = Harness::builder()
        .with_size([720.0, 460.0])
        .build(move |ctx| {
            picker::install_theme(ctx);
            let _ = picker::render(ctx, &mut query, &rows, selected, false, matches, total);
        });
    // Two frames: the theme set on frame 1 takes effect on frame 2.
    harness.run();
    harness.run();
    harness.snapshot(name);
}

#[test]
fn picker_empty() {
    // Empty query: all files listed, first selected.
    snapshot(
        "picker_empty",
        String::new(),
        rows(&[
            ("📦 atref.md", "second-brain"),
            ("AGENTS.md", "atref"),
            ("Cargo.toml", "atref"),
        ]),
        0,
        3,
        3,
    );
}

#[test]
fn picker_results() {
    // A query matching a few of many files, with a non-first selection.
    snapshot(
        "picker_results",
        "atref".to_string(),
        rows(&[
            ("📦 atref.md", "second-brain"),
            ("001-windows-mvp.md", "atref/specs"),
            ("AGENTS.md", "atref"),
        ]),
        1,
        3,
        1204,
    );
}

#[test]
fn picker_emoji() {
    // AC7: emoji-led filenames render legibly and keep the name visible.
    snapshot(
        "picker_emoji",
        String::new(),
        rows(&[
            ("📦 atref.md", "second-brain"),
            ("🏠 Family.md", "second-brain"),
            ("💼 Work.md", "second-brain"),
        ]),
        0,
        3,
        3,
    );
}
