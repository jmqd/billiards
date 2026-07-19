use billiards::{
    advance_motion_on_table, advance_within_phase_on_table,
    compute_next_ball_ball_collision_on_table,
    compute_next_n_ball_system_event_with_rails_and_pockets_on_table,
    compute_next_transition_on_table, settle_airborne_ball_on_next_table_contact,
    simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit,
    time_until_airborne_ball_reaches_table, AngularVelocity3, BallBallCollisionConfig,
    BallSetPhysicsSpec, BallState, CollisionModel, Inches, Inches2, InchesPerSecond,
    InchesPerSecondSq, MotionPhase, MotionPhaseConfig, MotionPhaseThresholds,
    MotionTransitionConfig, NBallSystemEvent, NBallSystemState, OnTableBallState,
    OnTableMotionConfig, RadiansPerSecond, RadiansPerSecondSq, RailCollisionProfile, RailModel,
    RollingResistanceModel, Seconds, SlidingFrictionModel, SlidingToRollingModel, SpinDecayModel,
    TableSpec, Velocity2, STANDARD_GRAVITY_INCHES_PER_SECOND_SQUARED,
};

fn inches2(x: f64, y: f64) -> Inches2 {
    Inches2::new(Inches::from_f64(x), Inches::from_f64(y))
}

fn velocity2(x: f64, y: f64) -> Velocity2 {
    Velocity2::new(Inches::from_f64(x), Inches::from_f64(y))
}

fn on_table(state: BallState) -> OnTableBallState {
    OnTableBallState::try_from(state).expect("test state should be on-table")
}

fn zero_threshold_motion() -> OnTableMotionConfig {
    MotionTransitionConfig {
        phase: MotionPhaseConfig {
            thresholds: MotionPhaseThresholds {
                airborne_height: Inches::zero(),
                airborne_vertical_speed: InchesPerSecond::zero(),
                rest_linear_speed: InchesPerSecond::zero(),
                rest_angular_speed: RadiansPerSecond::new(0.0),
            },
            sliding_to_rolling: SlidingToRollingModel::Thresholded {
                contact_speed_epsilon: InchesPerSecond::zero(),
            },
        },
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

#[test]
fn airborne_grazing_contact_between_one_millisecond_samples_is_not_tunneled() {
    let ball = BallSetPhysicsSpec::default();
    let diameter = 2.0 * ball.radius.as_f64();
    let landing_time = 0.2;
    let height = 1.0;
    let vertical_velocity =
        (0.5 * STANDARD_GRAVITY_INCHES_PER_SECOND_SQUARED * landing_time * landing_time - height)
            / landing_time;
    let relative_speed = 100.0;
    let closest_time = 0.1005;
    let overlap_half_duration = 0.00025;
    let half_chord = relative_speed * overlap_half_duration;
    let lateral_offset = (diameter * diameter - half_chord * half_chord).sqrt();
    let first = BallState::airborne(
        inches2(20.0, 20.0),
        Inches::from_f64(height),
        Velocity2::zero(),
        Inches::from_f64(vertical_velocity),
        AngularVelocity3::zero(),
    );
    let second = BallState::airborne(
        inches2(20.0 + relative_speed * closest_time, 20.0 + lateral_offset),
        Inches::from_f64(height),
        velocity2(-relative_speed, 0.0),
        Inches::from_f64(vertical_velocity),
        AngularVelocity3::zero(),
    );
    let states = [
        NBallSystemState::Airborne(first),
        NBallSystemState::Airborne(second),
    ];

    let event = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &states,
        &ball,
        &TableSpec::default(),
        &zero_threshold_motion(),
    )
    .expect("fixture geometry should validate")
    .expect("the approaching airborne balls have a real 3-D contact interval");

    let NBallSystemEvent::AirborneBallBallCollision { contact, .. } = event else {
        panic!("expected airborne ball-ball contact before landing, got {event:?}");
    };
    let actual_time = contact.time_until_contact.as_f64();
    let expected_entry = closest_time - overlap_half_duration;
    assert!(actual_time.is_finite());
    assert!(
        (actual_time - expected_entry).abs() < 1e-9,
        "expected first contact at {expected_entry}, got {actual_time}"
    );
    let dx = contact.second_at_contact.position.x().as_f64()
        - contact.first_at_contact.position.x().as_f64();
    let dy = contact.second_at_contact.position.y().as_f64()
        - contact.first_at_contact.position.y().as_f64();
    let dz = contact.second_at_contact.height.as_f64() - contact.first_at_contact.height.as_f64();
    let separation = (dx * dx + dy * dy + dz * dz).sqrt();
    assert!(separation.is_finite());
    assert!(
        (separation - diameter).abs() < 1e-9,
        "reported event must be at first physical contact"
    );
}

#[test]
fn subnormal_spin_advance_consumes_time_instead_of_recursing_at_zero_time() {
    let state = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, f64::from_bits(1)),
    ));
    let advanced = advance_motion_on_table(
        &state,
        Seconds::new(1.0),
        &BallSetPhysicsSpec::default(),
        &zero_threshold_motion(),
    );

    assert!(advanced.elapsed.as_f64().is_finite());
    assert!(advanced.state.angular_velocity.z().as_f64().is_finite());
    assert_eq!(advanced.elapsed.as_f64(), 1.0);
    assert_eq!(advanced.state.angular_velocity.z().as_f64(), 0.0);
}

