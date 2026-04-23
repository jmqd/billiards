use billiards::{
    AngularVelocity3, BallState, Inches2, MotionPhase, MotionPhaseThresholds, OnTableBallState,
    OnTableStateError, Position, RestingOnTableBallState, TableSpec, Velocity2,
    TYPICAL_BALL_RADIUS,
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
    assert_eq!(
        state.motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Rest
    );
}

#[test]
fn resting_at_preserves_position_and_zeroes_all_motion() {
    let state = BallState::resting_at(Inches2::new("12.5", "37.25"));

    assert_close(state.position.x().as_f64(), 12.5);
    assert_close(state.position.y().as_f64(), 37.25);
    assert_close(state.height.as_f64(), 0.0);
    assert_close(state.speed().as_f64(), 0.0);
    assert_close(state.vertical_velocity.as_f64(), 0.0);
    assert_eq!(
        state.motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Rest
    );
}

#[test]
fn on_table_ball_state_accepts_exactly_on_table_states() {
    let validated = OnTableBallState::try_from(BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(1.0, 2.0, 3.0),
    ))
    .expect("exact on-table states should validate");

    assert_close(validated.as_ball_state().height.as_f64(), 0.0);
    assert_close(validated.as_ball_state().vertical_velocity.as_f64(), 0.0);
}

#[test]
fn on_table_ball_state_rejects_nonzero_height_and_vertical_velocity() {
    let by_height = OnTableBallState::try_from(BallState::airborne(
        Inches2::new("1", "2"),
        "0.5",
        Velocity2::zero(),
        "0",
        AngularVelocity3::zero(),
    ));
    let by_vertical_velocity = OnTableBallState::try_from(BallState::new(
        Inches2::new("1", "2"),
        "0",
        Velocity2::zero(),
        "1.25",
        AngularVelocity3::zero(),
    ));

    assert!(by_height.is_err());
    assert!(by_vertical_velocity.is_err());
}

#[test]
fn resting_on_table_ball_state_accepts_exact_resting_states() {
    let resting =
        RestingOnTableBallState::try_from(BallState::resting_at(Inches2::new("10", "20")))
            .expect("exact resting states should validate");

    assert_close(resting.as_ball_state().position.x().as_f64(), 10.0);
    assert_close(resting.as_ball_state().position.y().as_f64(), 20.0);
    assert_close(resting.as_ball_state().speed().as_f64(), 0.0);
    assert_close(resting.as_ball_state().angular_velocity.x().as_f64(), 0.0);
    assert_close(resting.as_ball_state().angular_velocity.y().as_f64(), 0.0);
    assert_close(resting.as_ball_state().angular_velocity.z().as_f64(), 0.0);
    assert_eq!(
        resting
            .as_on_table_ball_state()
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Rest
    );
}

#[test]
fn resting_on_table_ball_state_rejects_on_table_motion() {
    let rolling = RestingOnTableBallState::try_from(BallState::on_table(
        Inches2::zero(),
        Velocity2::new("6", "8"),
        AngularVelocity3::new(
            -8.0 / TYPICAL_BALL_RADIUS.as_f64(),
            6.0 / TYPICAL_BALL_RADIUS.as_f64(),
            0.0,
        ),
    ));
    let spinning = RestingOnTableBallState::try_from(BallState::on_table(
        Inches2::zero(),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 3.0),
    ));

    assert!(rolling.is_err());
    assert!(spinning.is_err());
}

#[test]
fn resting_on_table_ball_state_threshold_validation_accepts_tiny_motion_and_normalizes_to_rest() {
    let thresholds = MotionPhaseThresholds {
        rest_linear_speed: Velocity2::new("0.000001", "0").speed(),
        rest_angular_speed: 0.000001_f64.into(),
        ..MotionPhaseThresholds::default()
    };
    let resting = RestingOnTableBallState::try_new_with_thresholds(
        BallState::on_table(
            Inches2::new("1", "2"),
            Velocity2::new("0.0000005", "0"),
            AngularVelocity3::new(0.0, 0.0, 0.0000005),
        ),
        &thresholds,
    )
    .expect("tiny residual motion within thresholds should validate as resting");

    assert_close(resting.as_ball_state().position.x().as_f64(), 1.0);
    assert_close(resting.as_ball_state().position.y().as_f64(), 2.0);
    assert_close(resting.as_ball_state().speed().as_f64(), 0.0);
    assert_close(resting.as_ball_state().angular_velocity.x().as_f64(), 0.0);
    assert_close(resting.as_ball_state().angular_velocity.y().as_f64(), 0.0);
    assert_close(resting.as_ball_state().angular_velocity.z().as_f64(), 0.0);
}

#[test]
fn on_table_ball_state_threshold_validation_accepts_tiny_vertical_noise() {
    let validated = OnTableBallState::try_new_with_thresholds(
        BallState::new(
            Inches2::new("1", "2"),
            "0.0000000001",
            Velocity2::zero(),
            "0.0000000001",
            AngularVelocity3::zero(),
        ),
        &MotionPhaseThresholds::default(),
    )
    .expect("tiny vertical noise within thresholds should validate");

    assert_close(validated.as_ball_state().height.as_f64(), 0.0);
    assert_close(validated.as_ball_state().vertical_velocity.as_f64(), 0.0);
}

#[test]
fn from_position_and_projected_position_round_trip_through_table_inches() {
    let table = TableSpec::default();
    let position = Position::new("2.75", "5.5");

    let state = BallState::from_position(&position, &table);
    let round_tripped = state.projected_position(&table);

    assert_close(state.position.x().as_f64(), 34.375);
    assert_close(state.position.y().as_f64(), 68.75);
    assert_close(
        round_tripped.x.magnitude.to_string().parse().expect("x"),
        2.75,
    );
    assert_close(
        round_tripped.y.magnitude.to_string().parse().expect("y"),
        5.5,
    );
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

    assert_eq!(
        state.motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Spinning
    );
}

#[test]
fn try_cloth_contact_helpers_reject_airborne_ball_states() {
    let by_height = BallState::airborne(
        Inches2::new("1", "2"),
        "0.5",
        Velocity2::new("3", "4"),
        "0",
        AngularVelocity3::zero(),
    );
    let by_vertical_velocity = BallState::new(
        Inches2::new("1", "2"),
        "0",
        Velocity2::new("3", "4"),
        "1.25",
        AngularVelocity3::zero(),
    );

    assert!(matches!(
        by_height.try_cloth_contact_velocity(TYPICAL_BALL_RADIUS.clone()),
        Err(OnTableStateError::HeightAboveTablePlane { .. })
    ));
    assert!(matches!(
        by_height.try_cloth_contact_speed(TYPICAL_BALL_RADIUS.clone()),
        Err(OnTableStateError::HeightAboveTablePlane { .. })
    ));
    assert!(matches!(
        by_vertical_velocity.try_cloth_contact_velocity(TYPICAL_BALL_RADIUS.clone()),
        Err(OnTableStateError::VerticalVelocityPresent { .. })
    ));
    assert!(matches!(
        by_vertical_velocity.try_cloth_contact_speed(TYPICAL_BALL_RADIUS.clone()),
        Err(OnTableStateError::VerticalVelocityPresent { .. })
    ));
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

    assert_eq!(
        by_height.motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Airborne
    );
    assert_eq!(
        by_vertical_velocity.motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Airborne
    );
}
