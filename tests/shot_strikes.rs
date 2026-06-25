use billiards::{
    cue_endmass_ratio_from_squirt, cue_natural_pivot_length,
    cue_squirt_angle_degrees_from_endmass_ratio, cue_tip_offset_for_pivot_angle,
    strike_resting_ball_on_table, Angle, BallSetPhysicsSpec, BallState, CueStrikeConfig,
    CueTipContact, Inches, Inches2, InchesPerSecond, MotionPhase, RestingOnTableBallState, Scale,
    Shot, ShotError, TYPICAL_BALL_RADIUS,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

fn assert_close_with_tolerance(actual: f64, expected: f64, tolerance: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta <= tolerance,
        "expected {expected} +/- {tolerance}, got {actual} (delta {delta})"
    );
}

fn resting_ball() -> RestingOnTableBallState {
    RestingOnTableBallState::try_from(BallState::resting_at(Inches2::new("10", "20")))
        .expect("resting test state should validate")
}

fn cue_config() -> CueStrikeConfig {
    CueStrikeConfig::new(Scale::from_f64(1.0), Scale::from_f64(0.1))
        .expect("test strike config should validate")
}

fn cue_from_tp_a30_center_ball_cor(cue_ounces: f64, cor: f64) -> CueStrikeConfig {
    let ball_to_cue_mass_ratio = 6.0 / cue_ounces;
    let efficiency = ((1.0 - ball_to_cue_mass_ratio * cor).powi(2)
        + ball_to_cue_mass_ratio * (1.0 + cor).powi(2))
        / (1.0 + ball_to_cue_mass_ratio).powi(2);

    CueStrikeConfig::new(
        Scale::from_f64(cue_ounces / 6.0),
        Scale::from_f64(1.0 - efficiency),
    )
    .expect("TP A.30 cue config should validate")
}

#[test]
fn a_center_ball_shot_seeds_forward_speed_without_spin() {
    let struck = strike_resting_ball_on_table(
        &resting_ball(),
        &Shot::new(
            Angle::from_north(0.0, 1.0),
            InchesPerSecond::new("10"),
            CueTipContact::center(),
        )
        .expect("shot should validate"),
        &cue_config(),
        &BallSetPhysicsSpec::default(),
    )
    .expect("center-ball strike should succeed");

    assert_close(struck.as_ball_state().position.x().as_f64(), 10.0);
    assert_close(struck.as_ball_state().position.y().as_f64(), 20.0);
    assert_close(struck.as_ball_state().velocity.x().as_f64(), 0.0);
    assert_close(
        struck.as_ball_state().velocity.y().as_f64(),
        10.0 * (1.0 + (0.8_f64).sqrt()) / 2.0,
    );
    assert_close(struck.as_ball_state().angular_velocity.x().as_f64(), 0.0);
    assert_close(struck.as_ball_state().angular_velocity.y().as_f64(), 0.0);
    assert_close(struck.as_ball_state().angular_velocity.z().as_f64(), 0.0);
    assert_eq!(
        struck
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Sliding
    );
}

#[test]
fn tp_a30_center_ball_tip_efficiency_and_cue_weight_benchmarks() {
    let speed_ratio_for = |cue_ounces, cor| {
        let cue = cue_from_tp_a30_center_ball_cor(cue_ounces, cor);
        let cue_speed = InchesPerSecond::from_mph(15.0);
        let shot = Shot::new(
            Angle::from_north(0.0, 1.0),
            cue_speed.clone(),
            CueTipContact::center(),
        )
        .expect("shot should validate");

        strike_resting_ball_on_table(&resting_ball(), &shot, &cue, &BallSetPhysicsSpec::default())
            .expect("center-ball TP A.30 strike should succeed")
            .as_ball_state()
            .speed()
            .as_f64()
            / cue_speed.as_f64()
    };

    let leather = speed_ratio_for(19.0, 0.73);
    let phenolic = speed_ratio_for(19.0, 0.87);
    assert_close(leather, 1.3148);
    assert_close(phenolic, 1.4212);
    assert_close(phenolic / leather, 1.0809248554913295);
    assert_close((phenolic / leather).powi(2), 1.1683985432189515);

    let light = speed_ratio_for(17.0, 0.87);
    let heavy = speed_ratio_for(22.0, 0.87);
    assert_close(light, 1.3821739130434782);
    assert_close(heavy, 1.4692857142857143);
    assert_close(heavy / light, 1.0630252100840336);
    assert_close((heavy / light).powi(2), 1.1300225972742038);
}

