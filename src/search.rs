//! Pure filtering + ranking — the testable seam for the picker (spec 002 FR5,
//! FR9; spec 005 FR6 frecency). Kept free of egui/OS so ranking is verified by
//! plain unit tests.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Matcher, Utf32Str};

use crate::index::Entry;

/// Width of a fuzzy-score "bucket". Frecency only reorders matches whose nucleo
/// scores fall in the same bucket (near-equal); a match in a higher bucket
/// always ranks first, so frecency can't float a clearly-worse fuzzy match
/// above a clearly-better one (spec 005 FR6 — bounded boost). nucleo path
/// scores run from a few dozen for scattered hits into the hundreds for clean
/// ones, so 24 groups genuinely-similar matches while keeping clear winners
/// apart.
const SCORE_BUCKET: u32 = 24;

/// Rank `entries` for `query`. Returns `(order, total)` where `order` is the
/// indices of the best `max` matches (best first) and `total` is the count of
/// ALL matching entries (for the picker's `matches / total` counter).
///
/// `frecency` is a per-entry score parallel to `entries` (see [`crate::frecency`]);
/// pass `&[]` to ignore it (every entry scores `0.0`, reproducing the spec-002
/// ordering). An **empty query** lists entries by frecency DESC, then folder
/// priority (`root_rank`) ASC, then path ASC — so recents lead and the rest keep
/// the spec-002 order. A **non-empty query** orders by nucleo fuzzy score
/// (bucketed — see [`SCORE_BUCKET`]), then frecency, then folder priority, then
/// path. Matching runs against each entry's path-relative-to-root, which yields
/// CamelCase / initialism ("CamelHumps") bonuses via nucleo's path-aware config.
pub fn rank(
    query: &str,
    entries: &[Entry],
    frecency: &[f64],
    matcher: &mut Matcher,
    max: usize,
) -> (Vec<usize>, usize) {
    if entries.is_empty() {
        return (Vec::new(), 0);
    }
    let frec = |i: usize| frecency.get(i).copied().unwrap_or(0.0);

    if query.is_empty() {
        let mut idx: Vec<usize> = (0..entries.len()).collect();
        idx.sort_by(|&a, &b| {
            frec(b)
                .total_cmp(&frec(a))
                .then_with(|| entries[a].root_rank.cmp(&entries[b].root_rank))
                .then_with(|| entries[a].rel.cmp(&entries[b].rel))
        });
        let total = idx.len();
        idx.truncate(max);
        return (idx, total);
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
    let total = scored.len();
    scored.sort_by(|a, b| {
        (b.0 / SCORE_BUCKET)
            .cmp(&(a.0 / SCORE_BUCKET))
            .then_with(|| frec(b.1).total_cmp(&frec(a.1)))
            .then_with(|| entries[a.1].root_rank.cmp(&entries[b.1].root_rank))
            .then_with(|| entries[a.1].rel.cmp(&entries[b.1].rel))
    });
    let order = scored.into_iter().take(max).map(|(_, i)| i).collect();
    (order, total)
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
        let (res, total) = rank("notes", &entries, &[], &mut matcher(), 10);
        assert_eq!(res, vec![1, 0], "root_rank 0 (entries[1]) ranks first");
        assert_eq!(total, 2, "both files match");

        // Reversing the folders reverses the ranking.
        let reversed = vec![entry("notes.md", 0), entry("notes.md", 1)];
        let (res, _) = rank("notes", &reversed, &[], &mut matcher(), 10);
        assert_eq!(res, vec![0, 1]);
    }

    #[test]
    fn empty_query_is_folder_priority_order() {
        // FR5: empty query lists files in (root_rank, path) order; total = all.
        let entries = vec![entry("b.md", 1), entry("a.md", 0), entry("c.md", 0)];
        let (res, total) = rank("", &entries, &[], &mut matcher(), 10);
        assert_eq!(res, vec![1, 2, 0]);
        assert_eq!(total, 3);
    }

    #[test]
    fn caps_order_at_max_but_counts_all_matches() {
        // The counter reflects ALL matches even when only `max` are shown.
        let entries: Vec<Entry> = (0..25).map(|i| entry(&format!("note{i}.md"), 0)).collect();
        let (res, total) = rank("note", &entries, &[], &mut matcher(), 10);
        assert_eq!(res.len(), 10, "only top `max` shown");
        assert_eq!(total, 25, "but all matches counted");
    }

    #[test]
    fn camelhumps_camelcase_initialism() {
        // AC10: `mclfi` finds and ranks `MyClassFile` at the top.
        let entries = vec![
            entry("MyOtherThing.rs", 0),
            entry("MyClassFile.rs", 0),
            entry("readme.md", 0),
        ];
        let (res, _) = rank("mclfi", &entries, &[], &mut matcher(), 10);
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
        let (res, _) = rank("fsfg", &entries, &[], &mut matcher(), 10);
        assert!(!res.is_empty(), "initialism should match");
        assert_eq!(entries[res[0]].rel, "Finite Seasons Family Gift.md");
    }

    #[test]
    fn empty_query_orders_by_frecency_then_folder() {
        // AC6: an empty query leads with the most-frecent file; ties fall back to
        // folder priority then path (all-zero frecency == spec-002 order).
        let entries = vec![entry("a.md", 0), entry("b.md", 0), entry("c.md", 1)];
        let frecency = vec![0.0, 8.0, 2.0]; // b most frecent, then c, then a
        let (res, total) = rank("", &entries, &frecency, &mut matcher(), 10);
        assert_eq!(res, vec![1, 2, 0], "frecency DESC: b, c, a");
        assert_eq!(total, 3);
    }

    #[test]
    fn query_frecency_breaks_near_equal_ties() {
        // AC7: equal-scoring matches (same rel) → the more-frecent one wins,
        // overriding the folder-priority tiebreak.
        let entries = vec![entry("notes.md", 0), entry("notes.md", 1)];
        let frecency = vec![0.0, 10.0]; // entries[1] far more frecent
        let (res, _) = rank("notes", &entries, &frecency, &mut matcher(), 10);
        assert_eq!(
            res,
            vec![1, 0],
            "frecency edges out the equal-scoring stranger"
        );
    }

    #[test]
    fn query_frecency_does_not_beat_clearly_better_match() {
        // AC7: a clearly-better fuzzy match outranks a clearly-worse one even
        // when the worse one is heavily frecent (bounded boost — the two land in
        // different score buckets).
        let entries = vec![entry("readme.md", 0), entry("rxexaxdxmxe.md", 0)];
        let frecency = vec![0.0, 1000.0]; // pile frecency on the worse match
        let (res, _) = rank("readme", &entries, &frecency, &mut matcher(), 10);
        assert_eq!(
            entries[res[0]].rel, "readme.md",
            "the better fuzzy match still wins"
        );
    }
}
