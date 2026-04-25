use billiards::{
    advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table,
    compute_next_ball_jaw_impact_on_table, compute_next_ball_pocket_capture_on_table,
    compute_next_n_ball_system_event_with_rails_and_pockets_on_table,
    simulate_n_balls_with_rails_and_pockets_on_table_until_rest, AngularVelocity3,
    BallSetPhysicsSpec, BallState, CollisionModel, Inches, Inches2, InchesPerSecondSq, MotionPhase,
    MotionPhaseConfig, MotionTransitionConfig, NBallSystemEvent, NBallSystemState,
    OnTableBallState, OnTableMotionConfig, Pocket, PocketShapeSpec, RadiansPerSecondSq, Rail,
    RollingResistanceModel, SlidingFrictionModel, SpinDecayModel, TableSpec, Velocity2,
    CENTER_SPOT, TYPICAL_BALL_RADIUS,
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

fn rolling_toward_center_right_side_pocket() -> OnTableBallState {
    let table = TableSpec::default();
    on_table(BallState::on_table(
        inches2(
            40.0,
            table.diamond_to_inches(CENTER_SPOT.y.clone()).as_f64(),
        ),
        Velocity2::new("10", "0"),
        AngularVelocity3::new(0.0, 10.0 / TYPICAL_BALL_RADIUS.as_f64(), 0.0),
    ))
}

#[test]
fn a_single_ball_heading_into_the_side_pocket_predicts_capture_before_the_rail() {
    let table = TableSpec::default();
    let state = rolling_toward_center_right_side_pocket();

    let capture = compute_next_ball_pocket_capture_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("the rolling ball should be captured by the side pocket");
    let event = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &[NBallSystemState::from(state)],
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("an event should be predicted");

    assert_eq!(capture.pocket, Pocket::CenterRight);
    assert_close(capture.time_until_capture.as_f64(), 1.0);
    match event {
        NBallSystemEvent::BallPocketCapture {
            ball_index,
            capture,
        } => {
            assert_eq!(ball_index, 0);
            assert_eq!(capture.pocket, Pocket::CenterRight);
            assert_close(capture.time_until_capture.as_f64(), 1.0);
        }
        other => panic!("expected pocket capture, got {other:?}"),
    }
}

#[test]
fn a_fast_side_pocket_entry_beyond_the_effective_target_angle_is_rejected() {
    let table = TableSpec::default();
    let pocket_center = Pocket::CenterRight.aiming_center();
    let pocket_x = table.diamond_to_inches(pocket_center.x).as_f64();
    let pocket_y = table.diamond_to_inches(pocket_center.y).as_f64();
    let angle_radians = 60.0_f64.to_radians();
    let distance = 10.0;
    let speed = 80.0;
    let state = on_table(BallState::on_table(
        inches2(
            pocket_x - distance * angle_radians.cos(),
            pocket_y - distance * angle_radians.sin(),
        ),
        Velocity2::new(
            Inches::from_f64(speed * angle_radians.cos()),
            Inches::from_f64(speed * angle_radians.sin()),
        ),
        AngularVelocity3::zero(),
    ));

    assert!(
        compute_next_ball_pocket_capture_on_table(
            &state,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .is_none(),
        "a steep fast side-pocket approach should now be rejected by the jaw-aware capture gate"
    );
}

#[test]
fn a_ball_aimed_at_a_side_pocket_jaw_predicts_a_jaw_impact() {
    let table = TableSpec::default();
    let state = on_table(BallState::on_table(
        inches2(44.0, 58.0),
        Velocity2::new("12", "-11"),
        AngularVelocity3::zero(),
    ));

    let impact = compute_next_ball_jaw_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("a jaw impact should be predicted");

    assert_eq!(impact.pocket, Pocket::CenterRight);
}

#[test]
fn advancing_a_near_jaw_side_pocket_entry_prefers_the_explicit_jaw_over_capture() {
    let table = TableSpec::default();
    let state = on_table(BallState::on_table(
        inches2(43.0, 57.0),
        Velocity2::new("12", "-10"),
        AngularVelocity3::zero(),
    ));

    let capture = compute_next_ball_pocket_capture_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("the coarse side-pocket gate should still see a capture candidate");
    let jaw = compute_next_ball_jaw_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("the same shot should also have an explicit jaw impact");
    assert!(
        jaw.time_until_impact.as_f64() > capture.time_until_capture.as_f64(),
        "the reproducer needs capture to arrive slightly before the jaw in the raw predictors"
    );
    assert!(
        jaw.time_until_impact.as_f64() - capture.time_until_capture.as_f64() < 0.005,
        "the reproducer should be a very small near-tie between capture and jaw"
    );

    let advanced = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &[NBallSystemState::from(state)],
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    );

    match advanced.event.expect("a first event should be predicted") {
        NBallSystemEvent::BallJawImpact { ball_index, impact } => {
            assert_eq!(ball_index, 0);
            assert_eq!(impact.pocket, Pocket::CenterRight);
        }
        other => panic!(
            "expected the pocket-aware scheduler to prefer the explicit jaw over the nearby coarse capture, got {other:?}"
        ),
    }
}

#[test]
fn a_near_jaw_entry_can_late_drop_on_the_same_jaw_impact_step() {
    let table = TableSpec::default();
    let state = on_table(BallState::on_table(
        inches2(43.0, 57.0),
        Velocity2::new("12", "-10"),
        AngularVelocity3::zero(),
    ));

    let advanced = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &[NBallSystemState::from(state)],
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    );

    match advanced.event.expect("a first event should be predicted") {
        NBallSystemEvent::BallJawImpact { ball_index, impact } => {
            assert_eq!(ball_index, 0);
            assert_eq!(impact.pocket, Pocket::CenterRight);
        }
        other => panic!("expected jaw impact, got {other:?}"),
    }
    match &advanced.states[0] {
        NBallSystemState::Pocketed {
            pocket,
            state_at_capture,
        } => {
            assert_eq!(*pocket, Pocket::CenterRight);
            assert!(
                state_at_capture.as_ball_state().speed().as_f64() > 0.0,
                "the late-drop path should preserve a meaningful post-jaw capture state"
            );
        }
        other => panic!("expected the jaw impact to resolve into a late drop, got {other:?}"),
    }
}

