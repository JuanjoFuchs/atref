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

/// Watch a single config file and invoke `on_change` (debounced) whenever *that
/// file* is created, modified, or replaced — including atomic replace-on-save,
/// by watching its parent directory and filtering on the file name. Sibling
/// files in the same directory (e.g. the `index.redb` store, which is written
/// often) are ignored, so they never trigger a reload. Returns an opaque guard
/// that must be kept alive (dropping it stops watching), or `None` if the watch
/// could not be created (spec 006).
pub fn spawn_config(
    config_path: PathBuf,
    debounce: Duration,
    on_change: impl Fn() + Send + 'static,
) -> Option<Box<dyn Any>> {
    let dir = config_path.parent()?.to_path_buf();
    let file_name = config_path.file_name()?.to_os_string();
    let mut debouncer = new_debouncer(debounce, None, move |res: DebounceEventResult| {
        if let Ok(events) = res {
            // A rename/replace surfaces the destination path, so matching on the
            // file name catches both in-place writes and atomic replace-on-save.
            let touched = events
                .iter()
                .flat_map(|ev| ev.paths.iter())
                .any(|p| p.file_name() == Some(file_name.as_os_str()));
            if touched {
                on_change();
            }
        }
    })
    .ok()?;
    // Non-recursive: the config dir holds only config.json + the store; we filter
    // to config.json above.
    debouncer.watch(&dir, RecursiveMode::NonRecursive).ok()?;

    Some(Box::new(debouncer) as Box<dyn Any>)
}
