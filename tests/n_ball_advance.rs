use billiards::{
    advance_to_next_n_ball_event_on_table, advance_to_next_n_ball_event_with_physics_on_table,
    advance_to_next_n_ball_event_with_rails_on_table,
    advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table, AngularVelocity3,
    BallBallCollisionConfig, BallSetPhysicsSpec, BallState, CollisionModel, Diamond, Inches,
    Inches2, InchesPerSecondSq, MotionPhase, MotionPhaseConfig, MotionTransitionConfig,
    NBallOnTableExecutionError, NBallSystemState, OnTableBallState, OnTableMotionConfig,
    RadiansPerSecondSq, RailCollisionProfile, RailModel, RollingResistanceModel, Scale,
    SlidingFrictionModel, SpinDecayModel, TableSpec, Velocity2, TYPICAL_BALL_RADIUS,
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

fn system_kinetic_energy_units(states: &[NBallSystemState], radius: f64) -> f64 {
    states
        .iter()
        .map(|state| {
            let state = state.as_ball_state();
            let linear = state.velocity.x().as_f64().powi(2)
                + state.velocity.y().as_f64().powi(2)
                + state.vertical_velocity.as_f64().powi(2);
            let angular = state.angular_velocity.x().as_f64().powi(2)
                + state.angular_velocity.y().as_f64().powi(2)
                + state.angular_velocity.z().as_f64().powi(2);
            0.5 * linear + radius * radius * angular / 5.0
        })
        .sum()
}

fn contact_components_for_test(
    first: &BallState,
    second: &BallState,
    normal_x: f64,
    normal_y: f64,
    radius: f64,
) -> (f64, f64, f64) {
    let tangent_x = normal_y;
    let tangent_y = -normal_x;
    let relative_normal = (second.velocity.x().as_f64() - first.velocity.x().as_f64()) * normal_x
        + (second.velocity.y().as_f64() - first.velocity.y().as_f64()) * normal_y;
    let tangential_slip = (first.velocity.x().as_f64() - second.velocity.x().as_f64()) * tangent_x
        + (first.velocity.y().as_f64() - second.velocity.y().as_f64()) * tangent_y
        - radius * (first.angular_velocity.z().as_f64() + second.angular_velocity.z().as_f64());
    let vertical_slip = first.vertical_velocity.as_f64() - second.vertical_velocity.as_f64()
        + radius
            * (normal_y
                * (first.angular_velocity.x().as_f64() + second.angular_velocity.x().as_f64())
                - normal_x
                    * (first.angular_velocity.y().as_f64() + second.angular_velocity.y().as_f64()));
    (relative_normal, tangential_slip, vertical_slip)
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
        &[a, b, c.clone()],
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
    )
    .expect("fixture should satisfy N-ball geometry");

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
    )
    .expect("fixture should satisfy N-ball geometry");

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
fn planar_n_ball_executor_rejects_nonideal_collision_models_before_scheduling() {
    let ball = BallSetPhysicsSpec::default();
    let radius = ball.radius.as_f64();
    let contact_offset = radius * 2.0_f64.sqrt();
    let cue_ball = on_table(BallState::on_table(
        inches2(-contact_offset, -contact_offset - 2.5),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    ));
    let object_ball = on_table(BallState::resting_at(inches2(0.0, 0.0)));

    assert!(matches!(
        advance_to_next_n_ball_event_with_physics_on_table(
            &[cue_ball, object_ball],
            &ball,
            &motion_config(),
            CollisionModel::ThrowAware,
            &BallBallCollisionConfig::human_tuned(),
        ),
        Err(
            billiards::NBallOnTableExecutionError::NonPlanarCollisionModel {
                collision_model: CollisionModel::ThrowAware
            }
        )
    ));
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
    )
    .expect("fixture should satisfy N-ball geometry");

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
    )
    .expect("fixture should satisfy N-ball geometry");

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
    assert_near(
        translational_energy_units(&advanced.states),
        incoming_speed.powi(2),
        1e-5,
    );
}

