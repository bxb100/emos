use jiff::Timestamp;
use jiff::civil::DateTime;

/// Clamp small rankings directly; scale larger ranges to scores from 1 through 5000.
pub(crate) fn normalize_score(x: i64, min: i64, max: i64) -> i64 {
    if min >= max {
        return 1;
    }

    if max < 5000 {
        return x.clamp(1, max);
    }

    let scale = ((x - min) as f64 / (max - min) as f64).clamp(0.0, 1.0) * 4999.0 + 1.0;
    scale.round() as i64
}

/// Score timestamps from 1 through 5000, with lower values for more recent dates.
/// Missing dates receive a score of 1.
pub(crate) fn recency_score<T: AsRef<str>>(date: Option<T>) -> i64 {
    if let Some(date_str) = date {
        let x = if let Ok(x) = date_str.as_ref().parse::<Timestamp>() {
            x.as_second()
        } else if let Ok(date_x) = date_str.as_ref().parse::<DateTime>() {
            date_x
                .duration_since(DateTime::constant(1970, 1, 1, 0, 0, 0, 0))
                .as_secs()
        } else {
            1
        };
        let max = Timestamp::now().as_second();
        5001 - normalize_score(x, 0, max)
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recency_score() {
        assert!(recency_score(Some("2026-06-01T00:00:00Z")) <= 5000);
        assert!(recency_score(Some("1888-10-15")) > 0);
    }

    #[test]
    fn small_rankings_keep_their_scale() {
        assert_eq!(normalize_score(0, 0, 100), 1);
        assert_eq!(normalize_score(50, 0, 100), 50);
        assert_eq!(normalize_score(100, 0, 100), 100);
        assert_eq!(normalize_score(150, 0, 150), 150);
    }

    #[test]
    fn values_above_the_range_stay_within_score_bounds() {
        let result = normalize_score(1788652800, 0, 1788645017);
        assert!((1..=5000).contains(&result), "result: {}", result);
    }
}
