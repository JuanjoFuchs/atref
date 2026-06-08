//! Frecency scoring — frequency × bucketed recency (spec 005 FR5). Pure and
//! free of I/O or `now()` so it can be unit-tested deterministically: callers
//! pass the already-computed `age` (time since the last pick). fzf-inspired
//! recency buckets give recent picks a strong, decaying edge.

use std::time::Duration;

const HOUR: u64 = 3_600;
const DAY: u64 = 24 * HOUR;
const WEEK: u64 = 7 * DAY;
const MONTH: u64 = 30 * DAY;

/// Recency multiplier for a pick `age` old (fzf-style buckets):
/// ≤1h ×4, ≤1d ×2, ≤1wk ×1, ≤30d ×0.5, older ×0.25.
pub fn recency_weight(age: Duration) -> f64 {
    let secs = age.as_secs();
    if secs <= HOUR {
        4.0
    } else if secs <= DAY {
        2.0
    } else if secs <= WEEK {
        1.0
    } else if secs <= MONTH {
        0.5
    } else {
        0.25
    }
}

/// `frecency = pick_count × recency_weight(age)`; a never-picked file
/// (`count == 0`) scores `0.0` (FR5).
pub fn score(count: u32, age: Duration) -> f64 {
    if count == 0 {
        0.0
    } else {
        count as f64 * recency_weight(age)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_picked_scores_zero() {
        // AC8: count 0 → 0.0 regardless of age (the age is meaningless then).
        assert_eq!(score(0, Duration::from_secs(0)), 0.0);
        assert_eq!(score(0, Duration::from_secs(WEEK)), 0.0);
    }

    #[test]
    fn recency_buckets_match_fr5() {
        // AC8: each bucket boundary maps to the documented weight.
        assert_eq!(recency_weight(Duration::from_secs(0)), 4.0);
        assert_eq!(recency_weight(Duration::from_secs(HOUR)), 4.0);
        assert_eq!(recency_weight(Duration::from_secs(HOUR + 1)), 2.0);
        assert_eq!(recency_weight(Duration::from_secs(DAY)), 2.0);
        assert_eq!(recency_weight(Duration::from_secs(DAY + 1)), 1.0);
        assert_eq!(recency_weight(Duration::from_secs(WEEK)), 1.0);
        assert_eq!(recency_weight(Duration::from_secs(WEEK + 1)), 0.5);
        assert_eq!(recency_weight(Duration::from_secs(MONTH)), 0.5);
        assert_eq!(recency_weight(Duration::from_secs(MONTH + 1)), 0.25);
    }

    #[test]
    fn frequency_scales_linearly_within_a_bucket() {
        // AC8: count is a linear multiplier on the recency weight.
        assert_eq!(score(1, Duration::from_secs(0)), 4.0);
        assert_eq!(score(3, Duration::from_secs(0)), 12.0);
        // A more-recent single pick can outrank an older, more-frequent one…
        assert!(score(1, Duration::from_secs(0)) > score(3, Duration::from_secs(MONTH + 1)));
        // …but enough frequency overcomes a one-bucket recency gap.
        assert!(score(5, Duration::from_secs(DAY)) > score(1, Duration::from_secs(0)));
    }
}
