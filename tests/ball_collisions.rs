use billiards::{
    collide_ball_ball_on_table, Angle, AngularVelocity3, BallState, CollisionModel, CutAngle,
    Inches, Inches2, OnTableBallState, Velocity2, TYPICAL_BALL_RADIUS,
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

fn velocity2(x: f64, y: f64) -> Velocity2 {
    Velocity2::new(Inches::from_f64(x), Inches::from_f64(y))
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
fn a_head_on_ideal_collision_transfers_forward_motion_without_transferring_spin() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(0.0, -2.0 * radius),
        velocity2(0.0, 10.0),
        AngularVelocity3::new(1.0, 2.0, 3.0),
    ));
    let object_ball = on_table(BallState::on_table(
        inches2(0.0, 0.0),
        velocity2(0.0, 0.0),
        AngularVelocity3::new(4.0, 5.0, 6.0),
    ));

    let cut_angle = CutAngle::from_headings(
        cue_ball
            .as_ball_state()
            .velocity
            .angle_from_north()
            .expect("moving cue ball should have a heading"),
        impact_heading(&cue_ball, &object_ball),
    );
    let (cue_after, object_after) =
        collide_ball_ball_on_table(&cue_ball, &object_ball, CollisionModel::Ideal);

    assert_close(cut_angle.as_degrees(), 0.0);
    assert_close(cue_after.as_ball_state().speed().as_f64(), 0.0);
    assert_close(object_after.as_ball_state().velocity.x().as_f64(), 0.0);
    assert_close(object_after.as_ball_state().velocity.y().as_f64(), 10.0);
    assert_eq!(
        cue_after.as_ball_state().angular_velocity,
        cue_ball.as_ball_state().angular_velocity
    );
    assert_eq!(
        object_after.as_ball_state().angular_velocity,
        object_ball.as_ball_state().angular_velocity
    );
    assert_eq!(
        cue_after.as_ball_state().position,
        cue_ball.as_ball_state().position
    );
    assert_eq!(
        object_after.as_ball_state().position,
        object_ball.as_ball_state().position
    );
}

#[test]
fn an_ideal_cut_collision_sends_the_object_ball_along_the_line_of_centers_and_the_cue_ball_along_the_tangent(
) {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let offset = radius * 2.0_f64.sqrt();
    let cue_ball = on_table(BallState::on_table(
        inches2(-offset, -offset),
        velocity2(0.0, 10.0),
        AngularVelocity3::zero(),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let impact_line = impact_heading(&cue_ball, &object_ball);
    let cut_angle = CutAngle::from_headings(
        cue_ball
            .as_ball_state()
            .velocity
            .angle_from_north()
            .expect("moving cue ball should have a heading"),
        impact_line,
    );
    let (cue_after, object_after) =
        collide_ball_ball_on_table(&cue_ball, &object_ball, CollisionModel::Ideal);
    let cue_after = cue_after.as_ball_state();
    let object_after = object_after.as_ball_state();
    let dot_product = cue_after.velocity.x().as_f64() * object_after.velocity.x().as_f64()
        + cue_after.velocity.y().as_f64() * object_after.velocity.y().as_f64();

    assert_close(cut_angle.as_degrees(), 45.0);
    assert_eq!(
        object_after
            .velocity
            .angle_from_north()
            .expect("moving object ball should have a heading"),
        impact_line
    );
    assert_close(object_after.velocity.x().as_f64(), 5.0);
    assert_close(object_after.velocity.y().as_f64(), 5.0);
    assert_close(cue_after.velocity.x().as_f64(), -5.0);
    assert_close(cue_after.velocity.y().as_f64(), 5.0);
    assert_close(dot_product, 0.0);
}