#[test]
fn a_shallow_side_jaw_glance_is_rejected_instead_of_late_dropping() {
    let table = TableSpec::default();
    let state = on_table(BallState::on_table(
        inches2(40.5, 56.0),
        Velocity2::new("13", "-6"),
        AngularVelocity3::zero(),
    ));

    let advanced = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &[NBallSystemState::from(state.clone())],
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    );

    match advanced.event.expect("a first event should be predicted") {
        NBallSystemEvent::BallJawImpact { ball_index, impact } => {
            assert_eq!(ball_index, 0);
            assert_eq!(impact.pocket, Pocket::CenterRight);
        }
        other => panic!("expected jaw impact, got {other:?}"),
    }
    match &advanced.states[0] {
        NBallSystemState::OnTable(state_after_jaw) => {
            assert!(
                state_after_jaw.as_ball_state().speed().as_f64() > 0.0,
                "the rejected jaw-glance should remain a live on-table state"
            );
        }
        other => panic!("expected the shallow jaw glance to stay on-table, got {other:?}"),
    }

    let simulated = simulate_n_balls_with_rails_and_pockets_on_table_until_rest(
        &[state],
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    );

    assert!(matches!(
        simulated.events.first(),
        Some(NBallSystemEvent::BallJawImpact { ball_index: 0, impact })
            if impact.pocket == Pocket::CenterRight
    ));
    assert!(!simulated.events.iter().any(|event| matches!(
        event,
        NBallSystemEvent::BallPocketCapture {
            ball_index: 0,
            capture,
        } if capture.pocket == Pocket::CenterRight
    )));
    match &simulated.states[0] {
        NBallSystemState::OnTable(state) => assert_eq!(
            state
                .as_ball_state()
                .motion_phase(TYPICAL_BALL_RADIUS.clone()),
            MotionPhase::Rest
        ),
        other => panic!(
            "expected the rejected jaw-glance to roll back out and stop on table, got {other:?}"
        ),
    }
}

