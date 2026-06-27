use billiards::{
    advance_to_next_n_ball_event_on_table, advance_to_next_n_ball_event_with_physics_on_table,
    advance_to_next_n_ball_event_with_rails_on_table,
    collide_ball_ball_on_table_with_radius_and_config, AngularVelocity3, BallBallCollisionConfig,
    BallSetPhysicsSpec, BallState, CollisionModel, Diamond, Inches, Inches2, InchesPerSecondSq,
    MotionPhase, MotionPhaseConfig, MotionTransitionConfig, OnTableBallState, OnTableMotionConfig,
    RadiansPerSecondSq, RailModel, RollingResistanceModel, SlidingFrictionModel, SpinDecayModel,
    TableSpec, Velocity2, TYPICAL_BALL_RADIUS,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

fn assert_near(actual: f64, expected: f64, tolerance: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta <= tolerance,
        "expected {expected}, got {actual} (delta {delta}, tolerance {tolerance})"
    );
}

fn assert_tp_b29_rounded_velocity(actual: f64, displayed_ratio: f64, incoming_speed: f64) {
    assert_near(
        actual,
        displayed_ratio * incoming_speed,
        0.0005 * incoming_speed,
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

fn translational_energy_units(states: &[OnTableBallState]) -> f64 {
    states
        .iter()
        .map(|state| {
            let velocity = &state.as_ball_state().velocity;
            velocity.x().as_f64().powi(2) + velocity.y().as_f64().powi(2)
        })
        .sum()
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
fn advancing_with_explicit_ball_ball_physics_uses_that_collision_config() {
    let ball = BallSetPhysicsSpec::default();
    let radius = ball.radius.as_f64();
    let contact_offset = radius * 2.0_f64.sqrt();
    let cue_ball = on_table(BallState::on_table(
        inches2(-contact_offset, -contact_offset - 2.5),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let human_tuned = BallBallCollisionConfig::human_tuned();
    let ideal_config = BallBallCollisionConfig::ideal();

    let advanced = advance_to_next_n_ball_event_with_physics_on_table(
        &[cue_ball, object_ball],
        &ball,
        &motion_config(),
        CollisionModel::ThrowAware,
        &human_tuned,
    );
    let collision = match &advanced.event {
        Some(billiards::NBallOnTableEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        }) => {
            assert_eq!((*first_ball_index, *second_ball_index), (0, 1));
            collision
        }
        other => panic!("expected ball-ball collision, got {other:?}"),
    };
    let expected = collide_ball_ball_on_table_with_radius_and_config(
        &collision.a_at_impact,
        &collision.b_at_impact,
        ball.radius.clone(),
        CollisionModel::ThrowAware,
        &human_tuned,
    );
    let old_default = collide_ball_ball_on_table_with_radius_and_config(
        &collision.a_at_impact,
        &collision.b_at_impact,
        ball.radius.clone(),
        CollisionModel::ThrowAware,
        &ideal_config,
    );

    assert_eq!(advanced.states[0], expected.0);
    assert_eq!(advanced.states[1], expected.1);
    assert!(
        (advanced.states[1].as_ball_state().velocity.y().as_f64()
            - old_default.1.as_ball_state().velocity.y().as_f64())
        .abs()
            > 1e-6,
        "explicit human-tuned ball-ball config should reach N-ball event execution instead of silently using ideal/default coefficients"
    );
}

#[test]
fn advancing_simultaneous_disjoint_pair_collisions_resolves_both_pairs_in_one_step() {
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

    let advanced = advance_to_next_n_ball_event_on_table(
        &[a, b, c, d],
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
    );

    match advanced.event.expect("an event should be reported") {
        billiards::NBallOnTableEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } => {
            assert_eq!((first_ball_index, second_ball_index), (0, 1));
            assert_close(collision.time_until_impact.as_f64(), 1.0);
        }
        other => panic!("expected primary ball-ball collision, got {other:?}"),
    }

    assert_close(advanced.elapsed.as_f64(), 1.0);
    assert_close(advanced.states[0].as_ball_state().speed().as_f64(), 0.0);
    assert_close(
        advanced.states[1].as_ball_state().velocity.y().as_f64(),
        5.0,
    );
    assert_close(advanced.states[2].as_ball_state().speed().as_f64(), 0.0);
    assert_close(
        advanced.states[3].as_ball_state().velocity.y().as_f64(),
        5.0,
    );
}

#[test]
fn advancing_frozen_three_ball_line_uses_tp_b29_coupled_velocity_split() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-(2.0 * radius + 7.5), 0.0),
        Velocity2::new("10", "0"),
        AngularVelocity3::new(0.0, 10.0 / radius, 0.0),
    ));
    let first_object = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let second_object = on_table(BallState::resting_at(inches2(2.0 * radius, 0.0)));

    let advanced = advance_to_next_n_ball_event_on_table(
        &[cue_ball, first_object, second_object],
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
    );

    match advanced.event.expect("an event should be reported") {
        billiards::NBallOnTableEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } => {
            assert_eq!((first_ball_index, second_ball_index), (0, 1));
            assert_close(collision.time_until_impact.as_f64(), 1.0);
        }
        other => panic!("expected opening ball-ball collision, got {other:?}"),
    }

    assert_close(advanced.elapsed.as_f64(), 1.0);
    let incoming_speed = 5.0;
    assert_tp_b29_rounded_velocity(
        advanced.states[0].as_ball_state().velocity.x().as_f64(),
        -0.071,
        incoming_speed,
    );
    assert_tp_b29_rounded_velocity(
        advanced.states[1].as_ball_state().velocity.x().as_f64(),
        0.076,
        incoming_speed,
    );
    assert_tp_b29_rounded_velocity(
        advanced.states[2].as_ball_state().velocity.x().as_f64(),
        0.995,
        incoming_speed,
    );
    assert_close(
        advanced.states[0].as_ball_state().velocity.y().as_f64(),
        0.0,
    );
    assert_close(
        advanced.states[1].as_ball_state().velocity.y().as_f64(),
        0.0,
    );
    assert_close(
        advanced.states[2].as_ball_state().velocity.y().as_f64(),
        0.0,
    );
    assert_close(
        advanced
            .states
            .iter()
            .map(|state| state.as_ball_state().velocity.x().as_f64())
            .sum::<f64>(),
        incoming_speed,
    );
    assert_close(
        translational_energy_units(&advanced.states),
        incoming_speed.powi(2),
    );
}

