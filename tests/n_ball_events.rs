use billiards::{
    advance_to_next_n_ball_event_on_table, compute_next_n_ball_event_on_table,
    compute_next_n_ball_event_with_rails_on_table, AngularVelocity3, BallSetPhysicsSpec, BallState,
    CollisionModel, Diamond, Inches, Inches2, InchesPerSecondSq, MotionPhase, MotionPhaseConfig,
    MotionTransitionConfig, NBallOnTableEvent, OnTableBallState, OnTableMotionConfig,
    RadiansPerSecondSq, Rail, RollingResistanceModel, SlidingFrictionModel, SpinDecayModel,
    TableSpec, Velocity2, TYPICAL_BALL_RADIUS,
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
fn the_n_ball_scheduler_picks_the_earliest_ball_ball_collision_across_pairs() {
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

    let event = compute_next_n_ball_event_on_table(
        &[&a, &b, &c],
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .expect("an event should be predicted");

    match event {
        NBallOnTableEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } => {
            assert_eq!(first_ball_index, 0);
            assert_eq!(second_ball_index, 1);
            assert_close(collision.time_until_impact.as_f64(), 1.0);
        }
        other => panic!("expected ball-ball collision, got {other:?}"),
    }
}

#[test]
fn the_n_ball_scheduler_picks_the_earliest_motion_transition_across_balls() {
    let a = on_table(BallState::on_table(
        inches2(-20.0, 0.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    ));
    let b = on_table(BallState::on_table(
        inches2(0.0, 0.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 1.0),
    ));
    let c = on_table(BallState::resting_at(inches2(20.0, 0.0)));

    let event = compute_next_n_ball_event_on_table(
        &[&a, &b, &c],
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .expect("an event should be predicted");

    match event {
        NBallOnTableEvent::MotionTransition {
            ball_index,
            transition,
        } => {
            assert_eq!(ball_index, 1);
            assert_eq!(transition.phase_before, MotionPhase::Spinning);
            assert_eq!(transition.phase_after, MotionPhase::Rest);
            assert_close(transition.time_until_transition.as_f64(), 0.5);
        }
        other => panic!("expected motion transition, got {other:?}"),
    }
}

#[test]
fn the_rail_aware_n_ball_scheduler_picks_the_earliest_rail_impact() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let a = on_table(BallState::on_table(
        inches2(10.0, top_plane - 7.5),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let b = on_table(BallState::on_table(
        inches2(30.0, -(2.0 * radius + 12.5)),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let c = on_table(BallState::resting_at(inches2(30.0, 0.0)));

    let event = compute_next_n_ball_event_with_rails_on_table(
        &[&a, &b, &c],
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("an event should be predicted");

    match event {
        NBallOnTableEvent::BallRailImpact { ball_index, impact } => {
            assert_eq!(ball_index, 0);
            assert_eq!(impact.rail, Rail::Top);
            assert_close(impact.time_until_impact.as_f64(), 1.0);
        }
        other => panic!("expected ball-rail impact, got {other:?}"),
    }
}

#[test]
fn simultaneous_pair_collisions_break_ties_by_lowest_index_pair() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let a = on_table(BallState::on_table(
        inches2(0.0, -(2.0 * radius + 7.5)),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let b = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let c = on_table(BallState::on_table(
        inches2(20.0, -(2.0 * radius + 7.5)),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let d = on_table(BallState::resting_at(inches2(20.0, 0.0)));

    let event = compute_next_n_ball_event_on_table(
        &[&a, &b, &c, &d],
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .expect("an event should be predicted");

    match event {
        NBallOnTableEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } => {
            assert_eq!((first_ball_index, second_ball_index), (0, 1));
            assert_close(collision.time_until_impact.as_f64(), 1.0);
        }
        other => panic!("expected ball-ball collision, got {other:?}"),
    }
}

#[test]
fn shared_simultaneous_ball_ball_contacts_are_reported_as_unsupported() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let contact_y = -3.0_f64.sqrt() * radius;
    let cue_ball = on_table(BallState::on_table(
        inches2(0.0, contact_y - 7.5),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let left_object = on_table(BallState::resting_at(inches2(-radius, 0.0)));
    let right_object = on_table(BallState::resting_at(inches2(radius, 0.0)));

    let event = compute_next_n_ball_event_on_table(
        &[&cue_ball, &left_object, &right_object],
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .expect("the symmetric double hit should be detected");

    match event {
        NBallOnTableEvent::UnsupportedSharedBallBallContact {
            time_until_contact,
            ball_indices,
            ball_ball_pairs,
        } => {
            assert_close(time_until_contact.as_f64(), 1.0);
            assert_eq!(ball_indices, vec![0, 1, 2]);
            assert_eq!(ball_ball_pairs, vec![(0, 1), (0, 2)]);
        }
        other => panic!("expected unsupported shared contact, got {other:?}"),
    }
}

#[test]
fn touching_follow_on_collision_is_scheduled_immediately_after_a_frozen_cluster_tie_break() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let a = on_table(BallState::on_table(
        inches2(-(2.0 * radius + 7.5), 0.0),
        Velocity2::new("10", "0"),
        AngularVelocity3::new(0.0, 10.0 / radius, 0.0),
    ));
    let b = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let c = on_table(BallState::resting_at(inches2(2.0 * radius, 0.0)));

    let advanced = advance_to_next_n_ball_event_on_table(
        &[a, b, c],
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
    );

    match advanced
        .event
        .as_ref()
        .expect("first cluster event should be predicted")
    {
        NBallOnTableEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } => {
            assert_eq!((*first_ball_index, *second_ball_index), (0, 1));
            assert_close(collision.time_until_impact.as_f64(), 1.0);
        }
        other => panic!("expected opening ball-ball collision, got {other:?}"),
    }

    let next_state_refs = advanced.states.iter().collect::<Vec<_>>();
    let next = compute_next_n_ball_event_on_table(
        &next_state_refs,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .expect("frozen neighbor contact should be preserved as an immediate follow-on collision");

    match next {
        NBallOnTableEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } => {
            assert_eq!((first_ball_index, second_ball_index), (1, 2));
            assert_close(collision.time_until_impact.as_f64(), 0.0);
        }
        other => panic!("expected immediate follow-on ball-ball collision, got {other:?}"),
    }
}

#[test]
fn the_n_ball_scheduler_returns_none_when_all_balls_are_resting_and_separated() {
    let a = on_table(BallState::resting_at(inches2(-20.0, 0.0)));
    let b = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let c = on_table(BallState::resting_at(inches2(20.0, 0.0)));

    assert!(compute_next_n_ball_event_on_table(
        &[&a, &b, &c],
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .is_none());
}