#[test]
fn a_two_fifths_high_center_hit_seeds_natural_roll() {
    let struck = strike_resting_ball_on_table(
        &resting_ball(),
        &Shot::new(
            Angle::from_north(0.0, 1.0),
            InchesPerSecond::new("10"),
            CueTipContact::new(Scale::zero(), Scale::from_f64(0.4)).expect("tip contact"),
        )
        .expect("shot should validate"),
        &cue_config(),
        &BallSetPhysicsSpec::default(),
    )
    .expect("slightly high center strike should succeed");

    assert_eq!(
        struck
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Rolling
    );
    assert_close(
        struck
            .as_ball_state()
            .cloth_contact_speed(TYPICAL_BALL_RADIUS.clone())
            .as_f64(),
        0.0,
    );
    assert_close(struck.as_ball_state().angular_velocity.z().as_f64(), 0.0);
}

#[test]
fn a_follow_shot_seeds_topspin_in_the_shot_frame() {
    let struck = strike_resting_ball_on_table(
        &resting_ball(),
        &Shot::new(
            Angle::from_north(0.0, 1.0),
            InchesPerSecond::new("10"),
            CueTipContact::new(Scale::zero(), Scale::from_f64(0.5)).expect("tip contact"),
        )
        .expect("shot should validate"),
        &cue_config(),
        &BallSetPhysicsSpec::default(),
    )
    .expect("follow strike should succeed");

    assert!(struck.as_ball_state().velocity.y().as_f64() > 0.0);
    assert!(struck.as_ball_state().angular_velocity.x().as_f64() < 0.0);
    assert_close(struck.as_ball_state().angular_velocity.y().as_f64(), 0.0);
    assert_close(struck.as_ball_state().angular_velocity.z().as_f64(), 0.0);
}

#[test]
fn a_draw_shot_seeds_reverse_topspin_in_the_shot_frame() {
    let struck = strike_resting_ball_on_table(
        &resting_ball(),
        &Shot::new(
            Angle::from_north(0.0, 1.0),
            InchesPerSecond::new("10"),
            CueTipContact::new(Scale::zero(), Scale::from_f64(-0.5)).expect("tip contact"),
        )
        .expect("shot should validate"),
        &cue_config(),
        &BallSetPhysicsSpec::default(),
    )
    .expect("draw strike should succeed");

    assert!(struck.as_ball_state().angular_velocity.x().as_f64() > 0.0);
    assert_close(struck.as_ball_state().angular_velocity.y().as_f64(), 0.0);
    assert_close(struck.as_ball_state().angular_velocity.z().as_f64(), 0.0);
}

#[test]
fn side_offset_seeds_vertical_axis_spin() {
    let struck = strike_resting_ball_on_table(
        &resting_ball(),
        &Shot::new(
            Angle::from_north(0.0, 1.0),
            InchesPerSecond::new("10"),
            CueTipContact::new(Scale::from_f64(0.5), Scale::zero()).expect("tip contact"),
        )
        .expect("shot should validate"),
        &cue_config(),
        &BallSetPhysicsSpec::default(),
    )
    .expect("side-spin strike should succeed");

    assert!(
        struck.as_ball_state().velocity.x().as_f64() < 0.0,
        "positive side offset should squirt the cue ball opposite the tip side"
    );
    assert!(struck.as_ball_state().velocity.y().as_f64() > 0.0);
    assert_close(struck.as_ball_state().angular_velocity.x().as_f64(), 0.0);
    assert_close(struck.as_ball_state().angular_velocity.y().as_f64(), 0.0);
    assert!(struck.as_ball_state().angular_velocity.z().as_f64() > 0.0);
}

