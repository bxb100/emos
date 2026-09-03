use jiff::Timestamp;
use jiff::civil::DateTime;

/// 1-5000
pub fn normalize_to_1_5000(x: i64, min: i64, max: i64) -> i64 {
    assert!(x >= min && x <= max, "{x} must be between {min} and {max}");

    // edge case
    let x = x.max(1);
    if max < 5000 {
        return x;
    }

    let y = 1.0 + (x - min) as f64 / (max - min) as f64 * 4999.0;
    (y.round() as i64).clamp(1, 5000)
}

/// 1-100
pub fn normalize_date<T: AsRef<str>>(date: Option<T>) -> i64 {
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
        5001 - normalize_to_1_5000(x, 0, max)
    } else {
        1
    }
}

#[test]
fn test_normalize_date() {
    assert!(normalize_date(Some("2026-06-01T00:00:00Z")) <= 5000);
    assert!(normalize_date(Some("1888-10-15")) > 0);
}

#[test]
fn test_normalize() {
    assert_eq!(normalize_to_1_5000(0, 0, 100), 1);
    assert_eq!(normalize_to_1_5000(50, 0, 100), 50);
    assert_eq!(normalize_to_1_5000(100, 0, 100), 100);
    assert_eq!(normalize_to_1_5000(150, 0, 150), 150);
}
