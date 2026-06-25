use billiards::{
    advance_motion_on_table, collide_ball_ball_analyzed_on_table,
    collide_ball_ball_detailed_on_table, collide_ball_ball_detailed_on_table_with_config,
    collide_ball_ball_on_table, compute_next_transition_on_table,
    estimate_post_contact_cue_ball_bend_on_table, estimate_post_contact_cue_ball_curve_on_table,
    gearing_english, Angle, AngularVelocity3, BallBallCollisionConfig, BallBallFrictionModel,
    BallSetPhysicsSpec, BallState, CollisionModel, CutAngle, Inches, Inches2, InchesPerSecond,
    InchesPerSecondSq, MotionPhase, MotionPhaseConfig, MotionTransitionConfig, OnTableBallState,
    OnTableMotionConfig, PlayingConditions, RadiansPerSecondSq, RollingResistanceModel, Scale,
    SlidingFrictionModel, SpinDecayModel, Velocity2, TYPICAL_BALL_RADIUS,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

fn assert_near(actual: f64, expected: f64, tolerance: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta <= tolerance,
        "expected {expected}, got {actual} (delta {delta}, tolerance {tolerance})"
    );
}

fn on_table(state: BallState) -> OnTableBallState {
    OnTableBallState::try_from(state).expect("test states should validate as on-table")
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

fn smallest_angle_distance_degrees(a: Angle, b: Angle) -> f64 {
    let delta = (a.as_degrees() - b.as_degrees()).abs().rem_euclid(360.0);
    delta.min(360.0 - delta)
}

fn signed_angle_difference_degrees(from: Angle, to: Angle) -> f64 {
    let delta = (to.as_degrees() - from.as_degrees()).rem_euclid(360.0);
    if delta > 180.0 {
        delta - 360.0
    } else {
        delta
    }
}

fn inches2(x: f64, y: f64) -> Inches2 {
    Inches2::new(Inches::from_f64(x), Inches::from_f64(y))
}

fn impact_heading(from: &OnTableBallState, to: &OnTableBallState) -> Angle {
    let from = from.as_ball_state();
    let to = to.as_ball_state();

    Angle::from_north(
        to.position.x().as_f64() - from.position.x().as_f64(),
        to.position.y().as_f64() - from.position.y().as_f64(),
    )
}

fn cue_ball_at_cut_angle_degrees(cut_angle_degrees: f64, speed: f64) -> OnTableBallState {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let radians = cut_angle_degrees.to_radians();
    on_table(BallState::on_table(
        inches2(-2.0 * radius * radians.sin(), -2.0 * radius * radians.cos()),
        Velocity2::new(Inches::zero(), Inches::from_f64(speed)),
        AngularVelocity3::zero(),
    ))
}

fn marlow_throw_for_stun_cut(cut_angle_degrees: f64, friction_scale: f64) -> f64 {
    let cue_ball = cue_ball_at_cut_angle_degrees(cut_angle_degrees, 3.0 * 17.6);
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let config = BallBallCollisionConfig::new_with_friction_model(
        Scale::from_f64(1.0),
        BallBallFrictionModel::marlow_speed_fit(Scale::from_f64(friction_scale)),
    );

    collide_ball_ball_detailed_on_table_with_config(
        &cue_ball,
        &object_ball,
        CollisionModel::ThrowAware,
        &config,
    )
    .throw_angle_degrees
    .expect("throw-aware collisions should report a throw angle")
    .abs()
}

fn expected_spin_seed_for_north_shot(
    radius: f64,
    phi_radians: f64,
    speed: f64,
    wx: f64,
    wy: f64,
    wz: f64,
) -> (f64, f64, f64, f64) {
    let mu_balls = 0.06;
    let tangential_contact_slip = speed * phi_radians.sin() - radius * wz;
    let vertical_contact_slip = radius * (wx * phi_radians.cos() + wy * phi_radians.sin());
    let denominator = (tangential_contact_slip.powi(2) + vertical_contact_slip.powi(2)).sqrt();
    let normal_impulse_per_mass = speed * phi_radians.cos().abs();
    let impulse_magnitude = (denominator / 7.0).min(mu_balls * normal_impulse_per_mass);
    let tangential_impulse_per_mass = if denominator <= f64::EPSILON {
        0.0
    } else {
        tangential_contact_slip * impulse_magnitude / denominator
    };
    let vertical_impulse_per_mass = if denominator <= f64::EPSILON {
        0.0
    } else {
        vertical_contact_slip * impulse_magnitude / denominator
    };
    let local_tangential_velocity = speed * phi_radians.sin() - tangential_impulse_per_mass;
    let velocity_x = local_tangential_velocity * phi_radians.cos();
    let velocity_y = local_tangential_velocity * phi_radians.sin();
    let angular_x = wx - (5.0 / (2.0 * radius)) * phi_radians.cos() * vertical_impulse_per_mass;
    let angular_y = wy - (5.0 / (2.0 * radius)) * phi_radians.sin() * vertical_impulse_per_mass;

    (velocity_x, velocity_y, angular_x, angular_y)
}

#[test]
fn throw_aware_head_on_collision_matches_ideal_and_reports_zero_throw() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(0.0, -2.0 * radius),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let ideal = collide_ball_ball_on_table(&cue_ball, &object_ball, CollisionModel::Ideal);
    let throw_aware =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);

    assert_close(
        throw_aware
            .throw_angle_degrees
            .expect("throw-aware collisions should report a throw angle"),
        0.0,
    );
    assert_eq!((throw_aware.a_after, throw_aware.b_after), ideal);
    assert!(throw_aware.transferred_spin.is_none());
}

