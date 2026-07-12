use std::fs;

use billiards::diagram::DiagramOutputFormat;
use billiards::dsl::{
    parse_dsl_to_scenario, ScenarioShotTrace, ScenarioShotTraceEventKind,
    ScenarioTraceRenderOptions,
};
use billiards::visualization::{BallPathRenderOptions, PathColorMode};
use billiards::{
    advance_motion_on_table, human_tuned_preview_motion_config, BallType, CollisionModel,
    DiagramBackground, DiagramRenderOptions, NBallSystemEvent, OnTableBallState, Pocket, Rail,
    RailModel, Seconds, TableKind, TYPICAL_BALL_RADIUS,
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
            .unwrap_or_else(|error| panic!("{path}: scenario should simulate: {error}"))
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
            .unwrap_or_else(|error| panic!("{path}: scenario should simulate: {error}"))
            .expect("scenario should contain a shot")
    };
    (scenario, trace)
}

#[test]
fn elevated_side_spin_examples_expose_height_and_z_spin_for_gallery_playback() {
    for (scenario_path, expected_z_sign) in [
        (
            "examples/scenarios/elevated_right_english_swerve_showcase.billiards",
            1.0,
        ),
        (
            "examples/scenarios/elevated_left_english_masse_showcase.billiards",
            -1.0,
        ),
    ] {
        let (scenario, trace) = trace_scenario(scenario_path, 18);
        let ball_set = scenario.ball_set_physics_spec();
        assert!(
            trace.event_log.iter().any(|event| {
                matches!(
                    &event.kind,
                    ScenarioShotTraceEventKind::BallTableBounce { ball }
                        if *ball == BallType::Cue
                )
            }),
            "{scenario_path}: elevated side-spin showcase should log a cue-ball table bounce"
        );

        let frames = trace.playback_frames(Seconds::new(0.02));
        let cue_states = frames
            .iter()
            .flat_map(|frame| &frame.balls)
            .filter(|ball| ball.ball == BallType::Cue)
            .map(|ball| &ball.state)
            .collect::<Vec<_>>();
        assert!(
            cue_states.iter().any(|state| state.height.as_f64() > 0.01),
            "{scenario_path}: playback should include airborne cue-ball height"
        );

        let strongest_z = cue_states
            .iter()
            .map(|state| state.angular_velocity.z().as_f64())
            .max_by(|a, b| a.abs().total_cmp(&b.abs()))
            .expect("cue-ball playback states should exist");
        assert!(
            strongest_z.abs() > 1.0,
            "{scenario_path}: playback should preserve visible z-spin, got {strongest_z:.3} rad/s"
        );
        assert_eq!(
            strongest_z.signum(),
            expected_z_sign,
            "{scenario_path}: z-spin sign should match the side tip offset"
        );

        let cue_trace = trace
            .ball_traces
            .iter()
            .find(|ball_trace| ball_trace.ball == BallType::Cue)
            .expect("cue-ball trace should exist");
        let curved_segment = cue_trace
            .timeline_segments
            .iter()
            .find(|segment| {
                let state = &segment.start;
                state.speed().as_f64() > 1.0
                    && state.height.as_f64() == 0.0
                    && state.vertical_velocity.as_f64() == 0.0
                    && expected_z_sign * state.angular_velocity.z().as_f64() > 1.0
            })
            .expect("elevated side-spin cue should have a sliding post-landing segment");
        let segment_start = OnTableBallState::try_from(curved_segment.start.clone())
            .expect("post-landing curve segment should start on the table");
        let start = segment_start.as_ball_state();
        let start_speed = start.speed().as_f64();
        let sample_dt = Seconds::new(0.03_f64.min(curved_segment.duration.as_f64() * 0.5));
        let sampled = advance_motion_on_table(
            &segment_start,
            sample_dt,
            &ball_set,
            &human_tuned_preview_motion_config(),
        )
        .state;
        let elapsed = sample_dt.as_f64();
        let linear_x = start.position.x().as_f64() + start.velocity.x().as_f64() * elapsed;
        let linear_y = start.position.y().as_f64() + start.velocity.y().as_f64() * elapsed;
        let right_x = start.velocity.y().as_f64() / start_speed;
        let right_y = -start.velocity.x().as_f64() / start_speed;
        let lateral_curve = (sampled.position.x().as_f64() - linear_x) * right_x
            + (sampled.position.y().as_f64() - linear_y) * right_y;
        assert!(
            expected_z_sign * lateral_curve > 1e-5,
            "{scenario_path}: post-landing playback sample should bend sideways; got {lateral_curve:.6}"
        );
    }
}

