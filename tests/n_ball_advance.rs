use billiards::{
    advance_to_next_n_ball_event_on_table, advance_to_next_n_ball_event_with_rails_on_table,
    AngularVelocity3, BallSetPhysicsSpec, BallState, CollisionModel, Diamond, Inches, Inches2,
    InchesPerSecondSq, MotionPhase, MotionPhaseConfig, MotionTransitionConfig, OnTableBallState,
    OnTableMotionConfig, RadiansPerSecondSq, RailModel, RollingResistanceModel,
    SlidingFrictionModel, SpinDecayModel, TableSpec, Velocity2, TYPICAL_BALL_RADIUS,
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
fn advancing_to_a_motion_transition_advances_all_n_balls_to_that_time() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
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
    let c = on_table(BallState::resting_at(inches2(-20.0, 0.0)));

    let advanced = advance_to_next_n_ball_event_on_table(
        &[a.clone(), b.clone(), c.clone()],
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
    );

    let event = advanced.event.expect("an event should be reported");
    assert_close(event.time().as_f64(), advanced.elapsed.as_f64());
    match event {
        billiards::NBallOnTableEvent::MotionTransition {
            ball_index,
            transition,
        } => {
            assert_eq!(ball_index, 0);
            assert_eq!(transition.phase_before, MotionPhase::Sliding);
            assert_eq!(transition.phase_after, MotionPhase::Rolling);
        }
        other => panic!("expected motion transition, got {other:?}"),
    }

    assert_close(advanced.elapsed.as_f64(), 4.0 / 7.0);
    assert_eq!(advanced.states.len(), 3);
    assert_eq!(
        advanced.states[0]
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Rolling
    );
    assert_eq!(
        advanced.states[1]
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Spinning
    );
    assert_eq!(advanced.states[2], c);
    assert_close(
        advanced.states[0]
            .as_ball_state()
            .angular_velocity
            .x()
            .as_f64(),
        -50.0 / (7.0 * radius),
    );
}

#[test]
fn advancing_to_a_ball_ball_collision_only_resolves_the_participating_pair() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let a = on_table(BallState::on_table(
        inches2(0.0, -(2.0 * radius + 7.5)),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let b = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let c = on_table(BallState::on_table(
        inches2(20.0, 0.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    ));

    let advanced = advance_to_next_n_ball_event_on_table(
        &[a, b.clone(), c.clone()],
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
    );

    let event = advanced.event.expect("an event should be reported");
    match event {
        billiards::NBallOnTableEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } => {
            assert_eq!((first_ball_index, second_ball_index), (0, 1));
            assert_close(collision.time_until_impact.as_f64(), 1.0);
        }
        other => panic!("expected ball-ball collision, got {other:?}"),
    }

    assert_close(advanced.elapsed.as_f64(), 1.0);
    assert_close(advanced.states[0].as_ball_state().speed().as_f64(), 0.0);
    assert_close(
        advanced.states[1].as_ball_state().velocity.y().as_f64(),
        5.0,
    );
    assert_eq!(
        advanced.states[2]
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Spinning
    );
    assert_close(
        advanced.states[2]
            .as_ball_state()
            .angular_velocity
            .z()
            .as_f64(),
        4.0,
    );
    assert_ne!(
        advanced.states[2], c,
        "passive balls should still advance in time"
    );
    assert_eq!(
        advanced.states[1].as_ball_state().angular_velocity,
        b.as_ball_state().angular_velocity
    );
}

#[test]
fn advancing_to_a_rail_impact_only_resolves_the_impacted_ball() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let a = on_table(BallState::on_table(
        inches2(10.0, top_plane - 7.5),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let b = on_table(BallState::on_table(
        inches2(30.0, 0.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    ));

    let advanced = advance_to_next_n_ball_event_with_rails_on_table(
        &[a, b.clone()],
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        RailModel::Mirror,
    );

    let event = advanced.event.expect("an event should be reported");
    match event {
        billiards::NBallOnTableEvent::BallRailImpact { ball_index, impact } => {
            assert_eq!(ball_index, 0);
            assert_eq!(impact.rail, billiards::Rail::Top);
            assert_close(impact.time_until_impact.as_f64(), 1.0);
        }
        other => panic!("expected rail impact, got {other:?}"),
    }

    assert_close(advanced.elapsed.as_f64(), 1.0);
    assert!(advanced.states[0].as_ball_state().velocity.y().as_f64() < 0.0);
    assert_eq!(
        advanced.states[1]
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Spinning
    );
    assert_close(
        advanced.states[1]
            .as_ball_state()
            .angular_velocity
            .z()
            .as_f64(),
        4.0,
    );
    assert_ne!(
        advanced.states[1], b,
        "passive balls should still advance in time"
    );
}

#[test]
fn advancing_with_no_future_event_returns_the_original_n_ball_state() {
    let a = on_table(BallState::resting_at(inches2(-10.0, 0.0)));
    let b = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let c = on_table(BallState::resting_at(inches2(10.0, 0.0)));

    let advanced = advance_to_next_n_ball_event_on_table(
        &[a.clone(), b.clone(), c.clone()],
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
    );

    assert_eq!(advanced.elapsed.as_f64(), 0.0);
    assert!(advanced.event.is_none());
    assert_eq!(advanced.states, vec![a, b, c]);
}
