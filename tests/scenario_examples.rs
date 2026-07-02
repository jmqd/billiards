use std::fs;

use billiards::diagram::DiagramOutputFormat;
use billiards::dsl::{parse_dsl_to_scenario, ScenarioShotTrace, ScenarioShotTraceEventKind};
use billiards::visualization::{BallPathRenderOptions, PathColorMode};
use billiards::{
    human_tuned_preview_motion_config, BallType, CollisionModel, DiagramBackground,
    DiagramRenderOptions, Pocket, Rail, RailModel, Seconds, TableKind,
};

fn trace_scenario(
    path: &str,
    max_events: usize,
) -> (billiards::dsl::DslScenario, ScenarioShotTrace) {
    let source = fs::read_to_string(path).expect("scenario should read");
    let scenario = parse_dsl_to_scenario(&source).expect("scenario should parse");
    let ball_set = scenario.ball_set_physics_spec();
    let trace = if max_events == 0 {
        scenario
            .simulate_shot_trace_with_preferred_physics_on_table_until_rest(
                &ball_set,
                &human_tuned_preview_motion_config(),
                CollisionModel::ThrowAware,
                RailModel::SpinAware,
            )
            .expect("scenario should simulate")
            .expect("scenario should contain a shot")
    } else {
        scenario
            .simulate_shot_trace_with_preferred_physics_on_table_until_event_limit(
                &ball_set,
                &human_tuned_preview_motion_config(),
                CollisionModel::ThrowAware,
                RailModel::SpinAware,
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
    assert!(has_collision(
        &double_rail_kick,
        BallType::Cue,
        BallType::One
    ));
    assert!(has_pocket(
        &double_rail_kick,
        BallType::One,
        Pocket::CenterLeft
    ));
}

#[test]
fn source_grounded_manual_checks_pocket_the_claimed_object_balls() {
    let (_, corey) = trace_scenario("examples/scenarios/corey_deuel_power_draw.billiards", 0);
    assert!(
        has_collision(&corey, BallType::Cue, BallType::Four),
        "Corey Deuel draw setup should first contact the 4"
    );
    assert!(
        has_pocket(&corey, BallType::Four, Pocket::TopRight),
        "Corey Deuel draw setup should pocket the 4 in the top-right corner"
    );

    let (_, bank) = trace_scenario(
        "examples/scenarios/bank_reference_track_one_rail.billiards",
        0,
    );
    assert!(
        has_collision(&bank, BallType::Cue, BallType::Two),
        "bank reference setup should contact the 2"
    );
    assert!(
        has_pocket(&bank, BallType::Two, Pocket::BottomRight),
        "bank reference setup should pocket the 2 in the bottom-right corner"
    );
}

#[test]
fn side_pocket_examples_match_claimed_outcomes() {
    let (_, five_degree) =
        trace_scenario("examples/scenarios/five_degree_side_pocket.billiards", 0);
    assert!(has_collision(&five_degree, BallType::Cue, BallType::One));
    assert!(has_pocket(&five_degree, BallType::One, Pocket::CenterRight));
    assert!(!has_pocket(
        &five_degree,
        BallType::Cue,
        Pocket::CenterRight
    ));

    let (_, straight_follow) = trace_scenario(
        "examples/scenarios/straight_follow_side_pocket.billiards",
        0,
    );
    assert!(has_pocket(
        &straight_follow,
        BallType::One,
        Pocket::CenterRight
    ));
    assert!(has_pocket(
        &straight_follow,
        BallType::Cue,
        Pocket::CenterRight
    ));

    let (_, straight_draw) =
        trace_scenario("examples/scenarios/straight_draw_side_pocket.billiards", 0);
    assert!(has_pocket(
        &straight_draw,
        BallType::One,
        Pocket::CenterRight
    ));
    assert!(has_pocket(
        &straight_draw,
        BallType::Cue,
        Pocket::CenterLeft
    ));

    let (_, stop_shot) = trace_scenario("examples/scenarios/stop_shot_side_pocket.billiards", 0);
    assert!(has_pocket(&stop_shot, BallType::One, Pocket::CenterRight));
    assert!(
        !has_pocket(&stop_shot, BallType::Cue, Pocket::CenterRight)
            && !has_pocket(&stop_shot, BallType::Cue, Pocket::CenterLeft),
        "stop shot should leave the cue ball on the table"
    );

    let (_, right_spin_stun) = trace_scenario(
        "examples/scenarios/right_spin_stun_side_pocket.billiards",
        0,
    );
    assert!(has_pocket(
        &right_spin_stun,
        BallType::One,
        Pocket::CenterRight
    ));
    assert!(
        !has_pocket(&right_spin_stun, BallType::Cue, Pocket::CenterRight),
        "right-spin stun example should leave the cue ball on the table"
    );
}

#[test]
fn corner_pocket_examples_match_claimed_outcomes() {
    let (_, routine_nine) = trace_scenario(
        "examples/scenarios/routine_nine_ball_corner_cut.billiards",
        0,
    );
    assert!(has_collision(&routine_nine, BallType::Cue, BallType::Nine));
    assert!(has_pocket(&routine_nine, BallType::Nine, Pocket::TopRight));
    assert!(
        cue_rail_sequence(&routine_nine).contains(&Rail::Right),
        "routine nine-ball cut should brush the right rail"
    );

    let (_, spot_shot) = trace_scenario("examples/scenarios/spot_shot_bottom_right.billiards", 0);
    assert!(has_collision(&spot_shot, BallType::Cue, BallType::One));
    assert!(has_pocket(&spot_shot, BallType::One, Pocket::BottomRight));
    assert!(has_pocket(&spot_shot, BallType::Cue, Pocket::BottomLeft));
}

#[test]
fn kick_bank_manual_checks_match_claimed_outcomes() {
    let (_, double_rail_kick) = trace_scenario(
        "examples/scenarios/double_rail_kick_side_pocket.billiards",
        0,
    );
    assert!(cue_rail_sequence(&double_rail_kick).starts_with(&[Rail::Right, Rail::Top]));
    assert!(has_collision(
        &double_rail_kick,
        BallType::Cue,
        BallType::One
    ));
    assert!(has_pocket(
        &double_rail_kick,
        BallType::One,
        Pocket::CenterLeft
    ));
    assert!(
        !has_pocket(&double_rail_kick, BallType::Cue, Pocket::CenterLeft),
        "double-rail kick should leave the cue ball on the table"
    );

    let (_, hustler_bank) =
        trace_scenario("examples/scenarios/hustler_frozen_rail_bank.billiards", 0);
    assert!(has_collision(&hustler_bank, BallType::Cue, BallType::Eight));
    assert!(has_pocket(&hustler_bank, BallType::Eight, Pocket::TopRight));

    let (_, two_rail_scratch) =
        trace_scenario("examples/scenarios/two_rail_bank_scratch.billiards", 0);
    assert!(cue_rail_sequence(&two_rail_scratch).starts_with(&[Rail::Right, Rail::Top]));
    assert!(has_pocket(
        &two_rail_scratch,
        BallType::Cue,
        Pocket::CenterLeft
    ));

    let (_, golden_break) =
        trace_scenario("examples/scenarios/golden_break_cut_break.billiards", 30);
    let golden_rails = cue_rail_sequence(&golden_break);
    assert!(
        golden_rails.contains(&Rail::Right) && golden_rails.contains(&Rail::Bottom),
        "golden-break default trace should include cue-ball route to rails; got {golden_rails:?}"
    );
    assert!(
        has_collision(&golden_break, BallType::Cue, BallType::Eight),
        "golden-break default trace should follow the cue ball back into the rack region"
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

#[test]
fn three_cushion_scenarios_use_pocketless_carom_physics_and_render_svg() {
    for scenario_path in [
        "examples/scenarios/three_cushion_opening_break.billiards",
        "examples/scenarios/three_cushion_short_angle.billiards",
        "examples/scenarios/three_cushion_long_rail_natural.billiards",
    ] {
        let (scenario, trace) = trace_scenario(scenario_path, 8);
        assert_eq!(
            scenario.game_state.table_spec.kind,
            TableKind::ThreeCushionCarom
        );
        assert!(!trace.event_log.iter().any(|event| {
            matches!(
                &event.kind,
                ScenarioShotTraceEventKind::BallPocketCapture { .. }
                    | ScenarioShotTraceEventKind::BallJawImpact { .. }
            )
        }));
        assert!(
            trace.event_log.iter().any(|event| matches!(
                &event.kind,
                ScenarioShotTraceEventKind::BallRailImpact { .. }
            )),
            "{scenario_path}: expected at least one rail impact"
        );

        let svg = scenario.game_state.render_2d_diagram_with_options(
            DiagramOutputFormat::Svg,
            &DiagramRenderOptions::default(),
        );
        let svg = String::from_utf8(svg).expect("carom SVG should be UTF-8");
        assert!(svg.contains("class=\"carom-table\""));
        assert_eq!(svg.matches("data-pocket=").count(), 0);
    }
}
