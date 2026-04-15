use billiards::{
    advance_motion_on_table, advance_to_next_event_for_two_on_table_balls,
    simulate_two_on_table_balls, AngularVelocity3, BallSetPhysicsSpec, BallState, CollisionModel,
    Inches, Inches2, InchesPerSecondSq, MotionPhase, MotionPhaseConfig, MotionTransitionConfig,
    OnTableBallState, OnTableMotionConfig, RadiansPerSecondSq, RollingResistanceModel, Seconds,
    SlidingFrictionModel, SpinDecayModel, TwoBallEventBall, TwoBallOnTableEvent, Velocity2,
    TYPICAL_BALL_RADIUS,
};

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
fn simulating_for_less_than_the_next_event_advances_both_balls_without_recording_any_event() {
    let config = motion_config();
    let dt = Seconds::new(0.25);
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

    let simulated = simulate_two_on_table_balls(
        &a,
        &b,
        dt,
        &BallSetPhysicsSpec::default(),
        &config,
        CollisionModel::Ideal,
    );
    let expected_a = OnTableBallState::try_from(
        advance_motion_on_table(&a, dt, &BallSetPhysicsSpec::default(), &config).state,
    )
    .expect("expected state should remain on-table");
    let expected_b = OnTableBallState::try_from(
        advance_motion_on_table(&b, dt, &BallSetPhysicsSpec::default(), &config).state,
    )
    .expect("expected state should remain on-table");

    assert_eq!(simulated.elapsed, dt);
    assert!(simulated.events.is_empty());
    assert_eq!(simulated.a, expected_a);
    assert_eq!(simulated.b, expected_b);
}

#[test]
fn simulating_past_one_event_records_it_and_consumes_the_remaining_time_afterward() {
    let config = motion_config();
    let dt = Seconds::new(0.6);
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

    let simulated = simulate_two_on_table_balls(
        &a,
        &b,
        dt,
        &BallSetPhysicsSpec::default(),
        &config,
        CollisionModel::Ideal,
    );
    let first = advance_to_next_event_for_two_on_table_balls(
        &a,
        &b,
        &BallSetPhysicsSpec::default(),
        &config,
        CollisionModel::Ideal,
    );
    let remaining = Seconds::new(dt.as_f64() - first.elapsed.as_f64());
    let expected_a = OnTableBallState::try_from(
        advance_motion_on_table(&first.a, remaining, &BallSetPhysicsSpec::default(), &config).state,
    )
    .expect("expected state should remain on-table");
    let expected_b = OnTableBallState::try_from(
        advance_motion_on_table(&first.b, remaining, &BallSetPhysicsSpec::default(), &config).state,
    )
    .expect("expected state should remain on-table");

    assert_eq!(simulated.elapsed, dt);
    assert_eq!(simulated.events.len(), 1);
    match &simulated.events[0] {
        TwoBallOnTableEvent::MotionTransition { ball, transition } => {
            assert_eq!(*ball, TwoBallEventBall::A);
            assert_eq!(transition.phase_before, MotionPhase::Sliding);
            assert_eq!(transition.phase_after, MotionPhase::Rolling);
        }
        other => panic!("expected motion transition, got {other:?}"),
    }
    assert_eq!(simulated.a, expected_a);
    assert_eq!(simulated.b, expected_b);
}

#[test]
fn simulating_through_multiple_events_records_them_in_order() {
    let config = motion_config();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let dt = Seconds::new(1.2);
    let a = on_table(BallState::on_table(
        inches2(0.0, -(2.0 * radius + 3.75)),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    ));
    let b = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let simulated = simulate_two_on_table_balls(
        &a,
        &b,
        dt,
        &BallSetPhysicsSpec::default(),
        &config,
        CollisionModel::Ideal,
    );
    let first = advance_to_next_event_for_two_on_table_balls(
        &a,
        &b,
        &BallSetPhysicsSpec::default(),
        &config,
        CollisionModel::Ideal,
    );
    let second = advance_to_next_event_for_two_on_table_balls(
        &first.a,
        &first.b,
        &BallSetPhysicsSpec::default(),
        &config,
        CollisionModel::Ideal,
    );
    let third = advance_to_next_event_for_two_on_table_balls(
        &second.a,
        &second.b,
        &BallSetPhysicsSpec::default(),
        &config,
        CollisionModel::Ideal,
    );
    let fourth = advance_to_next_event_for_two_on_table_balls(
        &third.a,
        &third.b,
        &BallSetPhysicsSpec::default(),
        &config,
        CollisionModel::Ideal,
    );
    let remaining = Seconds::new(
        dt.as_f64()
            - first.elapsed.as_f64()
            - second.elapsed.as_f64()
            - third.elapsed.as_f64()
            - fourth.elapsed.as_f64(),
    );
    let expected_a = OnTableBallState::try_from(
        advance_motion_on_table(
            &fourth.a,
            remaining,
            &BallSetPhysicsSpec::default(),
            &config,
        )
        .state,
    )
    .expect("expected state should remain on-table");
    let expected_b = OnTableBallState::try_from(
        advance_motion_on_table(
            &fourth.b,
            remaining,
            &BallSetPhysicsSpec::default(),
            &config,
        )
        .state,
    )
    .expect("expected state should remain on-table");

    assert_eq!(simulated.elapsed, dt);
    assert_eq!(simulated.events.len(), 4);
    match &simulated.events[0] {
        TwoBallOnTableEvent::BallBallCollision(_) => {}
        other => panic!("expected ball-ball collision, got {other:?}"),
    }
    match &simulated.events[1] {
        TwoBallOnTableEvent::MotionTransition { ball, transition } => {
            assert_eq!(*ball, TwoBallEventBall::A);
            assert_eq!(transition.phase_before, MotionPhase::Sliding);
            assert_eq!(transition.phase_after, MotionPhase::Rolling);
        }
        other => panic!("expected motion transition, got {other:?}"),
    }
    match &simulated.events[2] {
        TwoBallOnTableEvent::MotionTransition { ball, transition } => {
            assert_eq!(*ball, TwoBallEventBall::B);
            assert_eq!(transition.phase_before, MotionPhase::Sliding);
            assert_eq!(transition.phase_after, MotionPhase::Rolling);
        }
        other => panic!("expected motion transition, got {other:?}"),
    }
    match &simulated.events[3] {
        TwoBallOnTableEvent::MotionTransition { ball, transition } => {
            assert_eq!(*ball, TwoBallEventBall::A);
            assert_eq!(transition.phase_before, MotionPhase::Rolling);
            assert_eq!(transition.phase_after, MotionPhase::Rest);
        }
        other => panic!("expected motion transition, got {other:?}"),
    }
    assert_eq!(simulated.a, expected_a);
    assert_eq!(simulated.b, expected_b);
}