#[test]
fn spin_friction_matches_throw_aware_for_a_stationary_object_ball_cut() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, -6.0),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let throw_aware =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);
    let spin_friction =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::SpinFriction);

    assert_eq!(spin_friction, throw_aware);
}

#[test]
fn throw_aware_applies_the_contact_impulse_to_a_moving_object_ball() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    ));
    let object_ball = on_table(BallState::on_table(
        inches2(0.0, 0.0),
        Velocity2::new("2", "0"),
        AngularVelocity3::zero(),
    ));

    let ideal = collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::Ideal);
    let throw_aware =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);
    let spin_friction =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::SpinFriction);

    assert_eq!(spin_friction, throw_aware);
    assert!(
        throw_aware
            .throw_angle_degrees
            .expect("throw-aware collisions should report a throw angle")
            .abs()
            > 1e-9,
        "moving-object throw-aware collisions should no longer silently reduce to ideal"
    );
    assert!(throw_aware.transferred_spin.is_some());

    let ideal_heading = ideal
        .b_after
        .as_ball_state()
        .velocity
        .angle_from_north()
        .expect("ideal moving object ball should still have an outgoing heading");
    let throw_aware_heading = throw_aware
        .b_after
        .as_ball_state()
        .velocity
        .angle_from_north()
        .expect("throw-aware moving object ball should still have an outgoing heading");
    assert!(
        smallest_angle_distance_degrees(throw_aware_heading, ideal_heading) > 1e-6,
        "throw-aware should deflect the moving object ball away from the ideal departure heading"
    );
}

#[test]
fn throw_magnitude_follows_the_contact_impulse_instead_of_a_fixed_angle() {
    fn abs_throw_for_cut_degrees(cut_degrees: f64) -> f64 {
        let radius = TYPICAL_BALL_RADIUS.as_f64();
        let cut_radians = cut_degrees.to_radians();
        let cue_ball = on_table(BallState::on_table(
            inches2(
                -2.0 * radius * cut_radians.sin(),
                -2.0 * radius * cut_radians.cos(),
            ),
            Velocity2::new("0", "10"),
            AngularVelocity3::zero(),
        ));
        let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));

        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware)
            .throw_angle_degrees
            .expect("throw-aware collisions should report a throw angle")
            .abs()
    }

    let near_head_on_throw = abs_throw_for_cut_degrees(5.0);
    let medium_cut_throw = abs_throw_for_cut_degrees(30.0);

    assert!(
        near_head_on_throw < 1.5,
        "a small cut should no longer be mapped to the old fixed 5° throw cap; got {near_head_on_throw}°"
    );
    assert!(
        medium_cut_throw > 2.0 * near_head_on_throw,
        "larger cut/slip should produce materially more throw than a near-head-on cut; got {medium_cut_throw}° vs {near_head_on_throw}°"
    );
}

#[test]
fn tp_a17_friction_scale_does_not_change_no_slip_capped_small_cut_throw() {
    let half_friction = marlow_throw_for_stun_cut(5.0, 0.5);
    let average_friction = marlow_throw_for_stun_cut(5.0, 1.0);
    let high_friction = marlow_throw_for_stun_cut(5.0, 1.5);

    assert_close(half_friction, 0.716_067_169_662_019_8);
    assert_close(average_friction, 0.716_067_169_662_019_8);
    assert_close(high_friction, 0.716_067_169_662_019_8);
}

