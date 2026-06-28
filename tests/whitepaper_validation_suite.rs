use billiards::{
    advance_motion_on_table, cloth_contact_velocity_on_table, collide_ball_ball_detailed_on_table,
    collide_ball_ball_detailed_on_table_with_config, collide_ball_ball_on_table,
    compute_next_transition_on_table, AngularVelocity3, BallBallCollisionConfig,
    BallSetPhysicsSpec, BallState, CollisionModel, Inches, Inches2, InchesPerSecondSq, MotionPhase,
    MotionPhaseConfig, MotionTransitionConfig, OnTableBallState, OnTableMotionConfig,
    RadiansPerSecondSq, RollingResistanceModel, Scale, SlidingFrictionModel, SpinDecayModel,
    Velocity2, TYPICAL_BALL_RADIUS,
};

fn assert_near(actual: f64, expected: f64, tolerance: f64, context: &str) {
    let delta = (actual - expected).abs();
    assert!(
        delta <= tolerance,
        "{context}: expected {expected}, got {actual} (delta {delta}, tolerance {tolerance})"
    );
}

fn on_table(state: BallState) -> OnTableBallState {
    OnTableBallState::try_from(state).expect("test state should be valid on-table")
}

fn inches2(x: f64, y: f64) -> Inches2 {
    Inches2::new(Inches::from_f64(x), Inches::from_f64(y))
}

fn motion_config(sliding_friction_acceleration: f64) -> OnTableMotionConfig {
    MotionTransitionConfig {
        phase: MotionPhaseConfig::default(),
        sliding_friction: SlidingFrictionModel::ConstantAcceleration {
            acceleration_magnitude: InchesPerSecondSq::new(Inches::from_f64(
                sliding_friction_acceleration,
            )),
        },
        spin_decay: SpinDecayModel::ConstantAngularDeceleration {
            angular_deceleration: RadiansPerSecondSq::new(2.0),
        },
        rolling_resistance: RollingResistanceModel::ConstantDeceleration {
            linear_deceleration: InchesPerSecondSq::new("5"),
        },
    }
}

fn settle_until_rolling(
    state: &OnTableBallState,
    motion: &OnTableMotionConfig,
) -> OnTableBallState {
    let ball = BallSetPhysicsSpec::default();
    let transition = compute_next_transition_on_table(state, &ball, motion)
        .expect("sliding state should have a transition");
    assert_eq!(transition.phase_before, MotionPhase::Sliding);
    assert_eq!(transition.phase_after, MotionPhase::Rolling);

    OnTableBallState::try_from(
        advance_motion_on_table(state, transition.time_until_transition, &ball, motion).state,
    )
    .expect("settled state should remain on-table")
}

fn rolling_cue_ball_at_cut_angle_degrees(cut_angle_degrees: f64, speed: f64) -> OnTableBallState {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let radians = cut_angle_degrees.to_radians();
    on_table(BallState::on_table(
        inches2(-2.0 * radius * radians.sin(), -2.0 * radius * radians.cos()),
        Velocity2::new(Inches::zero(), Inches::from_f64(speed)),
        AngularVelocity3::new(-speed / radius, 0.0, 0.0),
    ))
}

fn cue_ball_carom_angle_degrees(state: &OnTableBallState) -> f64 {
    let velocity = &state.as_ball_state().velocity;
    velocity
        .x()
        .as_f64()
        .abs()
        .atan2(velocity.y().as_f64())
        .to_degrees()
}

#[test]
fn tp_a4_final_rolling_velocity_is_independent_of_cloth_friction() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let start = on_table(BallState::on_table(
        inches2(0.0, 0.0),
        Velocity2::new("24", "48"),
        AngularVelocity3::new(-9.0, 4.0, 2.0),
    ));
    let slip = cloth_contact_velocity_on_table(start.as_ball_state(), radius);
    assert!(
        slip.speed().as_f64() > 1.0,
        "fixture must start with visible sliding slip; got {slip:?}"
    );

    let fast_cloth = settle_until_rolling(&start, &motion_config(12.0));
    let slow_cloth = settle_until_rolling(&start, &motion_config(3.0));
    let expected_vx = start.as_ball_state().velocity.x().as_f64() - (2.0 / 7.0) * slip.x().as_f64();
    let expected_vy = start.as_ball_state().velocity.y().as_f64() - (2.0 / 7.0) * slip.y().as_f64();

    assert_near(
        fast_cloth.as_ball_state().velocity.x().as_f64(),
        expected_vx,
        1e-9,
        "TP A.4 fast-cloth vx",
    );
    assert_near(
        fast_cloth.as_ball_state().velocity.y().as_f64(),
        expected_vy,
        1e-9,
        "TP A.4 fast-cloth vy",
    );
    assert_near(
        slow_cloth.as_ball_state().velocity.x().as_f64(),
        expected_vx,
        1e-9,
        "TP A.4 slow-cloth vx",
    );
    assert_near(
        slow_cloth.as_ball_state().velocity.y().as_f64(),
        expected_vy,
        1e-9,
        "TP A.4 slow-cloth vy",
    );
}

