use billiards::{
    advance_motion_on_table, advance_to_next_two_ball_event_with_rails_on_table,
    collide_ball_rail_on_table_with_radius, compute_next_two_ball_event_with_rails_on_table,
    simulate_two_balls_with_rails_on_table, AngularVelocity3, BallSetPhysicsSpec, BallState,
    CollisionModel, Diamond, Inches, Inches2, InchesPerSecondSq, MotionPhase, MotionPhaseConfig,
    MotionTransitionConfig, OnTableBallState, OnTableMotionConfig, RadiansPerSecondSq, Rail,
    RailModel, RollingResistanceModel, Seconds, SlidingFrictionModel, SpinDecayModel, TableSpec,
    TwoBallEventBall, TwoBallOnTableEvent, Velocity2, TYPICAL_BALL_RADIUS,
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
fn advancing_to_a_ball_a_rail_impact_reflects_that_ball_and_advances_ball_b_too() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let a = on_table(BallState::on_table(
        inches2(10.0, top_plane - 7.5),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let b = on_table(BallState::on_table(
        inches2(30.0, 20.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    ));

    let advanced = advance_to_next_two_ball_event_with_rails_on_table(
        &a,
        &b,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        RailModel::Mirror,
    );
    match advanced
        .event
        .as_ref()
        .expect("an event should be reported")
    {
        TwoBallOnTableEvent::BallRailImpact { ball, impact } => {
            assert_eq!(*ball, TwoBallEventBall::A);
            assert_eq!(impact.rail, Rail::Top);
            assert_close(impact.time_until_impact.as_f64(), 1.0);
        }
        other => panic!("expected ball-rail impact, got {other:?}"),
    }

    assert_close(advanced.elapsed.as_f64(), 1.0);
    assert_close(advanced.a.as_ball_state().position.x().as_f64(), 10.0);
    assert_close(advanced.a.as_ball_state().position.y().as_f64(), top_plane);
    assert_close(advanced.a.as_ball_state().velocity.x().as_f64(), 0.0);
    assert_close(advanced.a.as_ball_state().velocity.y().as_f64(), -5.0);
    assert_close(
        advanced.a.as_ball_state().angular_velocity.x().as_f64(),
        -5.0 / radius,
    );
    let phase = advanced
        .a
        .as_ball_state()
        .motion_phase(TYPICAL_BALL_RADIUS.clone());
    assert_eq!(phase, MotionPhase::Sliding);

    assert_close(advanced.b.as_ball_state().position.x().as_f64(), 30.0);
    assert_close(advanced.b.as_ball_state().position.y().as_f64(), 20.0);
    assert_close(advanced.b.as_ball_state().speed().as_f64(), 0.0);
    assert_close(
        advanced.b.as_ball_state().angular_velocity.z().as_f64(),
        4.0,
    );
}

#[test]
fn advancing_to_a_ball_b_rail_impact_reflects_that_ball_and_advances_ball_a_too() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let right_plane = table.diamond_to_inches(Diamond::four()).as_f64() - radius;
    let a = on_table(BallState::on_table(
        inches2(5.0, 15.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    ));
    let b = on_table(BallState::on_table(
        inches2(right_plane - 7.5, 20.0),
        Velocity2::new("10", "0"),
        AngularVelocity3::new(0.0, 10.0 / radius, 0.0),
    ));

    let advanced = advance_to_next_two_ball_event_with_rails_on_table(
        &a,
        &b,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        RailModel::Mirror,
    );

    match advanced
        .event
        .as_ref()
        .expect("an event should be reported")
    {
        TwoBallOnTableEvent::BallRailImpact { ball, impact } => {
            assert_eq!(*ball, TwoBallEventBall::B);
            assert_eq!(impact.rail, Rail::Right);
            assert_close(impact.time_until_impact.as_f64(), 1.0);
        }
        other => panic!("expected ball-rail impact, got {other:?}"),
    }

    assert_close(advanced.elapsed.as_f64(), 1.0);
    assert_close(advanced.a.as_ball_state().position.x().as_f64(), 5.0);
    assert_close(advanced.a.as_ball_state().position.y().as_f64(), 15.0);
    assert_close(
        advanced.a.as_ball_state().angular_velocity.z().as_f64(),
        4.0,
    );

    assert_close(
        advanced.b.as_ball_state().position.x().as_f64(),
        right_plane,
    );
    assert_close(advanced.b.as_ball_state().position.y().as_f64(), 20.0);
    assert_close(advanced.b.as_ball_state().velocity.x().as_f64(), -5.0);
    assert_close(advanced.b.as_ball_state().velocity.y().as_f64(), 0.0);
    assert_close(
        advanced.b.as_ball_state().angular_velocity.y().as_f64(),
        5.0 / radius,
    );
    assert_eq!(
        advanced
            .b
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Sliding
    );
}

#[test]
fn advancing_to_a_spin_aware_rail_impact_uses_the_spin_aware_rebound_model() {
    let table = TableSpec::default();
    let ball = BallSetPhysicsSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let a = on_table(BallState::on_table(
        inches2(10.0, top_plane - 7.5),
        Velocity2::new("5", "10"),
        AngularVelocity3::new(-10.0 / radius, 5.0 / radius, 0.0),
    ));
    let b = on_table(BallState::resting_at(inches2(30.0, 20.0)));
    let event =
        compute_next_two_ball_event_with_rails_on_table(&a, &b, &ball, &table, &motion_config())
            .expect("an event should be predicted");
    let expected_a = match &event {
        TwoBallOnTableEvent::BallRailImpact {
            ball: TwoBallEventBall::A,
            impact,
        } => collide_ball_rail_on_table_with_radius(
            &impact.state_at_impact,
            impact.rail,
            ball.radius.clone(),
            RailModel::SpinAware,
        ),
        other => panic!("expected ball A rail impact, got {other:?}"),
    };

    let advanced = advance_to_next_two_ball_event_with_rails_on_table(
        &a,
        &b,
        &ball,
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        RailModel::SpinAware,
    );

    assert_eq!(advanced.event, Some(event));
    assert_eq!(advanced.a, expected_a);
}

#[test]
fn advancing_with_rails_and_no_future_event_returns_the_original_two_ball_state() {
    let table = TableSpec::default();
    let a = on_table(BallState::resting_at(inches2(-10.0, 0.0)));
    let b = on_table(BallState::resting_at(inches2(10.0, 0.0)));

    let advanced = advance_to_next_two_ball_event_with_rails_on_table(
        &a,
        &b,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        RailModel::Mirror,
    );

    assert_eq!(advanced.elapsed.as_f64(), 0.0);
    assert!(advanced.event.is_none());
    assert_eq!(advanced.a, a);
    assert_eq!(advanced.b, b);
}

#[test]
fn simulating_with_rails_records_a_rail_impact_and_consumes_remaining_time_afterward() {
    let table = TableSpec::default();
    let config = motion_config();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let dt = Seconds::new(1.25);
    let a = on_table(BallState::on_table(
        inches2(10.0, top_plane - 7.5),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let b = on_table(BallState::on_table(
        inches2(30.0, 20.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    ));

    let simulated = simulate_two_balls_with_rails_on_table(
        &a,
        &b,
        dt,
        &BallSetPhysicsSpec::default(),
        &table,
        &config,
        CollisionModel::Ideal,
        RailModel::Mirror,
    );
    let first = advance_to_next_two_ball_event_with_rails_on_table(
        &a,
        &b,
        &BallSetPhysicsSpec::default(),
        &table,
        &config,
        CollisionModel::Ideal,
        RailModel::Mirror,
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
        TwoBallOnTableEvent::BallRailImpact { ball, impact } => {
            assert_eq!(*ball, TwoBallEventBall::A);
            assert_eq!(impact.rail, Rail::Top);
        }
        other => panic!("expected ball-rail impact, got {other:?}"),
    }
    assert_eq!(simulated.a, expected_a);
    assert_eq!(simulated.b, expected_b);
}