#[test]
fn cue_squirt_angle_uses_the_configured_endmass_ratio_and_sign() {
    let regular_cue = cue_config();
    let low_squirt_cue = CueStrikeConfig::new_with_endmass_ratio(
        Scale::from_f64(1.0),
        Scale::from_f64(0.1),
        Scale::from_f64(40.0),
    )
    .expect("low-squirt test cue should validate");
    let right_tip_shot = Shot::new(
        Angle::from_north(0.0, 1.0),
        InchesPerSecond::new("10"),
        CueTipContact::new(Scale::from_f64(0.5), Scale::zero()).expect("tip contact"),
    )
    .expect("shot should validate");
    let left_tip_shot = Shot::new(
        Angle::from_north(0.0, 1.0),
        InchesPerSecond::new("10"),
        CueTipContact::new(Scale::from_f64(-0.5), Scale::zero()).expect("tip contact"),
    )
    .expect("shot should validate");

    let right_tip = strike_resting_ball_on_table(
        &resting_ball(),
        &right_tip_shot,
        &regular_cue,
        &BallSetPhysicsSpec::default(),
    )
    .expect("right-tip strike should succeed");
    let left_tip = strike_resting_ball_on_table(
        &resting_ball(),
        &left_tip_shot,
        &regular_cue,
        &BallSetPhysicsSpec::default(),
    )
    .expect("left-tip strike should succeed");
    let low_squirt = strike_resting_ball_on_table(
        &resting_ball(),
        &right_tip_shot,
        &low_squirt_cue,
        &BallSetPhysicsSpec::default(),
    )
    .expect("low-squirt strike should succeed");

    let regular_heading = right_tip
        .as_ball_state()
        .velocity
        .angle_from_north()
        .expect("moving ball should have a heading")
        .as_degrees();
    let mirror_heading = left_tip
        .as_ball_state()
        .velocity
        .angle_from_north()
        .expect("moving ball should have a heading")
        .as_degrees();
    let br = 0.5_f64;
    let transverse_contact_factor = (1.0 - br * br).sqrt();
    let expected_squirt_degrees = (2.5 * br * transverse_contact_factor
        / (1.0 + 20.151 + 2.5 * transverse_contact_factor * transverse_contact_factor))
        .atan()
        .to_degrees();

    assert_close(regular_heading, 360.0 - expected_squirt_degrees);
    assert_close(mirror_heading, expected_squirt_degrees);
    assert!(right_tip.as_ball_state().velocity.x().as_f64() < 0.0);
    assert!(left_tip.as_ball_state().velocity.x().as_f64() > 0.0);
    assert!(
        low_squirt.as_ball_state().velocity.x().as_f64().abs()
            < right_tip.as_ball_state().velocity.x().as_f64().abs(),
        "larger ball-to-endmass ratio should reduce squirt"
    );
    assert!(
        low_squirt.as_ball_state().angular_velocity.z().as_f64()
            > right_tip.as_ball_state().angular_velocity.z().as_f64(),
        "reduced squirt should leave a slightly larger effective side-spin offset"
    );
    let expected_effective_offset = (br.asin() - expected_squirt_degrees.to_radians()).sin();
    let post_strike_speed = right_tip.as_ball_state().speed().as_f64();
    assert_close(
        right_tip.as_ball_state().angular_velocity.z().as_f64(),
        2.5 * post_strike_speed / TYPICAL_BALL_RADIUS.as_f64() * expected_effective_offset,
    );
}

#[test]
fn cue_squirt_matches_tp_b1_real_cue_examples() {
    let ball = BallSetPhysicsSpec::default();

    for (name, tip_offset_inches, endmass_ratio, expected_squirt_degrees) in [
        ("Players regular cue", 0.51, 20.151, 2.5),
        ("Predator Z low-squirt cue", 0.51, 29.158, 1.8),
        ("Stinger break/jump cue", 0.3, 12.008, 2.4),
    ] {
        let cue = CueStrikeConfig::new_with_endmass_ratio(
            Scale::from_f64(1.0),
            Scale::from_f64(0.1),
            Scale::from_f64(endmass_ratio),
        )
        .expect("paper cue example should validate");
        let normalized_offset = tip_offset_inches / TYPICAL_BALL_RADIUS.as_f64();
        let shot = Shot::new(
            Angle::from_north(0.0, 1.0),
            InchesPerSecond::new("10"),
            CueTipContact::new(Scale::from_f64(normalized_offset), Scale::zero())
                .expect("paper tip offset should validate"),
        )
        .expect("paper squirt shot should validate");

        let struck = strike_resting_ball_on_table(&resting_ball(), &shot, &cue, &ball)
            .expect("paper squirt strike should succeed");
        let heading = struck
            .as_ball_state()
            .velocity
            .angle_from_north()
            .expect("struck cue ball should move")
            .as_degrees();
        let squirt = (360.0 - heading).rem_euclid(360.0);

        assert_close_with_tolerance(squirt, expected_squirt_degrees, 0.005);
        assert!(
            struck.as_ball_state().velocity.x().as_f64() < 0.0,
            "{name} should squirt opposite a positive side tip offset"
        );
    }
}

