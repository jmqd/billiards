use billiards::dsl::parse_dsl_to_scenario;
use billiards::{
    BallSetPhysicsSpec, BallType, CollisionModel, InchesPerSecondSq, MotionPhaseConfig,
    MotionTransitionConfig, NBallSystemState, OnTableMotionConfig, RadiansPerSecondSq, RailModel,
    RollingResistanceModel, SlidingFrictionModel, SpinDecayModel,
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

fn on_table_x(state: &NBallSystemState) -> f64 {
    match state {
        NBallSystemState::OnTable(state) => state.as_ball_state().position.x().as_f64(),
        NBallSystemState::Pocketed { .. } => panic!("expected an on-table cue ball"),
    }
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

    assert!(lines
        .iter()
        .any(|line| line.contains("cue -> one collision")));
    assert!(lines
        .iter()
        .any(|line| line.contains("one pocketed in center-right")));
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
    let initial_x = cue.initial_state.as_ball_state().position.x().as_f64();
    let final_x = on_table_x(&cue.final_state);

    assert!(lines
        .iter()
        .any(|line| line.contains("cue -> one collision")));
    assert!(lines
        .iter()
        .any(|line| line.contains("one pocketed in center-right")));
    assert!(lines
        .iter()
        .any(|line| line.contains("cue rail impact: right")));
    assert!(
        final_x > initial_x,
        "follow should carry the cue farther down-table after contact"
    );
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

    assert!(lines
        .iter()
        .any(|line| line.contains("cue -> one collision")));
    assert!(lines
        .iter()
        .any(|line| line.contains("one pocketed in center-right")));
    assert!(lines
        .iter()
        .any(|line| line.contains("cue rail impact: left")));
    assert!(
        !lines.iter().any(|line| line.contains("cue rail impact: right")),
        "the toned-down draw anchor should pull back instead of following through"
    );
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
        .any(|line| line.contains("two rail impact: right")));
    assert!(lines.len() > 8, "expected a busy multi-event example");
    assert!(
        !lines.iter().any(|line| line.contains("pocketed")),
        "the current tuned model keeps this example on the table"
    );
}
