use billiards::{
    advance_to_next_event_for_two_on_table_balls, AngularVelocity3, BallSetPhysicsSpec, BallState,
    CollisionModel, Inches, Inches2, InchesPerSecondSq, MotionPhase, MotionPhaseConfig,
    MotionTransitionConfig, OnTableBallState, OnTableMotionConfig, RadiansPerSecondSq,
    RollingResistanceModel, SlidingFrictionModel, SpinDecayModel, TwoBallEventBall,
    TwoBallOnTableEvent, Velocity2, TYPICAL_BALL_RADIUS,
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

#[test]
fn advancing_to_a_motion_transition_advances_both_balls_to_that_time() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let a = on_table(BallState::on_table(
        inches2(0.0, 0.0),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    ));
    let b = on_table(BallState::on_table(
        inches2(20.0, 0.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    ));

    let advanced = advance_to_next_event_for_two_on_table_balls(
        &a,
        &b,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
    );

    let reported_event = advanced
        .event
        .as_ref()
        .expect("an event should be reported");
    assert_eq!(reported_event.primary_ball(), Some(TwoBallEventBall::A));
    assert_close(reported_event.time().as_f64(), advanced.elapsed.as_f64());
    match reported_event {
        TwoBallOnTableEvent::MotionTransition { ball, transition } => {
            assert_eq!(*ball, TwoBallEventBall::A);
            assert_eq!(transition.phase_before, MotionPhase::Sliding);
            assert_eq!(transition.phase_after, MotionPhase::Rolling);
        }
        other => panic!("expected motion transition, got {other:?}"),
    }

    assert_close(advanced.elapsed.as_f64(), 4.0 / 7.0);
    assert_close(advanced.a.as_ball_state().position.x().as_f64(), 0.0);
    assert_close(
        advanced.a.as_ball_state().position.y().as_f64(),
        240.0 / 49.0,
    );
    assert_close(advanced.a.as_ball_state().velocity.x().as_f64(), 0.0);
    assert_close(advanced.a.as_ball_state().velocity.y().as_f64(), 50.0 / 7.0);
    assert_close(
        advanced.a.as_ball_state().angular_velocity.x().as_f64(),
        -50.0 / (7.0 * radius),
    );
    assert_eq!(
        advanced
            .a
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Rolling
    );

    assert_close(advanced.b.as_ball_state().position.x().as_f64(), 20.0);
    assert_close(advanced.b.as_ball_state().position.y().as_f64(), 0.0);
    assert_close(advanced.b.as_ball_state().speed().as_f64(), 0.0);
    assert_close(
        advanced.b.as_ball_state().angular_velocity.z().as_f64(),
        34.0 / 7.0,
    );
    assert_eq!(
        advanced
            .b
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Spinning
    );
}

#[test]
fn advancing_to_a_ball_ball_collision_resolves_the_immediate_post_collision_state() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let a = on_table(BallState::on_table(
        inches2(0.0, -(2.0 * radius + 7.5)),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let b = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let advanced = advance_to_next_event_for_two_on_table_balls(
        &a,
        &b,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
    );

    let reported_event = advanced
        .event
        .as_ref()
        .expect("an event should be reported");
    assert_eq!(reported_event.primary_ball(), None);
    assert_close(reported_event.time().as_f64(), advanced.elapsed.as_f64());
    match reported_event {
        TwoBallOnTableEvent::BallBallCollision(collision) => {
            assert_close(collision.time_until_impact.as_f64(), 1.0);
        }
        other => panic!("expected ball-ball collision, got {other:?}"),
    }

    assert_close(advanced.elapsed.as_f64(), 1.0);
    assert_close(advanced.a.as_ball_state().position.x().as_f64(), 0.0);
    assert_close(
        advanced.a.as_ball_state().position.y().as_f64(),
        -2.0 * radius,
    );
    assert_close(advanced.a.as_ball_state().speed().as_f64(), 0.0);
    assert_close(
        advanced.a.as_ball_state().angular_velocity.x().as_f64(),
        -5.0 / radius,
    );
    assert_close(
        advanced.a.as_ball_state().angular_velocity.y().as_f64(),
        0.0,
    );
    assert_close(
        advanced.a.as_ball_state().angular_velocity.z().as_f64(),
        0.0,
    );

    assert_close(advanced.b.as_ball_state().position.x().as_f64(), 0.0);
    assert_close(advanced.b.as_ball_state().position.y().as_f64(), 0.0);
    assert_close(advanced.b.as_ball_state().velocity.x().as_f64(), 0.0);
    assert_close(advanced.b.as_ball_state().velocity.y().as_f64(), 5.0);
    assert_eq!(
        advanced.b.as_ball_state().angular_velocity,
        b.as_ball_state().angular_velocity
    );
}

#[test]
fn advancing_with_no_future_event_returns_the_original_two_ball_state() {
    let a = on_table(BallState::resting_at(inches2(-10.0, 0.0)));
    let b = on_table(BallState::resting_at(inches2(10.0, 0.0)));

    let advanced = advance_to_next_event_for_two_on_table_balls(
        &a,
        &b,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
    );

    assert_eq!(advanced.elapsed.as_f64(), 0.0);
    assert!(advanced.event.is_none());
    assert_eq!(advanced.a, a);
    assert_eq!(advanced.b, b);
}
