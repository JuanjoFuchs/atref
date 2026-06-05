//! Pure filtering + ranking — the testable seam for the picker (spec 002 FR5,
//! FR9). Kept free of egui/OS so ranking is verified by plain unit tests.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Matcher, Utf32Str};

use crate::index::Entry;

/// Rank `entries` for `query`, returning indices into `entries` (best first,
/// at most `max`).
///
/// Ordering: nucleo fuzzy score DESC, then folder priority (`root_rank`) ASC,
/// then path ASC. An empty query lists entries in folder-priority order. The
/// match runs against each entry's path-relative-to-root, which yields
/// CamelCase / initialism ("CamelHumps") bonuses via nucleo's path-aware config.
pub fn rank(query: &str, entries: &[Entry], matcher: &mut Matcher, max: usize) -> Vec<usize> {
    if entries.is_empty() {
        return Vec::new();
    }

    if query.is_empty() {
        let mut idx: Vec<usize> = (0..entries.len()).collect();
        idx.sort_by(|&a, &b| {
            entries[a]
                .root_rank
                .cmp(&entries[b].root_rank)
                .then_with(|| entries[a].rel.cmp(&entries[b].rel))
        });
        idx.truncate(max);
        return idx;
    }

    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut buf = Vec::new();
    let mut scored: Vec<(u32, usize)> = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        let haystack = Utf32Str::new(&entry.rel, &mut buf);
        if let Some(score) = pattern.score(haystack, matcher) {
            scored.push((score, i));
        }
    }
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| entries[a.1].root_rank.cmp(&entries[b.1].root_rank))
            .then_with(|| entries[a.1].rel.cmp(&entries[b.1].rel))
    });
    scored.into_iter().take(max).map(|(_, i)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nucleo_matcher::Config as MatcherConfig;
    use std::path::PathBuf;

    fn entry(rel: &str, root_rank: usize) -> Entry {
        Entry {
            abs: PathBuf::from(rel),
            root: PathBuf::new(),
            rel: rel.to_string(),
            root_rank,
        }
    }

    fn matcher() -> Matcher {
        Matcher::new(MatcherConfig::DEFAULT.match_paths())
    }

    #[test]
    fn folder_priority_breaks_ties() {
        // AC5: equal-scoring matches → lower root_rank first, regardless of the
        // order they appear in the index.
        let entries = vec![entry("notes.md", 1), entry("notes.md", 0)];
        let res = rank("notes", &entries, &mut matcher(), 10);
        assert_eq!(res, vec![1, 0], "root_rank 0 (entries[1]) ranks first");

        // Reversing the folders reverses the ranking.
        let reversed = vec![entry("notes.md", 0), entry("notes.md", 1)];
        let res = rank("notes", &reversed, &mut matcher(), 10);
        assert_eq!(res, vec![0, 1]);
    }

    #[test]
    fn empty_query_is_folder_priority_order() {
        // FR5: empty query lists files in (root_rank, path) order.
        let entries = vec![entry("b.md", 1), entry("a.md", 0), entry("c.md", 0)];
        let res = rank("", &entries, &mut matcher(), 10);
        // root_rank 0 first (a.md, c.md by path), then root_rank 1 (b.md).
        assert_eq!(res, vec![1, 2, 0]);
    }

    #[test]
    fn camelhumps_camelcase_initialism() {
        // AC10: `mclfi` finds and ranks `MyClassFile` at the top.
        let entries = vec![
            entry("MyOtherThing.rs", 0),
            entry("MyClassFile.rs", 0),
            entry("readme.md", 0),
        ];
        let res = rank("mclfi", &entries, &mut matcher(), 10);
        assert!(!res.is_empty(), "initialism should match");
        assert_eq!(entries[res[0]].rel, "MyClassFile.rs");
    }

    #[test]
    fn camelhumps_word_boundary_initialism() {
        // AC10: `fsfg` finds and ranks `Finite Seasons Family Gift` at the top.
        let entries = vec![
            entry("Finance Spreadsheet.md", 0),
            entry("Finite Seasons Family Gift.md", 0),
            entry("notes.md", 0),
        ];
        let res = rank("fsfg", &entries, &mut matcher(), 10);
        assert!(!res.is_empty(), "initialism should match");
        assert_eq!(entries[res[0]].rel, "Finite Seasons Family Gift.md");
    }
}