#[test]
fn tp_a17_friction_scale_increases_friction_limited_larger_cut_throw() {
    let half_friction = marlow_throw_for_stun_cut(45.0, 0.5);
    let average_friction = marlow_throw_for_stun_cut(45.0, 1.0);
    let high_friction = marlow_throw_for_stun_cut(45.0, 1.5);

    assert_close(half_friction, 1.387_423_680_680_192_7);
    assert_close(average_friction, 2.773_222_175_141_301);
    assert_close(high_friction, 4.155_781_690_420_278_5);
    assert!(half_friction < average_friction);
    assert!(average_friction < high_friction);
}

#[test]
fn playing_conditions_scale_throw_aware_ball_ball_friction_behavior() {
    let cue_ball = cue_ball_at_cut_angle_degrees(45.0, 3.0 * 17.6);
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let base = BallBallCollisionConfig::human_tuned();
    let fast_clean = base.applying_conditions(&PlayingConditions::fast_clean());
    let humid_dirty = base.applying_conditions(&PlayingConditions::humid_dirty());
    let outcome_for = |config: &BallBallCollisionConfig, model| {
        collide_ball_ball_detailed_on_table_with_config(&cue_ball, &object_ball, model, config)
    };

    let fast_clean_outcome = outcome_for(&fast_clean, CollisionModel::ThrowAware);
    let neutral_outcome = outcome_for(&base, CollisionModel::ThrowAware);
    let humid_dirty_outcome = outcome_for(&humid_dirty, CollisionModel::ThrowAware);
    let humid_dirty_spin_friction = outcome_for(&humid_dirty, CollisionModel::SpinFriction);
    let throw_abs = |outcome: &billiards::CollisionOutcome| {
        outcome
            .throw_angle_degrees
            .expect("throw-aware collisions should report a throw angle")
            .abs()
    };
    let transferred_z_abs = |outcome: &billiards::CollisionOutcome| {
        outcome
            .transferred_spin
            .as_ref()
            .expect("friction-limited cut should transfer spin")
            .z()
            .as_f64()
            .abs()
    };

    assert!(throw_abs(&fast_clean_outcome) < throw_abs(&neutral_outcome));
    assert!(throw_abs(&neutral_outcome) < throw_abs(&humid_dirty_outcome));
    assert!(transferred_z_abs(&fast_clean_outcome) < transferred_z_abs(&neutral_outcome));
    assert!(transferred_z_abs(&neutral_outcome) < transferred_z_abs(&humid_dirty_outcome));
    assert_eq!(humid_dirty_spin_friction, humid_dirty_outcome);
}

#[test]
fn throw_aware_impulses_conserve_horizontal_momentum() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(6.0, 0.0, -6.0),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let outcome =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);
    let before_x = cue_ball.as_ball_state().velocity.x().as_f64()
        + object_ball.as_ball_state().velocity.x().as_f64();
    let before_y = cue_ball.as_ball_state().velocity.y().as_f64()
        + object_ball.as_ball_state().velocity.y().as_f64();
    let after_x = outcome.a_after.as_ball_state().velocity.x().as_f64()
        + outcome.b_after.as_ball_state().velocity.x().as_f64();
    let after_y = outcome.a_after.as_ball_state().velocity.y().as_f64()
        + outcome.b_after.as_ball_state().velocity.y().as_f64();

    assert_close(after_x, before_x);
    assert_close(after_y, before_y);
}

#[test]
fn moving_object_ball_throw_aware_impulses_conserve_horizontal_momentum() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(0.0, 0.0, -6.0),
    ));
    let object_ball = on_table(BallState::on_table(
        inches2(0.0, 0.0),
        Velocity2::new("2", "0"),
        AngularVelocity3::new(0.0, 0.0, 3.0),
    ));

    let outcome =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);
    let before_x = cue_ball.as_ball_state().velocity.x().as_f64()
        + object_ball.as_ball_state().velocity.x().as_f64();
    let before_y = cue_ball.as_ball_state().velocity.y().as_f64()
        + object_ball.as_ball_state().velocity.y().as_f64();
    let after_x = outcome.a_after.as_ball_state().velocity.x().as_f64()
        + outcome.b_after.as_ball_state().velocity.x().as_f64();
    let after_y = outcome.a_after.as_ball_state().velocity.y().as_f64()
        + outcome.b_after.as_ball_state().velocity.y().as_f64();

    assert_close(after_x, before_x);
    assert_close(after_y, before_y);
}

