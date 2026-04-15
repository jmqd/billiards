use billiards::{
    collide_ball_ball_detailed_on_table, collide_ball_ball_on_table, gearing_english, Angle,
    AngularVelocity3, BallState, CollisionModel, CutAngle, Inches, Inches2, OnTableBallState,
    Velocity2, TYPICAL_BALL_RADIUS,
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
fn a_cut_shot_without_side_spin_produces_cut_induced_throw() {
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
fn over_gearing_flips_the_throw_direction() {
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
}
