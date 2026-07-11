use billiards::{
    collide_ball_ball_detailed_on_table, compute_next_event_for_two_on_table_balls,
    AngularVelocity3, BallSetPhysicsSpec, BallState, CollisionModel, Inches, Inches2,
    InchesPerSecondSq, MotionPhase, MotionPhaseConfig, MotionTransitionConfig, OnTableBallState,
    OnTableMotionConfig, RadiansPerSecondSq, RollingResistanceModel, SlidingFrictionModel,
    SpinDecayModel, TwoBallEventBall, TwoBallOnTableEvent, Velocity2, TYPICAL_BALL_RADIUS,
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
    .expect("test geometry should validate")
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
    .expect("test geometry should validate")
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
    .expect("test geometry should validate")
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
    .expect("test geometry should validate")
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
fn the_scheduler_picks_stop_when_a_rolling_ball_reaches_contact_with_zero_speed() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let a = on_table(BallState::on_table(
        inches2(0.0, -(2.0 * radius + 10.0)),
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
    .expect("test geometry should validate")
    .expect("the rolling ball should at least stop");

    match event {
        TwoBallOnTableEvent::MotionTransition { ball, transition } => {
            assert_eq!(ball, TwoBallEventBall::A);
            assert_eq!(transition.phase_before, MotionPhase::Rolling);
            assert_eq!(transition.phase_after, MotionPhase::Rest);
            assert_close(transition.time_until_transition.as_f64(), 2.0);
        }
        other => panic!("expected zero-speed contact to resolve as a stop, got {other:?}"),
    }
}

#[test]
fn a_post_contact_continuation_exposes_full_branches_and_rejects_airborne_scheduling() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let object_ball_1 = on_table(BallState::resting_at(inches2(7.2, 40.0)));
    let object_ball_2 = on_table(BallState::resting_at(inches2(4.0, 36.8)));
    let follow_outside = on_table(BallState::on_table(
        inches2(
            7.2 - radius * 2.0_f64.sqrt(),
            40.0 - radius * 2.0_f64.sqrt(),
        ),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-6.0, 0.0, -6.0),
    ));
    let outcome = collide_ball_ball_detailed_on_table(
        &follow_outside,
        &object_ball_1,
        CollisionModel::ThrowAware,
    );
    let continuation = outcome.cue_ball_continuation();

    assert_eq!(continuation.source_contact(), &outcome);
    assert_eq!(continuation.cue_ball(), &outcome.a_after);
    assert_eq!(continuation.struck_ball(), &outcome.b_after);
    assert_eq!(
        continuation.clone().into_source_contact(),
        outcome,
        "continuations should round-trip their source collision outcome"
    );
    assert!(matches!(
        continuation.next_collision_against_ball(
            &object_ball_2,
            &BallSetPhysicsSpec::default(),
            &motion_config(),
        ),
        Err(billiards::OnTableStateError::VerticalVelocityPresent { .. })
    ));
    assert!(matches!(
        continuation.next_event_against_ball(
            &object_ball_2,
            &BallSetPhysicsSpec::default(),
            &motion_config(),
        ),
        Err(billiards::PostContactContinuationError::NotOnTable(
            billiards::OnTableStateError::VerticalVelocityPresent { .. }
        ))
    ));
}

#[test]
fn a_post_contact_continuation_can_follow_the_struck_ball_into_a_combo() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-2.0 * radius, 0.0),
        Velocity2::new("10", "0"),
        AngularVelocity3::zero(),
    ));
    let first_object = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let second_object = on_table(BallState::resting_at(inches2(2.0 * radius + 4.0, 0.0)));
    let continuation =
        collide_ball_ball_detailed_on_table(&cue_ball, &first_object, CollisionModel::Ideal)
            .into_cue_ball_continuation();

    assert!(
        continuation
            .next_collision_against_ball(
                &second_object,
                &BallSetPhysicsSpec::default(),
                &motion_config(),
            )
            .expect("ideal continuation must remain on the table")
            .is_none(),
        "the cue ball should stop after the opening ideal head-on hit"
    );

    let collision = continuation
        .next_collision_from_struck_ball_against_ball(
            &second_object,
            &BallSetPhysicsSpec::default(),
            &motion_config(),
        )
        .expect("ideal continuation must remain on the table")
        .expect("the struck ball should continue into the second object ball");
    assert_close(
        collision.time_until_impact.as_f64(),
        0.450_806_661_517_033_2,
    );

    match continuation
        .next_event_from_struck_ball_against_ball(
            &second_object,
            &BallSetPhysicsSpec::default(),
            &motion_config(),
        )
        .expect("test geometry should validate")
        .expect("the struck ball should produce the next combo event")
    {
        TwoBallOnTableEvent::BallBallCollision(collision) => {
            assert_close(
                collision.time_until_impact.as_f64(),
                0.450_806_661_517_033_2,
            );
        }
        other => panic!("expected struck-ball combo collision, got {other:?}"),
    }
}

#[test]
fn follow_and_english_require_richer_scheduler_after_first_contact() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let object_ball_1 = on_table(BallState::resting_at(inches2(7.2, 40.0)));
    let object_ball_2 = on_table(BallState::resting_at(inches2(4.0, 36.8)));
    let follow_outside = on_table(BallState::on_table(
        inches2(
            7.2 - radius * 2.0_f64.sqrt(),
            40.0 - radius * 2.0_f64.sqrt(),
        ),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-6.0, 0.0, -6.0),
    ));
    let follow_inside = on_table(BallState::on_table(
        inches2(
            7.2 - radius * 2.0_f64.sqrt(),
            40.0 - radius * 2.0_f64.sqrt(),
        ),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-6.0, 0.0, 6.0),
    ));

    for continuation in [
        collide_ball_ball_detailed_on_table(
            &follow_outside,
            &object_ball_1,
            CollisionModel::ThrowAware,
        )
        .into_cue_ball_continuation(),
        collide_ball_ball_detailed_on_table(
            &follow_inside,
            &object_ball_1,
            CollisionModel::ThrowAware,
        )
        .into_cue_ball_continuation(),
    ] {
        assert!(matches!(
            continuation.next_event_against_ball(
                &object_ball_2,
                &BallSetPhysicsSpec::default(),
                &motion_config(),
            ),
            Err(billiards::PostContactContinuationError::NotOnTable(
                billiards::OnTableStateError::VerticalVelocityPresent { .. }
            ))
        ));
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
    .expect("test geometry should validate")
    .is_none());
}