#[test]
fn a_nearly_head_on_rolling_collision_does_not_pick_up_throw_from_tiny_tangent_noise() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-2.0 * radius, 0.0),
        Velocity2::new("120", "0.00000000000001"),
        AngularVelocity3::new(0.0, 120.0 / radius, 0.0),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let ideal = collide_ball_ball_on_table(&cue_ball, &object_ball, CollisionModel::Ideal);
    let outcome =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);

    assert_close(
        outcome
            .throw_angle_degrees
            .expect("throw-aware collisions should report a throw angle"),
        0.0,
    );
    assert_close(
        outcome.b_after.as_ball_state().velocity.x().as_f64(),
        ideal.1.as_ball_state().velocity.x().as_f64(),
    );
    assert_close(outcome.b_after.as_ball_state().velocity.y().as_f64(), 0.0);
}

#[test]
fn follow_and_draw_reduce_near_head_on_throw_relative_to_stun() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cut_angle_radians = 5.0_f64.to_radians();
    let cue_position = inches2(
        -2.0 * radius * cut_angle_radians.sin(),
        -2.0 * radius * cut_angle_radians.cos(),
    );
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let stun = on_table(BallState::on_table(
        cue_position.clone(),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    ));
    let follow = on_table(BallState::on_table(
        cue_position.clone(),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let draw = on_table(BallState::on_table(
        cue_position,
        Velocity2::new("0", "10"),
        AngularVelocity3::new(10.0 / radius, 0.0, 0.0),
    ));

    let stun_throw =
        collide_ball_ball_detailed_on_table(&stun, &object_ball, CollisionModel::ThrowAware)
            .throw_angle_degrees
            .expect("throw-aware collisions should report a throw angle")
            .abs();
    let follow_throw =
        collide_ball_ball_detailed_on_table(&follow, &object_ball, CollisionModel::ThrowAware)
            .throw_angle_degrees
            .expect("throw-aware collisions should report a throw angle")
            .abs();
    let draw_throw =
        collide_ball_ball_detailed_on_table(&draw, &object_ball, CollisionModel::ThrowAware)
            .throw_angle_degrees
            .expect("throw-aware collisions should report a throw angle")
            .abs();

    // Follow/draw push much of the contact slip into the vertical plane. With the impulse model,
    // the reduced horizontal fraction must still produce less throw than the comparable stun hit.
    let reduction_limit = 0.5 * stun_throw;
    assert!(
        follow_throw < reduction_limit,
        "near-head-on follow should reduce throw; got follow {follow_throw} vs stun {stun_throw}"
    );
    assert!(
        draw_throw < reduction_limit,
        "near-head-on draw should reduce throw; got draw {draw_throw} vs stun {stun_throw}"
    );
    assert_close(follow_throw, draw_throw);
}

#[test]
fn tp_a24_follow_and_draw_match_half_ball_throw_and_object_ball_curve_anchors() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let speed = InchesPerSecond::from_mph(2.0).as_f64();
    let cut_angle_radians = 30.0_f64.to_radians();
    let cue_position = inches2(
        -2.0 * radius * cut_angle_radians.sin(),
        -2.0 * radius * cut_angle_radians.cos(),
    );
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let config = BallBallCollisionConfig::new_with_friction_model(
        Scale::from_f64(1.0),
        BallBallFrictionModel::marlow_speed_fit(Scale::from_f64(1.0)),
    );
    let ball = BallSetPhysicsSpec::default();
    let motion = motion_config();

    // TP A.24 gives these half-ball anchors for v=2 mph with the TP A.14/Marlow friction fit.
    for (name, omega_x, expected_throw, expected_curve_delta) in [
        ("stun", 0.0, 4.366, 0.0),
        ("draw", speed / radius, 1.454, -0.061),
        ("follow", -speed / radius, 1.454, 0.067),
    ] {
        let cue_ball = on_table(BallState::on_table(
            cue_position.clone(),
            Velocity2::new(Inches::zero(), Inches::from_f64(speed)),
            AngularVelocity3::new(omega_x, 0.0, 0.0),
        ));
        let outcome = collide_ball_ball_detailed_on_table_with_config(
            &cue_ball,
            &object_ball,
            CollisionModel::ThrowAware,
            &config,
        );
        let immediate_heading = outcome
            .b_after
            .as_ball_state()
            .velocity
            .angle_from_north()
            .expect("object ball should move immediately after impact");
        let transition = compute_next_transition_on_table(&outcome.b_after, &ball, &motion)
            .expect("object ball should slide before settling to natural roll");
        let settled = advance_motion_on_table(
            &outcome.b_after,
            transition.time_until_transition,
            &ball,
            &motion,
        );
        let settled_heading = settled
            .state
            .velocity
            .angle_from_north()
            .expect("object ball should still move after the TP A.24 curve");
        let throw_angle = outcome
            .throw_angle_degrees
            .expect("throw-aware collisions should report a throw angle");
        let curve_delta = signed_angle_difference_degrees(immediate_heading, settled_heading)
            * throw_angle.signum();

        assert_eq!(transition.phase_before, MotionPhase::Sliding, "{name}");
        assert_eq!(transition.phase_after, MotionPhase::Rolling, "{name}");
        assert_near(throw_angle.abs(), expected_throw, 0.001);
        assert_near(curve_delta, expected_curve_delta, 0.001);
    }
}

