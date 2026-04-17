use billiards::dsl::parse_dsl_to_scenario;
use billiards::{
    BallSetPhysicsSpec, CollisionModel, InchesPerSecondSq, MotionPhaseConfig,
    MotionTransitionConfig, OnTableMotionConfig, RadiansPerSecondSq, RailModel,
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
        .any(|line| line.contains("cue rail impact: left")));
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
        .any(|line| line.contains("one pocketed in top-left")));
    assert!(lines.len() > 8, "expected a busy multi-event example");
}
