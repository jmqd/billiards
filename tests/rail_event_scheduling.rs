use billiards::{
    compute_next_ball_rail_impact_on_table, compute_next_two_ball_event_with_rails_on_table,
    AngularVelocity3, BallSetPhysicsSpec, BallState, Diamond, Inches, Inches2, InchesPerSecondSq,
    MotionPhase, MotionPhaseConfig, MotionTransitionConfig, OnTableBallState, OnTableMotionConfig,
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
