use bigdecimal::ToPrimitive;
use billiards::{Angle, Diamond, Position, TableSpec, CENTER_SPOT};

fn angle_degrees(angle: Angle) -> f64 {
    angle.to_string().parse().expect("angle degrees")
}

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

#[test]
fn given_cardinal_vectors_when_measuring_angles_from_north_then_expected_degrees_are_returned() {
    assert_close(angle_degrees(Angle::from_north(0.0, 1.0)), 0.0);
    assert_close(angle_degrees(Angle::from_north(1.0, 0.0)), 90.0);
    assert_close(angle_degrees(Angle::from_north(0.0, -1.0)), 180.0);
    assert_close(angle_degrees(Angle::from_north(-1.0, 0.0)), 270.0);
}

#[test]
fn given_an_angle_when_flipped_then_it_points_180_degrees_opposite() {
    let original = Angle::from_north(1.0, 1.0);

    let flipped = original.flipped();

    assert_close(angle_degrees(flipped), 225.0);
}

#[test]
fn given_standard_table_when_converting_diamonds_to_inches_and_back_then_the_value_round_trips() {
    let table = TableSpec::default();

    let diamonds = Diamond::from("2.75");
    let round_tripped = table.inches_to_diamond(table.diamond_to_inches(diamonds.clone()));

    assert_close(
        round_tripped.magnitude.to_f64().expect("diamond magnitude"),
        diamonds.magnitude.to_f64().expect("diamond magnitude"),
    );
}

#[test]
fn given_the_center_spot_when_checking_side_helpers_then_it_is_on_neither_side_of_either_axis() {
    assert!(!CENTER_SPOT.is_left_of_center());
    assert!(!CENTER_SPOT.is_right_of_center());
    assert!(!CENTER_SPOT.is_above_center());
    assert!(!CENTER_SPOT.is_below_center());
}

#[test]
fn given_positions_on_the_center_lines_when_checking_side_helpers_then_only_strict_relations_are_true(
) {
    let above_center = Position::new(2u8, 6u8);
    let right_of_center = Position::new(3u8, 4u8);

    assert!(!above_center.is_left_of_center());
    assert!(!above_center.is_right_of_center());
    assert!(above_center.is_above_center());
    assert!(!above_center.is_below_center());

    assert!(!right_of_center.is_left_of_center());
    assert!(right_of_center.is_right_of_center());
    assert!(!right_of_center.is_above_center());
    assert!(!right_of_center.is_below_center());
}
