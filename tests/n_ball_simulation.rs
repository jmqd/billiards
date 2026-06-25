use billiards::{
    simulate_n_balls_on_table_until_rest, simulate_n_balls_with_physics_on_table_until_rest,
    simulate_n_balls_with_rails_on_table_until_rest, AngularVelocity3, BallBallCollisionConfig,
    BallSetPhysicsSpec, BallState, CollisionModel, Diamond, Inches, Inches2, InchesPerSecond,
    InchesPerSecondSq, MotionPhase, MotionPhaseConfig, MotionTransitionConfig, NBallOnTableEvent,
    OnTableBallState, OnTableMotionConfig, RadiansPerSecondSq, Rail, RailModel,
    RollingResistanceModel, Scale, SlidingFrictionModel, SpinDecayModel, TableSpec, Velocity2,
    STANDARD_GRAVITY_INCHES_PER_SECOND_SQUARED, TYPICAL_BALL_RADIUS,
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

fn tp_b5_motion_config() -> OnTableMotionConfig {
    MotionTransitionConfig {
        phase: MotionPhaseConfig::default(),
        sliding_friction: SlidingFrictionModel::ConstantAcceleration {
            acceleration_magnitude: InchesPerSecondSq::new(Inches::from_f64(
                0.20 * STANDARD_GRAVITY_INCHES_PER_SECOND_SQUARED,
            )),
        },
        spin_decay: SpinDecayModel::ConstantAngularDeceleration {
            angular_deceleration: RadiansPerSecondSq::new(10.0),
        },
        rolling_resistance: RollingResistanceModel::ConstantDeceleration {
            linear_deceleration: InchesPerSecondSq::new(Inches::from_f64(
                0.01 * STANDARD_GRAVITY_INCHES_PER_SECOND_SQUARED,
            )),
        },
    }
}

fn on_table(state: BallState) -> OnTableBallState {
    OnTableBallState::try_from(state).expect("test states should validate as on-table")
}

fn inches2(x: f64, y: f64) -> Inches2 {
    Inches2::new(Inches::from_f64(x), Inches::from_f64(y))
}

fn distance_between(a: &Inches2, b: &Inches2) -> f64 {
    let dx = b.x().as_f64() - a.x().as_f64();
    let dy = b.y().as_f64() - a.y().as_f64();
    dx.hypot(dy)
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
fn tp_b5_rolling_direct_hit_travel_distance_ratio_matches_published_anchor() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_start = inches2(0.0, -2.0 * radius);
    let object_start = inches2(0.0, 0.0);

    for impact_speed in [
        InchesPerSecond::from_mph(3.0),
        InchesPerSecond::from_mph(7.0),
    ] {
        let speed = impact_speed.as_f64();
        let cue = on_table(BallState::on_table(
            cue_start.clone(),
            Velocity2::new(Inches::zero(), Inches::from_f64(speed)),
            AngularVelocity3::new(-speed / radius, 0.0, 0.0),
        ));
        let object = on_table(BallState::resting_at(object_start.clone()));
        let collision_config =
            BallBallCollisionConfig::new(Scale::from_f64(0.94), Scale::from_f64(0.06));

        let simulated = simulate_n_balls_with_physics_on_table_until_rest(
            &[cue, object],
            &BallSetPhysicsSpec::default(),
            &tp_b5_motion_config(),
            CollisionModel::ThrowAware,
            &collision_config,
        );

        let cue_distance =
            distance_between(&cue_start, &simulated.states[0].as_ball_state().position);
        let object_distance =
            distance_between(&object_start, &simulated.states[1].as_ball_state().position);
        let ratio = object_distance / cue_distance;

        assert!(
            (ratio - 6.08).abs() < 0.08,
            "TP B.5 predicts OB/CB travel ratio of about 6.08 after a rolling direct hit; got {ratio} at {} mph",
            impact_speed.as_mph()
        );
    }
}

#[test]
fn simulating_a_frozen_three_ball_chain_uses_the_tp_b29_coupled_contact_split() {
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
        !simulated.events.is_empty(),
        "expected the opening cluster collision to be recorded"
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
    for event in simulated.events.iter().skip(1) {
        if let NBallOnTableEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } = event
        {
            assert_ne!(
                (
                    *first_ball_index,
                    *second_ball_index,
                    collision.time_until_impact.as_f64()
                ),
                (1, 2, 0.0),
                "TP B.29 coupled resolution should not be followed by a synthetic immediate frozen-neighbor pair collision"
            );
        }
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