#[test]
fn tp_a31_squirt_model_intentionally_differs_from_shepard_table_i() {
    let ball_radius = Inches::from_f64(2.25 / 2.0);
    let tip_offset = Inches::from_f64(ball_radius.as_f64() * 3.0 / 8.0);

    // TP A.31 / TP B.1 explicitly uses a different `b` convention than Shepard. Table I is a
    // useful comparison, but it is not the oracle for this helper's contract.
    for (endmass_ratio, expected_tp_a31_squirt_degrees, shepard_table_i_squirt_degrees) in [
        (20.0, 2.150104931702617, 2.331),
        (30.0, 1.5018367423454364, 1.588),
        (50.0, 0.9368198000602023, 0.970),
        (100.0, 0.48273894428091646, 0.491),
    ] {
        let squirt = cue_squirt_angle_degrees_from_endmass_ratio(
            tip_offset.clone(),
            Scale::from_f64(endmass_ratio),
            ball_radius.clone(),
        );

        assert_close(squirt, expected_tp_a31_squirt_degrees);
        assert!(
            (squirt - shepard_table_i_squirt_degrees).abs() > 0.005,
            "Shepard Table I uses a different convention and should not silently become this helper's oracle"
        );
    }
}

#[test]
fn cue_pivot_helpers_match_tp_b1_real_cue_examples() {
    let ball_radius = Inches::from_f64(2.25 / 2.0);
    let dime_tip_radius = Inches::from_f64(0.705 / 2.0);
    let break_tip_radius = Inches::from_f64(0.5);

    for (
        name,
        tip_offset_inches,
        squirt_degrees,
        tip_radius,
        expected_endmass_ratio,
        expected_pivot_length_inches,
    ) in [
        (
            "Players regular cue",
            0.51,
            2.5,
            dime_tip_radius.clone(),
            20.151,
            14.231,
        ),
        (
            "Predator Z low-squirt cue",
            0.51,
            1.8,
            dime_tip_radius.clone(),
            29.158,
            20.199,
        ),
        (
            "Stinger break/jump cue",
            0.3,
            2.4,
            break_tip_radius.clone(),
            12.008,
            9.223,
        ),
    ] {
        let tip_offset = Inches::from_f64(tip_offset_inches);
        let endmass_ratio =
            cue_endmass_ratio_from_squirt(tip_offset.clone(), squirt_degrees, ball_radius.clone());
        let pivot_length = cue_natural_pivot_length(
            squirt_degrees,
            tip_offset.clone(),
            tip_radius.clone(),
            ball_radius.clone(),
        );
        let round_trip_offset = cue_tip_offset_for_pivot_angle(
            squirt_degrees,
            pivot_length.clone(),
            tip_radius,
            ball_radius.clone(),
        );
        let round_trip_squirt = cue_squirt_angle_degrees_from_endmass_ratio(
            tip_offset.clone(),
            endmass_ratio.clone(),
            ball_radius.clone(),
        );

        assert_close_with_tolerance(endmass_ratio.as_f64(), expected_endmass_ratio, 0.001);
        assert_close_with_tolerance(pivot_length.as_f64(), expected_pivot_length_inches, 0.001);
        assert_close(round_trip_offset.as_f64(), tip_offset_inches);
        assert_close_with_tolerance(round_trip_squirt, squirt_degrees, 0.001);
        assert!(
            pivot_length.as_f64() > 0.0,
            "{name} should have a positive natural pivot length"
        );
    }
}

#[test]
fn cue_squirt_is_relative_to_the_shot_heading() {
    let struck = strike_resting_ball_on_table(
        &resting_ball(),
        &Shot::new(
            Angle::from_north(1.0, 0.0),
            InchesPerSecond::new("10"),
            CueTipContact::new(Scale::from_f64(0.5), Scale::zero()).expect("tip contact"),
        )
        .expect("shot should validate"),
        &cue_config(),
        &BallSetPhysicsSpec::default(),
    )
    .expect("side-spin strike should succeed");

    assert!(struck.as_ball_state().velocity.x().as_f64() > 0.0);
    assert!(
        struck.as_ball_state().velocity.y().as_f64() > 0.0,
        "positive side offset on an eastward shot should squirt toward table north, the shot-frame left side"
    );
    assert!(struck.as_ball_state().angular_velocity.z().as_f64() > 0.0);
}