#[test]
fn a_rolling_cut_shot_with_english_uses_the_tp_a8_style_cue_ball_post_impact_state() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, -6.0),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let outcome =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);
    let phi = (-45.0_f64).to_radians();
    let speed: f64 = 10.0;
    let (expected_velocity_x, expected_velocity_y, expected_angular_x, expected_angular_y) =
        expected_spin_seed_for_north_shot(radius, phi, speed, -10.0 / radius, 0.0, -6.0);

    assert_close(
        outcome.a_after.as_ball_state().velocity.x().as_f64(),
        expected_velocity_x,
    );
    assert_close(
        outcome.a_after.as_ball_state().velocity.y().as_f64(),
        expected_velocity_y,
    );
    assert_close(
        outcome
            .a_after
            .as_ball_state()
            .angular_velocity
            .x()
            .as_f64(),
        expected_angular_x,
    );
    assert_close(
        outcome
            .a_after
            .as_ball_state()
            .angular_velocity
            .y()
            .as_f64(),
        expected_angular_y,
    );
    assert_eq!(
        outcome
            .a_after
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Sliding
    );
}

#[test]
fn a_sliding_cut_shot_with_english_uses_the_broader_post_impact_english_model() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(0.0, 0.0, -6.0),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let outcome =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);
    let phi = (-45.0_f64).to_radians();
    let speed: f64 = 10.0;
    let (expected_velocity_x, expected_velocity_y, _, _) =
        expected_spin_seed_for_north_shot(radius, phi, speed, 0.0, 0.0, -6.0);

    assert_close(
        outcome.a_after.as_ball_state().velocity.x().as_f64(),
        expected_velocity_x,
    );
    assert_close(
        outcome.a_after.as_ball_state().velocity.y().as_f64(),
        expected_velocity_y,
    );
    assert_close(
        outcome
            .a_after
            .as_ball_state()
            .angular_velocity
            .x()
            .as_f64(),
        0.0,
    );
    assert_close(
        outcome
            .a_after
            .as_ball_state()
            .angular_velocity
            .y()
            .as_f64(),
        0.0,
    );
    assert_eq!(
        outcome
            .a_after
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Sliding
    );
}

#[test]
fn a_follow_cut_shot_with_english_uses_the_broader_post_impact_spin_model() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-6.0, 0.0, -6.0),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let outcome =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);
    let phi = (-45.0_f64).to_radians();
    let speed: f64 = 10.0;
    let (expected_velocity_x, expected_velocity_y, expected_angular_x, expected_angular_y) =
        expected_spin_seed_for_north_shot(radius, phi, speed, -6.0, 0.0, -6.0);

    assert_close(
        outcome.a_after.as_ball_state().velocity.x().as_f64(),
        expected_velocity_x,
    );
    assert_close(
        outcome.a_after.as_ball_state().velocity.y().as_f64(),
        expected_velocity_y,
    );
    assert_close(
        outcome
            .a_after
            .as_ball_state()
            .angular_velocity
            .x()
            .as_f64(),
        expected_angular_x,
    );
    assert_close(
        outcome
            .a_after
            .as_ball_state()
            .angular_velocity
            .y()
            .as_f64(),
        expected_angular_y,
    );
}

#[test]
fn a_draw_cut_shot_with_english_uses_the_broader_post_impact_spin_model() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(6.0, 0.0, -6.0),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let outcome =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);
    let phi = (-45.0_f64).to_radians();
    let speed: f64 = 10.0;
    let (expected_velocity_x, expected_velocity_y, expected_angular_x, expected_angular_y) =
        expected_spin_seed_for_north_shot(radius, phi, speed, 6.0, 0.0, -6.0);

    assert_close(
        outcome.a_after.as_ball_state().velocity.x().as_f64(),
        expected_velocity_x,
    );
    assert_close(
        outcome.a_after.as_ball_state().velocity.y().as_f64(),
        expected_velocity_y,
    );
    assert_close(
        outcome
            .a_after
            .as_ball_state()
            .angular_velocity
            .x()
            .as_f64(),
        expected_angular_x,
    );
    assert_close(
        outcome
            .a_after
            .as_ball_state()
            .angular_velocity
            .y()
            .as_f64(),
        expected_angular_y,
    );
}