#[test]
fn tp_3_3_rolling_cue_ball_carom_angles_match_formula_anchors() {
    let speed = 30.0;
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    for (name, cut_angle_degrees, expected_degrees) in [
        ("half-ball", 30.0, 33.670_496_508_315_11),
        ("maximum carom angle", 28.126, 33.748_988_590_190_16),
    ] {
        let cue_ball = rolling_cue_ball_at_cut_angle_degrees(cut_angle_degrees, speed);
        let (cue_after, _) =
            collide_ball_ball_on_table(&cue_ball, &object_ball, CollisionModel::Ideal);
        let cue_rolling = settle_until_rolling(&cue_after, &motion_config(5.0));
        let actual_degrees = cue_ball_carom_angle_degrees(&cue_rolling);

        assert_near(actual_degrees, expected_degrees, 1e-9, name);
    }
}

#[test]
fn non_ideal_collision_diagnostics_expose_contact_impulse_terms() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cut = 35.0_f64.to_radians();
    let cue_ball = on_table(BallState::on_table(
        inches2(-2.0 * radius * cut.sin(), -2.0 * radius * cut.cos()),
        Velocity2::new("0", "52.8"),
        AngularVelocity3::new(-52.8 / radius, 0.0, -4.0),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let config = BallBallCollisionConfig::new(Scale::from_f64(0.92), Scale::from_f64(0.08));

    let outcome = collide_ball_ball_detailed_on_table_with_config(
        &cue_ball,
        &object_ball,
        CollisionModel::ThrowAware,
        &config,
    );
    let diagnostics = outcome
        .diagnostics
        .as_ref()
        .expect("throw-aware collisions should report contact diagnostics");
    let slip_norm = diagnostics
        .tangential_contact_slip_before
        .hypot(diagnostics.vertical_contact_slip_before);
    let impulse_norm = diagnostics
        .tangential_impulse_per_mass
        .hypot(diagnostics.vertical_impulse_per_mass);
    let expected_impulse_norm = (slip_norm / 7.0)
        .min(diagnostics.contact_friction_coefficient * diagnostics.normal_impulse_per_mass);
    let basis_dot = diagnostics.normal_basis.0 * diagnostics.tangent_basis.0
        + diagnostics.normal_basis.1 * diagnostics.tangent_basis.1;

    assert_near(
        diagnostics.normal_basis.0.hypot(diagnostics.normal_basis.1),
        1.0,
        1e-12,
        "unit normal basis",
    );
    assert_near(
        diagnostics
            .tangent_basis
            .0
            .hypot(diagnostics.tangent_basis.1),
        1.0,
        1e-12,
        "unit tangent basis",
    );
    assert_near(basis_dot, 0.0, 1e-12, "orthogonal contact basis");
    assert_near(
        impulse_norm,
        expected_impulse_norm,
        1e-9,
        "TP A.24/Peskin friction impulse cap",
    );
    assert!(
        outcome.throw_angle_degrees.unwrap().abs() > 0.0,
        "fixture should produce measurable throw"
    );
}

#[test]
fn analyzed_collision_reports_bend_curve_slots_and_preserves_outcome() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-6.0, 0.0, 0.0),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let ball = BallSetPhysicsSpec::default();
    let motion = motion_config(5.0);
    let detailed =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);
    let analyzed = detailed.with_post_contact_cue_ball_analysis(&ball, &motion);

    assert_eq!(analyzed.outcome, detailed);
    assert!(analyzed.cue_ball_bend.is_some());
    assert_eq!(
        analyzed.cue_ball_curve,
        analyzed
            .outcome
            .estimate_post_contact_cue_ball_curve(&ball, &motion)
    );
}