#[test]
fn advancing_zero_friction_nonideal_slipping_three_ball_line_uses_tp_b29() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-(2.0 * radius + 7.5), 0.0),
        Velocity2::new("10", "0"),
        AngularVelocity3::new(0.0, 10.0 / radius, 0.0),
    ));
    let first_object = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let second_object = on_table(BallState::resting_at(inches2(2.0 * radius, 0.0)));
    let zero_friction = BallBallCollisionConfig::new(Scale::from_f64(1.0), Scale::zero());

    for collision_model in [CollisionModel::ThrowAware, CollisionModel::SpinFriction] {
        let advanced = advance_to_next_n_ball_event_with_physics_on_table(
            &[
                cue_ball.clone(),
                first_object.clone(),
                second_object.clone(),
            ],
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            collision_model,
            &zero_friction,
        )
        .expect("zero-friction nonideal line contact should remain planar");

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
        assert_near(
            translational_energy_units(&advanced.states),
            incoming_speed.powi(2),
            1e-5,
        );
    }
}

#[test]
fn four_ball_line_uses_full_component_solve_in_planar_and_system_executors() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let states = [
        on_table(BallState::on_table(
            inches2(-(2.0 * radius + 7.5), 0.0),
            Velocity2::new("10", "0"),
            AngularVelocity3::new(0.0, 10.0 / radius, 0.0),
        )),
        on_table(BallState::resting_at(inches2(0.0, 0.0))),
        on_table(BallState::resting_at(inches2(2.0 * radius, 0.0))),
        on_table(BallState::resting_at(inches2(4.0 * radius, 0.0))),
    ];
    let planar = advance_to_next_n_ball_event_on_table(
        &states,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
    )
    .expect("four-ball planar line should resolve through the full contact graph");
    let system_states = states
        .iter()
        .cloned()
        .map(NBallSystemState::OnTable)
        .collect::<Vec<_>>();
    let system = advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table(
        &system_states,
        &BallSetPhysicsSpec::default(),
        &TableSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
        &BallBallCollisionConfig::ideal(),
        RailModel::SpinAware,
        &RailCollisionProfile::default(),
    )
    .expect("four-ball system line should resolve through the full contact graph");
    let permutation = [3usize, 1, 0, 2];
    let permuted_states = permutation.map(|original_index| states[original_index].clone());
    let permuted = advance_to_next_n_ball_event_on_table(
        &permuted_states,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
    )
    .expect("index permutation must preserve the physical four-ball response");

    for (ball_index, (planar_state, system_state)) in
        planar.states.iter().zip(&system.states).enumerate()
    {
        let planar_state = planar_state.as_ball_state();
        let system_state = system_state.as_ball_state();
        assert_near(
            planar_state.velocity.x().as_f64(),
            system_state.velocity.x().as_f64(),
            1e-9,
        );
        assert_near(
            planar_state.velocity.y().as_f64(),
            system_state.velocity.y().as_f64(),
            1e-9,
        );
        assert_near(
            planar_state.angular_velocity.y().as_f64(),
            system_state.angular_velocity.y().as_f64(),
            1e-9,
        );
        assert!(
            ball_index != 3 || system_state.velocity.x().as_f64() > 2.5,
            "the fourth ball must carry a substantial part of the wave"
        );
    }
    let planar_velocities = planar
        .states
        .iter()
        .map(|state| state.as_ball_state().velocity.x().as_f64())
        .collect::<Vec<_>>();
    for edge in 0..3 {
        assert!(
            planar_velocities[edge + 1] - planar_velocities[edge] >= -1e-7,
            "four-ball onset edge {edge} remains closing"
        );
    }
    assert_near(planar_velocities.iter().sum(), 5.0, 1e-8);
    assert_near(translational_energy_units(&planar.states), 25.0, 1e-5);
    assert!(
        (planar_velocities[0] - -0.071 * 5.0).abs() > 0.001
            || (planar_velocities[1] - 0.076 * 5.0).abs() > 0.001,
        "four-ball solve must not embed the deleted three-ball terminal constants"
    );
    for (permuted_index, original_index) in permutation.into_iter().enumerate() {
        assert_near(
            permuted.states[permuted_index]
                .as_ball_state()
                .velocity
                .x()
                .as_f64(),
            planar.states[original_index]
                .as_ball_state()
                .velocity
                .x()
                .as_f64(),
            1e-8,
        );
    }
}

