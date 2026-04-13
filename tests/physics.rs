use bigdecimal::BigDecimal;
use billiards::{
    gearing_english, Angle, CutAngle, Displacement, Inches, InchesPerSecond, Pocket, Position,
};

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
    let omega = gearing_english(CutAngle::new(0.0), shot_speed_ips(10));

    assert_close(omega.as_f64(), 0.0);
}

#[test]
fn gearing_english_uses_radians_for_the_cut_angle() {
    let omega = gearing_english(CutAngle::new(30.0), shot_speed_ips(10));

    assert_close(omega.as_f64(), 10.0 * 0.5 / 1.125);
}

#[test]
fn given_cue_and_object_ball_headings_when_measuring_cut_angle_then_the_acute_impact_magnitude_is_returned() {
    let cue_heading = Angle::from_north(0.5, 0.866_025_403_784_438_6);
    let object_ball_heading = Angle::from_north(0.0, 1.0);
    let opposite_line_heading = Angle::from_north(0.0, -1.0);

    let cut_angle = CutAngle::from_headings(cue_heading, object_ball_heading);
    let cut_angle_from_opposite_line = CutAngle::from_headings(cue_heading, opposite_line_heading);

    assert_close(cut_angle.as_degrees(), 30.0);
    assert_close(cut_angle_from_opposite_line.as_degrees(), 30.0);
}

#[test]
fn given_a_three_four_displacement_when_measuring_its_angle_from_north_then_the_expected_heading_is_returned() {
    let displacement = Displacement::new("3", "4");

    let angle = displacement.angle_from_north();

    assert_close(angle.to_string().parse().expect("angle degrees"), 36.869_897_645_844_02);
}

#[test]
fn given_a_position_and_a_pocket_when_measuring_to_and_from_the_pocket_then_the_directions_are_opposites() {
    let position = Position::new(2u8, 4u8);

    let to_pocket = position.angle_to_pocket(Pocket::TopRight);
    let from_pocket = position.angle_from_pocket(Pocket::TopRight);
    let to_pocket_degrees = to_pocket.to_string().parse::<f64>().expect("to-pocket angle");
    let from_pocket_degrees = from_pocket
        .to_string()
        .parse::<f64>()
        .expect("from-pocket angle");

    assert!(to_pocket_degrees > 0.0);
    assert!(to_pocket_degrees < 90.0);
    assert_close(from_pocket_degrees, (to_pocket_degrees + 180.0).rem_euclid(360.0));
}
