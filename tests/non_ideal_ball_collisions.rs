use billiards::{
    collide_ball_ball_analyzed_on_table, collide_ball_ball_detailed_on_table,
    collide_ball_ball_on_table, estimate_post_contact_cue_ball_bend_on_table,
    estimate_post_contact_cue_ball_curve_on_table, gearing_english, Angle, AngularVelocity3,
    BallSetPhysicsSpec, BallState, CollisionModel, CutAngle, Inches, Inches2, InchesPerSecondSq,
    MotionPhase, MotionPhaseConfig, MotionTransitionConfig, OnTableBallState, OnTableMotionConfig,
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
    let tangential_impulse_per_mass =
        -mu_balls * speed * phi_radians.cos() * tangential_contact_slip / denominator;
    let vertical_impulse_per_mass =
        -mu_balls * speed * phi_radians.cos() * vertical_contact_slip / denominator;
    let local_tangential_velocity = speed * phi_radians.sin() + tangential_impulse_per_mass;
    let velocity_x = local_tangential_velocity * phi_radians.cos();
    let velocity_y = local_tangential_velocity * phi_radians.sin();
    let angular_x = wx + (5.0 / (2.0 * radius)) * phi_radians.cos() * vertical_impulse_per_mass;
    let angular_y = wy + (5.0 / (2.0 * radius)) * phi_radians.sin() * vertical_impulse_per_mass;

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
fn side_spin_produces_a_signed_post_contact_curve_estimate() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-radius * 2.0_f64.sqrt(), -radius * 2.0_f64.sqrt()),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(0.0, 0.0, -6.0),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let outcome =
        collide_ball_ball_detailed_on_table(&cue_ball, &object_ball, CollisionModel::ThrowAware);
    let curve = outcome
        .estimate_post_contact_cue_ball_curve(&BallSetPhysicsSpec::default(), &motion_config())
        .expect("sidespin should produce a post-contact curve estimate");

    assert_eq!(
        outcome
            .a_after
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Sliding
    );
    assert_close(curve.time_until_curve_starts.as_f64(), 0.0);
    assert!(curve.time_until_curve_completes.as_f64() > curve.time_until_curve_starts.as_f64());
    assert!(curve.curve_angle_degrees.abs() > 1e-9);
    assert_eq!(
        curve,
        estimate_post_contact_cue_ball_curve_on_table(
            &outcome.a_after,
            &BallSetPhysicsSpec::default(),
            &motion_config(),
        )
        .expect("direct helper should agree with the outcome method")
    );
}

#[test]
fn opposite_english_signs_produce_opposite_curve_directions() {
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
    let right_curve = collide_ball_ball_detailed_on_table(
        &right_english,
        &object_ball,
        CollisionModel::ThrowAware,
    )
    .estimate_post_contact_cue_ball_curve(&BallSetPhysicsSpec::default(), &motion_config())
    .expect("right english should curve");
    let left_curve = collide_ball_ball_detailed_on_table(
        &left_english,
        &object_ball,
        CollisionModel::ThrowAware,
    )
    .estimate_post_contact_cue_ball_curve(&BallSetPhysicsSpec::default(), &motion_config())
    .expect("left english should curve");

    assert!(right_curve.curve_angle_degrees.signum() != left_curve.curve_angle_degrees.signum());
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