#[test]
fn advancing_throw_aware_zero_slip_frozen_three_ball_line_rejects_nonplanar_model() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-2.0 * radius, 0.0),
        Velocity2::new("10", "0"),
        AngularVelocity3::zero(),
    ));
    let first_object = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let second_object = on_table(BallState::resting_at(inches2(2.0 * radius, 0.0)));

    assert!(matches!(
        advance_to_next_n_ball_event_with_physics_on_table(
            &[cue_ball, first_object, second_object],
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            CollisionModel::ThrowAware,
            &BallBallCollisionConfig::ideal(),
        ),
        Err(NBallOnTableExecutionError::NonPlanarCollisionModel {
            collision_model: CollisionModel::ThrowAware,
        })
    ));
}

#[test]
fn advancing_throw_aware_slipping_frozen_three_ball_line_rejects_nonplanar_model() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-2.0 * radius, 0.0),
        Velocity2::new("10", "0"),
        AngularVelocity3::new(0.0, 10.0 / radius, 0.0),
    ));
    let first_object = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let second_object = on_table(BallState::resting_at(inches2(2.0 * radius, 0.0)));

    assert!(matches!(
        advance_to_next_n_ball_event_with_physics_on_table(
            &[cue_ball, first_object, second_object],
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            CollisionModel::ThrowAware,
            &BallBallCollisionConfig::ideal(),
        ),
        Err(NBallOnTableExecutionError::NonPlanarCollisionModel {
            collision_model: CollisionModel::ThrowAware,
        })
    ));
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
    )
    .expect("fixture should satisfy N-ball geometry");

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
            assert_eq!(resolution.as_str(), "coupled_normal");
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
    assert_near(translational_energy_units(&advanced.states), 25.0, 1e-5);
}

#[test]
fn zero_friction_nonideal_shared_contact_uses_the_coupled_normal_solution() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(0.0, -3.0_f64.sqrt() * radius),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    ));
    let left_object = on_table(BallState::resting_at(inches2(-radius, 0.0)));
    let right_object = on_table(BallState::resting_at(inches2(radius, 0.0)));
    let states = [cue_ball, left_object, right_object];
    let config = BallBallCollisionConfig::new(Scale::from_f64(1.0), Scale::zero());

    for collision_model in [CollisionModel::ThrowAware, CollisionModel::SpinFriction] {
        let advanced = advance_to_next_n_ball_event_with_physics_on_table(
            &states,
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            collision_model,
            &config,
        )
        .expect("zero-friction nonideal contact graph should remain planar and solvable");

        match advanced.event.expect("an event should be reported") {
            billiards::NBallOnTableEvent::SharedBallBallContact {
                time_until_contact,
                ball_ball_pairs,
                resolution,
                ..
            } => {
                assert_close(time_until_contact.as_f64(), 0.0);
                assert_eq!(ball_ball_pairs, vec![(0, 1), (0, 2)]);
                assert_eq!(resolution.as_str(), "coupled_normal");
            }
            other => panic!("expected shared contact, got {other:?}"),
        }

        let cue = advanced.states[0].as_ball_state();
        let left = advanced.states[1].as_ball_state();
        let right = advanced.states[2].as_ball_state();
        assert_near(cue.velocity.x().as_f64(), 0.0, 1e-8);
        assert_near(
            left.velocity.x().as_f64(),
            -right.velocity.x().as_f64(),
            1e-8,
        );
        assert_near(
            left.velocity.y().as_f64(),
            right.velocity.y().as_f64(),
            1e-8,
        );
        for (leaf, normal_x) in [(left, -0.5), (right, 0.5)] {
            let relative_normal = (leaf.velocity.x().as_f64() - cue.velocity.x().as_f64())
                * normal_x
                + (leaf.velocity.y().as_f64() - cue.velocity.y().as_f64()) * (0.5 * 3.0_f64.sqrt());
            assert!(
                relative_normal >= -1e-7,
                "compliant shared edge remains closing: {relative_normal}"
            );
        }
        assert_close(
            advanced
                .states
                .iter()
                .map(|state| state.as_ball_state().velocity.y().as_f64())
                .sum(),
            10.0,
        );
        assert_near(translational_energy_units(&advanced.states), 100.0, 5e-5);
    }
}

