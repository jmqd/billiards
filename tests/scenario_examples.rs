use billiards::dsl::parse_dsl_to_scenario;
use billiards::{
    advance_motion_on_table, BallSetPhysicsSpec, BallType, CollisionModel, InchesPerSecondSq,
    MotionPhase, MotionPhaseConfig, MotionTransitionConfig, NBallSystemState, OnTableMotionConfig,
    Pocket, RadiansPerSecondSq, RailModel, RollingResistanceModel, Seconds, SlidingFrictionModel,
    SpinDecayModel,
};

fn motion_config() -> OnTableMotionConfig {
    MotionTransitionConfig {
        phase: MotionPhaseConfig::default(),
        sliding_friction: SlidingFrictionModel::ConstantAcceleration {
            acceleration_magnitude: InchesPerSecondSq::new("15"),
        },
        spin_decay: SpinDecayModel::ConstantAngularDeceleration {
            angular_deceleration: RadiansPerSecondSq::new(10.9),
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

fn trace_path_length_inches(ball_trace: &billiards::dsl::ScenarioBallTrace) -> f64 {
    ball_trace
        .segments
        .iter()
        .map(|segment| {
            let start = segment.start.as_ball_state();
            let end = segment.end.as_ball_state();
            let dx = end.position.x().as_f64() - start.position.x().as_f64();
            let dy = end.position.y().as_f64() - start.position.y().as_f64();
            dx.hypot(dy)
        })
        .sum()
}

fn point_line_distance(x: f64, y: f64, x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let denom = dx.hypot(dy);
    if denom <= f64::EPSILON {
        return x.hypot(y);
    }
    ((x - x0) * dy - (y - y0) * dx).abs() / denom
}

fn final_cue_sliding_segment_max_chord_deviation_inches(
    trace: &billiards::dsl::ScenarioShotTrace,
) -> f64 {
    let cue = cue_trace(trace);
    let ball = BallSetPhysicsSpec::default();
    let radius = ball.radius.clone();
    let segment = cue
        .segments
        .iter()
        .rev()
        .find(|segment| {
            segment.start.as_ball_state().motion_phase(radius.clone()) == MotionPhase::Sliding
        })
        .expect("expected a final sliding cue segment");
    let start = segment.start.as_ball_state();
    let end = segment.end.as_ball_state();
    let x0 = start.position.x().as_f64();
    let y0 = start.position.y().as_f64();
    let x1 = end.position.x().as_f64();
    let y1 = end.position.y().as_f64();
    let mut max_deviation = 0.0;
    for step in 1..64 {
        let t = segment.duration.as_f64() * step as f64 / 64.0;
        let state =
            advance_motion_on_table(&segment.start, Seconds::new(t), &ball, &motion_config()).state;
        let deviation = point_line_distance(
            state.position.x().as_f64(),
            state.position.y().as_f64(),
            x0,
            y0,
            x1,
            y1,
        );
        if deviation > max_deviation {
            max_deviation = deviation;
        }
    }
    max_deviation
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
fn five_degree_side_pocket_example_runs_end_to_end() {
    let scenario = parse_dsl_to_scenario(include_str!(
        "../examples/scenarios/five_degree_side_pocket.billiards"
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
    assert!(matches!(
        cue_trace(&trace).final_state,
        NBallSystemState::OnTable(_)
    ));
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
fn stop_shot_side_pocket_example_runs_end_to_end() {
    let scenario = parse_dsl_to_scenario(include_str!(
        "../examples/scenarios/stop_shot_side_pocket.billiards"
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
        .any(|line| line.contains("cue Rolling -> Rest")));
    assert!(lines
        .iter()
        .any(|line| line.contains("one pocketed in center-right")));
    assert!(matches!(&cue.final_state, NBallSystemState::OnTable(_)));
}

#[test]
fn right_spin_stun_side_pocket_example_runs_end_to_end() {
    let scenario = parse_dsl_to_scenario(include_str!(
        "../examples/scenarios/right_spin_stun_side_pocket.billiards"
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
    assert!(matches!(
        cue_trace(&trace).final_state,
        NBallSystemState::OnTable(_)
    ));
    assert!(trace.simulation.elapsed.as_f64() < 5.0);
}

#[test]
fn long_cut_top_right_rail_example_runs_end_to_end() {
    let scenario = parse_dsl_to_scenario(include_str!(
        "../examples/scenarios/long_cut_top_right_rail.billiards"
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
        .any(|line| line.contains("one pocketed in top-right")));
    assert!(lines
        .iter()
        .any(|line| line.contains("cue rail impact: right")));
    assert!(
        final_cue_sliding_segment_max_chord_deviation_inches(&trace) < 0.05,
        "the final post-rail cue path should stay close to straight in this long-cut example"
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
fn routine_nine_ball_corner_cut_example_runs_end_to_end() {
    let scenario = parse_dsl_to_scenario(include_str!(
        "../examples/scenarios/routine_nine_ball_corner_cut.billiards"
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
        .any(|line| line.contains("cue -> nine collision")));
    assert!(lines
        .iter()
        .any(|line| line.contains("nine pocketed in top-right")));
    assert!(lines
        .iter()
        .any(|line| line.contains("cue rail impact: right")));
    assert!(matches!(
        cue_trace(&trace).final_state,
        NBallSystemState::OnTable(_)
    ));
}

#[test]
fn force_follow_scratch_example_runs_end_to_end() {
    let scenario = parse_dsl_to_scenario(include_str!(
        "../examples/scenarios/force_follow_scratch.billiards"
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
        other => panic!("expected follow-through cue scratch, got {other:?}"),
    }
}

#[test]
fn double_rail_kick_side_pocket_example_runs_end_to_end() {
    let scenario = parse_dsl_to_scenario(include_str!(
        "../examples/scenarios/double_rail_kick_side_pocket.billiards"
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
        .any(|line| line.contains("cue -> one collision")));
    assert!(lines
        .iter()
        .any(|line| line.contains("one pocketed in center-left")));
    assert!(matches!(
        cue_trace(&trace).final_state,
        NBallSystemState::OnTable(_)
    ));
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
    assert!(
        !lines.iter().any(|line| line.contains("pocketed")),
        "the current jaw-aware pocket gate should keep this bank path on the table as a near-miss"
    );
    assert!(matches!(
        cue_trace(&trace).final_state,
        NBallSystemState::OnTable(_)
    ));
    assert!(
        final_cue_sliding_segment_max_chord_deviation_inches(&trace) < 0.08,
        "the final post-rail cue path should be materially calmer than the old exaggerated bank-scratch bow"
    );
}

#[test]
fn mini_break_cluster_example_runs_end_to_end() {
    let scenario = parse_dsl_to_scenario(include_str!(
        "../examples/scenarios/mini_break_cluster.billiards"
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

    let visibly_moving_balls = trace
        .ball_traces
        .iter()
        .filter(|ball_trace| trace_path_length_inches(ball_trace) > 1.0)
        .count();

    assert!(lines
        .iter()
        .any(|line| line.contains("cue -> one collision")));
    assert!(lines
        .iter()
        .any(|line| line.contains("one -> two collision")));
    assert!(lines
        .iter()
        .any(|line| line.contains("one -> three collision")));
    assert!(lines.len() >= 25, "expected a busy break-style spread");
    assert!(
        !lines.iter().any(|line| line.contains("pocketed")),
        "the current tuned break setup should spread without pocketing"
    );
    assert_eq!(
        visibly_moving_balls, 7,
        "the tuned mini-break example should show all seven balls taking visible paths"
    );
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
        .any(|line| line.contains("two rail impact: left")));
    assert!(
        lines.len() >= 12,
        "expected a busy multi-event chain example"
    );
    assert!(
        !lines.iter().any(|line| line.contains("pocketed")),
        "the current exploratory pinball example is meant to stay on the table"
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
