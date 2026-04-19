use billiards::{
    advance_ball_state, advance_motion_on_table, advance_spin_on_table,
    advance_within_phase_on_table, compute_next_transition_on_table,
    estimate_post_contact_cue_ball_curve_on_table, AngularVelocity3, BallSetPhysicsSpec, BallState,
    Inches2, InchesPerSecondSq, MotionPhase, MotionPhaseConfig, MotionTransitionConfig,
    OnTableBallState, OnTableMotionConfig, RadiansPerSecondSq, RollingResistanceModel, Seconds,
    SlidingFrictionModel, SpinDecayModel, Velocity2, TYPICAL_BALL_RADIUS,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

fn motion_config() -> OnTableMotionConfig {
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

fn on_table(state: BallState) -> OnTableBallState {
    OnTableBallState::try_from(state).expect("test states should validate as on-table")
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

fn rolling_with_small_vertical_spin_state() -> BallState {
    let radius = TYPICAL_BALL_RADIUS.clone();
    BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius.as_f64(), 0.0, 2.0),
    )
}

fn sliding_stun_state() -> BallState {
    BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    )
}

fn sliding_with_vertical_spin_state() -> BallState {
    BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(0.0, 0.0, 6.0),
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
fn advance_motion_on_table_reports_the_first_transition_crossed() {
    let state = on_table(sliding_stun_state());

    let advanced = advance_motion_on_table(
        &state,
        Seconds::new(1.0),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );

    let transition = advanced
        .transition
        .expect("the first sliding-to-rolling boundary should be reported");
    assert_eq!(transition.phase_before, MotionPhase::Sliding);
    assert_eq!(transition.phase_after, MotionPhase::Rolling);
    assert_eq!(
        advanced.state.motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Rolling
    );
    assert_close(advanced.elapsed.as_f64(), 1.0);
}

#[test]
fn advance_within_phase_on_table_clamps_at_the_phase_boundary() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(sliding_stun_state());

    let advanced = advance_within_phase_on_table(
        &state,
        MotionPhase::Sliding,
        Seconds::new(1.0),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );

    assert_close(advanced.as_ball_state().velocity.y().as_f64(), 50.0 / 7.0);
    assert_eq!(
        advanced.as_ball_state().motion_phase(radius),
        MotionPhase::Rolling
    );
}

#[test]
fn advancing_spin_on_table_depends_on_ball_state_and_total_spin() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    ));

    let angular = advance_spin_on_table(
        &state,
        Seconds::new(2.0 / 7.0),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );
    let curve_angle_radians: f64 = 9.0 / 650.0;
    let base_x = -25.0 / (7.0 * radius.as_f64());

    assert_close(angular.x().as_f64(), base_x * curve_angle_radians.cos());
    assert_close(angular.y().as_f64(), -base_x * curve_angle_radians.sin());
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
fn advancing_a_sliding_ball_with_vertical_spin_curves_during_the_sliding_phase() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = sliding_with_vertical_spin_state();
    let dt = Seconds::new(2.0 / 7.0);

    let advanced = advance_ball_state(&state, dt, &BallSetPhysicsSpec::default(), &motion_config());

    assert!(advanced.position.x().as_f64() > 10.0);
    assert!(advanced.velocity.x().as_f64() > 0.0);
    assert_close(advanced.angular_velocity.z().as_f64(), 38.0 / 7.0);
    assert_eq!(advanced.motion_phase(radius), MotionPhase::Sliding);
}

#[test]
fn the_curve_estimate_starts_immediately_for_a_sliding_state_with_vertical_spin() {
    let state = on_table(sliding_with_vertical_spin_state());
    let config = motion_config();
    let transition_time =
        compute_next_transition_on_table(&state, &BallSetPhysicsSpec::default(), &config)
            .expect("sliding balls should predict a rolling transition")
            .time_until_transition;
    let curve = estimate_post_contact_cue_ball_curve_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &config,
    )
    .expect("sliding state with residual sidespin should have an immediate curve estimate");
    let advanced = advance_motion_on_table(
        &state,
        curve.time_until_curve_completes,
        &BallSetPhysicsSpec::default(),
        &config,
    );
    let heading = advanced
        .state
        .velocity
        .angle_from_north()
        .expect("curve completion should still leave translational speed");

    assert_close(curve.time_until_curve_starts.as_f64(), 0.0);
    assert_close(
        curve.time_until_curve_completes.as_f64(),
        transition_time.as_f64(),
    );
    assert_close(heading.as_degrees(), curve.heading_after_curve.as_degrees());
}

#[test]
fn advancing_a_sliding_stun_ball_to_the_transition_time_reaches_pure_rolling() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(sliding_stun_state());
    let config = motion_config();
    let transition_time =
        compute_next_transition_on_table(&state, &BallSetPhysicsSpec::default(), &config)
            .expect("sliding balls should predict a rolling transition")
            .time_until_transition;

    let advanced = advance_ball_state(
        state.as_ball_state(),
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
    let state = on_table(sliding_stun_state());
    let config = motion_config();
    let transition_time =
        compute_next_transition_on_table(&state, &BallSetPhysicsSpec::default(), &config)
            .expect("sliding balls should predict a rolling transition")
            .time_until_transition;

    let advanced = advance_ball_state(
        state.as_ball_state(),
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
fn advancing_a_rolling_ball_with_vertical_spin_no_longer_curls_once_it_is_in_pure_rolling_motion() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = rolling_with_vertical_spin_state();
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
    assert_close(advanced.angular_velocity.x().as_f64(), -5.0 / radius.as_f64());
    assert_close(advanced.angular_velocity.y().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.z().as_f64(), 4.0);
    assert_eq!(advanced.motion_phase(radius), MotionPhase::Rolling);
}

#[test]
fn the_curve_estimate_is_none_for_a_pure_rolling_state_with_residual_z_spin() {
    let state = on_table(rolling_with_small_vertical_spin_state());

    assert!(estimate_post_contact_cue_ball_curve_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .is_none());
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
    let state = on_table(rolling_state());
    let config = motion_config();
    let stop_time =
        compute_next_transition_on_table(&state, &BallSetPhysicsSpec::default(), &config)
            .expect("rolling balls should predict a rest transition")
            .time_until_transition;

    let advanced = advance_ball_state(
        state.as_ball_state(),
        stop_time,
        &BallSetPhysicsSpec::default(),
        &config,
    );

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
    let state = on_table(rolling_state());
    let config = motion_config();
    let stop_time =
        compute_next_transition_on_table(&state, &BallSetPhysicsSpec::default(), &config)
            .expect("rolling balls should predict a rest transition")
            .time_until_transition;

    let at_stop = advance_ball_state(
        state.as_ball_state(),
        stop_time,
        &BallSetPhysicsSpec::default(),
        &config,
    );
    let after_stop = advance_ball_state(
        state.as_ball_state(),
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
