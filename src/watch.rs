//! Live file-watcher seam (spec 002 FR6–FR8). Lives in the lib and is agnostic
//! of the GUI's message type — callers pass an `on_change(Vec<Entry>)` closure —
//! so the watcher is testable headlessly: a test owns the closure's channel and
//! the returned guard. The returned value keeps the watcher alive; drop it to
//! stop watching.

use std::any::Any;
use std::path::PathBuf;
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult};

use crate::index::{self, Entry};

/// Watch `folders` recursively. On each debounced batch of filesystem changes,
/// rebuild the index — re-applying the same git / hidden / exclude filters as
/// the initial build, so a newly-created ignored or hidden file is never added
/// (FR6/FR7) — and hand the fresh index to `on_change`. Returns an opaque guard
/// that must be kept alive (dropping it stops watching), or `None` if the
/// watcher could not be created.
pub fn spawn(
    folders: Vec<PathBuf>,
    exclude: Vec<String>,
    git_aware: bool,
    debounce: Duration,
    on_change: impl Fn(Vec<Entry>) + Send + 'static,
) -> Option<Box<dyn Any>> {
    let build_folders = folders.clone();
    let mut debouncer = new_debouncer(debounce, None, move |res: DebounceEventResult| {
        if res.is_ok() {
            let index = index::build(&build_folders, &exclude, git_aware);
            on_change(index);
        }
    })
    .ok()?;

    for root in &folders {
        // Swallow per-folder errors so one bad path doesn't stop the rest (FR7).
        let _ = debouncer.watch(root, RecursiveMode::Recursive);
    }

    Some(Box::new(debouncer) as Box<dyn Any>)
}
