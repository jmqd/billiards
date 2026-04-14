use billiards::{
    AngularVelocity3, BallState, Inches2, MotionPhase, Position, TableSpec,
    TYPICAL_BALL_RADIUS, Velocity2,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

#[test]
fn default_ball_state_is_a_resting_ball_at_the_simulation_origin() {
    let state = BallState::default();

    assert_close(state.position.x().as_f64(), 0.0);
    assert_close(state.position.y().as_f64(), 0.0);
    assert_close(state.height.as_f64(), 0.0);
    assert_close(state.speed().as_f64(), 0.0);
    assert_close(state.vertical_velocity.as_f64(), 0.0);
    assert_close(state.angular_velocity.x().as_f64(), 0.0);
    assert_close(state.angular_velocity.y().as_f64(), 0.0);
    assert_close(state.angular_velocity.z().as_f64(), 0.0);
    assert_eq!(state.motion_phase(TYPICAL_BALL_RADIUS.clone()), MotionPhase::Rest);
}

#[test]
fn resting_at_preserves_position_and_zeroes_all_motion() {
    let state = BallState::resting_at(Inches2::new("12.5", "37.25"));

    assert_close(state.position.x().as_f64(), 12.5);
    assert_close(state.position.y().as_f64(), 37.25);
    assert_close(state.height.as_f64(), 0.0);
    assert_close(state.speed().as_f64(), 0.0);
    assert_close(state.vertical_velocity.as_f64(), 0.0);
    assert_eq!(state.motion_phase(TYPICAL_BALL_RADIUS.clone()), MotionPhase::Rest);
}

#[test]
fn from_position_and_projected_position_round_trip_through_table_inches() {
    let table = TableSpec::default();
    let position = Position::new("2.75", "5.5");

    let state = BallState::from_position(&position, &table);
    let round_tripped = state.projected_position(&table);

    assert_close(state.position.x().as_f64(), 34.375);
    assert_close(state.position.y().as_f64(), 68.75);
    assert_close(round_tripped.x.magnitude.to_string().parse().expect("x"), 2.75);
    assert_close(round_tripped.y.magnitude.to_string().parse().expect("y"), 5.5);
}

#[test]
fn rolling_without_slip_has_zero_cloth_contact_speed_and_classifies_as_rolling() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = BallState::on_table(
        Inches2::zero(),
        Velocity2::new("6", "8"),
        AngularVelocity3::new(-8.0 / radius.as_f64(), 6.0 / radius.as_f64(), 0.0),
    );

    let contact_velocity = state.cloth_contact_velocity(radius.clone());

    assert_close(contact_velocity.x().as_f64(), 0.0);
    assert_close(contact_velocity.y().as_f64(), 0.0);
    assert_close(state.cloth_contact_speed(radius.clone()).as_f64(), 0.0);
    assert_eq!(state.motion_phase(radius), MotionPhase::Rolling);
}

#[test]
fn a_stationary_ball_with_only_vertical_axis_spin_classifies_as_spinning() {
    let state = BallState::on_table(
        Inches2::zero(),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 3.0),
    );

    assert_eq!(state.motion_phase(TYPICAL_BALL_RADIUS.clone()), MotionPhase::Spinning);
}

#[test]
fn a_ball_with_height_or_vertical_velocity_classifies_as_airborne() {
    let by_height = BallState::airborne(
        Inches2::new("1", "2"),
        "0.5",
        Velocity2::new("0", "0"),
        "0",
        AngularVelocity3::zero(),
    );
    let by_vertical_velocity = BallState::new(
        Inches2::new("1", "2"),
        "0",
        Velocity2::new("0", "0"),
        "1.25",
        AngularVelocity3::zero(),
    );

    assert_eq!(by_height.motion_phase(TYPICAL_BALL_RADIUS.clone()), MotionPhase::Airborne);
    assert_eq!(
        by_vertical_velocity.motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Airborne
    );
}
