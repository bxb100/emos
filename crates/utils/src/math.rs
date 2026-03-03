pub fn normalize_to_1_100(x: usize, min: usize, max: usize) -> u32 {
    let y = 1.0 + (x - min) as f64 / (max - min) as f64 * 99.0;
    y.round() as u32
}

#[test]
fn test_normalize() {
    assert_eq!(normalize_to_1_100(0, 0, 100), 1);
    assert_eq!(normalize_to_1_100(50, 0, 100), 51);
    assert_eq!(normalize_to_1_100(100, 0, 100), 100);
    assert_eq!(normalize_to_1_100(150, 0, 150), 100);
}
