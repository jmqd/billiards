use bigdecimal::BigDecimal;
use billiards::{gearing_english, Angle, Inches, InchesPerSecond};

fn shot_speed_ips(ips: i64) -> InchesPerSecond {
    InchesPerSecond::new(Inches {
        magnitude: BigDecimal::from(ips),
    })
}

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

#[test]
fn gearing_english_is_zero_for_a_straight_shot() {
    let angle = Angle::from_north(0.0, 1.0);

    let omega = gearing_english(angle, shot_speed_ips(10));

    assert_close(omega.as_f64(), 0.0);
}

#[test]
fn gearing_english_uses_radians_for_the_cut_angle() {
    let angle = Angle::from_north(0.5, 0.866_025_403_784_438_6);

    let omega = gearing_english(angle, shot_speed_ips(10));

    assert_close(omega.as_f64(), 10.0 * 0.5 / 1.125);
}
