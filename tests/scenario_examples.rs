use billiards::dsl::parse_dsl_to_scenario;
use billiards::{
    BallSetPhysicsSpec, BallType, CollisionModel, InchesPerSecondSq, MotionPhaseConfig,
    MotionTransitionConfig, NBallSystemState, OnTableMotionConfig, Pocket, RadiansPerSecondSq,
    RailModel, RollingResistanceModel, SlidingFrictionModel, SpinDecayModel,
};

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

fn cue_trace(trace: &billiards::dsl::ScenarioShotTrace) -> &billiards::dsl::ScenarioBallTrace {
    trace
        .ball_traces
        .iter()
        .find(|ball_trace| ball_trace.ball == BallType::Cue)
        .expect("examples should include a cue-ball trace")
}

#[test]
fn straight_in_side_pocket_example_runs_end_to_end() {
    let scenario = parse_dsl_to_scenario(include_str!(
        "../examples/scenarios/straight_in_side_pocket.billiards"
    ))
    .expect("example should parse");
    let trace = scenario
        .simulate_shot_trace_with_rails_and_pockets_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        )
        .expect("example should simulate")
        .expect("example contains a shot");
    let lines = trace.event_lines();
    let object = trace
        .ball_traces
        .iter()
        .find(|ball_trace| ball_trace.ball == BallType::One)
        .expect("example should include an object-ball trace");
    let pocket_center_y = scenario
        .game_state
        .table_spec
        .diamond_to_inches(Pocket::CenterRight.aiming_center().y.clone())
        .as_f64();

    assert!(lines
        .iter()
        .any(|line| line.contains("cue -> one collision")));
    assert!(lines
        .iter()
        .any(|line| line.contains("one pocketed in center-right")));
    match &object.final_state {
        NBallSystemState::Pocketed {
            pocket,
            state_at_capture,
        } => {
            assert_eq!(*pocket, Pocket::CenterRight);
            assert!(
                (state_at_capture.as_ball_state().position.y().as_f64() - pocket_center_y).abs()
                    < 1e-9,
                "the straight side-pocket example should enter the pocket centered in y"
            );
        }
        other => panic!("expected object ball to be pocketed, got {other:?}"),
    }
}

#[test]
fn straight_follow_side_pocket_example_runs_end_to_end() {
    let scenario = parse_dsl_to_scenario(include_str!(
        "../examples/scenarios/straight_follow_side_pocket.billiards"
    ))
    .expect("example should parse");
    let trace = scenario
        .simulate_shot_trace_with_rails_and_pockets_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        )
        .expect("example should simulate")
        .expect("example contains a shot");
    let lines = trace.event_lines();
    let cue = cue_trace(&trace);

    assert!(lines
        .iter()
        .any(|line| line.contains("cue -> one collision")));
    assert!(lines
        .iter()
        .any(|line| line.contains("one pocketed in center-right")));
    assert!(lines
        .iter()
        .any(|line| line.contains("cue pocketed in center-right")));
    match &cue.final_state {
        NBallSystemState::Pocketed { pocket, .. } => assert_eq!(*pocket, Pocket::CenterRight),
        other => panic!("expected cue ball to scratch in the shooting-side pocket, got {other:?}"),
    }
}

#[test]
fn straight_draw_side_pocket_example_runs_end_to_end() {
    let scenario = parse_dsl_to_scenario(include_str!(
        "../examples/scenarios/straight_draw_side_pocket.billiards"
    ))
    .expect("example should parse");
    let trace = scenario
        .simulate_shot_trace_with_rails_and_pockets_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        )
        .expect("example should simulate")
        .expect("example contains a shot");
    let lines = trace.event_lines();

    let cue = cue_trace(&trace);

    assert!(lines
        .iter()
        .any(|line| line.contains("cue -> one collision")));
    assert!(lines
        .iter()
        .any(|line| line.contains("one pocketed in center-right")));
    assert!(lines
        .iter()
        .any(|line| line.contains("cue pocketed in center-left")));
    match &cue.final_state {
        NBallSystemState::Pocketed { pocket, .. } => assert_eq!(*pocket, Pocket::CenterLeft),
        other => panic!("expected draw cue ball to scratch opposite-side, got {other:?}"),
    }
}

#[test]
fn spot_shot_bottom_right_example_runs_end_to_end() {
    let scenario = parse_dsl_to_scenario(include_str!(
        "../examples/scenarios/spot_shot_bottom_right.billiards"
    ))
    .expect("example should parse");
    let trace = scenario
        .simulate_shot_trace_with_rails_and_pockets_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        )
        .expect("example should simulate")
        .expect("example contains a shot");
    let lines = trace.event_lines();

    assert!(lines
        .iter()
        .any(|line| line.contains("cue -> one collision")));
    assert!(lines
        .iter()
        .any(|line| line.contains("one pocketed in bottom-right")));
    assert!(lines
        .iter()
        .any(|line| line.contains("cue pocketed in bottom-left")));
}

#[test]
fn two_rail_bank_scratch_example_runs_end_to_end() {
    let scenario = parse_dsl_to_scenario(include_str!(
        "../examples/scenarios/two_rail_bank_scratch.billiards"
    ))
    .expect("example should parse");
    let trace = scenario
        .simulate_shot_trace_with_rails_and_pockets_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        )
        .expect("example should simulate")
        .expect("example contains a shot");
    let lines = trace.event_lines();

    assert!(lines
        .iter()
        .any(|line| line.contains("cue rail impact: right")));
    assert!(lines
        .iter()
        .any(|line| line.contains("cue rail impact: top")));
    assert!(lines
        .iter()
        .any(|line| line.contains("cue pocketed in center-left")));
}

#[test]
fn three_ball_pinball_example_runs_end_to_end() {
    let scenario = parse_dsl_to_scenario(include_str!(
        "../examples/scenarios/three_ball_pinball.billiards"
    ))
    .expect("example should parse");
    let trace = scenario
        .simulate_shot_trace_with_rails_and_pockets_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        )
        .expect("example should simulate")
        .expect("example contains a shot");
    let lines = trace.event_lines();

    assert!(lines
        .iter()
        .any(|line| line.contains("cue -> one collision")));
    assert!(lines
        .iter()
        .any(|line| line.contains("one -> two collision")));
    assert!(lines
        .iter()
        .any(|line| line.contains("two pocketed in center-right")));
    assert!(lines.len() >= 7, "expected a multi-event chain example");
    assert!(
        lines
            .iter()
            .any(|line| line.contains("cue Rolling -> Rest")),
        "the cue ball should settle on the table in the corrected example"
    );
}

#[test]
fn named_physics_pinball_example_runs_end_to_end() {
    let scenario = parse_dsl_to_scenario(include_str!(
        "../examples/scenarios/named_physics_pinball.billiards"
    ))
    .expect("example should parse");
    let trace = scenario
        .simulate_shot_trace_with_simulation_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            "human_pinball",
        )
        .expect("example should simulate")
        .expect("example contains a shot");
    let lines = trace.event_lines();

    assert!(lines
        .iter()
        .any(|line| line.contains("cue -> one collision")));
    assert!(lines
        .iter()
        .any(|line| line.contains("one -> two collision")));
    assert!(
        lines.len() >= 5,
        "expected the named-physics example to produce a multi-event chain"
    );
    assert_eq!(
        scenario
            .simulation_named("human_pinball")
            .expect("named simulation")
            .ball_ball_name,
        "human"
    );
}
