use billiards::{
    simulate_n_balls_on_table_until_rest, simulate_n_balls_with_rails_on_table_until_rest,
    AngularVelocity3, BallSetPhysicsSpec, BallState, CollisionModel, Diamond, Inches, Inches2,
    InchesPerSecondSq, MotionPhase, MotionPhaseConfig, MotionTransitionConfig, NBallOnTableEvent,
    OnTableBallState, OnTableMotionConfig, RadiansPerSecondSq, Rail, RailModel,
    RollingResistanceModel, SlidingFrictionModel, SpinDecayModel, TableSpec, Velocity2,
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
fn simulating_n_balls_until_rest_with_no_motion_returns_immediately() {
    let a = on_table(BallState::resting_at(inches2(-10.0, 0.0)));
    let b = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let c = on_table(BallState::resting_at(inches2(10.0, 0.0)));

    let simulated = simulate_n_balls_on_table_until_rest(
        &[a.clone(), b.clone(), c.clone()],
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
    );

    assert_eq!(simulated.elapsed.as_f64(), 0.0);
    assert!(simulated.events.is_empty());
    assert_eq!(simulated.states, vec![a, b, c]);
}

#[test]
fn simulating_n_balls_until_rest_records_collision_and_transition_events_until_everything_stops() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let a = on_table(BallState::on_table(
        inches2(0.0, -(2.0 * radius + 3.75)),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    ));
    let b = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let c = on_table(BallState::on_table(
        inches2(20.0, 0.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    ));

    let simulated = simulate_n_balls_on_table_until_rest(
        &[a, b, c],
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
    );

    assert!(
        !simulated.events.is_empty(),
        "expected at least one event before the system comes to rest"
    );
    match &simulated.events[0] {
        NBallOnTableEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            ..
        } => assert_eq!((*first_ball_index, *second_ball_index), (0, 1)),
        other => panic!("expected opening collision, got {other:?}"),
    }
    assert!(simulated
        .events
        .iter()
        .any(|event| matches!(event, NBallOnTableEvent::MotionTransition { .. })));
    for state in &simulated.states {
        assert_eq!(
            state
                .as_ball_state()
                .motion_phase(TYPICAL_BALL_RADIUS.clone()),
            MotionPhase::Rest
        );
    }
}

#[test]
fn simulating_a_frozen_three_ball_chain_records_the_zero_time_follow_on_collision() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let a = on_table(BallState::on_table(
        inches2(-(2.0 * radius + 7.5), 0.0),
        Velocity2::new("10", "0"),
        AngularVelocity3::new(0.0, 10.0 / radius, 0.0),
    ));
    let b = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let c = on_table(BallState::resting_at(inches2(2.0 * radius, 0.0)));

    let simulated = simulate_n_balls_on_table_until_rest(
        &[a, b, c],
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
    );

    assert!(
        simulated.events.len() >= 2,
        "expected both cluster collisions to be recorded"
    );
    match &simulated.events[0] {
        NBallOnTableEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } => {
            assert_eq!((*first_ball_index, *second_ball_index), (0, 1));
            assert_close(collision.time_until_impact.as_f64(), 1.0);
        }
        other => panic!("expected opening cluster collision, got {other:?}"),
    }
    match &simulated.events[1] {
        NBallOnTableEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } => {
            assert_eq!((*first_ball_index, *second_ball_index), (1, 2));
            assert_close(collision.time_until_impact.as_f64(), 0.0);
        }
        other => panic!("expected immediate follow-on collision, got {other:?}"),
    }
    assert!(
        simulated.states[2].as_ball_state().position.x().as_f64() > 2.0 * radius,
        "the third ball should inherit the chain's forward motion"
    );
    for state in &simulated.states {
        assert_eq!(
            state
                .as_ball_state()
                .motion_phase(TYPICAL_BALL_RADIUS.clone()),
            MotionPhase::Rest
        );
    }
}

#[test]
fn simulating_n_balls_with_rails_until_rest_records_rail_impacts_and_ends_at_rest() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let a = on_table(BallState::on_table(
        inches2(10.0, top_plane - 7.5),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let b = on_table(BallState::resting_at(inches2(30.0, 30.0)));

    let simulated = simulate_n_balls_with_rails_on_table_until_rest(
        &[a, b],
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        RailModel::Mirror,
    );

    assert!(simulated.events.iter().any(|event| matches!(
        event,
        NBallOnTableEvent::BallRailImpact {
            ball_index: 0,
            impact,
        } if impact.rail == Rail::Top
    )));
    for state in &simulated.states {
        assert_eq!(
            state
                .as_ball_state()
                .motion_phase(TYPICAL_BALL_RADIUS.clone()),
            MotionPhase::Rest
        );
    }
}
