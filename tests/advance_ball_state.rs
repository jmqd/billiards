use billiards::{
    advance_angular_velocity_on_table, advance_ball_state, compute_next_transition,
    AngularVelocity3, BallSetPhysicsSpec, BallState, Inches2, InchesPerSecondSq, MotionPhase,
    MotionPhaseConfig, MotionTransitionConfig, RadiansPerSecondSq, RollingResistanceModel, Seconds,
    SlidingFrictionModel, SpinDecayModel, Velocity2, TYPICAL_BALL_RADIUS,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

fn motion_config() -> MotionTransitionConfig {
    MotionTransitionConfig {
        phase: MotionPhaseConfig::default(),
        sliding_friction: SlidingFrictionModel::ConstantAcceleration {
            acceleration_magnitude: InchesPerSecondSq::new("5"),
        },
        spin_decay: SpinDecayModel::ConstantAngularDeceleration {
            angular_deceleration: RadiansPerSecondSq::new(2.0),
        },
        rolling_resistance: RollingResistanceModel::ConstantDeceleration {
            linear_deceleration: InchesPerSecondSq::new("5"),
        },
    }
}

fn rolling_state() -> BallState {
    let radius = TYPICAL_BALL_RADIUS.clone();
    BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius.as_f64(), 0.0, 0.0),
    )
}

fn rolling_with_vertical_spin_state() -> BallState {
    let radius = TYPICAL_BALL_RADIUS.clone();
    BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius.as_f64(), 0.0, 6.0),
    )
}

fn sliding_stun_state() -> BallState {
    BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    )
}

fn spinning_state() -> BallState {
    BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    )
}

#[test]
fn advancing_a_resting_ball_leaves_it_unchanged() {
    let state = BallState::resting_at(Inches2::new("12.5", "37.25"));
    let advanced = advance_ball_state(
        &state,
        Seconds::new(3.0),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );

    assert_eq!(advanced, state);
}

#[test]
fn advancing_angular_velocity_on_table_depends_on_ball_state_and_total_spin() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    );

    let angular = advance_angular_velocity_on_table(
        &state,
        Seconds::new(2.0 / 7.0),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );

    assert_close(angular.x().as_f64(), -25.0 / (7.0 * radius.as_f64()));
    assert_close(angular.y().as_f64(), 0.0);
    assert_close(angular.z().as_f64(), 38.0 / 7.0);
}

#[test]
fn advancing_a_sliding_stun_ball_halfway_matches_the_section_7_3_linear_velocity_and_spin_equations(
) {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = sliding_stun_state();
    let dt = Seconds::new(2.0 / 7.0);

    let advanced = advance_ball_state(&state, dt, &BallSetPhysicsSpec::default(), &motion_config());

    assert_close(advanced.position.x().as_f64(), 10.0);
    assert_close(advanced.position.y().as_f64(), 20.0 + 130.0 / 49.0);
    assert_close(advanced.velocity.x().as_f64(), 0.0);
    assert_close(advanced.velocity.y().as_f64(), 60.0 / 7.0);
    assert_close(
        advanced.angular_velocity.x().as_f64(),
        -25.0 / (7.0 * radius.as_f64()),
    );
    assert_close(advanced.angular_velocity.y().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.z().as_f64(), 0.0);
    assert_eq!(advanced.motion_phase(radius), MotionPhase::Sliding);
}

#[test]
fn advancing_a_sliding_stun_ball_to_the_transition_time_reaches_pure_rolling() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = sliding_stun_state();
    let config = motion_config();
    let transition_time = compute_next_transition(&state, &BallSetPhysicsSpec::default(), &config)
        .expect("sliding balls should predict a rolling transition")
        .time_until_transition;

    let advanced = advance_ball_state(
        &state,
        transition_time,
        &BallSetPhysicsSpec::default(),
        &config,
    );

    assert_close(advanced.position.x().as_f64(), 10.0);
    assert_close(advanced.position.y().as_f64(), 20.0 + 240.0 / 49.0);
    assert_close(advanced.velocity.x().as_f64(), 0.0);
    assert_close(advanced.velocity.y().as_f64(), 50.0 / 7.0);
    assert_close(
        advanced.angular_velocity.x().as_f64(),
        -50.0 / (7.0 * radius.as_f64()),
    );
    assert_close(advanced.angular_velocity.y().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.z().as_f64(), 0.0);
    assert_eq!(advanced.motion_phase(radius.clone()), MotionPhase::Rolling);
    assert_close(advanced.cloth_contact_speed(radius).as_f64(), 0.0);
}