#[test]
fn shot_heading_rotates_velocity_and_horizontal_spin_into_table_coordinates() {
    let struck = strike_resting_ball_on_table(
        &resting_ball(),
        &Shot::new(
            Angle::from_north(1.0, 0.0),
            InchesPerSecond::new("10"),
            CueTipContact::new(Scale::zero(), Scale::from_f64(0.5)).expect("tip contact"),
        )
        .expect("shot should validate"),
        &cue_config(),
        &BallSetPhysicsSpec::default(),
    )
    .expect("rotated follow strike should succeed");

    assert!(struck.as_ball_state().velocity.x().as_f64() > 0.0);
    assert_close(struck.as_ball_state().velocity.y().as_f64(), 0.0);
    assert_close(struck.as_ball_state().angular_velocity.x().as_f64(), 0.0);
    assert!(struck.as_ball_state().angular_velocity.y().as_f64() > 0.0);
}

#[test]
fn excessive_tip_offset_reports_a_miscue_before_other_strike_failures() {
    let error = strike_resting_ball_on_table(
        &resting_ball(),
        &Shot::new(
            Angle::from_north(0.0, 1.0),
            InchesPerSecond::new("10"),
            CueTipContact::new(Scale::from_f64(0.9), Scale::zero()).expect("tip contact"),
        )
        .expect("shot should validate"),
        &cue_config(),
        &BallSetPhysicsSpec::default(),
    )
    .expect_err("offsets beyond the configured miscue limit should miscue");

    assert!(matches!(error, ShotError::Miscue { .. }));
}

#[test]
fn default_miscue_limit_accepts_the_tp_a22_half_radius_boundary() {
    let ball = BallSetPhysicsSpec::default();
    let cue = cue_config();
    let side_boundary = Shot::new(
        Angle::from_north(0.0, 1.0),
        InchesPerSecond::new("10"),
        CueTipContact::new(Scale::from_f64(0.5), Scale::zero()).expect("tip contact"),
    )
    .expect("boundary side tip shot should validate");
    let diagonal_boundary = Shot::new(
        Angle::from_north(0.0, 1.0),
        InchesPerSecond::new("10"),
        CueTipContact::new(Scale::from_f64(0.3), Scale::from_f64(0.4)).expect("tip contact"),
    )
    .expect("boundary diagonal tip shot should validate");
    let just_over_boundary = Shot::new(
        Angle::from_north(0.0, 1.0),
        InchesPerSecond::new("10"),
        CueTipContact::new(Scale::from_f64(0.5 + 1e-9), Scale::zero()).expect("tip contact"),
    )
    .expect("just-over-boundary side tip shot should validate");

    strike_resting_ball_on_table(&resting_ball(), &side_boundary, &cue, &ball)
        .expect("TP A.22 half-radius side offset should remain inside the miscue limit");
    strike_resting_ball_on_table(&resting_ball(), &diagonal_boundary, &cue, &ball)
        .expect("TP A.22 half-radius radial offset should remain inside the miscue limit");
    let error = strike_resting_ball_on_table(&resting_ball(), &just_over_boundary, &cue, &ball)
        .expect_err("offsets just beyond the half-radius limit should miscue");

    assert!(matches!(error, ShotError::Miscue { .. }));
}

#[test]
fn unsupported_large_tip_offset_can_still_report_no_separation_with_a_relaxed_miscue_limit() {
    let relaxed_miscue_limit = CueStrikeConfig::new_with_miscue_offset_limit(
        Scale::from_f64(1.0),
        Scale::from_f64(0.1),
        Scale::from_f64(0.95),
    )
    .expect("relaxed miscue limit should validate");
    let error = strike_resting_ball_on_table(
        &resting_ball(),
        &Shot::new(
            Angle::from_north(0.0, 1.0),
            InchesPerSecond::new("10"),
            CueTipContact::new(Scale::from_f64(0.9), Scale::zero()).expect("tip contact"),
        )
        .expect("shot should validate"),
        &relaxed_miscue_limit,
        &BallSetPhysicsSpec::default(),
    )
    .expect_err("within the relaxed miscue limit, the no-separation guard should still apply");

    assert!(matches!(
        error,
        ShotError::CueBallDoesNotSeparateFromCue { .. }
    ));
}
