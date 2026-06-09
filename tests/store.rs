//! Integration tests for the persistent store (spec 005 AC1–AC5). Uses a real
//! redb file in a unique temp path (no `tempfile` dep, matching the other
//! integration tests). The entries reference paths that need not exist on disk
//! — size/mtime are whatever the entry carries from index time (spec 010 FR1).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use atref::index::Entry;
use atref::store::Store;

/// A unique store path per test (tag) so tests can run in parallel.
fn unique_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("atref_it_store_{tag}_{}.redb", std::process::id()))
}

fn entry(root: &str, rel: &str, root_rank: usize) -> Entry {
    let root = PathBuf::from(root);
    Entry {
        abs: root.join(rel),
        root,
        rel: rel.to_string(),
        root_rank,
        size: 42,
        mtime: 1_700_000_000,
    }
}

/// Sorted relative paths, for order-independent comparison.
fn rels(entries: &[Entry]) -> Vec<String> {
    let mut v: Vec<String> = entries.iter().map(|e| e.rel.clone()).collect();
    v.sort();
    v
}

#[test]
fn round_trips_entries_across_reopen() {
    // AC1/AC3: persist an index, reopen the store with a fresh handle, and load
    // it back unchanged — i.e. the next launch loads the cache, no walk needed.
    let path = unique_path("roundtrip");
    let _ = fs::remove_file(&path);

    let entries = vec![
        entry(r"D:\vault", "a.md", 0),
        entry(r"D:\vault", r"sub\b.md", 0),
        entry(r"D:\code", "c.rs", 1),
    ];
    {
        let store = Store::open_or_reset(&path);
        store.persist(&entries);
    } // drop the handle so the reopen below is a genuinely separate open

    let store = Store::open_or_reset(&path);
    let loaded = store.load_entries();
    assert_eq!(rels(&loaded), rels(&entries), "all rows survive a reopen");

    let by_rel = |rel: &str| loaded.iter().find(|e| e.rel == rel).unwrap();
    assert_eq!(by_rel("a.md").root_rank, 0, "root_rank preserved");
    assert_eq!(by_rel("c.rs").root_rank, 1, "root_rank preserved");
    assert_eq!(by_rel("a.md").abs, PathBuf::from(r"D:\vault\a.md"));
    assert_eq!(by_rel("a.md").root, PathBuf::from(r"D:\vault"));
    // Spec 010 AC1: size/mtime round-trip, so rows render size with no stat
    // or content read on the search path.
    assert_eq!(by_rel("a.md").size, 42, "size preserved");
    assert_eq!(by_rel("a.md").mtime, 1_700_000_000, "mtime preserved");

    let _ = fs::remove_file(&path);
}

#[test]
fn reconcile_adds_new_and_drops_deleted() {
    // AC2: a second persist (the background reconcile) reflects added files and
    // drops removed ones, so the cache matches the filesystem.
    let path = unique_path("reconcile");
    let _ = fs::remove_file(&path);
    let store = Store::open_or_reset(&path);

    store.persist(&[entry(r"D:\v", "a.md", 0), entry(r"D:\v", "b.md", 0)]);
    store.persist(&[entry(r"D:\v", "b.md", 0), entry(r"D:\v", "c.md", 0)]);

    let loaded = rels(&store.load_entries());
    assert_eq!(loaded, vec!["b.md".to_string(), "c.md".to_string()]);
    assert!(
        !loaded.contains(&"a.md".to_string()),
        "deleted file dropped"
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn records_picks_with_count_and_recent_time() {
    // AC5: accepting a file records a pick (count++, last-picked = now).
    let path = unique_path("picks");
    let _ = fs::remove_file(&path);
    let store = Store::open_or_reset(&path);

    let picked = Path::new(r"D:\v\notes.md");
    store.record_pick(picked);
    store.record_pick(picked);
    store.record_pick(picked);

    let frec = store.load_frecency();
    let (count, last) = frec
        .get(&PathBuf::from(r"D:\v\notes.md"))
        .copied()
        .expect("pick was recorded");
    assert_eq!(count, 3, "three picks counted");
    let age = SystemTime::now().duration_since(last).unwrap_or_default();
    assert!(age < Duration::from_secs(60), "last-picked is recent");
    assert!(
        !frec.contains_key(&PathBuf::from(r"D:\v\never.md")),
        "never-picked files have no row"
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn rebuilds_corrupt_store_without_panicking() {
    // AC4: a corrupt/unreadable file is rebuilt instead of crashing.
    let path = unique_path("corrupt");
    let _ = fs::remove_file(&path);
    fs::write(&path, b"this is not a redb database, just junk bytes").unwrap();

    // Must not panic; a rebuilt store starts empty.
    let store = Store::open_or_reset(&path);
    assert!(
        store.load_entries().is_empty(),
        "a rebuilt store starts empty"
    );

    // …and it is fully usable afterward.
    store.persist(&[entry(r"D:\v", "fresh.md", 0)]);
    assert_eq!(rels(&store.load_entries()), vec!["fresh.md".to_string()]);

    let _ = fs::remove_file(&path);
}