#[test]
fn injected_pocket_shape_changes_the_predicted_jaw_impact_time() {
    let state = on_table(BallState::on_table(
        inches2(44.0, 58.0),
        Velocity2::new("12", "-11"),
        AngularVelocity3::zero(),
    ));
    let point_table =
        TableSpec::default().with_pocket_shape(Pocket::CenterRight, PocketShapeSpec::point_noses());
    let rounded_table = TableSpec::default().with_pocket_shape(
        Pocket::CenterRight,
        PocketShapeSpec::rounded_noses(Inches::from_f64(0.75)),
    );

    let point_impact = compute_next_ball_jaw_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &point_table,
        &motion_config(),
    )
    .expect("point jaws should still predict an impact");
    let rounded_impact = compute_next_ball_jaw_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &rounded_table,
        &motion_config(),
    )
    .expect("rounded jaws should still predict an impact");

    assert_eq!(point_impact.pocket, Pocket::CenterRight);
    assert_eq!(rounded_impact.pocket, Pocket::CenterRight);
    assert!(
        rounded_impact.time_until_impact.as_f64() < point_impact.time_until_impact.as_f64(),
        "larger rounded jaws should be struck earlier than point jaws"
    );
}

#[test]
fn a_ball_heading_cleanly_into_a_corner_pocket_still_predicts_capture() {
    let table = TableSpec::default();
    let pocket_center = Pocket::TopRight.aiming_center();
    let pocket_x = table.diamond_to_inches(pocket_center.x).as_f64();
    let pocket_y = table.diamond_to_inches(pocket_center.y).as_f64();
    let diagonal = 0.5_f64.sqrt();
    let distance = 10.0;
    let speed = 40.0;
    let state = on_table(BallState::on_table(
        inches2(
            pocket_x - distance * diagonal,
            pocket_y - distance * diagonal,
        ),
        Velocity2::new(
            Inches::from_f64(speed * diagonal),
            Inches::from_f64(speed * diagonal),
        ),
        AngularVelocity3::zero(),
    ));

    let capture = compute_next_ball_pocket_capture_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("a straight-in corner-pocket entry should still be accepted");
    assert_eq!(capture.pocket, Pocket::TopRight);
}

#[test]
fn pocket_aware_advancing_also_batches_disjoint_same_time_ball_ball_collisions() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let a = NBallSystemState::from(on_table(BallState::on_table(
        inches2(0.0, -(2.0 * radius + 7.5)),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    )));
    let b = NBallSystemState::from(on_table(BallState::resting_at(inches2(0.0, 0.0))));
    let c = NBallSystemState::from(on_table(BallState::on_table(
        inches2(20.0, -(2.0 * radius + 7.5)),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    )));
    let d = NBallSystemState::from(on_table(BallState::resting_at(inches2(20.0, 0.0))));

    let advanced = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &[a, b, c, d],
        &BallSetPhysicsSpec::default(),
        &TableSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    );

    match advanced.event.expect("an event should be reported") {
        NBallSystemEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } => {
            assert_eq!((first_ball_index, second_ball_index), (0, 1));
            assert_close(collision.time_until_impact.as_f64(), 1.0);
        }
        other => panic!("expected primary ball-ball collision, got {other:?}"),
    }
    match &advanced.states[0] {
        NBallSystemState::OnTable(state) => {
            assert_close(state.as_ball_state().speed().as_f64(), 0.0)
        }
        other => panic!("expected first ball to remain on table, got {other:?}"),
    }
    match &advanced.states[1] {
        NBallSystemState::OnTable(state) => {
            assert_close(state.as_ball_state().velocity.y().as_f64(), 5.0)
        }
        other => panic!("expected second ball to remain on table, got {other:?}"),
    }
    match &advanced.states[2] {
        NBallSystemState::OnTable(state) => {
            assert_close(state.as_ball_state().speed().as_f64(), 0.0)
        }
        other => panic!("expected third ball to remain on table, got {other:?}"),
    }
    match &advanced.states[3] {
        NBallSystemState::OnTable(state) => {
            assert_close(state.as_ball_state().velocity.y().as_f64(), 5.0)
        }
        other => panic!("expected fourth ball to remain on table, got {other:?}"),
    }
}

