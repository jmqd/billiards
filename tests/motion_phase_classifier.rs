use billiards::{
    classify_motion_phase, cloth_contact_speed_on_table, projected_position, AngularVelocity3,
    BallSetPhysicsSpec, BallState, Inches2, MotionPhase, MotionPhaseConfig, Position,
    SlidingToRollingModel, TableSpec, TYPICAL_BALL_RADIUS, Velocity2,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

#[test]
fn a_resting_ball_is_classified_as_rest() {
    let state = BallState::resting_at(Inches2::new("12.5", "37.25"));

    assert_eq!(
        classify_motion_phase(&state, &BallSetPhysicsSpec::default(), &MotionPhaseConfig::default()),
        MotionPhase::Rest
    );
}

#[test]
fn a_stationary_ball_with_only_z_spin_is_classified_as_spinning() {
    let state = BallState::on_table(
        Inches2::zero(),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 3.0),
    );

    assert_eq!(
        classify_motion_phase(&state, &BallSetPhysicsSpec::default(), &MotionPhaseConfig::default()),
        MotionPhase::Spinning
    );
}

#[test]
fn rolling_without_slip_is_classified_as_rolling() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = BallState::on_table(
        Inches2::zero(),
        Velocity2::new("6", "8"),
        AngularVelocity3::new(-8.0 / radius.as_f64(), 6.0 / radius.as_f64(), 0.0),
    );

    assert_close(cloth_contact_speed_on_table(&state, radius.clone()).as_f64(), 0.0);
    assert_eq!(
        classify_motion_phase(
            &state,
            &BallSetPhysicsSpec { radius },
            &MotionPhaseConfig {
                sliding_to_rolling: SlidingToRollingModel::Thresholded {
                    contact_speed_epsilon: billiards::InchesPerSecond::new("0.000001"),
                },
                ..MotionPhaseConfig::default()
            }
        ),
        MotionPhase::Rolling
    );
}

#[test]
fn a_draw_ball_is_classified_as_sliding() {
    let state = BallState::on_table(
        Inches2::zero(),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(4.0, 0.0, 0.0),
    );

    assert!(cloth_contact_speed_on_table(&state, TYPICAL_BALL_RADIUS.clone()).as_f64() > 0.0);
    assert_eq!(
        classify_motion_phase(&state, &BallSetPhysicsSpec::default(), &MotionPhaseConfig::default()),
        MotionPhase::Sliding
    );
}

#[test]
fn an_overspin_ball_is_classified_as_sliding_until_roll_develops() {
    let state = BallState::on_table(
        Inches2::zero(),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-12.0, 0.0, 0.0),
    );

    assert!(cloth_contact_speed_on_table(&state, TYPICAL_BALL_RADIUS.clone()).as_f64() > 0.0);
    assert_eq!(
        classify_motion_phase(&state, &BallSetPhysicsSpec::default(), &MotionPhaseConfig::default()),
        MotionPhase::Sliding
    );
}

#[test]
fn a_ball_with_vertical_state_is_classified_as_airborne() {
    let state = BallState::airborne(
        Inches2::new("1", "2"),
        "0.5",
        Velocity2::zero(),
        "0",
        AngularVelocity3::zero(),
    );

    assert_eq!(
        classify_motion_phase(&state, &BallSetPhysicsSpec::default(), &MotionPhaseConfig::default()),
        MotionPhase::Airborne
    );
}

#[test]
fn projected_position_free_function_round_trips_through_table_inches() {
    let table = TableSpec::default();
    let position = Position::new("2.75", "5.5");

    let state = BallState::from_position(&position, &table);
    let round_tripped = projected_position(&state, &table);

    assert_close(round_tripped.x.magnitude.to_string().parse().expect("x"), 2.75);
    assert_close(round_tripped.y.magnitude.to_string().parse().expect("y"), 5.5);
}
