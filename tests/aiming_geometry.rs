use bigdecimal::ToPrimitive;
use billiards::{Angle, Ball, BallSpec, BallType, Diamond, Pocket, Position, TableSpec};

fn diamond_value(diamond: &Diamond) -> f64 {
    diamond.magnitude.to_f64().expect("diamond magnitude")
}

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

fn object_ball_at(x: &str, y: &str) -> Ball {
    Ball {
        ty: BallType::Eight,
        position: Position::new(x, y),
        spec: BallSpec::default(),
    }
}

#[test]
fn given_an_object_ball_and_destination_when_locating_the_center_to_center_ghost_ball_then_it_sits_one_ball_diameter_behind_the_object_ball(
) {
    let table = TableSpec::default();
    let object_ball = object_ball_at("2", "6");

    let ghost_ball = object_ball.center_to_center_ghost_ball(&Position::new(2u8, 8u8), &table);

    assert_close(diamond_value(&ghost_ball.x), 2.0);
    assert_close(diamond_value(&ghost_ball.y), 5.82);
}

#[test]
fn given_a_straight_in_shot_when_calculating_the_center_to_center_potting_angle_then_it_points_straight_at_the_ghost_ball(
) {
    let table = TableSpec::default();
    let object_ball = object_ball_at("2", "6");
    let shooting_position = Position::new(2u8, 4u8);

    let angle = object_ball.center_to_center_potting_angle(
        &Position::new(2u8, 8u8),
        &shooting_position,
        &table,
    );

    assert_close(angle_degrees(angle), 0.0);
}

#[test]
fn given_an_object_ball_and_pocket_when_calculating_the_center_to_center_potting_angle_then_the_pocket_helper_uses_the_pocket_opening_center(
) {
    let table = TableSpec::default();
    let object_ball = object_ball_at("2", "6");
    let shooting_position = Position::new(2u8, 4u8);

    let angle_via_destination = object_ball.center_to_center_potting_angle(
        &Pocket::TopRight.aiming_center(),
        &shooting_position,
        &table,
    );
    let angle_via_pocket = object_ball.center_to_center_potting_angle_to_pocket(
        Pocket::TopRight,
        &shooting_position,
        &table,
    );

    assert_close(
        angle_degrees(angle_via_pocket),
        angle_degrees(angle_via_destination),
    );
}