#[test]
fn finite_inputs_with_unrepresentable_impact_time_return_no_event_instead_of_panicking() {
    let separation: f64 = 1e150;
    let closing_speed: f64 = -1e-161;
    assert!(separation.is_finite() && closing_speed.is_finite());
    let first = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let second = on_table(BallState::on_table(
        inches2(separation, 0.0),
        velocity2(closing_speed, 0.0),
        AngularVelocity3::zero(),
    ));

    let collision =
        compute_next_ball_ball_collision_on_table(&first, &second, &BallSetPhysicsSpec::default());
    assert!(
        collision.is_none(),
        "an impact later than f64::MAX seconds is not a representable scheduled event"
    );
}

#[test]
fn two_point_two_six_inch_slow_near_miss_remains_no_collision() {
    let ball = BallSetPhysicsSpec::default();
    let first = on_table(BallState::on_table(
        inches2(0.0, 0.0),
        velocity2(1e-9, 0.0),
        AngularVelocity3::zero(),
    ));
    let lateral_separation = 2.26;
    let second = on_table(BallState::resting_at(inches2(10.0, lateral_separation)));
    assert!(lateral_separation.is_finite());
    assert!(lateral_separation > 2.0 * ball.radius.as_f64());

    let collision = compute_next_ball_ball_collision_on_table(&first, &second, &ball);
    assert!(
        collision.is_none(),
        "parallel paths separated by more than one diameter never touch, got {collision:?}"
    );
}

#[test]
fn sub_epsilon_sliding_impulse_scales_with_half_transition_time() {
    let initial_speed = f64::EPSILON / 2.0;
    let state = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        velocity2(initial_speed, 0.0),
        AngularVelocity3::zero(),
    ));
    let ball = BallSetPhysicsSpec::default();
    let motion = zero_threshold_motion();
    let transition = compute_next_transition_on_table(&state, &ball, &motion)
        .expect("every nonzero sliding state has a future rolling transition");
    let transition_time = transition.time_until_transition.as_f64();
    assert_eq!(transition.phase_before, MotionPhase::Sliding);
    assert!(transition_time.is_finite());
    assert!(transition_time > 0.0 && transition_time < f64::EPSILON);

    let advanced = advance_within_phase_on_table(
        &state,
        MotionPhase::Sliding,
        Seconds::new(transition_time / 2.0),
        &ball,
        &motion,
    );
    let speed_ratio = advanced.as_ball_state().velocity.x().as_f64() / initial_speed;
    assert!(speed_ratio.is_finite());
    assert!(
        (speed_ratio - 6.0 / 7.0).abs() < 1e-12,
        "half the transition time must apply half the total 2/7 impulse; ratio was {speed_ratio}"
    );
}

