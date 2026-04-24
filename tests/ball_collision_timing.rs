use billiards::{
    collide_ball_ball_on_table, compute_next_ball_ball_collision_during_current_phases_on_table,
    compute_next_ball_ball_collision_on_table, Angle, AngularVelocity3, BallSetPhysicsSpec,
    BallState, CollisionModel, CutAngle, Inches, Inches2, InchesPerSecondSq, MotionPhaseConfig,
    MotionTransitionConfig, OnTableBallState, OnTableMotionConfig, RadiansPerSecondSq,
    RollingResistanceModel, SlidingFrictionModel, SpinDecayModel, Velocity2, TYPICAL_BALL_RADIUS,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
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

fn on_table(state: BallState) -> OnTableBallState {
    OnTableBallState::try_from(state).expect("test states should validate as on-table")
}

fn inches2(x: f64, y: f64) -> Inches2 {
    Inches2::new(Inches::from_f64(x), Inches::from_f64(y))
}

fn velocity2(x: f64, y: f64) -> Velocity2 {
    Velocity2::new(Inches::from_f64(x), Inches::from_f64(y))
}

fn center_distance(a: &OnTableBallState, b: &OnTableBallState) -> f64 {
    let a = a.as_ball_state();
    let b = b.as_ball_state();
    let dx = b.position.x().as_f64() - a.position.x().as_f64();
    let dy = b.position.y().as_f64() - a.position.y().as_f64();

    dx.hypot(dy)
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
fn a_head_on_closing_ball_predicts_the_first_impact_time_and_feeds_the_ideal_collision_response() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(0.0, -(2.0 * radius + 10.0)),
        velocity2(0.0, 5.0),
        AngularVelocity3::new(1.0, 2.0, 3.0),
    ));
    let object_ball = on_table(BallState::on_table(
        inches2(0.0, 0.0),
        Velocity2::zero(),
        AngularVelocity3::new(4.0, 5.0, 6.0),
    ));

    let predicted = compute_next_ball_ball_collision_on_table(
        &cue_ball,
        &object_ball,
        &BallSetPhysicsSpec::default(),
    )
    .expect("closing balls should predict a future impact");
    let (cue_after, object_after) = collide_ball_ball_on_table(
        &predicted.a_at_impact,
        &predicted.b_at_impact,
        CollisionModel::Ideal,
    );

    assert_close(predicted.time_until_impact.as_f64(), 2.0);
    assert_close(
        predicted.a_at_impact.as_ball_state().position.x().as_f64(),
        0.0,
    );
    assert_close(
        predicted.a_at_impact.as_ball_state().position.y().as_f64(),
        -2.0 * radius,
    );
    assert_close(
        center_distance(&predicted.a_at_impact, &predicted.b_at_impact),
        2.0 * radius,
    );
    assert_close(cue_after.as_ball_state().speed().as_f64(), 0.0);
    assert_close(object_after.as_ball_state().velocity.x().as_f64(), 0.0);
    assert_close(object_after.as_ball_state().velocity.y().as_f64(), 5.0);
}

#[test]
fn balls_moving_apart_do_not_predict_a_future_collision() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::resting_at(inches2(0.0, -2.0 * radius)));
    let object_ball = on_table(BallState::on_table(
        inches2(0.0, 2.0 * radius),
        velocity2(0.0, 5.0),
        AngularVelocity3::zero(),
    ));

    assert!(compute_next_ball_ball_collision_on_table(
        &cue_ball,
        &object_ball,
        &BallSetPhysicsSpec::default()
    )
    .is_none());
}

#[test]
fn an_off_line_trajectory_that_misses_returns_none() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(0.0, -10.0),
        velocity2(0.0, 5.0),
        AngularVelocity3::zero(),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(3.0 * radius, 0.0)));

    assert!(compute_next_ball_ball_collision_on_table(
        &cue_ball,
        &object_ball,
        &BallSetPhysicsSpec::default()
    )
    .is_none());
}