#[test]
fn jump_examples_clear_the_blocker_and_hit_the_target_before_landing() {
    for (scenario_path, expected_elevation_degrees) in [
        (
            "examples/scenarios/jump_over_full_ball_showcase.billiards",
            45.0,
        ),
        (
            "examples/scenarios/long_jump_over_blocker_showcase.billiards",
            32.0,
        ),
    ] {
        let (scenario, trace) = trace_scenario(scenario_path, 8);
        let table = &scenario.game_state.table_spec;
        let shot = scenario
            .shot
            .as_ref()
            .expect("jump scenario should contain a shot");
        assert!(
            (shot.shot.cue_elevation().as_degrees() - expected_elevation_degrees).abs() < 1e-9,
            "{scenario_path}: jump alias should set the requested cue elevation"
        );

        let obstacle_ball = scenario
            .game_state
            .select_ball(BallType::One)
            .expect("jump obstacle should be placed");
        let obstacle_x = table
            .diamond_to_inches(obstacle_ball.position.x.clone())
            .as_f64();
        let obstacle_y = table
            .diamond_to_inches(obstacle_ball.position.y.clone())
            .as_f64();
        let target_contact_index = trace
            .event_log
            .iter()
            .position(|event| {
                matches!(
                    &event.kind,
                    ScenarioShotTraceEventKind::AirborneBallBallCollision {
                        first_ball,
                        second_ball,
                    } if (*first_ball == BallType::Cue && *second_ball == BallType::Two)
                        || (*first_ball == BallType::Two && *second_ball == BallType::Cue)
                )
            })
            .expect("airborne cue ball should contact the target");
        let landing_index = trace
            .event_log
            .iter()
            .position(|event| {
                matches!(
                    &event.kind,
                    ScenarioShotTraceEventKind::BallTableBounce { ball }
                        if *ball == BallType::Cue
                )
            })
            .expect("resolved jump shot should eventually land");
        assert!(
            target_contact_index < landing_index,
            "{scenario_path}: the airborne target contact must precede the landing bounce"
        );

        let frames = trace.playback_frames(Seconds::new(0.005));
        let nearest_obstacle_state = frames
            .iter()
            .flat_map(|frame| &frame.balls)
            .filter(|ball| ball.ball == BallType::Cue)
            .min_by(|a, b| {
                let a_dx = a.state.position.x().as_f64() - obstacle_x;
                let a_dy = a.state.position.y().as_f64() - obstacle_y;
                let b_dx = b.state.position.x().as_f64() - obstacle_x;
                let b_dy = b.state.position.y().as_f64() - obstacle_y;
                a_dx.hypot(a_dy).total_cmp(&b_dx.hypot(b_dy))
            })
            .expect("cue-ball playback states should exist near obstacle");
        assert!(
            nearest_obstacle_state.state.height.as_f64() > 2.25,
            "{scenario_path}: cue ball should clear a full-ball obstacle; height {:.3}in",
            nearest_obstacle_state.state.height.as_f64()
        );
    }
}