#[test]
fn cancellation_safe_downward_table_contact_stays_positive_and_finite() {
    let height = 0.001;
    let downward_speed = 1e9;
    let state = BallState::airborne(
        inches2(20.0, 20.0),
        Inches::from_f64(height),
        Velocity2::zero(),
        Inches::from_f64(-downward_speed),
        AngularVelocity3::zero(),
    );
    let expected_time = 2.0 * height
        / ((downward_speed * downward_speed
            + 2.0 * STANDARD_GRAVITY_INCHES_PER_SECOND_SQUARED * height)
            .sqrt()
            + downward_speed);
    let predicted = time_until_airborne_ball_reaches_table(&state)
        .expect("finite downward trajectory must reach the table");
    let settled =
        settle_airborne_ball_on_next_table_contact(&state, &BallSetPhysicsSpec::default())
            .expect("finite downward trajectory must settle");
    let predicted_time = predicted.as_f64();
    let settled_time = settled.time_until_contact.as_f64();
    let contact_height = settled.state_at_contact.height.as_f64();

    assert!(expected_time.is_finite() && expected_time > 0.0);
    assert!(
        predicted_time.is_finite() && predicted_time > 0.0,
        "positive flight height requires a positive finite contact time"
    );
    assert!(settled_time.is_finite() && settled_time > 0.0);
    assert!(contact_height.is_finite());
    assert!(
        (predicted_time - expected_time).abs() <= expected_time * 1e-12,
        "expected stable positive root {expected_time:e}, got {predicted_time:e}"
    );
    assert_eq!(settled_time, predicted_time);
    assert_eq!(contact_height, 0.0);
}

#[test]
fn separating_touching_mixed_pair_recollides_after_sliding_friction_reverses_motion() {
    let ball = BallSetPhysicsSpec::default();
    let radius = ball.radius.as_f64();
    let airborne = BallState::airborne(
        inches2(20.0, 20.0),
        Inches::zero(),
        Velocity2::zero(),
        Inches::from_f64(10.0),
        AngularVelocity3::zero(),
    );
    let airborne_table_contact = time_until_airborne_ball_reaches_table(&airborne)
        .expect("an upward launch from table height must return to the table")
        .as_f64();
    let sliding = BallState::on_table(
        inches2(20.0 + 2.0 * radius, 20.0),
        velocity2(0.01, 0.0),
        AngularVelocity3::new(0.0, (0.01 - 1.0) / radius, 0.0),
    );
    let states = [
        NBallSystemState::Airborne(airborne),
        NBallSystemState::OnTable(on_table(sliding)),
    ];

    let event = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &states,
        &ball,
        &TableSpec::default(),
        &zero_threshold_motion(),
    )
    .expect("fixture geometry should validate");

    let Some(NBallSystemEvent::AirborneBallBallCollision { contact, .. }) = event else {
        panic!("expected re-entry collision before airborne table contact, got {event:?}");
    };
    let collision_time = contact.time_until_contact.as_f64();
    assert!(
        collision_time > 0.0,
        "initial separation must exclude the touching t=0 root"
    );
    assert!(
        (collision_time - 0.0354).abs() < 1e-4,
        "expected 3-D re-entry near 0.0354 s, got {collision_time}"
    );
    assert!(
        collision_time < airborne_table_contact,
        "re-entry collision at {collision_time} must precede table contact at {airborne_table_contact}"
    );
}

#[test]
fn subthreshold_touching_airborne_pair_does_not_stall_event_loop_at_zero_time() {
    let ball = BallSetPhysicsSpec::default();
    let diameter = 2.0 * ball.radius.as_f64();
    let shared_height = Inches::from_f64(1.0);
    let shared_vertical_velocity = Inches::zero();
    let states = [
        NBallSystemState::Airborne(BallState::airborne(
            inches2(20.0, 20.0),
            shared_height.clone(),
            Velocity2::zero(),
            shared_vertical_velocity.clone(),
            AngularVelocity3::zero(),
        )),
        NBallSystemState::Airborne(BallState::airborne(
            inches2(20.0 + diameter, 20.0),
            shared_height,
            velocity2(-1e-11, 0.0),
            shared_vertical_velocity,
            AngularVelocity3::zero(),
        )),
        NBallSystemState::Airborne(BallState::airborne(
            inches2(40.0, 20.0),
            Inches::from_f64(0.25),
            velocity2(1.0, 0.0),
            Inches::zero(),
            AngularVelocity3::zero(),
        )),
    ];

    let simulation = simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit(
        &states,
        &ball,
        &TableSpec::default(),
        &zero_threshold_motion(),
        CollisionModel::Ideal,
        &BallBallCollisionConfig::default(),
        RailModel::Mirror,
        &RailCollisionProfile::default(),
        Some(4),
    )
    .expect("the exactly touching airborne fixture should remain valid");

    let zero_time_airborne_contacts = simulation
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                NBallSystemEvent::AirborneBallBallCollision { contact, .. }
                    if contact.time_until_contact.as_f64() == 0.0
            )
        })
        .count();
    assert!(
        simulation.elapsed.as_f64() > 0.0,
        "the event loop must advance past a subthreshold t=0 contact; events={}, zero_time_airborne_contacts={zero_time_airborne_contacts}, elapsed={:e}",
        simulation.events.len(),
        simulation.elapsed.as_f64()
    );
    assert!(
        matches!(
            simulation.events.first(),
            Some(NBallSystemEvent::BallTableBounce {
                ball_index: 2,
                contact,
            }) if contact.time_until_contact.as_f64() > 0.0
        ),
        "the first resolved event should be the unrelated ball's later table contact, got {:?}",
        simulation.events.first()
    );
}

