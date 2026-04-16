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
fn follow_and_english_can_change_whether_the_scheduler_reaches_a_second_ball_after_contact() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();

    // This staged regression approximates a 3-ball pattern by first resolving CB->OB1 contact,
    // then asking the existing 2-ball scheduler what happens next between the post-impact cue ball
    // and OB2. The local references motivating this are:
    //
    // - `whitepapers/tp_a_4_post_impact_cue_ball_trajectory_for_any_cut_angle_speed_and_spin.pdf`
    //   for the post-impact cue-ball path basis,
    // - `whitepapers/tp_a_8_the_effects_of_english_on_the_30_degree_rule.pdf` for English on the
    //   cue-ball departure, and
    // - `whitepapers/tp_a_24_the_effects_of_follow_and_draw_on_throw_and_ob_swerve.pdf` for the
    //   combined follow/draw + English slip decomposition used by the current cue-ball seed model.
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
    let follow_outside_after = collide_ball_ball_detailed_on_table(
        &follow_outside,
        &object_ball_1,
        CollisionModel::ThrowAware,
    )
    .a_after;
    let follow_inside_after = collide_ball_ball_detailed_on_table(
        &follow_inside,
        &object_ball_1,
        CollisionModel::ThrowAware,
    )
    .a_after;

    let outside_event = compute_next_event_for_two_on_table_balls(
        &follow_outside_after,
        &object_ball_2,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .expect("outside english should produce a next event");
    let inside_event = compute_next_event_for_two_on_table_balls(
        &follow_inside_after,
        &object_ball_2,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .expect("inside english should produce a next event");

    match outside_event {
        TwoBallOnTableEvent::BallBallCollision(collision) => {
            assert!(collision.time_until_impact.as_f64() < 0.25);
            assert_eq!(
                collision
                    .a_at_impact
                    .as_ball_state()
                    .motion_phase(TYPICAL_BALL_RADIUS.clone()),
                MotionPhase::Sliding
            );
        }
        other => panic!("expected second-ball collision, got {other:?}"),
    }
    match inside_event {
        TwoBallOnTableEvent::MotionTransition { ball, transition } => {
            assert_eq!(ball, TwoBallEventBall::A);
            assert_eq!(transition.phase_before, MotionPhase::Sliding);
            assert_eq!(transition.phase_after, MotionPhase::Rolling);
            assert_close(
                transition.time_until_transition.as_f64(),
                0.27520658498952827,
            );
        }
        other => panic!("expected motion transition before second-ball contact, got {other:?}"),
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
