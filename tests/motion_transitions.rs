use billiards::{
    compute_next_transition, AngularVelocity3, BallSetPhysicsSpec, BallState, Inches2,
    InchesPerSecondSq, MotionPhase, MotionPhaseConfig, MotionTransitionConfig,
    RollingResistanceModel, SlidingFrictionModel, TYPICAL_BALL_RADIUS, Velocity2,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

fn transition_config() -> MotionTransitionConfig {
    MotionTransitionConfig {
        phase: MotionPhaseConfig::default(),
        sliding_friction: SlidingFrictionModel::ConstantAcceleration {
            acceleration_magnitude: InchesPerSecondSq::new("5"),
        },
        rolling_resistance: RollingResistanceModel::ConstantDeceleration {
            linear_deceleration: InchesPerSecondSq::new("5"),
        },
    }
}

#[test]
fn a_resting_ball_has_no_next_transition() {
    let state = BallState::resting_at(Inches2::new("12.5", "37.25"));

    assert!(compute_next_transition(&state, &BallSetPhysicsSpec::default(), &transition_config())
        .is_none());
}

#[test]
fn a_sliding_stun_ball_predicts_the_time_until_rolling_from_coulomb_friction() {
    let state = BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    );

    let transition = compute_next_transition(&state, &BallSetPhysicsSpec::default(), &transition_config())
        .expect("sliding balls should predict a rolling transition");

    assert_eq!(transition.phase_before, MotionPhase::Sliding);
    assert_eq!(transition.phase_after, MotionPhase::Rolling);
    assert_close(transition.time_until_transition.as_f64(), 4.0 / 7.0);
}

#[test]
fn a_sliding_draw_ball_uses_initial_cloth_contact_slip_speed_in_the_transition_time() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(4.0, 0.0, 0.0),
    );

    let transition = compute_next_transition(&state, &BallSetPhysicsSpec::default(), &transition_config())
        .expect("sliding balls should predict a rolling transition");

    // Eq. (M4) gives the initial slip speed: ||WEi|| = vy + R * wx for this straight-shot draw state.
    let expected_slip_speed = 10.0 + radius.as_f64() * 4.0;
    let expected_time = (2.0 / 7.0) * expected_slip_speed / 5.0;

    assert_eq!(transition.phase_before, MotionPhase::Sliding);
    assert_eq!(transition.phase_after, MotionPhase::Rolling);
    assert_close(transition.time_until_transition.as_f64(), expected_time);
}

#[test]
fn a_rolling_ball_predicts_the_time_until_rest_using_constant_deceleration() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius.as_f64(), 0.0, 0.0),
    );

    let transition = compute_next_transition(&state, &BallSetPhysicsSpec::default(), &transition_config())
        .expect("rolling balls should predict a rest transition");

    assert_eq!(transition.phase_before, MotionPhase::Rolling);
    assert_eq!(transition.phase_after, MotionPhase::Rest);
    assert_close(transition.time_until_transition.as_f64(), 2.0);
}