#[test]
fn advancing_past_the_sliding_transition_continues_into_rolling_for_the_remaining_time() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = sliding_stun_state();
    let config = motion_config();
    let transition_time = compute_next_transition(&state, &BallSetPhysicsSpec::default(), &config)
        .expect("sliding balls should predict a rolling transition")
        .time_until_transition;

    let advanced = advance_ball_state(
        &state,
        Seconds::new(transition_time.as_f64() + 1.0),
        &BallSetPhysicsSpec::default(),
        &config,
    );

    assert_close(advanced.position.x().as_f64(), 10.0);
    assert_close(advanced.position.y().as_f64(), 20.0 + 935.0 / 98.0);
    assert_close(advanced.velocity.x().as_f64(), 0.0);
    assert_close(advanced.velocity.y().as_f64(), 15.0 / 7.0);
    assert_close(
        advanced.angular_velocity.x().as_f64(),
        -15.0 / (7.0 * radius.as_f64()),
    );
    assert_close(advanced.angular_velocity.y().as_f64(), 0.0);
    assert_eq!(advanced.motion_phase(radius), MotionPhase::Rolling);
}

#[test]
fn advancing_a_pure_spinning_ball_leaves_position_fixed_and_decays_z_spin_linearly() {
    let state = spinning_state();

    let advanced = advance_ball_state(
        &state,
        Seconds::new(1.0),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );

    assert_close(advanced.position.x().as_f64(), 10.0);
    assert_close(advanced.position.y().as_f64(), 20.0);
    assert_close(advanced.speed().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.x().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.y().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.z().as_f64(), 4.0);
    assert_eq!(
        advanced.motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Spinning
    );
}

#[test]
fn advancing_a_rolling_ball_with_vertical_spin_can_enter_the_spinning_phase() {
    let state = rolling_with_vertical_spin_state();

    let advanced = advance_ball_state(
        &state,
        Seconds::new(2.5),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );

    assert_close(advanced.position.x().as_f64(), 10.0);
    assert_close(advanced.position.y().as_f64(), 30.0);
    assert_close(advanced.speed().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.x().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.y().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.z().as_f64(), 1.0);
    assert_eq!(
        advanced.motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Spinning
    );
}

#[test]
fn advancing_a_rolling_ball_updates_position_speed_and_spin_consistently() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = rolling_state();
    let advanced = advance_ball_state(
        &state,
        Seconds::new(1.0),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );

    assert_close(advanced.position.x().as_f64(), 10.0);
    assert_close(advanced.position.y().as_f64(), 27.5);
    assert_close(advanced.speed().as_f64(), 5.0);
    assert_close(advanced.velocity.x().as_f64(), 0.0);
    assert_close(advanced.velocity.y().as_f64(), 5.0);
    assert_close(
        advanced.angular_velocity.x().as_f64(),
        -5.0 / radius.as_f64(),
    );
    assert_close(advanced.angular_velocity.y().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.z().as_f64(), 0.0);
    assert_eq!(advanced.motion_phase(radius), MotionPhase::Rolling);
}

#[test]
fn advancing_by_the_predicted_stop_time_reaches_rest() {
    let state = rolling_state();
    let config = motion_config();
    let stop_time = compute_next_transition(&state, &BallSetPhysicsSpec::default(), &config)
        .expect("rolling balls should predict a rest transition")
        .time_until_transition;

    let advanced = advance_ball_state(&state, stop_time, &BallSetPhysicsSpec::default(), &config);

    assert_close(advanced.position.x().as_f64(), 10.0);
    assert_close(advanced.position.y().as_f64(), 30.0);
    assert_close(advanced.speed().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.x().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.y().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.z().as_f64(), 0.0);
    assert_eq!(
        advanced.motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Rest
    );
}

#[test]
fn advancing_past_the_predicted_stop_time_clamps_at_the_same_rest_state() {
    let state = rolling_state();
    let config = motion_config();
    let stop_time = compute_next_transition(&state, &BallSetPhysicsSpec::default(), &config)
        .expect("rolling balls should predict a rest transition")
        .time_until_transition;

    let at_stop = advance_ball_state(&state, stop_time, &BallSetPhysicsSpec::default(), &config);
    let after_stop = advance_ball_state(
        &state,
        Seconds::new(stop_time.as_f64() + 1.0),
        &BallSetPhysicsSpec::default(),
        &config,
    );

    assert_eq!(after_stop, at_stop);
}

#[test]
fn advancing_by_zero_seconds_is_an_identity_operation() {
    let state = rolling_state();
    let advanced = advance_ball_state(
        &state,
        Seconds::zero(),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );

    assert_eq!(advanced, state);
}
