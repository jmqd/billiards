use billiards::{
    compute_next_event_for_two_on_table_balls, AngularVelocity3, BallSetPhysicsSpec, BallState,
    Inches, Inches2, InchesPerSecondSq, MotionPhase, MotionPhaseConfig, MotionTransitionConfig,
    OnTableBallState, OnTableMotionConfig, RadiansPerSecondSq, RollingResistanceModel,
    SlidingFrictionModel, SpinDecayModel, TwoBallEventBall, TwoBallOnTableEvent, Velocity2,
    TYPICAL_BALL_RADIUS,
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
fn the_scheduler_picks_a_ball_ball_collision_when_it_arrives_before_any_motion_transition() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let a = on_table(BallState::on_table(
        inches2(0.0, -(2.0 * radius + 7.5)),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let b = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let event = compute_next_event_for_two_on_table_balls(
        &a,
        &b,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .expect("an event should be predicted");

    match event {
        TwoBallOnTableEvent::BallBallCollision(collision) => {
            assert_close(collision.time_until_impact.as_f64(), 1.0);
        }
        other => panic!("expected ball-ball collision, got {other:?}"),
    }
}

#[test]
fn the_scheduler_picks_a_motion_transition_when_it_precedes_a_later_collision() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let a = on_table(BallState::on_table(
        inches2(0.0, -(2.0 * radius + 10.0)),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    ));
    let b = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let event = compute_next_event_for_two_on_table_balls(
        &a,
        &b,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .expect("an event should be predicted");

    match event {
        TwoBallOnTableEvent::MotionTransition { ball, transition } => {
            assert_eq!(ball, TwoBallEventBall::A);
            assert_eq!(transition.phase_before, MotionPhase::Sliding);
            assert_eq!(transition.phase_after, MotionPhase::Rolling);
            assert_close(transition.time_until_transition.as_f64(), 4.0 / 7.0);
        }
        other => panic!("expected motion transition, got {other:?}"),
    }
}

#[test]
fn the_scheduler_compares_the_two_balls_motion_transitions_and_returns_the_earliest_one() {
    let a = on_table(BallState::on_table(
        inches2(-20.0, 0.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    ));
    let b = on_table(BallState::on_table(
        inches2(20.0, 0.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 1.0),
    ));

    let event = compute_next_event_for_two_on_table_balls(
        &a,
        &b,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .expect("an event should be predicted");

    match event {
        TwoBallOnTableEvent::MotionTransition { ball, transition } => {
            assert_eq!(ball, TwoBallEventBall::B);
            assert_eq!(transition.phase_before, MotionPhase::Spinning);
            assert_eq!(transition.phase_after, MotionPhase::Rest);
            assert_close(transition.time_until_transition.as_f64(), 0.5);
        }
        other => panic!("expected motion transition, got {other:?}"),
    }
}

#[test]
fn the_scheduler_uses_phase_aware_collision_timing_and_picks_stop_when_a_rolling_ball_cannot_reach_contact(
) {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let a = on_table(BallState::on_table(
        inches2(0.0, -(2.0 * radius + 11.0)),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let b = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    let event = compute_next_event_for_two_on_table_balls(
        &a,
        &b,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .expect("an event should be predicted");

    match event {
        TwoBallOnTableEvent::MotionTransition { ball, transition } => {
            assert_eq!(ball, TwoBallEventBall::A);
            assert_eq!(transition.phase_before, MotionPhase::Rolling);
            assert_eq!(transition.phase_after, MotionPhase::Rest);
            assert_close(transition.time_until_transition.as_f64(), 2.0);
        }
        other => panic!("expected motion transition, got {other:?}"),
    }
}

#[test]
fn the_scheduler_returns_none_when_both_balls_are_resting_and_not_colliding() {
    let a = on_table(BallState::resting_at(inches2(-10.0, 0.0)));
    let b = on_table(BallState::resting_at(inches2(10.0, 0.0)));

    assert!(compute_next_event_for_two_on_table_balls(
        &a,
        &b,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .is_none());
}
