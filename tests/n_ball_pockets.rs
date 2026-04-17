use billiards::{
    advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table,
    compute_next_ball_pocket_capture_on_table,
    compute_next_n_ball_system_event_with_rails_and_pockets_on_table,
    simulate_n_balls_with_rails_and_pockets_on_table_until_rest, AngularVelocity3,
    BallSetPhysicsSpec, BallState, CollisionModel, Inches, Inches2, InchesPerSecondSq, MotionPhase,
    MotionPhaseConfig, MotionTransitionConfig, NBallSystemEvent, NBallSystemState,
    OnTableBallState, OnTableMotionConfig, Pocket, RadiansPerSecondSq, Rail,
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