#[test]
fn advancing_to_a_pocket_capture_marks_that_ball_pocketed_and_advances_other_balls() {
    let table = TableSpec::default();
    let a = NBallSystemState::from(rolling_toward_center_right_side_pocket());
    let b = NBallSystemState::from(on_table(BallState::on_table(
        inches2(20.0, 20.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    )));

    let advanced = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &[a, b],
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    );

    let event = advanced.event.expect("an event should be reported");
    match event {
        NBallSystemEvent::BallPocketCapture {
            ball_index,
            capture,
        } => {
            assert_eq!(ball_index, 0);
            assert_eq!(capture.pocket, Pocket::CenterRight);
        }
        other => panic!("expected pocket capture, got {other:?}"),
    }
    match &advanced.states[0] {
        NBallSystemState::Pocketed {
            pocket,
            state_at_capture,
        } => {
            assert_eq!(*pocket, Pocket::CenterRight);
            assert_eq!(
                state_at_capture
                    .as_ball_state()
                    .motion_phase(TYPICAL_BALL_RADIUS.clone()),
                MotionPhase::Rolling
            );
        }
        other => panic!("expected pocketed state, got {other:?}"),
    }
    match &advanced.states[1] {
        NBallSystemState::OnTable(state) => {
            assert_eq!(
                state
                    .as_ball_state()
                    .motion_phase(TYPICAL_BALL_RADIUS.clone()),
                MotionPhase::Spinning
            );
            assert_close(state.as_ball_state().angular_velocity.z().as_f64(), 4.0);
        }
        other => panic!("expected on-table passive state, got {other:?}"),
    }
}

#[test]
fn simulating_with_pockets_until_rest_keeps_pocketed_balls_out_of_play_and_stops_the_rest() {
    let table = TableSpec::default();
    let a = rolling_toward_center_right_side_pocket();
    let b = on_table(BallState::on_table(
        inches2(20.0, 20.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    ));

    let simulated = simulate_n_balls_with_rails_and_pockets_on_table_until_rest(
        &[a, b],
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    );

    assert!(simulated.events.iter().any(|event| matches!(
        event,
        NBallSystemEvent::BallPocketCapture {
            ball_index: 0,
            capture,
        } if capture.pocket == Pocket::CenterRight
    )));
    match &simulated.states[0] {
        NBallSystemState::Pocketed { pocket, .. } => assert_eq!(*pocket, Pocket::CenterRight),
        other => panic!("expected first ball to be pocketed, got {other:?}"),
    }
    match &simulated.states[1] {
        NBallSystemState::OnTable(state) => assert_eq!(
            state
                .as_ball_state()
                .motion_phase(TYPICAL_BALL_RADIUS.clone()),
            MotionPhase::Rest
        ),
        other => panic!("expected second ball to remain on the table, got {other:?}"),
    }
    assert!(!simulated.events.iter().any(|event| matches!(
        event,
        NBallSystemEvent::BallRailImpact {
            ball_index: 0,
            impact,
        } if impact.rail == Rail::Right
    )));
}

#[test]
fn cached_pocket_aware_until_rest_simulation_matches_manual_event_stepping() {
    let table = TableSpec::default();
    let ball_set = BallSetPhysicsSpec::default();
    let motion = motion_config();
    let seed_a = rolling_toward_center_right_side_pocket();
    let seed_b = on_table(BallState::on_table(
        inches2(20.0, 20.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    ));
    let mut manual_states = vec![
        NBallSystemState::from(seed_a.clone()),
        NBallSystemState::from(seed_b.clone()),
    ];
    let mut manual_elapsed = 0.0;
    let mut manual_events = Vec::new();

    loop {
        let advanced = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
            &manual_states,
            &ball_set,
            &table,
            &motion,
            CollisionModel::Ideal,
            billiards::RailModel::Mirror,
        );
        let Some(event) = advanced.event else {
            break;
        };
        manual_elapsed += advanced.elapsed.as_f64();
        manual_states = advanced.states;
        manual_events.push(event);
    }

    let cached = simulate_n_balls_with_rails_and_pockets_on_table_until_rest(
        &[seed_a, seed_b],
        &ball_set,
        &table,
        &motion,
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    );

    assert_close(cached.elapsed.as_f64(), manual_elapsed);
    assert_eq!(cached.events, manual_events);
    assert_eq!(cached.states, manual_states);
}