#[test]
fn frictional_shared_contact_uses_coupled_coulomb_impulses_without_slip_reversal() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let sqrt_three = 3.0_f64.sqrt();
    let cue = on_table(BallState::on_table(
        inches2(0.0, -sqrt_three * radius),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    ));
    let left = on_table(BallState::on_table(
        inches2(-radius, 0.0),
        Velocity2::new(Inches::from_f64(-3.0 * sqrt_three), Inches::from_f64(-3.0)),
        AngularVelocity3::zero(),
    ));
    let right = on_table(BallState::on_table(
        inches2(radius, 0.0),
        Velocity2::new(Inches::from_f64(-3.0 * sqrt_three), Inches::from_f64(3.0)),
        AngularVelocity3::zero(),
    ));
    let before = [
        NBallSystemState::OnTable(cue),
        NBallSystemState::OnTable(left),
        NBallSystemState::OnTable(right),
    ];
    let normals = [(-0.5, 0.5 * sqrt_three), (0.5, 0.5 * sqrt_three)];
    let pre_slips = normals.map(|(normal_x, normal_y)| {
        contact_components_for_test(
            before[0].as_ball_state(),
            before[if normal_x < 0.0 { 1 } else { 2 }].as_ball_state(),
            normal_x,
            normal_y,
            radius,
        )
    });
    assert!(pre_slips.iter().all(|slip| slip.1 > 0.0));

    let friction = 1.0;
    let collision_config =
        BallBallCollisionConfig::new(Scale::from_f64(1.0), Scale::from_f64(friction));
    let advanced = advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table(
        &before,
        &BallSetPhysicsSpec::default(),
        &TableSpec::default(),
        &motion_config(),
        CollisionModel::ThrowAware,
        &collision_config,
        RailModel::SpinAware,
        &RailCollisionProfile::default(),
    )
    .expect("frictional shared contact should resolve");

    let before_energy = system_kinetic_energy_units(&before, radius);
    let after_energy = system_kinetic_energy_units(&advanced.states, radius);
    assert!(
        after_energy <= before_energy + 1e-9 * before_energy.max(1.0),
        "friction increased energy: before={before_energy}, after={after_energy}"
    );

    for (contact_index, ((normal_x, normal_y), pre_slip)) in
        normals.into_iter().zip(pre_slips).enumerate()
    {
        let leaf_index = contact_index + 1;
        let first_after = advanced.states[0].as_ball_state();
        let leaf_after = advanced.states[leaf_index].as_ball_state();
        let post = contact_components_for_test(first_after, leaf_after, normal_x, normal_y, radius);
        assert!(
            post.0 >= -1e-9,
            "contact {contact_index} remains closing: normal velocity {}",
            post.0
        );
        assert!(
            pre_slip.1 * post.1 + pre_slip.2 * post.2 >= -1e-9,
            "contact {contact_index} reversed slip: before=({}, {}), after=({}, {})",
            pre_slip.1,
            pre_slip.2,
            post.1,
            post.2
        );
        assert!(
            post.1.hypot(post.2) <= 1e-8,
            "high-friction contact {contact_index} should stick, residual slip=({}, {})",
            post.1,
            post.2
        );

        let leaf_before = before[leaf_index].as_ball_state();
        let delta_vx = leaf_after.velocity.x().as_f64() - leaf_before.velocity.x().as_f64();
        let delta_vy = leaf_after.velocity.y().as_f64() - leaf_before.velocity.y().as_f64();
        let normal_impulse = delta_vx * normal_x + delta_vy * normal_y;
        let tangential_impulse = delta_vx * normal_y - delta_vy * normal_x;
        let vertical_impulse =
            leaf_after.vertical_velocity.as_f64() - leaf_before.vertical_velocity.as_f64();
        assert!(
            tangential_impulse.hypot(vertical_impulse) <= friction * normal_impulse + 1e-9,
            "contact {contact_index} exceeded Coulomb disk: |Jt|={}, mu*Jn={}",
            tangential_impulse.hypot(vertical_impulse),
            friction * normal_impulse
        );
    }
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
    )
    .expect("fixture should satisfy N-ball geometry");

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
    )
    .expect("fixture should satisfy N-ball geometry");

    assert_eq!(advanced.elapsed.as_f64(), 0.0);
    assert!(advanced.event.is_none());
    assert_eq!(advanced.states, vec![a, b, c]);
}