#[test]
fn svg_trace_marks_original_cue_ball_origin_without_restoring_event_numbers() {
    let (scenario, trace) = trace_scenario(
        "examples/scenarios/bank_reference_track_one_rail.billiards",
        1,
    );
    let rendered = trace.rendered_final_layout_with_trace_options(
        &scenario,
        &ScenarioTraceRenderOptions {
            start_ghost_balls: true,
            event_markers: true,
            labels: false,
            ..ScenarioTraceRenderOptions::default()
        },
    );
    let svg = String::from_utf8(rendered.render_2d_diagram_with_options(
        DiagramOutputFormat::Svg,
        &DiagramRenderOptions {
            background: DiagramBackground::Transparent,
            ..DiagramRenderOptions::default()
        },
    ))
    .expect("scenario trace SVG should be UTF-8");

    assert_eq!(svg.matches("class=\"overlay origin-marker\"").count(), 1);
    assert!(svg.contains(">O</text>"));
    assert!(svg.contains("data-event-label=\"(1)\""));
    assert!(svg.contains("<title>(1) t="));
    assert!(svg.contains("cue Sliding -&gt; Rolling"));
    assert!(!svg.contains(">(1)</text>"));
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

fn has_any_pocket(trace: &ScenarioShotTrace, ball: BallType) -> bool {
    [
        Pocket::TopRight,
        Pocket::CenterRight,
        Pocket::BottomRight,
        Pocket::BottomLeft,
        Pocket::CenterLeft,
        Pocket::TopLeft,
    ]
    .into_iter()
    .any(|pocket| has_pocket(trace, ball.clone(), pocket))
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

#[derive(Debug, PartialEq)]
enum CueCaromStep {
    Object(BallType),
    Rail(Rail),
}

fn cue_carom_sequence(trace: &ScenarioShotTrace) -> Vec<CueCaromStep> {
    trace
        .event_log
        .iter()
        .filter_map(|event| match &event.kind {
            ScenarioShotTraceEventKind::BallBallCollision {
                first_ball,
                second_ball,
            } if first_ball == &BallType::Cue => Some(CueCaromStep::Object(second_ball.clone())),
            ScenarioShotTraceEventKind::BallBallCollision {
                first_ball,
                second_ball,
            } if second_ball == &BallType::Cue => Some(CueCaromStep::Object(first_ball.clone())),
            ScenarioShotTraceEventKind::BallRailImpact { ball, rail } if ball == &BallType::Cue => {
                Some(CueCaromStep::Rail(*rail))
            }
            _ => None,
        })
        .collect()
}

fn has_ball_rail_impact(trace: &ScenarioShotTrace, ball_type: BallType, rail_type: Rail) -> bool {
    trace.event_log.iter().any(|event| {
        matches!(
            &event.kind,
            ScenarioShotTraceEventKind::BallRailImpact { ball, rail }
                if ball == &ball_type && *rail == rail_type
        )
    })
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
    assert!(straight_follow.event_log.iter().any(|event| {
        matches!(
            &event.kind,
            ScenarioShotTraceEventKind::BallTableBounce { ball } if *ball == BallType::Cue
        )
    }));

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
    assert!(has_collision(&stop_shot, BallType::Cue, BallType::One));
    assert!(has_pocket(&stop_shot, BallType::One, Pocket::CenterRight));
    assert!(!has_any_pocket(&stop_shot, BallType::Cue));

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
        !has_any_pocket(&right_spin_stun, BallType::Cue),
        "right-spin stun example should leave the cue ball on the table"
    );
}

#[test]
fn low_left_spin_throw_transfer_scenario_uses_near_full_face_vertical_impact_and_tracks_transfer() {
    let (scenario, trace) = trace_scenario(
        "examples/scenarios/low_left_spin_throw_transfer.billiards",
        2,
    );
    assert!(has_collision(&trace, BallType::Cue, BallType::One));

    let shot = scenario
        .shot
        .as_ref()
        .expect("scenario should contain a shot");
    assert!(
        shot.shot.heading().as_degrees().abs() < 1e-12,
        "low-left transfer diagnostic should aim due north/up; got {:.12}deg",
        shot.shot.heading().as_degrees()
    );
    assert!(
        shot.cue_strike.cue_ball_to_endmass_ratio().as_f64() >= 1.0e8,
        "low-left transfer diagnostic should use a near-zero-deflection cue so side-tip squirt cannot fake a cut"
    );

    let cue_index = scenario
        .game_state
        .balls()
        .iter()
        .position(|ball| ball.ty == BallType::Cue)
        .expect("scenario should contain the cue ball");
    let one_index = scenario
        .game_state
        .balls()
        .iter()
        .position(|ball| ball.ty == BallType::One)
        .expect("scenario should contain the 1-ball");

    let cue_start = &scenario.game_state.balls()[cue_index].position;
    let one_start = &scenario.game_state.balls()[one_index].position;
    assert_eq!(
        cue_start.x, one_start.x,
        "low-left setup should place cue and 1-ball on the same vertical line"
    );
    assert!(
        one_start.y > cue_start.y,
        "1-ball should start directly above the cue ball toward the top cushion"
    );

    let collision_event = trace
        .simulation
        .events
        .iter()
        .find(|event| {
            matches!(
                event,
                NBallSystemEvent::BallBallCollision {
                    first_ball_index,
                    second_ball_index,
                    ..
                } if (*first_ball_index == cue_index && *second_ball_index == one_index)
                    || (*first_ball_index == one_index && *second_ball_index == cue_index)
            )
        })
        .expect("scenario should include the cue -> one collision after the launch bounce");
    let (cue_at_impact, one_at_impact) = match collision_event {
        NBallSystemEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } if *first_ball_index == cue_index && *second_ball_index == one_index => {
            (&collision.a_at_impact, &collision.b_at_impact)
        }
        NBallSystemEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } if *first_ball_index == one_index && *second_ball_index == cue_index => {
            (&collision.b_at_impact, &collision.a_at_impact)
        }
        _ => unreachable!("the filtered event is a cue-to-one collision"),
    };
    let cue_at_impact = cue_at_impact.as_ball_state();
    let one_at_impact = one_at_impact.as_ball_state();
    let line_dx = one_at_impact.position.x().as_f64() - cue_at_impact.position.x().as_f64();
    let line_dy = one_at_impact.position.y().as_f64() - cue_at_impact.position.y().as_f64();
    let ball_diameter = 2.0 * TYPICAL_BALL_RADIUS.as_f64();
    assert!(
        line_dx.abs() < 0.01,
        "massé drift should leave the low-left hit nearly full-face; got centerline dx {line_dx:.12} in"
    );
    assert!(
        (line_dx.hypot(line_dy) - ball_diameter).abs() < 1e-9,
        "collision should occur at exactly one ball diameter; got separation {:.12} in",
        line_dx.hypot(line_dy)
    );
    let impact_bearing_degrees = line_dx.atan2(line_dy).to_degrees();
    assert!(
        impact_bearing_degrees.abs() < 0.3,
        "massé drift should preserve a nearly-up line-of-centers bearing; got {impact_bearing_degrees:.12}deg"
    );
    let cue_bearing_degrees = cue_at_impact
        .velocity
        .angle_from_north()
        .expect("cue ball should still be moving at impact")
        .as_degrees();
    let cue_bearing_error = cue_bearing_degrees.min(360.0 - cue_bearing_degrees);
    assert!(
        cue_bearing_error < 0.3,
        "cue ball should arrive nearly due north after massé drift; got {cue_bearing_degrees:.12}deg"
    );

    let one_after = trace.simulation.states[one_index].as_ball_state();
    let one_vx = one_after.velocity.x().as_f64();
    let one_vy = one_after.velocity.y().as_f64();
    assert!(
        one_vy > 0.0,
        "1-ball should travel toward the top cushion after contact"
    );
    assert!(
        one_vy > one_vx.abs() * 50.0,
        "1-ball should travel almost straight toward the top cushion after a full-face hit; got vx {one_vx:.6}, vy {one_vy:.6}"
    );
    assert!(
        one_after.angular_velocity.z().as_f64() > 0.0,
        "low-left cue-ball spin should transfer a touch of opposite/right z-spin to the 1-ball; got wz {:.6}",
        one_after.angular_velocity.z().as_f64()
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
fn additional_pocket_billiards_examples_match_claimed_outcomes() {
    let (_, thin_cut) = trace_scenario("examples/scenarios/thin_cut_top_left_corner.billiards", 0);
    assert!(has_collision(&thin_cut, BallType::Cue, BallType::Seven));
    assert!(has_pocket(&thin_cut, BallType::Seven, Pocket::TopLeft));
    assert!(
        cue_rail_sequence(&thin_cut).contains(&Rail::Left),
        "thin-cut example should show the cue brushing the left rail"
    );

    let (_, long_rail_cut) =
        trace_scenario("examples/scenarios/long_cut_bottom_left_rail.billiards", 0);
    assert!(has_collision(
        &long_rail_cut,
        BallType::Cue,
        BallType::Three
    ));
    assert!(has_ball_rail_impact(
        &long_rail_cut,
        BallType::Three,
        Rail::Left
    ));
    assert!(has_pocket(
        &long_rail_cut,
        BallType::Three,
        Pocket::BottomLeft
    ));

    let (_, combo) = trace_scenario("examples/scenarios/one_nine_corner_combo.billiards", 0);
    assert!(has_collision(&combo, BallType::Cue, BallType::One));
    assert!(has_collision(&combo, BallType::One, BallType::Nine));
    assert!(has_pocket(&combo, BallType::Nine, Pocket::TopRight));
    let (_, seven_breakout) = trace_scenario(
        "examples/scenarios/seven_ball_force_follow_breakout.billiards",
        0,
    );
    assert!(has_collision(
        &seven_breakout,
        BallType::Cue,
        BallType::Seven
    ));
    assert!(has_pocket(
        &seven_breakout,
        BallType::Seven,
        Pocket::TopRight
    ));
    assert!(has_collision(
        &seven_breakout,
        BallType::Cue,
        BallType::Eight
    ));
    assert!(has_collision(
        &seven_breakout,
        BallType::Eight,
        BallType::Nine
    ));
    assert!(
        !has_any_pocket(&seven_breakout, BallType::Cue),
        "force-follow breakout should leave the cue ball on the table"
    );
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
        !has_any_pocket(&double_rail_kick, BallType::Cue),
        "double-rail kick should leave the cue ball on the table"
    );

    for (scenario_path, object_ball, claimed_pocket) in [
        (
            "examples/scenarios/hustler_frozen_rail_bank.billiards",
            BallType::Eight,
            Pocket::TopRight,
        ),
        (
            "examples/scenarios/mirror_frozen_rail_bank_top_left.billiards",
            BallType::Six,
            Pocket::TopLeft,
        ),
        (
            "examples/scenarios/frozen_rail_bank_bottom_right.billiards",
            BallType::Seven,
            Pocket::BottomRight,
        ),
    ] {
        let (_, trace) = trace_scenario(scenario_path, 0);
        assert!(
            trace.event_log.iter().any(|event| {
                matches!(
                    &event.kind,
                    ScenarioShotTraceEventKind::AirborneBallBallCollision {
                        first_ball,
                        second_ball,
                    } if (*first_ball == BallType::Cue && *second_ball == object_ball)
                        || (*first_ball == object_ball && *second_ball == BallType::Cue)
                )
            }),
            "{scenario_path}: rail-frozen elevated contact should resolve before the claimed bank"
        );
        assert!(
            has_pocket(&trace, object_ball, claimed_pocket),
            "{scenario_path}: resolved mixed contact should make the claimed source pocket"
        );
    }

    let (_, golden_break) =
        trace_scenario("examples/scenarios/golden_break_cut_break.billiards", 48);
    assert!(has_collision(&golden_break, BallType::Cue, BallType::One));
    assert!(
        !has_any_pocket(&golden_break, BallType::Eight)
            && !has_any_pocket(&golden_break, BallType::Nine),
        "golden-break spread should leave the eight and nine on the table"
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
                    ..billiards::dsl::ScenarioTraceRenderOptions::default().path_render
                },
                start_ghost_balls: true,
                event_markers: true,
                labels: true,
                spin_glyphs: true,
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
        "examples/scenarios/three_cushion_right_top_left_score.billiards",
        "examples/scenarios/three_cushion_left_top_right_score.billiards",
        "examples/scenarios/three_cushion_bottom_left_top_score.billiards",
        "examples/scenarios/three_cushion_top_right_left_score.billiards",
        "examples/scenarios/three_cushion_left_bottom_right_score.billiards",
        "examples/scenarios/three_cushion_bottom_right_top_score.billiards",
        "examples/scenarios/three_cushion_teketeke_corner_score.billiards",
        "examples/scenarios/three_cushion_double_rail_return_score.billiards",
        "examples/scenarios/three_cushion_double_rail_top_return_score.billiards",
        "examples/scenarios/three_cushion_double_rail_side_mirror_score.billiards",
        "examples/scenarios/three_cushion_stun_check_long_rail_fast_score.billiards",
        "examples/scenarios/three_cushion_stun_check_long_rail_hold_score.billiards",
        "examples/scenarios/three_cushion_stun_check_long_rail_nip_score.billiards",
        "examples/scenarios/three_cushion_three_rails_first_score.billiards",
        "examples/scenarios/three_cushion_hako_dama_long_box_behind_score.billiards",
        "examples/scenarios/three_cushion_hako_dama_short_side_check_score.billiards",
        "examples/scenarios/three_cushion_natural_angle_standard_score.billiards",
        "examples/scenarios/three_cushion_short_angle_running_score.billiards",
        "examples/scenarios/three_cushion_reverse_english_hold_score.billiards",
        "examples/scenarios/three_cushion_five_cushion_double_around_score.billiards",
        "examples/scenarios/three_cushion_two_rails_first_umbrella_score.billiards",
        "examples/scenarios/three_cushion_ticky_repeated_rail_score.billiards",
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

#[test]
fn source_backed_three_cushion_repertoire_scores_under_umb_event_order() {
    use CueCaromStep::{Object, Rail as Cushion};

    for (scenario_path, expected) in [
        (
            "examples/scenarios/three_cushion_natural_angle_standard_score.billiards",
            vec![
                Object(BallType::YellowCue),
                Cushion(Rail::Right),
                Cushion(Rail::Bottom),
                Cushion(Rail::Left),
                Object(BallType::Red),
            ],
        ),
        (
            "examples/scenarios/three_cushion_short_angle_running_score.billiards",
            vec![
                Object(BallType::YellowCue),
                Cushion(Rail::Right),
                Cushion(Rail::Top),
                Cushion(Rail::Left),
                Object(BallType::Red),
            ],
        ),
        (
            "examples/scenarios/three_cushion_reverse_english_hold_score.billiards",
            vec![
                Object(BallType::YellowCue),
                Cushion(Rail::Left),
                Cushion(Rail::Bottom),
                Cushion(Rail::Right),
                Object(BallType::Red),
            ],
        ),
        (
            "examples/scenarios/three_cushion_five_cushion_double_around_score.billiards",
            vec![
                Object(BallType::YellowCue),
                Cushion(Rail::Right),
                Cushion(Rail::Top),
                Cushion(Rail::Left),
                Cushion(Rail::Bottom),
                Cushion(Rail::Right),
                Object(BallType::Red),
            ],
        ),
        (
            "examples/scenarios/three_cushion_two_rails_first_umbrella_score.billiards",
            vec![
                Cushion(Rail::Right),
                Cushion(Rail::Top),
                Object(BallType::YellowCue),
                Cushion(Rail::Left),
                Object(BallType::Red),
            ],
        ),
        (
            "examples/scenarios/three_cushion_ticky_repeated_rail_score.billiards",
            vec![
                Cushion(Rail::Left),
                Object(BallType::YellowCue),
                Cushion(Rail::Left),
                Cushion(Rail::Bottom),
                Object(BallType::Red),
            ],
        ),
    ] {
        let (_, trace) = trace_scenario(scenario_path, 0);
        let actual = cue_carom_sequence(&trace);
        assert!(
            actual.starts_with(&expected),
            "{scenario_path}: expected scoring prefix {expected:?}, got {actual:?}"
        );
    }
}

#[test]
fn three_cushion_score_examples_preserve_planned_leading_cushion_order() {
    for (scenario_path, expected_rails) in [
        (
            "examples/scenarios/three_cushion_right_top_left_score.billiards",
            [Rail::Right, Rail::Top, Rail::Left],
        ),
        (
            "examples/scenarios/three_cushion_left_top_right_score.billiards",
            [Rail::Left, Rail::Top, Rail::Right],
        ),
        (
            "examples/scenarios/three_cushion_bottom_left_top_score.billiards",
            [Rail::Bottom, Rail::Left, Rail::Top],
        ),
        (
            "examples/scenarios/three_cushion_top_right_left_score.billiards",
            [Rail::Top, Rail::Right, Rail::Left],
        ),
        (
            "examples/scenarios/three_cushion_left_bottom_right_score.billiards",
            [Rail::Left, Rail::Bottom, Rail::Right],
        ),
        (
            "examples/scenarios/three_cushion_bottom_right_top_score.billiards",
            [Rail::Bottom, Rail::Right, Rail::Top],
        ),
        (
            "examples/scenarios/three_cushion_teketeke_corner_score.billiards",
            [Rail::Left, Rail::Left, Rail::Top],
        ),
        (
            "examples/scenarios/three_cushion_double_rail_return_score.billiards",
            [Rail::Bottom, Rail::Top, Rail::Bottom],
        ),
        (
            "examples/scenarios/three_cushion_double_rail_top_return_score.billiards",
            [Rail::Top, Rail::Bottom, Rail::Top],
        ),
        (
            "examples/scenarios/three_cushion_double_rail_side_mirror_score.billiards",
            [Rail::Bottom, Rail::Top, Rail::Bottom],
        ),
        (
            "examples/scenarios/three_cushion_stun_check_long_rail_fast_score.billiards",
            [Rail::Bottom, Rail::Top, Rail::Bottom],
        ),
        (
            "examples/scenarios/three_cushion_stun_check_long_rail_nip_score.billiards",
            [Rail::Bottom, Rail::Top, Rail::Bottom],
        ),
        (
            "examples/scenarios/three_cushion_three_rails_first_score.billiards",
            [Rail::Left, Rail::Right, Rail::Left],
        ),
        (
            "examples/scenarios/three_cushion_hako_dama_long_box_behind_score.billiards",
            [Rail::Right, Rail::Top, Rail::Left],
        ),
        (
            "examples/scenarios/three_cushion_hako_dama_short_side_check_score.billiards",
            [Rail::Left, Rail::Bottom, Rail::Right],
        ),
    ] {
        let (scenario, trace) = trace_scenario(scenario_path, 24);
        let rails = cue_rail_sequence(&trace);
        let required_rails: &[Rail] = if scenario_path.contains("three_rails_first") {
            &expected_rails[..2]
        } else {
            &expected_rails
        };
        assert!(
            rails.starts_with(required_rails),
            "{scenario_path}: expected cue rail sequence to start with {required_rails:?}, got {rails:?}"
        );
        if scenario_path.contains("double_rail") || scenario_path.contains("stun_check") {
            assert_eq!(
                expected_rails[0], expected_rails[2],
                "{scenario_path}: double-rail examples must count the first cushion again as the third rail"
            );
            let side_offset = scenario
                .shot
                .as_ref()
                .expect("double-rail scenario should include a shot")
                .shot
                .tip_contact()
                .side_offset()
                .as_f64()
                .abs();
            assert!(
                side_offset >= 0.20,
                "{scenario_path}: double-rail return needs strong check side, got {side_offset:.3}R"
            );
        }
    }
}
