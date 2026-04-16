use billiards::{
    strike_resting_ball_on_table, Angle, BallSetPhysicsSpec, BallState, CueStrikeConfig,
    CueTipContact, Inches2, InchesPerSecond, MotionPhase, RestingOnTableBallState, Scale, Shot,
    ShotError, TYPICAL_BALL_RADIUS,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
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

    assert_close(struck.as_ball_state().angular_velocity.x().as_f64(), 0.0);
    assert_close(struck.as_ball_state().angular_velocity.y().as_f64(), 0.0);
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
fn unsupported_large_tip_offset_reports_that_the_cue_ball_would_not_separate_cleanly() {
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
    .expect_err("large offsets should leave the model's supported automatic-separation regime");

    assert!(matches!(
        error,
        ShotError::CueBallDoesNotSeparateFromCue { .. }
    ));
}