#[test]
fn advancing_throw_aware_zero_slip_frozen_three_ball_line_uses_tp_b29_coupled_velocity_split() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-2.0 * radius, 0.0),
        Velocity2::new("10", "0"),
        AngularVelocity3::zero(),
    ));
    let first_object = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let second_object = on_table(BallState::resting_at(inches2(2.0 * radius, 0.0)));

    let advanced = advance_to_next_n_ball_event_with_physics_on_table(
        &[cue_ball, first_object, second_object],
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::ThrowAware,
        &BallBallCollisionConfig::ideal(),
    );

    match advanced.event.expect("an event should be reported") {
        billiards::NBallOnTableEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } => {
            assert_eq!((first_ball_index, second_ball_index), (0, 1));
            assert_close(collision.time_until_impact.as_f64(), 0.0);
        }
        other => panic!("expected opening ball-ball collision, got {other:?}"),
    }

    assert_close(advanced.elapsed.as_f64(), 0.0);
    let incoming_speed = 10.0;
    assert_tp_b29_rounded_velocity(
        advanced.states[0].as_ball_state().velocity.x().as_f64(),
        -0.071,
        incoming_speed,
    );
    assert_tp_b29_rounded_velocity(
        advanced.states[1].as_ball_state().velocity.x().as_f64(),
        0.076,
        incoming_speed,
    );
    assert_tp_b29_rounded_velocity(
        advanced.states[2].as_ball_state().velocity.x().as_f64(),
        0.995,
        incoming_speed,
    );
    assert_close(
        advanced
            .states
            .iter()
            .map(|state| state.as_ball_state().velocity.x().as_f64())
            .sum::<f64>(),
        incoming_speed,
    );
    assert_close(
        translational_energy_units(&advanced.states),
        incoming_speed.powi(2),
    );

    let next = advance_to_next_n_ball_event_with_physics_on_table(
        &advanced.states,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::ThrowAware,
        &BallBallCollisionConfig::ideal(),
    );
    assert!(
        next.event.as_ref().is_none_or(|event| event.time().as_f64() > 1e-9),
        "the coupled frozen-line solve should not leave a synthetic immediate follow-on collision, got {:?}",
        next.event
    );
}