#[test]
fn an_oblique_predicted_impact_preserves_the_expected_line_of_centers_geometry() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(0.0, -10.0),
        velocity2(0.0, 5.0),
        AngularVelocity3::zero(),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(radius * 2.0_f64.sqrt(), 0.0)));

    let predicted = compute_next_ball_ball_collision_on_table(
        &cue_ball,
        &object_ball,
        &BallSetPhysicsSpec::default(),
    )
    .expect("the oblique path should intersect the target contact circle");
    let impact_line = impact_heading(&predicted.a_at_impact, &predicted.b_at_impact);
    let cut_angle = CutAngle::from_headings(
        cue_ball
            .as_ball_state()
            .velocity
            .angle_from_north()
            .expect("moving cue ball should have a heading"),
        impact_line,
    );
    let (cue_after, object_after) = collide_ball_ball_on_table(
        &predicted.a_at_impact,
        &predicted.b_at_impact,
        CollisionModel::Ideal,
    );
    let cue_after = cue_after.as_ball_state();
    let object_after = object_after.as_ball_state();
    let dot_product = cue_after.velocity.x().as_f64() * object_after.velocity.x().as_f64()
        + cue_after.velocity.y().as_f64() * object_after.velocity.y().as_f64();

    assert_close(cut_angle.as_degrees(), 45.0);
    assert_close(
        object_after
            .velocity
            .angle_from_north()
            .expect("moving object ball should have a heading")
            .as_degrees(),
        impact_line.as_degrees(),
    );
    assert_close(
        center_distance(&predicted.a_at_impact, &predicted.b_at_impact),
        2.0 * radius,
    );
    assert_close(dot_product, 0.0);
}

#[test]
fn the_phase_aware_predictor_uses_the_current_rolling_model_before_contact() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(0.0, -(2.0 * radius + 7.5)),
        velocity2(0.0, 10.0),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let predicted = compute_next_ball_ball_collision_during_current_phases_on_table(
        &cue_ball,
        &object_ball,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .expect("the rolling ball should still reach contact before it stops");

    assert_close(predicted.time_until_impact.as_f64(), 1.0);
    assert_close(
        predicted.a_at_impact.as_ball_state().position.y().as_f64(),
        -2.0 * radius,
    );
    assert_close(
        predicted.a_at_impact.as_ball_state().velocity.y().as_f64(),
        5.0,
    );
}

#[test]
fn the_phase_aware_predictor_finds_a_grazing_collision_between_fixed_scan_samples() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let contact_distance = 2.0 * radius;
    let rolling_speed = 120.0;
    let rolling_deceleration = 5.0;
    let object_x = 60.0;
    let object_y = contact_distance - 0.0001;
    let contact_x_clearance = (contact_distance * contact_distance - object_y * object_y).sqrt();
    let contact_travel = object_x - contact_x_clearance;
    let expected_time = (rolling_speed
        - (rolling_speed * rolling_speed - 2.0 * rolling_deceleration * contact_travel).sqrt())
        / rolling_deceleration;
    let cue_ball = on_table(BallState::on_table(
        inches2(0.0, 0.0),
        velocity2(rolling_speed, 0.0),
        AngularVelocity3::new(0.0, rolling_speed / radius, 0.0),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(object_x, object_y)));

    let predicted = compute_next_ball_ball_collision_during_current_phases_on_table(
        &cue_ball,
        &object_ball,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .expect("a near-tangent contact should be found even when it falls between coarse samples");

    assert_close(predicted.time_until_impact.as_f64(), expected_time);
    assert_close(
        center_distance(&predicted.a_at_impact, &predicted.b_at_impact),
        contact_distance,
    );
}

#[test]
fn the_phase_aware_predictor_returns_none_when_the_ball_stops_before_contact() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(0.0, -(2.0 * radius + 11.0)),
        velocity2(0.0, 10.0),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    assert!(
        compute_next_ball_ball_collision_during_current_phases_on_table(
            &cue_ball,
            &object_ball,
            &BallSetPhysicsSpec::default(),
            &motion_config(),
        )
        .is_none()
    );
}
