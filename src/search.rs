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

/// Match positions for `query` over `rel`, as Unicode code-point indices into
/// `rel` — produced by the same nucleo pattern construction [`rank`] scores
/// with, so highlights are the ranker's own match (spec 009 FR3/TC1). Sorted
/// and deduplicated (nucleo documents its indices as possibly unordered and
/// duplicated). Empty for an empty query or a non-matching row. Intended to be
/// called only for the rows a search returns, not the whole index (NFR1).
pub fn match_indices(rel: &str, query: &str, matcher: &mut Matcher) -> Vec<u32> {
    if query.is_empty() {
        return Vec::new();
    }
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut buf = Vec::new();
    let haystack = Utf32Str::new(rel, &mut buf);
    let mut indices = Vec::new();
    if pattern.indices(haystack, matcher, &mut indices).is_none() {
        return Vec::new();
    }
    indices.sort_unstable();
    indices.dedup();
    indices
}

/// Split rel-relative match positions onto the two strings a row displays
/// (spec 009 TC2): the basename, and the location line (`root_name`, plus
/// `/`-joined parent when the file is nested — `\` → `/` preserves char
/// counts, so positions carry over by offset). The boundary separator between
/// parent and basename is shown in neither string, so positions landing on it
/// are dropped. Returns `(name_hl, loc_hl)` as code-point indices into the
/// displayed strings.
pub fn split_highlights(rel: &str, root_name: &str, indices: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let sep = rel
        .chars()
        .enumerate()
        .filter(|(_, c)| *c == '\\' || *c == '/')
        .map(|(i, _)| i as u32)
        .last();
    let Some(sep) = sep else {
        // Root-level file: the whole rel is the basename; the location line is
        // just the root folder name, which contains no rel characters.
        return (indices.to_vec(), Vec::new());
    };
    let root_chars = root_name.chars().count() as u32;
    let mut name_hl = Vec::new();
    let mut loc_hl = Vec::new();
    for &idx in indices {
        if idx > sep {
            name_hl.push(idx - sep - 1);
        } else if idx < sep {
            loc_hl.push(root_chars + 1 + idx);
        }
    }
    (name_hl, loc_hl)
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
            size: 0,
            mtime: 0,
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
    fn indices_come_from_the_scoring_pattern() {
        // Spec 009 AC1: the indices-producing call returns the same score the
        // ranking call produced — highlights are the ranker's own match, not a
        // second divergent pass.
        let mut m = matcher();
        let hay = "specs\\009-match-highlighting.md";
        let pattern = Pattern::parse("spec", CaseMatching::Smart, Normalization::Smart);
        let mut buf = Vec::new();
        let score = pattern.score(Utf32Str::new(hay, &mut buf), &mut m);
        let mut raw = Vec::new();
        let indices_score = pattern.indices(Utf32Str::new(hay, &mut buf), &mut m, &mut raw);
        assert_eq!(score, indices_score, "same match, same score");
        assert!(score.is_some());
        assert!(!raw.is_empty());

        let got = match_indices(hay, "spec", &mut m);
        assert!(!got.is_empty());
        assert!(got.windows(2).all(|w| w[0] < w[1]), "sorted + deduplicated");
    }

    #[test]
    fn match_indices_empty_query_or_no_match_is_empty() {
        // Spec 009 AC4 (payload side): empty query → nothing to highlight; a
        // non-matching row yields nothing either.
        let mut m = matcher();
        assert!(match_indices("notes.md", "", &mut m).is_empty());
        assert!(match_indices("notes.md", "zzqx", &mut m).is_empty());
    }

    #[test]
    fn match_indices_non_ascii_positions_are_code_points() {
        // Spec 009 AC2: positions index code points, not bytes — `é` counts as
        // one position, matching JS `Array.from` iteration.
        let mut m = matcher();
        let got = match_indices("nota-café.md", "café", &mut m);
        assert_eq!(got, vec![5, 6, 7, 8], "c-a-f-é at code points 5..=8");
    }

    #[test]
    fn match_indices_camelhumps_is_non_contiguous() {
        // Spec 009 AC3: an initialism query highlights the actual humps nucleo
        // matched — non-contiguous positions whose chars echo the query.
        let mut m = matcher();
        let hay = "FiniteSeasonsGift.md";
        let got = match_indices(hay, "fsg", &mut m);
        assert_eq!(got.len(), 3);
        assert!(
            got.windows(2).any(|w| w[1] - w[0] > 1),
            "humps are not adjacent: {got:?}"
        );
        let chars: Vec<char> = hay.chars().collect();
        let hit: String = got
            .iter()
            .map(|&i| chars[i as usize].to_ascii_lowercase())
            .collect();
        assert_eq!(hit, "fsg", "highlighted chars echo the query");
    }

    #[test]
    fn split_highlights_maps_to_displayed_strings() {
        // Spec 009 AC2: rel positions split onto basename + location, location
        // offset past the root folder name and the joining `/`; the boundary
        // separator (index 4 here) is dropped — it is displayed in neither.
        let (name_hl, loc_hl) = split_highlights("docs\\demo.mp4", "atref", &[0, 1, 4, 5, 6]);
        assert_eq!(name_hl, vec![0, 1], "d, e of demo.mp4");
        assert_eq!(loc_hl, vec![6, 7], "d, o of docs after `atref/`");
    }

    #[test]
    fn split_highlights_root_level_file_has_no_location_positions() {
        // Spec 009 AC2: a root-level file displays its whole rel as the name;
        // the location line is the bare root folder name (no rel chars).
        let (name_hl, loc_hl) = split_highlights("notes.md", "vault", &[0, 1, 2]);
        assert_eq!(name_hl, vec![0, 1, 2]);
        assert!(loc_hl.is_empty());
    }

    #[test]
    fn split_highlights_nested_parent_keeps_inner_separators() {
        // A multi-level parent displays its inner separators (as `/`), so a
        // position on one survives the mapping; only the boundary one drops.
        // rel: a\b\c.md → sep at 3; location "root/a/b".
        let (name_hl, loc_hl) = split_highlights("a\\b\\c.md", "root", &[0, 1, 2, 3, 4]);
        assert_eq!(name_hl, vec![0], "c of c.md");
        assert_eq!(loc_hl, vec![5, 6, 7], "a, /, b after `root/`");
    }

    #[test]
    fn match_indices_timing_smoke_over_synthetic_corpus() {
        // Spec 009 AC5/NFR1: extraction for a returned page of rows is cheap.
        // Generous debug-build bound — this guards against accidentally running
        // indices over the whole corpus instead of the returned rows.
        let entries: Vec<Entry> = (0..5000)
            .map(|i| entry(&format!("dir{}\\note-file-{i}.md", i % 40), 0))
            .collect();
        let mut m = matcher();
        let (idx, _) = rank("note", &entries, &[], &mut m, 50);
        assert_eq!(idx.len(), 50);
        let start = std::time::Instant::now();
        for &i in &idx {
            let positions = match_indices(&entries[i].rel, "note", &mut m);
            assert!(!positions.is_empty());
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed < std::time::Duration::from_millis(250),
            "50 rows took {elapsed:?}"
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
