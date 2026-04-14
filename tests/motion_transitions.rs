use billiards::{
    next_transition, AngularVelocity3, BallSetPhysicsSpec, BallState, Inches2,
    InchesPerSecondSq, MotionPhase, MotionPhaseConfig, MotionTransitionConfig,
    RollingResistanceModel, TYPICAL_BALL_RADIUS, Velocity2,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

#[test]
fn a_resting_ball_has_no_next_transition() {
    let state = BallState::resting_at(Inches2::new("12.5", "37.25"));
    let config = MotionTransitionConfig {
        phase: MotionPhaseConfig::default(),
        rolling_resistance: RollingResistanceModel::ConstantDeceleration {
            linear_deceleration: InchesPerSecondSq::new("5"),
        },
    };

    assert!(next_transition(&state, &BallSetPhysicsSpec::default(), &config).is_none());
}

#[test]
fn a_rolling_ball_predicts_the_time_until_rest_using_constant_deceleration() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius.as_f64(), 0.0, 0.0),
    );
    let config = MotionTransitionConfig {
        phase: MotionPhaseConfig::default(),
        rolling_resistance: RollingResistanceModel::ConstantDeceleration {
            linear_deceleration: InchesPerSecondSq::new("5"),
        },
    };

    let transition = next_transition(&state, &BallSetPhysicsSpec::default(), &config)
        .expect("rolling balls should predict a rest transition");

    assert_eq!(transition.phase_before, MotionPhase::Rolling);
    assert_eq!(transition.phase_after, MotionPhase::Rest);
    assert_close(transition.time_until_transition.as_f64(), 2.0);
}
