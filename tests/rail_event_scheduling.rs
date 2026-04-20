use billiards::{
    collide_ball_ball_detailed_on_table, compute_next_ball_rail_impact_on_table,
    compute_next_two_ball_event_with_rails_on_table, AngularVelocity3, BallSetPhysicsSpec,
    BallState, CollisionModel, Diamond, Inches, Inches2, InchesPerSecondSq, MotionPhase,
    MotionPhaseConfig, MotionTransitionConfig, OnTableBallState, OnTableMotionConfig,
    RadiansPerSecondSq, Rail, RollingResistanceModel, SlidingFrictionModel, SpinDecayModel,
    TableSpec, TwoBallEventBall, TwoBallOnTableEvent, Velocity2, TYPICAL_BALL_RADIUS,
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
fn a_rolling_ball_predicts_a_top_rail_impact_before_it_stops() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let state = on_table(BallState::on_table(
        inches2(10.0, top_plane - 7.5),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));

    let impact = compute_next_ball_rail_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("the rolling ball should reach the rail before stopping");

    assert_eq!(impact.rail, Rail::Top);
    assert_close(impact.time_until_impact.as_f64(), 1.0);
    assert_close(
        impact.state_at_impact.as_ball_state().position.y().as_f64(),
        top_plane,
    );
    assert_eq!(
        impact
            .state_at_impact
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Rolling
    );
}

#[test]
fn a_rolling_ball_returns_none_when_it_stops_before_the_rail() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let state = on_table(BallState::on_table(
        inches2(10.0, top_plane - 11.0),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));

    assert!(compute_next_ball_rail_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .is_none());
}

#[test]
fn post_collision_side_spin_can_change_whether_the_cue_ball_reaches_a_rail_during_sliding() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let object_ball = on_table(BallState::resting_at(inches2(4.2, 40.0)));
    let outside_continuation = collide_ball_ball_detailed_on_table(
        &on_table(BallState::on_table(
            inches2(
                4.2 - radius * 2.0_f64.sqrt(),
                40.0 - radius * 2.0_f64.sqrt(),
            ),
            Velocity2::new("0", "10"),
            AngularVelocity3::new(-10.0 / radius, 0.0, -6.0),
        )),
        &object_ball.clone(),
        CollisionModel::ThrowAware,
    )
    .into_cue_ball_continuation();
    let inside_continuation = collide_ball_ball_detailed_on_table(
        &on_table(BallState::on_table(
            inches2(
                4.2 - radius * 2.0_f64.sqrt(),
                40.0 - radius * 2.0_f64.sqrt(),
            ),
            Velocity2::new("0", "10"),
            AngularVelocity3::new(-10.0 / radius, 0.0, 6.0),
        )),
        &object_ball,
        CollisionModel::ThrowAware,
    )
    .into_cue_ball_continuation();

    let outside_impact = outside_continuation.next_rail_impact(
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    );
    let inside_impact = inside_continuation.next_rail_impact(
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    );

    let outside_impact = outside_impact.expect(
        "outside english should still be able to reach the left rail in this staged post-contact case",
    );
    let inside_impact = inside_impact
        .expect("inside english should also still reach the left rail here, but later");
    assert_eq!(outside_impact.rail, Rail::Left);
    assert_eq!(inside_impact.rail, Rail::Left);
    assert_eq!(
        outside_impact
            .state_at_impact
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Sliding
    );
    assert_eq!(
        inside_impact
            .state_at_impact
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Sliding
    );
    assert!(outside_impact.time_until_impact.as_f64() < inside_impact.time_until_impact.as_f64());
}

