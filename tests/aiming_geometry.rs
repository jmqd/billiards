use bigdecimal::ToPrimitive;
use billiards::{
    Angle, Ball, BallSpec, BallType, Diamond, InchesPerSecond, Pocket, Position, TableSpec,
};

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
fn given_an_object_ball_and_destination_when_locating_the_ghost_ball_then_it_sits_one_ball_diameter_behind_the_object_ball(
) {
    let table = TableSpec::default();
    let object_ball = object_ball_at("2", "6");

    let ghost_ball = object_ball.ghost_ball(&Position::new(2u8, 8u8), &table);

    assert_close(diamond_value(&ghost_ball.x), 2.0);
    assert_close(diamond_value(&ghost_ball.y), 5.82);
}

#[test]
fn given_a_straight_in_shot_when_calculating_the_aim_angle_then_it_points_straight_at_the_ghost_ball(
) {
    let table = TableSpec::default();
    let object_ball = object_ball_at("2", "6");
    let shooting_position = Position::new(2u8, 4u8);

    let angle = object_ball.aim_angle(&Position::new(2u8, 8u8), &shooting_position, &table);

    assert_close(angle_degrees(angle), 0.0);
}

#[test]
fn given_an_object_ball_and_pocket_when_calculating_the_aim_angle_then_the_pocket_helper_uses_the_slow_effective_target(
) {
    let table = TableSpec::default();
    let object_ball = object_ball_at("2", "6");
    let shooting_position = Position::new(2u8, 4u8);
    let target_center =
        object_ball.pocket_target_center(Pocket::TopRight, InchesPerSecond::new("0"), &table);

    let angle_via_destination = object_ball.aim_angle(&target_center, &shooting_position, &table);
    let angle_via_pocket =
        object_ball.aim_angle_to_pocket(Pocket::TopRight, &shooting_position, &table);

    assert_close(
        angle_degrees(angle_via_pocket),
        angle_degrees(angle_via_destination),
    );
}

#[test]
fn angled_pocket_target_centers_shift_with_the_effective_target_model_and_speed() {
    let table = TableSpec::default();
    let object_ball = object_ball_at("2", "5");
    let shooting_position = Position::new(1u8, 5u8);
    let pocket_center = Pocket::CenterRight.aiming_center();

    let slow_target =
        object_ball.pocket_target_center(Pocket::CenterRight, InchesPerSecond::new("0"), &table);
    let fast_target =
        object_ball.pocket_target_center(Pocket::CenterRight, InchesPerSecond::new("120"), &table);
    let slow_angle =
        object_ball.aim_angle_to_pocket(Pocket::CenterRight, &shooting_position, &table);
    let fast_angle = object_ball.aim_angle_to_pocket_with_speed(
        Pocket::CenterRight,
        &shooting_position,
        InchesPerSecond::new("120"),
        &table,
    );

    assert_close(
        diamond_value(&slow_target.x),
        diamond_value(&pocket_center.x),
    );
    assert_close(
        diamond_value(&fast_target.x),
        diamond_value(&pocket_center.x),
    );
    assert!(
        (diamond_value(&slow_target.y) - diamond_value(&pocket_center.y)).abs() > 1e-6,
        "an angled side-pocket entry should not target the geometric mouth center"
    );
    assert!(
        (diamond_value(&fast_target.y) - diamond_value(&slow_target.y)).abs() > 1e-6,
        "fast TP 3.7/3.8 targets should differ from slow TP 3.5/3.6 targets"
    );
    assert_close(
        angle_degrees(slow_angle),
        angle_degrees(object_ball.aim_angle(&slow_target, &shooting_position, &table)),
    );
    assert_close(
        angle_degrees(fast_angle),
        angle_degrees(object_ball.aim_angle(&fast_target, &shooting_position, &table)),
    );
}

#[test]
fn over_limit_fast_side_pocket_target_falls_back_to_geometric_center() {
    let table = TableSpec::default();
    let object_ball = object_ball_at("2", "7");
    let pocket_center = Pocket::CenterRight.aiming_center();

    let fast_target =
        object_ball.pocket_target_center(Pocket::CenterRight, InchesPerSecond::new("120"), &table);

    assert_close(
        diamond_value(&fast_target.x),
        diamond_value(&pocket_center.x),
    );
    assert_close(
        diamond_value(&fast_target.y),
        diamond_value(&pocket_center.y),
    );
}