#[test]
fn head_on_backspin_transfers_forward_spin_to_the_object_ball() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(0.0, -2.0 * radius),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(4.0, 0.0, 0.0),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let outcome =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);
    let transferred_spin = outcome
        .transferred_spin
        .expect("backspin should transfer horizontal spin to the object ball");

    assert_close(
        outcome
            .throw_angle_degrees
            .expect("throw-aware collisions should report a throw angle"),
        0.0,
    );
    assert!(transferred_spin.x().as_f64() < 0.0);
    assert_close(transferred_spin.y().as_f64(), 0.0);
    assert_close(transferred_spin.z().as_f64(), 0.0);
    assert_close(
        outcome
            .b_after
            .as_ball_state()
            .angular_velocity
            .x()
            .as_f64(),
        transferred_spin.x().as_f64(),
    );
    assert!(
        outcome
            .b_after
            .as_ball_state()
            .angular_velocity
            .x()
            .as_f64()
            < 0.0
    );
}

#[test]
fn a_cut_shot_without_side_spin_produces_cut_induced_throw_and_transferred_spin() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let ideal_line = impact_heading(&cue_ball, &object_ball);
    let outcome =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);
    let object_heading = outcome
        .b_after
        .as_ball_state()
        .velocity
        .angle_from_north()
        .expect("the object ball should move after impact");
    let transferred_spin = outcome
        .transferred_spin
        .expect("a slipping cut shot should transfer z-spin");

    assert!(
        outcome
            .throw_angle_degrees
            .expect("throw-aware collisions should report a throw angle")
            .abs()
            > 1e-9
    );
    assert!(
        (object_heading.as_degrees() - ideal_line.as_degrees()).abs() > 1e-9,
        "cut-induced throw should deflect the object ball away from the ideal line"
    );
    assert!(transferred_spin.z().as_f64().abs() > 1e-9);
    assert_close(
        outcome
            .b_after
            .as_ball_state()
            .angular_velocity
            .z()
            .as_f64(),
        transferred_spin.z().as_f64(),
    );
}

#[test]
fn a_cut_shot_without_initial_english_does_not_seed_exaggerated_cue_ball_side_spin() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball_heading = Angle::from_north(0.0, 10.0);
    let line_of_centers = Angle::from_north(radius * 2.0_f64.sqrt(), radius * 2.0_f64.sqrt());
    let cut_angle = CutAngle::from_headings(cue_ball_heading, line_of_centers);
    let cue_ball = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let outcome =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);
    let cue_side_spin = outcome
        .a_after
        .as_ball_state()
        .angular_velocity
        .z()
        .as_f64()
        .abs();
    let gearing_limit = gearing_english(cut_angle, Velocity2::new("0", "10").speed()).as_f64();

    assert!(
        cue_side_spin < 0.5 * gearing_limit,
        "ordinary no-English cut shots should not seed cue-ball side spin near the gearing-english scale; got {cue_side_spin} vs gearing {gearing_limit}"
    );
}

#[test]
fn gearing_english_cancels_throw_for_a_stationary_object_ball_cut() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let shot_speed = Velocity2::new("0", "10").speed();
    let cue_ball_heading = Angle::from_north(0.0, 10.0);
    let line_of_centers = Angle::from_north(radius * 2.0_f64.sqrt(), radius * 2.0_f64.sqrt());
    let cut_angle = CutAngle::from_headings(cue_ball_heading, line_of_centers);
    let cue_ball = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(0.0, 0.0, -gearing_english(cut_angle, shot_speed).as_f64()),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let ideal = collide_ball_ball_on_table(&cue_ball, &object_ball, CollisionModel::Ideal);
    let throw_aware =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);

    assert!(
        throw_aware
            .throw_angle_degrees
            .expect("throw-aware collisions should report a throw angle")
            .abs()
            < 1e-9
    );
    assert!(throw_aware.transferred_spin.is_none());
    assert_close(
        throw_aware.a_after.as_ball_state().velocity.x().as_f64(),
        ideal.0.as_ball_state().velocity.x().as_f64(),
    );
    assert_close(
        throw_aware.a_after.as_ball_state().velocity.y().as_f64(),
        ideal.0.as_ball_state().velocity.y().as_f64(),
    );
    assert_close(
        throw_aware.b_after.as_ball_state().velocity.x().as_f64(),
        ideal.1.as_ball_state().velocity.x().as_f64(),
    );
    assert_close(
        throw_aware.b_after.as_ball_state().velocity.y().as_f64(),
        ideal.1.as_ball_state().velocity.y().as_f64(),
    );
}