#[test]
fn follow_and_english_can_change_the_next_rail_aware_event_after_first_contact() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();

    // Like the staged second-ball regression, this approximates a 3-ball pattern by first
    // resolving CB->OB1 contact and then asking the existing rail-aware two-ball scheduler what
    // happens next between the post-impact cue ball and a passive distant ball. The rail-aware
    // decision is motivated by the same local references used by the current post-impact cue-ball
    // model:
    //
    // - `whitepapers/tp_a_4_post_impact_cue_ball_trajectory_for_any_cut_angle_speed_and_spin.pdf`
    //   for the post-impact cue-ball path basis,
    // - `whitepapers/tp_a_8_the_effects_of_english_on_the_30_degree_rule.pdf` for English on the
    //   cue-ball departure, and
    // - `whitepapers/tp_a_24_the_effects_of_follow_and_draw_on_throw_and_ob_swerve.pdf` for the
    //   combined follow/draw + English slip decomposition.
    let object_ball_1 = on_table(BallState::resting_at(inches2(4.2, 40.0)));
    let passive_ball = on_table(BallState::resting_at(inches2(30.0, 30.0)));
    let follow_outside_continuation = collide_ball_ball_detailed_on_table(
        &on_table(BallState::on_table(
            inches2(
                4.2 - radius * 2.0_f64.sqrt(),
                40.0 - radius * 2.0_f64.sqrt(),
            ),
            Velocity2::new("0", "10"),
            AngularVelocity3::new(-6.0, 0.0, -6.0),
        )),
        &object_ball_1.clone(),
        CollisionModel::ThrowAware,
    )
    .into_cue_ball_continuation();
    let follow_inside_continuation = collide_ball_ball_detailed_on_table(
        &on_table(BallState::on_table(
            inches2(
                4.2 - radius * 2.0_f64.sqrt(),
                40.0 - radius * 2.0_f64.sqrt(),
            ),
            Velocity2::new("0", "10"),
            AngularVelocity3::new(-6.0, 0.0, 6.0),
        )),
        &object_ball_1,
        CollisionModel::ThrowAware,
    )
    .into_cue_ball_continuation();

    let outside_event = follow_outside_continuation
        .next_event_against_ball_with_rails(
            &passive_ball,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .expect("outside english should produce a next event");
    let inside_event = follow_inside_continuation
        .next_event_against_ball_with_rails(
            &passive_ball,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .expect("inside english should produce a next event");

    let outside_time = match outside_event {
        TwoBallOnTableEvent::MotionTransition { ball, transition } => {
            assert_eq!(ball, TwoBallEventBall::A);
            assert_eq!(transition.phase_before, MotionPhase::Sliding);
            assert_eq!(transition.phase_after, MotionPhase::Rolling);
            transition.time_until_transition.as_f64()
        }
        other => panic!(
            "expected motion transition under the conservative side-spin model, got {other:?}"
        ),
    };
    let inside_time = match inside_event {
        TwoBallOnTableEvent::MotionTransition { ball, transition } => {
            assert_eq!(ball, TwoBallEventBall::A);
            assert_eq!(transition.phase_before, MotionPhase::Sliding);
            assert_eq!(transition.phase_after, MotionPhase::Rolling);
            transition.time_until_transition.as_f64()
        }
        other => panic!(
            "expected motion transition under the conservative side-spin model, got {other:?}"
        ),
    };
    assert!(
        outside_time < inside_time,
        "outside vs inside english should still change the next-event timing even when neither branch reaches a rail during sliding"
    );
    assert_close(outside_time, 0.24875327809825132);
    assert_close(inside_time, 0.27520658498952827);
}

#[test]
fn the_rail_aware_scheduler_picks_a_rail_impact_before_a_later_motion_transition() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let a = on_table(BallState::on_table(
        inches2(10.0, top_plane - 7.5),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let b = on_table(BallState::resting_at(inches2(30.0, 30.0)));

    let event = compute_next_two_ball_event_with_rails_on_table(
        &a,
        &b,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("an event should be predicted");

    match event {
        TwoBallOnTableEvent::BallRailImpact { ball, impact } => {
            assert_eq!(ball, TwoBallEventBall::A);
            assert_eq!(impact.rail, Rail::Top);
            assert_close(impact.time_until_impact.as_f64(), 1.0);
        }
        other => panic!("expected ball-rail impact, got {other:?}"),
    }
}

#[test]
fn the_rail_aware_scheduler_still_prefers_motion_transition_when_the_rail_is_not_reachable() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let a = on_table(BallState::on_table(
        inches2(10.0, top_plane - 11.0),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let b = on_table(BallState::resting_at(inches2(30.0, 30.0)));

    let event = compute_next_two_ball_event_with_rails_on_table(
        &a,
        &b,
        &BallSetPhysicsSpec::default(),
        &table,
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
