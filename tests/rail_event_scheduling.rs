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

fn rolling_ball_with_side_spin(x: f64, y: f64, vertical_spin: f64) -> OnTableBallState {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    on_table(BallState::on_table(
        inches2(x, y),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, vertical_spin),
    ))
}

#[test]
fn curved_rolling_ball_reaches_the_right_rail_before_its_transition() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let right_plane = table.diamond_to_inches(Diamond::four()).as_f64() - radius;
    let state = rolling_ball_with_side_spin(48.871, 20.0, 2.0);

    let impact = compute_next_ball_rail_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("the canonical rightward curve reaches the rail before one second");

    assert_eq!(impact.rail, Rail::Right);
    assert!(impact.time_until_impact.as_f64() > 0.0 && impact.time_until_impact.as_f64() < 1.0);
    assert_close(
        impact.state_at_impact.as_ball_state().position.x().as_f64(),
        right_plane,
    );
    assert!(
        impact.state_at_impact.as_ball_state().velocity.x().as_f64() > 0.0,
        "right-rail contact must be entering"
    );
}

#[test]
fn opposite_spin_curve_away_does_not_create_a_right_rail_impact() {
    let state = rolling_ball_with_side_spin(48.871, 20.0, -2.0);

    assert!(compute_next_ball_rail_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &TableSpec::default(),
        &motion_config(),
    )
    .is_none());
}

#[test]
fn a_ball_already_touching_a_rail_and_moving_into_it_predicts_an_immediate_impact() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let state = on_table(BallState::on_table(
        inches2(10.0, top_plane),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));

    let impact = compute_next_ball_rail_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect(
        "a frozen-to-rail incoming ball should rebound immediately instead of tunneling through the cushion",
    );

    assert_eq!(impact.rail, Rail::Top);
    assert_close(impact.time_until_impact.as_f64(), 0.0);
    assert_close(
        impact.state_at_impact.as_ball_state().position.y().as_f64(),
        top_plane,
    );
}

#[test]
fn a_ball_touching_a_rail_and_accelerating_into_it_predicts_an_immediate_impact() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let state = on_table(BallState::on_table(
        inches2(10.0, top_plane),
        Velocity2::new("0", "0"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));

    let impact = compute_next_ball_rail_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect(
        "a frozen-to-rail ball whose sliding friction accelerates it into the cushion should rebound immediately",
    );

    assert_eq!(impact.rail, Rail::Top);
    assert_close(impact.time_until_impact.as_f64(), 0.0);
    assert_close(
        impact.state_at_impact.as_ball_state().position.y().as_f64(),
        top_plane,
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
fn post_collision_follow_requires_richer_rail_scheduling() {
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
        &object_ball,
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

    for continuation in [outside_continuation, inside_continuation] {
        assert!(matches!(
            continuation
                .next_rail_impact(&BallSetPhysicsSpec::default(), &table, &motion_config(),),
            Err(billiards::OnTableStateError::VerticalVelocityPresent { .. })
        ));
    }
}

#[test]
fn follow_and_english_require_richer_rail_aware_scheduling_after_contact() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let object_ball_1 = on_table(BallState::resting_at(inches2(4.2, 40.0)));
    let passive_ball = on_table(BallState::resting_at(inches2(30.0, 30.0)));
    let follow_outside = on_table(BallState::on_table(
        inches2(
            4.2 - radius * 2.0_f64.sqrt(),
            40.0 - radius * 2.0_f64.sqrt(),
        ),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-6.0, 0.0, -6.0),
    ));
    let follow_inside = on_table(BallState::on_table(
        inches2(
            4.2 - radius * 2.0_f64.sqrt(),
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
            continuation.next_event_against_ball_with_rails(
                &passive_ball,
                &BallSetPhysicsSpec::default(),
                &table,
                &motion_config(),
            ),
            Err(billiards::PostContactContinuationError::NotOnTable(
                billiards::OnTableStateError::VerticalVelocityPresent { .. }
            ))
        ));
    }
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
    .expect("test geometry should validate")
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