#[test]
fn object_ball_side_spin_can_cancel_cut_contact_slip_without_throw_or_spin_transfer() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let shot_speed = Velocity2::new("0", "10").speed();
    let cue_ball_heading = Angle::from_north(0.0, 10.0);
    let line_of_centers = Angle::from_north(radius * 2.0_f64.sqrt(), radius * 2.0_f64.sqrt());
    let cut_angle = CutAngle::from_headings(cue_ball_heading, line_of_centers);
    let geared_spin = gearing_english(cut_angle, shot_speed).as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    ));
    let object_ball = on_table(BallState::on_table(
        inches2(0.0, 0.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, -geared_spin),
    ));

    let ideal = collide_ball_ball_on_table(&cue_ball, &object_ball, CollisionModel::Ideal);
    let throw_aware =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);

    assert!(
        throw_aware
            .throw_angle_degrees
            .expect("throw-aware collisions should report a throw angle")
            .abs()
            < 1e-9
    );
    assert!(throw_aware.transferred_spin.is_none());
    assert_close(
        throw_aware.a_after.as_ball_state().velocity.x().as_f64(),
        ideal.0.as_ball_state().velocity.x().as_f64(),
    );
    assert_close(
        throw_aware.a_after.as_ball_state().velocity.y().as_f64(),
        ideal.0.as_ball_state().velocity.y().as_f64(),
    );
    assert_close(
        throw_aware.b_after.as_ball_state().velocity.x().as_f64(),
        ideal.1.as_ball_state().velocity.x().as_f64(),
    );
    assert_close(
        throw_aware.b_after.as_ball_state().velocity.y().as_f64(),
        ideal.1.as_ball_state().velocity.y().as_f64(),
    );
    assert_close(
        throw_aware
            .b_after
            .as_ball_state()
            .angular_velocity
            .z()
            .as_f64(),
        -geared_spin,
    );
}

#[test]
fn over_gearing_flips_the_throw_and_transferred_spin_directions() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball_heading = Angle::from_north(0.0, 10.0);
    let line_of_centers = Angle::from_north(radius * 2.0_f64.sqrt(), radius * 2.0_f64.sqrt());
    let cut_angle = CutAngle::from_headings(cue_ball_heading, line_of_centers);
    let geared_spin = gearing_english(cut_angle, Velocity2::new("0", "10").speed()).as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(0.0, 0.0, -2.0 * geared_spin),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let outcome =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);

    assert!(
        outcome
            .throw_angle_degrees
            .expect("throw-aware collisions should report a throw angle")
            > 0.0
    );
    assert!(
        outcome
            .transferred_spin
            .expect("over-gearing should produce transferred spin")
            .z()
            .as_f64()
            > 0.0
    );
}

#[test]
fn the_analyzed_collision_helper_threads_the_post_contact_bend_estimate() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-6.0, 0.0, 0.0),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let detailed =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);
    let analyzed = collide_ball_ball_analyzed_on_table(
        &cue_ball,
        &object_ball,
        CollisionModel::ThrowAware,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );

    assert_eq!(analyzed.outcome, detailed);
    assert_eq!(
        analyzed.cue_ball_bend,
        detailed
            .estimate_post_contact_cue_ball_bend(&BallSetPhysicsSpec::default(), &motion_config())
    );
    assert!(analyzed.cue_ball_bend.is_some());
}

#[test]
fn the_collision_outcome_convenience_method_reports_no_bend_when_the_cue_ball_stops_immediately() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(0.0, -2.0 * radius),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let outcome =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);

    assert!(outcome
        .estimate_post_contact_cue_ball_bend(&BallSetPhysicsSpec::default(), &motion_config())
        .is_none());
}

#[test]
fn the_side_spin_curve_estimate_is_none_without_residual_z_spin() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(0.0, -2.0 * radius),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let outcome =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);

    assert!(outcome
        .estimate_post_contact_cue_ball_curve(&BallSetPhysicsSpec::default(), &motion_config())
        .is_none());
}

#[test]
fn side_spin_alone_no_longer_produces_a_post_contact_curve_estimate_in_the_horizontal_model() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(0.0, 0.0, -6.0),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let outcome =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);

    assert_eq!(
        outcome
            .a_after
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Sliding
    );
    assert!(outcome
        .estimate_post_contact_cue_ball_curve(&BallSetPhysicsSpec::default(), &motion_config())
        .is_none());
    assert!(estimate_post_contact_cue_ball_curve_on_table(
        &outcome.a_after,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .is_none());
}