#[test]
fn suprathreshold_touching_airborne_pair_still_predicts_zero_time_collision() {
    let ball = BallSetPhysicsSpec::default();
    let diameter = 2.0 * ball.radius.as_f64();
    let states = [
        NBallSystemState::Airborne(BallState::airborne(
            inches2(20.0, 20.0),
            Inches::from_f64(1.0),
            Velocity2::zero(),
            Inches::zero(),
            AngularVelocity3::zero(),
        )),
        NBallSystemState::Airborne(BallState::airborne(
            inches2(20.0 + diameter, 20.0),
            Inches::from_f64(1.0),
            velocity2(-1e-6, 0.0),
            Inches::zero(),
            AngularVelocity3::zero(),
        )),
    ];

    let event = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &states,
        &ball,
        &TableSpec::default(),
        &zero_threshold_motion(),
    )
    .expect("the exactly touching airborne fixture should remain valid");

    let Some(NBallSystemEvent::AirborneBallBallCollision {
        first_ball_index,
        second_ball_index,
        contact,
    }) = event
    else {
        panic!("expected a suprathreshold immediate airborne collision, got {event:?}");
    };
    assert_eq!((first_ball_index, second_ball_index), (0, 1));
    assert_eq!(contact.time_until_contact.as_f64(), 0.0);
}

#[test]
fn subthreshold_touching_airborne_on_table_pair_still_predicts_zero_time_collision() {
    let ball = BallSetPhysicsSpec::default();
    let diameter = 2.0 * ball.radius.as_f64();
    let airborne_height = 0.25;
    let horizontal_separation = (diameter * diameter - airborne_height * airborne_height).sqrt();
    let states = [
        NBallSystemState::Airborne(BallState::airborne(
            inches2(20.0, 20.0),
            Inches::from_f64(airborne_height),
            velocity2(1e-11, 0.0),
            Inches::zero(),
            AngularVelocity3::zero(),
        )),
        NBallSystemState::OnTable(on_table(BallState::on_table(
            inches2(20.0 + horizontal_separation, 20.0),
            Velocity2::zero(),
            AngularVelocity3::zero(),
        ))),
    ];

    {
        let event = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
            &states,
            &ball,
            &TableSpec::default(),
            &zero_threshold_motion(),
        )
        .expect("the exactly touching mixed-height fixture should remain valid");

        let Some(NBallSystemEvent::AirborneBallBallCollision {
            first_ball_index,
            second_ball_index,
            contact,
        }) = event
        else {
            panic!("expected a subthreshold immediate mixed-height collision, got {event:?}");
        };
        assert_eq!((first_ball_index, second_ball_index), (0, 1));
        assert_eq!(contact.time_until_contact.as_f64(), 0.0);
    }

    let simulation = simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit(
        &states,
        &ball,
        &TableSpec::default(),
        &zero_threshold_motion(),
        CollisionModel::Ideal,
        &BallBallCollisionConfig::default(),
        RailModel::Mirror,
        &RailCollisionProfile::default(),
        Some(1),
    );
    assert!(
        !matches!(
            &simulation,
            Err(billiards::NBallGeometryError::ZeroTimeNoProgress)
        ),
        "the predicted mixed-height t=0 collision must make progress when executed"
    );
    let simulation = simulation.expect("the predicted mixed-height collision should be executable");
    assert!(
        matches!(
            simulation.events.as_slice(),
            [NBallSystemEvent::AirborneBallBallCollision {
                first_ball_index: 0,
                second_ball_index: 1,
                contact,
            }] if contact.time_until_contact.as_f64() == 0.0
        ),
        "the fallible simulation must execute the predicted zero-time collision, got {:?}",
        simulation.events
    );
    assert!(
        simulation
            .states
            .iter()
            .zip(&states)
            .any(|(after, before)| after != before),
        "executing the zero-time collision must change at least one ball state"
    );
}
