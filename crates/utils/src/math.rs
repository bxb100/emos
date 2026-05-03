use jiff::Timestamp;
use jiff::civil::{DateTime};

pub fn normalize_to_1_100(x: i64, min: i64, max: i64) -> i64 {
    let y = 1.0 + (x - min) as f64 / (max - min) as f64 * 99.0;
    y.round() as i64
}

pub fn normalize_date<T: AsRef<str>>(date: Option<T>) -> i64 {
    if let Some(date_str) = date {
        let x = if let Ok(x) = date_str.as_ref().parse::<Timestamp>() {
            x.as_second()
        } else if let Ok(date_x) = date_str.as_ref().parse::<DateTime>() {
            date_x.duration_since(DateTime::constant(1970, 1, 1, 0, 0, 0, 0)).as_secs()
        } else {
            0
        };
        let max = Timestamp::now().as_second();
        100 - normalize_to_1_100(x, 0, max)
    } else {
        0
    }
}

#[test]
fn test_normalize_date() {
    normalize_date(Some("2026-01-01T00:00:00Z"));
    normalize_date(Some("1999-10-15"));
}

#[test]
fn test_normalize() {
    assert_eq!(normalize_to_1_100(0, 0, 100), 1);
    assert_eq!(normalize_to_1_100(50, 0, 100), 51);
    assert_eq!(normalize_to_1_100(100, 0, 100), 100);
    assert_eq!(normalize_to_1_100(150, 0, 150), 100);
}