#[test]
fn opposite_english_signs_do_not_produce_separate_curve_estimates_in_the_horizontal_model() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let right_english = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(0.0, 0.0, -6.0),
    ));
    let left_english = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    ));

    assert!(collide_ball_ball_detailed_on_table(
        &right_english,
        &object_ball,
        CollisionModel::ThrowAware,
    )
    .estimate_post_contact_cue_ball_curve(&BallSetPhysicsSpec::default(), &motion_config())
    .is_none());
    assert!(collide_ball_ball_detailed_on_table(
        &left_english,
        &object_ball,
        CollisionModel::ThrowAware,
    )
    .estimate_post_contact_cue_ball_curve(&BallSetPhysicsSpec::default(), &motion_config())
    .is_none());
}

#[test]
fn follow_bends_the_post_contact_cue_ball_path_toward_the_incoming_shot_line() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-6.0, 0.0, 0.0),
    ));
    let incoming_heading = cue_ball
        .as_ball_state()
        .velocity
        .angle_from_north()
        .expect("incoming cue ball should be moving");
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let outcome =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);
    let immediate_heading = outcome
        .a_after
        .as_ball_state()
        .velocity
        .angle_from_north()
        .expect("cue ball should still be moving after the cut shot");
    let bend = estimate_post_contact_cue_ball_bend_on_table(
        &outcome.a_after,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .expect("follow should produce a sliding cue-ball bend estimate");
    let bent_heading = bend
        .state_after_bend
        .as_ball_state()
        .velocity
        .angle_from_north()
        .expect("cue ball should still be moving after the bend");

    assert!(bend.time_until_bend_completes.as_f64() > 0.0);
    assert_eq!(
        bend.state_after_bend
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Rolling
    );
    assert!(
        smallest_angle_distance_degrees(bent_heading, incoming_heading)
            < smallest_angle_distance_degrees(immediate_heading, incoming_heading),
        "follow should bend the cue ball toward the incoming shot line"
    );
}

#[test]
fn draw_bends_the_post_contact_cue_ball_path_away_from_the_incoming_shot_line() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(6.0, 0.0, 0.0),
    ));
    let incoming_heading = cue_ball
        .as_ball_state()
        .velocity
        .angle_from_north()
        .expect("incoming cue ball should be moving");
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let outcome =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);
    let immediate_heading = outcome
        .a_after
        .as_ball_state()
        .velocity
        .angle_from_north()
        .expect("cue ball should still be moving after the cut shot");
    let bend = estimate_post_contact_cue_ball_bend_on_table(
        &outcome.a_after,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .expect("draw should produce a sliding cue-ball bend estimate");
    let bent_heading = bend
        .state_after_bend
        .as_ball_state()
        .velocity
        .angle_from_north()
        .expect("cue ball should still be moving after the bend");

    assert!(bend.time_until_bend_completes.as_f64() > 0.0);
    assert_eq!(
        bend.state_after_bend
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Rolling
    );
    assert!(
        smallest_angle_distance_degrees(bent_heading, incoming_heading)
            > smallest_angle_distance_degrees(immediate_heading, incoming_heading),
        "draw should bend the cue ball away from the incoming shot line"
    );
}

#[test]
fn tp_a20_half_ball_draw_bend_matches_published_final_angles() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_speed = 10.0;
    let cut_angle = 30.0_f64.to_radians();
    let immediate_velocity = Velocity2::new(
        Inches::from_f64(cue_speed * cut_angle.sin() * cut_angle.cos()),
        Inches::from_f64(cue_speed * cut_angle.sin() * cut_angle.sin()),
    );

    for (spin_rate_factor, expected_heading) in [
        (0.625 * 0.75, 81.787),
        (0.625, 90.0),
        (0.625 * 1.25, 98.213),
    ] {
        let state = on_table(BallState::on_table(
            inches2(0.0, 0.0),
            immediate_velocity.clone(),
            AngularVelocity3::new(spin_rate_factor * cue_speed / radius, 0.0, 0.0),
        ));
        let bend = estimate_post_contact_cue_ball_bend_on_table(
            &state,
            &BallSetPhysicsSpec::default(),
            &motion_config(),
        )
        .expect("TP A.20 draw should produce a sliding bend estimate");
        let heading = bend
            .state_after_bend
            .as_ball_state()
            .velocity
            .angle_from_north()
            .expect("cue ball should still be moving after the bend")
            .as_degrees();

        assert_near(heading, expected_heading, 0.001);
        assert_near(bend.bend_angle_degrees, expected_heading - 60.0, 0.001);
    }
}
