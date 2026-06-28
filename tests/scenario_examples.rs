use std::fs;

use billiards::dsl::{parse_dsl_to_scenario, ScenarioShotTrace, ScenarioShotTraceEventKind};
use billiards::visualization::{BallPathRenderOptions, PathColorMode};
use billiards::{
    human_tuned_preview_motion_config, BallBallCollisionConfig, BallSetPhysicsSpec, BallType,
    CollisionModel, DiagramBackground, DiagramRenderOptions, Pocket, Rail, RailCollisionProfile,
    RailModel, Seconds,
};

fn trace_scenario(
    path: &str,
    max_events: usize,
) -> (billiards::dsl::DslScenario, ScenarioShotTrace) {
    let source = fs::read_to_string(path).expect("scenario should read");
    let scenario = parse_dsl_to_scenario(&source).expect("scenario should parse");
    let trace = if max_events == 0 {
        scenario
            .simulate_shot_trace_with_physics_on_table_until_rest(
                &BallSetPhysicsSpec::default(),
                &human_tuned_preview_motion_config(),
                CollisionModel::ThrowAware,
                &BallBallCollisionConfig::human_tuned(),
                RailModel::SpinAware,
                &RailCollisionProfile::default(),
            )
            .expect("scenario should simulate")
            .expect("scenario should contain a shot")
    } else {
        scenario
            .simulate_shot_trace_with_physics_on_table_until_event_limit(
                &BallSetPhysicsSpec::default(),
                &human_tuned_preview_motion_config(),
                CollisionModel::ThrowAware,
                &BallBallCollisionConfig::human_tuned(),
                RailModel::SpinAware,
                &RailCollisionProfile::default(),
                max_events,
            )
            .expect("scenario should simulate")
            .expect("scenario should contain a shot")
    };
    (scenario, trace)
}

fn has_pocket(trace: &ScenarioShotTrace, ball: BallType, pocket: Pocket) -> bool {
    trace.event_log.iter().any(|event| {
        matches!(
            &event.kind,
            ScenarioShotTraceEventKind::BallPocketCapture {
                ball: actual_ball,
                pocket: actual_pocket,
            } if actual_ball == &ball && *actual_pocket == pocket
        )
    })
}

fn has_collision(trace: &ScenarioShotTrace, first: BallType, second: BallType) -> bool {
    trace.event_log.iter().any(|event| {
        matches!(
            &event.kind,
            ScenarioShotTraceEventKind::BallBallCollision {
                first_ball,
                second_ball,
            } if (first_ball == &first && second_ball == &second)
                || (first_ball == &second && second_ball == &first)
        )
    })
}
fn cue_rail_sequence(trace: &ScenarioShotTrace) -> Vec<Rail> {
    trace
        .event_log
        .iter()
        .filter_map(|event| match &event.kind {
            ScenarioShotTraceEventKind::BallRailImpact { ball, rail } if ball == &BallType::Cue => {
                Some(*rail)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn selected_manual_scenarios_keep_stable_current_event_flavor() {
    let (_, straight_in) =
        trace_scenario("examples/scenarios/straight_in_side_pocket.billiards", 0);
    assert!(has_collision(&straight_in, BallType::Cue, BallType::One));
    assert!(has_pocket(&straight_in, BallType::One, Pocket::CenterRight));
    assert!(!has_pocket(
        &straight_in,
        BallType::Cue,
        Pocket::CenterRight
    ));

    let (_, double_rail_kick) = trace_scenario(
        "examples/scenarios/double_rail_kick_side_pocket.billiards",
        0,
    );
    let rails = cue_rail_sequence(&double_rail_kick);
    assert!(
        rails.starts_with(&[Rail::Right, Rail::Top]),
        "double-rail kick should open right-rail then top-rail; got {rails:?}"
    );
    assert!(
        !has_collision(&double_rail_kick, BallType::Cue, BallType::One),
        "current manual kick layout is a near-miss diagnostic, not a pocketing oracle"
    );
}

#[test]
fn professional_manual_check_diagrams_parse_simulate_and_render_with_debug_overlays() {
    for scenario_path in [
        "examples/scenarios/corey_deuel_power_draw.billiards",
        "examples/scenarios/golden_break_cut_break.billiards",
        "examples/scenarios/frozen_proposition_kiss.billiards",
        "examples/scenarios/magic_spot_three_rail_kick.billiards",
        "examples/scenarios/bank_reference_track_one_rail.billiards",
        "examples/scenarios/hustler_frozen_rail_bank.billiards",
    ] {
        let (scenario, trace) = trace_scenario(scenario_path, 12);
        assert!(
            !trace.event_log.is_empty(),
            "{scenario_path}: expected at least one simulated event"
        );
        assert_eq!(
            trace.ball_traces.len(),
            scenario.game_state.balls().len(),
            "{scenario_path}: expected one trace per scenario ball"
        );
        assert!(
            !trace.event_lines().is_empty(),
            "{scenario_path}: expected human-readable event diagnostics"
        );

        let rendered = trace.rendered_final_layout_with_trace_options(
            &scenario,
            &billiards::dsl::ScenarioTraceRenderOptions {
                path_render: BallPathRenderOptions {
                    max_time_step: Seconds::new(0.02),
                    ..BallPathRenderOptions::default()
                },
                start_ghost_balls: true,
                event_markers: true,
                labels: true,
                path_color_mode: PathColorMode::MotionPhase,
            },
        );
        let image = rendered.draw_2d_diagram_with_options(&DiagramRenderOptions {
            scale_factor: 1,
            background: DiagramBackground::Transparent,
        });
        assert!(!image.is_empty(), "{scenario_path}: empty render");
    }
}
