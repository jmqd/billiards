use billiards::{
    compute_next_transition_on_table, AngularVelocity3, BallSetPhysicsSpec, BallState, Inches2,
    InchesPerSecondSq, MotionPhase, MotionPhaseConfig, OnTableBallState, OnTableMotionConfig,
    RadiansPerSecondSq, RollingResistanceModel, SlidingFrictionModel, SpinDecayModel, Velocity2,
    TYPICAL_BALL_RADIUS,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

fn transition_config() -> OnTableMotionConfig {
    OnTableMotionConfig {
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

fn calibrated_transition_config() -> OnTableMotionConfig {
    OnTableMotionConfig {
        phase: MotionPhaseConfig::default(),
        sliding_friction: SlidingFrictionModel::ConstantAcceleration {
            acceleration_magnitude: InchesPerSecondSq::new("15"),
        },
        spin_decay: SpinDecayModel::ConstantAngularDeceleration {
            angular_deceleration: RadiansPerSecondSq::new(10.9),
        },
        rolling_resistance: RollingResistanceModel::ConstantDeceleration {
            linear_deceleration: InchesPerSecondSq::new("5"),
        },
    }
}

fn on_table(state: BallState) -> OnTableBallState {
    OnTableBallState::try_from(state).expect("test states should validate as on-table")
}

#[test]
fn a_resting_ball_has_no_next_transition() {
    let state = on_table(BallState::resting_at(Inches2::new("12.5", "37.25")));

    assert!(compute_next_transition_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &transition_config()
    )
    .is_none());
}

#[test]
fn a_sliding_stun_ball_predicts_the_time_until_rolling_from_coulomb_friction() {
    let state = on_table(BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    ));

    let transition = compute_next_transition_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &transition_config(),
    )
    .expect("sliding balls should predict a rolling transition");

    assert_eq!(transition.phase_before, MotionPhase::Sliding);
    assert_eq!(transition.phase_after, MotionPhase::Rolling);
    assert_close(transition.time_until_transition.as_f64(), 4.0 / 7.0);
}

#[test]
fn a_sliding_draw_ball_uses_initial_cloth_contact_slip_speed_in_the_transition_time() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(4.0, 0.0, 0.0),
    ));

    let transition = compute_next_transition_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &transition_config(),
    )
    .expect("sliding balls should predict a rolling transition");

    let expected_slip_speed = 10.0 + radius.as_f64() * 4.0;
    let expected_time = (2.0 / 7.0) * expected_slip_speed / 5.0;

    assert_eq!(transition.phase_before, MotionPhase::Sliding);
    assert_eq!(transition.phase_after, MotionPhase::Rolling);
    assert_close(transition.time_until_transition.as_f64(), expected_time);
}

#[test]
fn a_rolling_ball_without_residual_z_spin_predicts_rest_at_linear_stop() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius.as_f64(), 0.0, 0.0),
    ));

    let transition = compute_next_transition_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &transition_config(),
    )
    .expect("rolling balls should predict a rest transition");

    assert_eq!(transition.phase_before, MotionPhase::Rolling);
    assert_eq!(transition.phase_after, MotionPhase::Rest);
    assert_close(transition.time_until_transition.as_f64(), 2.0);
}

#[test]
fn a_rolling_ball_with_residual_z_spin_predicts_spinning_after_linear_stop() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius.as_f64(), 0.0, 6.0),
    ));

    let transition = compute_next_transition_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &transition_config(),
    )
    .expect("rolling balls should predict a transition");

    assert_eq!(transition.phase_before, MotionPhase::Rolling);
    assert_eq!(transition.phase_after, MotionPhase::Spinning);
    assert_close(transition.time_until_transition.as_f64(), 2.0);
}

#[test]
fn a_spinning_ball_predicts_the_time_until_rest_using_constant_angular_deceleration() {
    let state = on_table(BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    ));

    let transition = compute_next_transition_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &transition_config(),
    )
    .expect("spinning balls should predict a rest transition");

    assert_eq!(transition.phase_before, MotionPhase::Spinning);
    assert_eq!(transition.phase_after, MotionPhase::Rest);
    assert_close(transition.time_until_transition.as_f64(), 3.0);
}

#[test]
fn calibrated_spin_decay_sends_a_rolling_ball_directly_to_rest_when_spin_stops_first() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius.as_f64(), 0.0, 20.0),
    ));

    let transition = compute_next_transition_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &calibrated_transition_config(),
    )
    .expect("rolling balls should predict a transition");

    assert_eq!(transition.phase_before, MotionPhase::Rolling);
    assert_eq!(transition.phase_after, MotionPhase::Rest);
    assert_close(transition.time_until_transition.as_f64(), 2.0);
}

#[test]
fn calibrated_spin_decay_preserves_a_spinning_tail_only_when_spin_outlasts_roll_stop() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius.as_f64(), 0.0, 30.0),
    ));

    let transition = compute_next_transition_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &calibrated_transition_config(),
    )
    .expect("rolling balls should predict a transition");

    assert_eq!(transition.phase_before, MotionPhase::Rolling);
    assert_eq!(transition.phase_after, MotionPhase::Spinning);
    assert_close(transition.time_until_transition.as_f64(), 2.0);
}