#[test]
fn advancing_throw_aware_slipping_frozen_three_ball_line_skips_tp_b29_normal_only_split() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-2.0 * radius, 0.0),
        Velocity2::new("10", "0"),
        AngularVelocity3::new(0.0, 10.0 / radius, 0.0),
    ));
    let first_object = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let second_object = on_table(BallState::resting_at(inches2(2.0 * radius, 0.0)));

    let advanced = advance_to_next_n_ball_event_with_physics_on_table(
        &[cue_ball, first_object, second_object],
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::ThrowAware,
        &BallBallCollisionConfig::ideal(),
    );

    match advanced.event.expect("an event should be reported") {
        billiards::NBallOnTableEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } => {
            assert_eq!((first_ball_index, second_ball_index), (0, 1));
            assert_close(collision.time_until_impact.as_f64(), 0.0);
        }
        other => panic!("expected opening ball-ball collision, got {other:?}"),
    }

    assert_close(advanced.elapsed.as_f64(), 0.0);
    assert!(
        (advanced.states[0].as_ball_state().velocity.x().as_f64() - -0.070_744_905_113_215 * 10.0)
            .abs()
            > 1e-6
    );
    assert!(
        (advanced.states[1].as_ball_state().velocity.x().as_f64() - 0.076_162_352_228_028 * 10.0)
            .abs()
            > 1e-6
    );
    assert!(
        (advanced.states[2].as_ball_state().velocity.x().as_f64() - 0.994_582_552_885_187 * 10.0)
            .abs()
            > 1e-6,
        "slipping non-ideal contacts should not use TP B.29's normal-only outgoing split"
    );
}

#[test]
fn advancing_shared_simultaneous_contacts_transfers_motion_into_the_cluster() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let contact_y = -3.0_f64.sqrt() * radius;
    let cue_ball = on_table(BallState::on_table(
        inches2(0.0, contact_y - 7.5),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let left_object = on_table(BallState::resting_at(inches2(-radius, 0.0)));
    let right_object = on_table(BallState::resting_at(inches2(radius, 0.0)));

    let advanced = advance_to_next_n_ball_event_on_table(
        &[cue_ball, left_object, right_object],
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
    );

    match advanced.event.expect("an event should be reported") {
        billiards::NBallOnTableEvent::SharedBallBallContact {
            time_until_contact,
            ball_indices,
            ball_ball_pairs,
            resolution,
        } => {
            assert_close(time_until_contact.as_f64(), 1.0);
            assert_eq!(ball_indices, vec![0, 1, 2]);
            assert_eq!(ball_ball_pairs, vec![(0, 1), (0, 2)]);
            assert_eq!(
                resolution.as_str(),
                "coupled_ideal_or_iterative_pairwise_approximation"
            );
        }
        other => panic!("expected shared contact, got {other:?}"),
    }

    assert!(
        advanced.states[1].as_ball_state().speed().as_f64() > 0.0,
        "left object ball should move after the shared contact"
    );
    assert!(
        advanced.states[2].as_ball_state().speed().as_f64() > 0.0,
        "right object ball should move after the shared contact"
    );

    let cue = advanced.states[0].as_ball_state();
    let left = advanced.states[1].as_ball_state();
    let right = advanced.states[2].as_ball_state();

    assert_close(cue.velocity.x().as_f64(), 0.0);
    assert_close(left.velocity.x().as_f64(), -right.velocity.x().as_f64());
    assert_close(left.velocity.y().as_f64(), right.velocity.y().as_f64());
    assert_close(left.speed().as_f64(), right.speed().as_f64());
    assert_close(translational_energy_units(&advanced.states), 25.0);
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
