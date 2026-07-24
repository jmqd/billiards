use std::fs;

use billiards::diagram::DiagramOutputFormat;
use billiards::dsl::{
    parse_dsl_to_scenario, DslScenario, ScenarioShotTrace, ScenarioShotTraceEventKind,
    ScenarioTraceRenderOptions,
};
use billiards::visualization::{BallPathRenderOptions, PathColorMode};
use billiards::{
    advance_motion_on_table, execute_shot, human_tuned_preview_motion_config,
    project_three_cushion, BallId, BallType, CaromBallRole, CollisionModel, DiagramBackground,
    DiagramRenderOptions, Inches2, NBallSystemEvent, NBallSystemState, OnTableBallState,
    OwnedShotResult, PhysicsProfile, Pocket, Rail, RailModel, ResolvedEffect, SceneBall, Seconds,
    ShotCommand, ShotLayout, ShotLimit, TableKind, ThreeCushionAdjudication, TYPICAL_BALL_RADIUS,
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

fn svg_attr_f32(element: &str, attribute: &str) -> f32 {
    let prefix = format!("{attribute}=\"");
    let start = element
        .find(&prefix)
        .unwrap_or_else(|| panic!("missing SVG attribute {attribute} in {element}"))
        + prefix.len();
    let end = element[start..]
        .find('"')
        .unwrap_or_else(|| panic!("unterminated SVG attribute {attribute} in {element}"))
        + start;
    element[start..end]
        .parse()
        .unwrap_or_else(|error| panic!("invalid SVG attribute {attribute}: {error}"))
}

fn svg_translate(element: &str) -> (f32, f32) {
    let prefix = "transform=\"translate(";
    let start = element
        .find(prefix)
        .unwrap_or_else(|| panic!("missing SVG translate transform in {element}"))
        + prefix.len();
    let end = element[start..]
        .find(")\"")
        .unwrap_or_else(|| panic!("unterminated SVG translate transform in {element}"))
        + start;
    let mut values = element[start..end].split_ascii_whitespace().map(|value| {
        value
            .parse()
            .unwrap_or_else(|error| panic!("invalid SVG translate value {value}: {error}"))
    });
    let x = values.next().expect("SVG translate should contain x");
    let y = values.next().expect("SVG translate should contain y");
    assert!(
        values.next().is_none(),
        "SVG translate should contain x and y"
    );
    (x, y)
}

fn svg_path_numbers(element: &str) -> Vec<f32> {
    let prefix = "d=\"";
    let start = element
        .find(prefix)
        .unwrap_or_else(|| panic!("missing SVG path data in {element}"))
        + prefix.len();
    let end = element[start..]
        .find('"')
        .unwrap_or_else(|| panic!("unterminated SVG path data in {element}"))
        + start;

    element[start..end]
        .split(|ch: char| ch.is_ascii_alphabetic() || ch == ',' || ch.is_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse()
                .unwrap_or_else(|error| panic!("invalid SVG path number {part}: {error}"))
        })
        .collect()
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
            .shots
            .first()
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
fn long_jump_svg_uses_bounded_dotted_airborne_paths() {
    let (scenario, trace) = trace_scenario(
        "examples/scenarios/long_jump_over_blocker_showcase.billiards",
        8,
    );
    let rendered = trace.rendered_final_layout_with_trace_options(
        &scenario,
        &ScenarioTraceRenderOptions::default(),
    );
    let svg = String::from_utf8(rendered.render_2d_diagram_with_options(
        DiagramOutputFormat::Svg,
        &DiagramRenderOptions::default(),
    ))
    .expect("jump scenario SVG should be UTF-8");
    let viewport = billiards::diagram::DiagramViewport::default();
    let airborne_lines = svg
        .lines()
        .filter(|line| line.contains("class=\"overlay dashed-line airborne-path\""))
        .collect::<Vec<_>>();
    assert!(
        !airborne_lines.is_empty(),
        "jump trace must contain a distinct airborne path"
    );
    for line in airborne_lines {
        assert!(line.contains("stroke-dasharray="));
        assert!(line.contains("clip-path=\"url(#diagram-outer-table-clip)\""));
        for (attribute, maximum) in [
            ("x1", viewport.width_px),
            ("x2", viewport.width_px),
            ("y1", viewport.height_px),
            ("y2", viewport.height_px),
        ] {
            let coordinate = svg_attr_f32(line, attribute);
            assert!(
                coordinate.is_finite() && (0.0..=maximum).contains(&coordinate),
                "{attribute}={coordinate} must remain inside 0..={maximum}"
            );
        }
    }
    assert!(svg.contains("class=\"overlay smooth-polyline\""));
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
    assert!(!svg.contains("airborne-path"));
}

#[test]
fn svg_pocketed_ball_marker_preserves_identity_scale_and_capture_time() {
    let (scenario, trace) = trace_scenario(
        "examples/scenarios/routine_nine_ball_corner_cut.billiards",
        0,
    );
    let nine_trace = trace
        .ball_traces
        .iter()
        .find(|ball_trace| ball_trace.ball == BallType::Nine)
        .expect("nine-ball trace should exist");
    let expected_capture_time = nine_trace
        .timeline_segments
        .last()
        .map(|segment| segment.start_time.as_f64() + segment.duration.as_f64())
        .expect("pocketed ball should have a terminal timeline segment");

    let rendered = trace.rendered_final_layout_with_trace_options(
        &scenario,
        &ScenarioTraceRenderOptions::rich_defaults(),
    );
    let scene = rendered.to_diagram_scene(&DiagramRenderOptions::default());
    let marker = scene
        .pocketed_balls
        .iter()
        .find(|ball| ball.ty == BallType::Nine)
        .expect("pocketed nine should remain in the diagram scene");
    assert_eq!(marker.pocket, Pocket::TopRight);
    assert!((marker.captured_at_seconds - expected_capture_time).abs() < 1e-9);

    let full_radius = scene
        .viewport
        .ball_radius_px(&scene.table_spec, &marker.spec);
    let svg = String::from_utf8(rendered.render_2d_diagram_with_options(
        DiagramOutputFormat::Svg,
        &DiagramRenderOptions::default(),
    ))
    .expect("scenario trace SVG should be UTF-8");
    let marker_start = svg
        .find("class=\"ball ball-nine pocketed-ball\"")
        .expect("pocketed nine artwork should be present");
    let marker_svg = &svg[marker_start..];
    assert!(marker_svg.starts_with(
        "class=\"ball ball-nine pocketed-ball\" data-ball=\"nine\" data-ball-style=\"stripe\" data-pocket=\"top-right\""
    ));
    assert!((svg_attr_f32(marker_svg, "data-depth-scale") - 0.5).abs() < 1e-6);
    assert!(
        (svg_attr_f32(marker_svg, "data-pocketed-at-seconds") - expected_capture_time as f32).abs()
            < 1e-5
    );
    let shell_start = marker_svg
        .find("class=\"ball-shell\"")
        .expect("pocketed nine should retain pool-ball artwork");
    let marker_radius = svg_attr_f32(&marker_svg[shell_start..], "r");
    assert!(
        (marker_radius - full_radius * 0.5).abs() < 0.001,
        "pocketed ball radius should be exactly half scale"
    );
}

#[test]
fn svg_direct_jaw_capture_shows_modeled_outgoing_direction() {
    let (scenario, trace) = trace_scenario(
        "examples/scenarios/nine_ball_three_rail_bank_side_pocket.billiards",
        0,
    );
    assert_eq!(
        final_pocket(&trace, BallType::Eight),
        Some(Pocket::CenterLeft)
    );
    assert!(
        !has_pocket(&trace, BallType::Eight, Pocket::CenterLeft),
        "the fixture must capture directly from a jaw event, not a later pocket-capture event"
    );
    let eight_trace = trace
        .ball_traces
        .iter()
        .find(|ball_trace| ball_trace.ball == BallType::Eight)
        .expect("eight-ball trace should exist");
    let NBallSystemState::Pocketed {
        state_at_capture, ..
    } = &eight_trace.final_state
    else {
        panic!("eight should finish pocketed");
    };
    let expected_heading = state_at_capture
        .as_ball_state()
        .velocity
        .angle_from_north()
        .expect("jaw response should retain an outgoing direction")
        .as_degrees();

    let rendered = trace.rendered_final_layout_with_trace_options(
        &scenario,
        &ScenarioTraceRenderOptions::rich_defaults(),
    );
    let svg = String::from_utf8(rendered.render_2d_diagram_with_options(
        DiagramOutputFormat::Svg,
        &DiagramRenderOptions::default(),
    ))
    .expect("scenario trace SVG should be UTF-8");
    let marker_start = svg
        .find("class=\"ball ball-eight pocketed-ball\"")
        .expect("side-pocketed eight artwork should be present");
    let marker_svg = &svg[marker_start..];
    assert!(marker_svg.starts_with(
        "class=\"ball ball-eight pocketed-ball\" data-ball=\"eight\" data-ball-style=\"solid\" data-pocket=\"center-left\""
    ));
    let (marker_x, marker_y) = svg_translate(marker_svg);
    let shell_start = marker_svg
        .find("class=\"ball-shell\"")
        .expect("side-pocketed eight should retain ball artwork");
    let marker_radius = svg_attr_f32(&marker_svg[shell_start..], "r");
    let viewport = rendered
        .to_diagram_scene(&DiagramRenderOptions::default())
        .viewport;
    let side_well = svg
        .lines()
        .filter(|line| line.contains("class=\"table-pocket-well\" data-pocket=\"side\""))
        .map(svg_path_numbers)
        .find(|points| (points[0] - viewport.playfield_left_px).abs() < 0.001)
        .expect("center-left black side-pocket well should be present");
    let (well_min_x, well_max_x, well_min_y, well_max_y) = side_well.chunks_exact(2).fold(
        (
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ),
        |(min_x, max_x, min_y, max_y), point| {
            (
                min_x.min(point[0]),
                max_x.max(point[0]),
                min_y.min(point[1]),
                max_y.max(point[1]),
            )
        },
    );
    assert!(
        marker_x - marker_radius > well_min_x
            && marker_x + marker_radius < viewport.playfield_left_px.min(well_max_x)
            && marker_y - marker_radius > well_min_y
            && marker_y + marker_radius < well_max_y,
        "center-left marker ({marker_x}, {marker_y}) r={marker_radius} should remain inside the black well bounds x={well_min_x}..{well_max_x}, y={well_min_y}..{well_max_y}"
    );
    let expected_center_y = (viewport.playfield_top_px + viewport.playfield_bottom_px) * 0.5;
    assert!(
        (marker_y - expected_center_y).abs() < 0.001,
        "center-left marker y={marker_y} should be centered in the side-pocket well"
    );
    let direction_start = svg
        .find("class=\"overlay jaw-rebound-direction\"")
        .expect("direct jaw capture should expose its modeled outgoing direction");
    let direction_svg = &svg[direction_start..];
    assert!(direction_svg.starts_with(
        "class=\"overlay jaw-rebound-direction\" data-ball=\"eight\" data-pocket=\"center-left\" data-jaw=\"jaw-1\""
    ));
    assert!(
        (f64::from(svg_attr_f32(direction_svg, "data-heading-deg")) - expected_heading).abs()
            < 0.001
    );
    assert!(direction_svg.contains("class=\"jaw-rebound-direction-halo\""));
    assert!(direction_svg.contains("class=\"jaw-rebound-direction-line\""));
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

fn ball_rail_sequence(trace: &ScenarioShotTrace, ball_type: BallType) -> Vec<Rail> {
    trace
        .event_log
        .iter()
        .filter_map(|event| match &event.kind {
            ScenarioShotTraceEventKind::BallRailImpact { ball, rail } if ball == &ball_type => {
                Some(*rail)
            }
            _ => None,
        })
        .collect()
}

fn final_pocket(trace: &ScenarioShotTrace, ball_type: BallType) -> Option<Pocket> {
    trace
        .ball_traces
        .iter()
        .find(|ball_trace| ball_trace.ball == ball_type)
        .and_then(|ball_trace| match &ball_trace.final_state {
            NBallSystemState::Pocketed { pocket, .. } => Some(*pocket),
            NBallSystemState::OnTable(_) | NBallSystemState::Airborne(_) => None,
        })
}

fn final_position_inches(trace: &ScenarioShotTrace, ball_type: BallType) -> (f64, f64) {
    let state = trace
        .ball_traces
        .iter()
        .find(|ball_trace| ball_trace.ball == ball_type)
        .unwrap_or_else(|| panic!("{ball_type:?} trace should exist"))
        .final_state
        .as_ball_state();
    (state.position.x().as_f64(), state.position.y().as_f64())
}

fn final_spread_inches(trace: &ScenarioShotTrace) -> f64 {
    let positions = [BallType::Cue, BallType::YellowCue, BallType::Red]
        .map(|ball| final_position_inches(trace, ball));
    let mut spread: f64 = 0.0;
    for first in 0..positions.len() {
        for second in first + 1..positions.len() {
            spread = spread.max(
                (positions[first].0 - positions[second].0)
                    .hypot(positions[first].1 - positions[second].1),
            );
        }
    }
    spread
}

fn angle_between_degrees(first: (f64, f64), second: (f64, f64)) -> f64 {
    let first_length = first.0.hypot(first.1);
    let second_length = second.0.hypot(second.1);
    let cosine = ((first.0 * second.0 + first.1 * second.1) / (first_length * second_length))
        .clamp(-1.0, 1.0);
    cosine.acos().to_degrees()
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
        .shots
        .first()
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
        "examples/scenarios/three_cushion_reverse_the_corner_score.billiards",
        "examples/scenarios/three_cushion_kiss_back_score.billiards",
        "examples/scenarios/three_cushion_gather_control_score.billiards",
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

fn three_cushion_physics_profile(scenario: &DslScenario) -> Result<PhysicsProfile, String> {
    let simulation_name = scenario
        .preferred_simulation_name()
        .ok_or_else(|| "scenario has no unambiguous preferred simulation".to_string())?;
    let simulation = scenario
        .simulation_named(simulation_name)
        .map_err(|error| format!("preferred simulation is invalid: {error}"))?;
    let conditions = &simulation.conditions;
    let collision = scenario
        .ball_ball_config_named(&simulation.ball_ball_name)
        .map_err(|error| format!("preferred ball-ball configuration is invalid: {error}"))?
        .applying_conditions(conditions);
    let rails = scenario
        .rail_profile_named(&simulation.rails_name)
        .map_err(|error| format!("preferred rail configuration is invalid: {error}"))?
        .applying_conditions(conditions);

    PhysicsProfile::new(
        scenario.game_state.table_spec.clone(),
        scenario.ball_set_physics_spec(),
        human_tuned_preview_motion_config().applying_conditions(conditions),
        simulation.collision_model,
        collision,
        simulation.rail_model,
        rails,
    )
    .map_err(|error| format!("typed physics profile is invalid: {error}"))
}

fn three_cushion_ball_identity(ball: &BallType) -> Result<(BallId, CaromBallRole), String> {
    match ball {
        BallType::Cue => Ok((BallId::WHITE, CaromBallRole::Cue)),
        BallType::YellowCue => Ok((BallId::YELLOW, CaromBallRole::YellowCue)),
        BallType::Red => Ok((BallId::RED, CaromBallRole::Red)),
        other => Err(format!("non-carom ball in three-cushion layout: {other:?}")),
    }
}

fn three_cushion_shot_layout(
    scenario: &DslScenario,
    physics: &PhysicsProfile,
) -> Result<ShotLayout, String> {
    let table = &scenario.game_state.table_spec;
    let balls = scenario
        .game_state
        .balls()
        .iter()
        .map(|ball| {
            let (id, role) = three_cushion_ball_identity(&ball.ty)?;
            let position = Inches2::new(
                table.diamond_to_inches(ball.position.x.clone()),
                table.diamond_to_inches(ball.position.y.clone()),
            );
            Ok(SceneBall::resting(id, role, position))
        })
        .collect::<Result<Vec<_>, String>>()?;

    ShotLayout::new(physics, balls)
        .map_err(|error| format!("typed three-cushion layout is invalid: {error}"))
}

fn three_cushion_shot_command(scenario: &DslScenario) -> Result<ShotCommand, String> {
    let shot = scenario
        .shots
        .first()
        .ok_or_else(|| "scenario has no shot".to_string())?;
    let (cue_ball, _) = three_cushion_ball_identity(&shot.ball)?;
    ShotCommand::new(cue_ball, shot.shot.clone(), shot.cue_strike.clone())
        .map_err(|error| format!("typed shot command is invalid: {error}"))
}

fn three_cushion_shot_limit(scenario: &DslScenario) -> Result<ShotLimit, String> {
    let simulation_name = scenario
        .preferred_simulation_name()
        .ok_or_else(|| "scenario has no unambiguous preferred simulation".to_string())?;
    let simulation = scenario
        .simulation_named(simulation_name)
        .map_err(|error| format!("preferred simulation is invalid: {error}"))?;
    Ok(simulation
        .max_events
        .or(scenario.trace_max_events)
        .map_or(ShotLimit::UntilSettled, ShotLimit::EventCount))
}

fn three_cushion_event_evidence(result: &OwnedShotResult) -> String {
    let evidence = result
        .events
        .iter()
        .enumerate()
        .filter_map(|(event_index, event)| {
            let effects = event
                .effects
                .iter()
                .filter(|effect| {
                    matches!(
                        effect,
                        ResolvedEffect::BallBallContact { first, second, .. }
                            if *first == result.roles.cue || *second == result.roles.cue
                    ) || matches!(
                        effect,
                        ResolvedEffect::BallRailContact { ball, .. }
                            if *ball == result.roles.cue
                    ) || matches!(effect, ResolvedEffect::UnsupportedContact { .. })
                })
                .collect::<Vec<_>>();
            (!effects.is_empty())
                .then(|| format!("event {event_index} at {:?}: {effects:?}", event.at))
        })
        .collect::<Vec<_>>();

    if evidence.is_empty() {
        "<no cue-ball contact evidence>".to_string()
    } else {
        evidence.join("\n")
    }
}

#[test]
fn every_three_cushion_score_scenario_scores_under_umb_article_83() {
    let mut scenario_paths = fs::read_dir("examples/scenarios")
        .expect("scenario directory should read")
        .map(|entry| entry.expect("scenario directory entry should read").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("three_cushion") && name.ends_with("_score.billiards")
                })
        })
        .collect::<Vec<_>>();
    scenario_paths.sort();
    assert!(
        !scenario_paths.is_empty(),
        "expected at least one three-cushion score scenario"
    );

    let mut failures = Vec::new();
    for scenario_path in scenario_paths {
        let path = scenario_path.display().to_string();
        let outcome = (|| -> Result<(OwnedShotResult, ThreeCushionAdjudication), String> {
            let source = fs::read_to_string(&scenario_path)
                .map_err(|error| format!("scenario should read: {error}"))?;
            let mut scenario = parse_dsl_to_scenario(&source)
                .map_err(|error| format!("scenario should parse: {error}"))?;
            scenario.game_state.resolve_positions();
            let physics = three_cushion_physics_profile(&scenario)?;
            let layout = three_cushion_shot_layout(&scenario, &physics)?;
            let command = three_cushion_shot_command(&scenario)?;
            let limit = three_cushion_shot_limit(&scenario)?;
            let result = execute_shot(&physics, &layout, &command, limit)
                .map_err(|error| format!("typed shot should execute: {error}"))?;
            let adjudication = project_three_cushion(&result);
            Ok((result, adjudication))
        })();

        match outcome {
            Ok((result, adjudication)) if !adjudication.is_scored() => failures.push(format!(
                "{path}\nadjudication: {adjudication:#?}\ntermination: {:?}\nevent evidence:\n{}",
                result.termination,
                three_cushion_event_evidence(&result)
            )),
            Err(error) => failures.push(format!("{path}\nadapter/execution error: {error}")),
            Ok(_) => {}
        }
    }

    assert!(
        failures.is_empty(),
        "{} three-cushion score scenario(s) did not score:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn recognized_three_cushion_reverse_the_corner_returns_to_same_side_rail_before_scoring() {
    let scenario_path = "examples/scenarios/three_cushion_reverse_the_corner_score.billiards";
    let (_, trace) = trace_scenario(scenario_path, 0);
    let route = cue_carom_sequence(&trace);
    assert!(
        route.starts_with(&[
            CueCaromStep::Object(BallType::YellowCue),
            CueCaromStep::Rail(Rail::Right),
            CueCaromStep::Rail(Rail::Top),
            CueCaromStep::Rail(Rail::Right),
            CueCaromStep::Object(BallType::Red),
        ]),
        "{scenario_path}: expected yellow, right/top/right, then red; got {route:?}"
    );
}

#[test]
fn recognized_three_cushion_kiss_back_recontacts_first_object_before_scoring() {
    let scenario_path = "examples/scenarios/three_cushion_kiss_back_score.billiards";
    let (_, trace) = trace_scenario(scenario_path, 0);
    let yellow_contacts = trace
        .event_log
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            matches!(
                &event.kind,
                ScenarioShotTraceEventKind::BallBallCollision {
                    first_ball,
                    second_ball,
                } if (first_ball == &BallType::Cue && second_ball == &BallType::YellowCue)
                    || (first_ball == &BallType::YellowCue && second_ball == &BallType::Cue)
            )
            .then_some(index)
        })
        .collect::<Vec<_>>();
    assert!(
        yellow_contacts.len() >= 2,
        "{scenario_path}: expected an intentional second cue-yellow contact"
    );
    assert!(
        trace.event_log[yellow_contacts[0] + 1..yellow_contacts[1]]
            .iter()
            .any(|event| matches!(
                &event.kind,
                ScenarioShotTraceEventKind::BallRailImpact { ball, rail }
                    if ball == &BallType::YellowCue && *rail == Rail::Left
            )),
        "{scenario_path}: yellow should rebound from the left rail before the kiss-back"
    );

    let route = cue_carom_sequence(&trace);
    assert!(
        route.starts_with(&[
            CueCaromStep::Object(BallType::YellowCue),
            CueCaromStep::Object(BallType::YellowCue),
            CueCaromStep::Rail(Rail::Right),
            CueCaromStep::Rail(Rail::Top),
            CueCaromStep::Rail(Rail::Left),
            CueCaromStep::Object(BallType::Red),
        ]),
        "{scenario_path}: expected yellow/kiss-back, right/top/left, then red; got {route:?}"
    );
}

#[test]
fn recognized_three_cushion_gather_control_leaves_balls_within_16_3_inches() {
    let scenario_path = "examples/scenarios/three_cushion_gather_control_score.billiards";
    let (_, trace) = trace_scenario(scenario_path, 0);
    let route = cue_carom_sequence(&trace);
    assert!(
        route.starts_with(&[
            CueCaromStep::Object(BallType::YellowCue),
            CueCaromStep::Rail(Rail::Top),
            CueCaromStep::Rail(Rail::Right),
            CueCaromStep::Rail(Rail::Left),
            CueCaromStep::Object(BallType::Red),
        ]),
        "{scenario_path}: expected yellow, top/right/left, then red; got {route:?}"
    );
    let spread = final_spread_inches(&trace);
    assert!(
        spread <= 16.3,
        "{scenario_path}: expected a <=16.3 in final leave, got {spread:.6} in"
    );
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
        (
            "examples/scenarios/three_cushion_reverse_the_corner_score.billiards",
            [Rail::Right, Rail::Top, Rail::Right],
        ),
        (
            "examples/scenarios/three_cushion_kiss_back_score.billiards",
            [Rail::Right, Rail::Top, Rail::Left],
        ),
        (
            "examples/scenarios/three_cushion_gather_control_score.billiards",
            [Rail::Top, Rail::Right, Rail::Left],
        ),
    ] {
        let (_, trace) = trace_scenario(scenario_path, 24);
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
        }
    }
}

#[test]
fn advanced_two_rail_kick_routes_around_blockers_and_pockets_the_legal_ball() {
    let (_, trace) = trace_scenario(
        "examples/scenarios/nine_ball_two_rail_kick_side_pocket.billiards",
        0,
    );
    let route = cue_carom_sequence(&trace);
    assert!(
        route.starts_with(&[
            CueCaromStep::Rail(Rail::Right),
            CueCaromStep::Rail(Rail::Top),
            CueCaromStep::Object(BallType::Five),
        ]),
        "two-rail kick should contact right, top, then the legal 5; got {route:?}"
    );
    assert!(has_pocket(&trace, BallType::Five, Pocket::CenterLeft));
    assert!(!has_collision(&trace, BallType::Cue, BallType::Six));
    assert!(!has_collision(&trace, BallType::Cue, BallType::Eight));
    assert!(!has_any_pocket(&trace, BallType::Cue));
}

#[test]
fn advanced_three_rail_bank_pockets_the_eight_after_bottom_right_top() {
    let (_, trace) = trace_scenario(
        "examples/scenarios/nine_ball_three_rail_bank_side_pocket.billiards",
        0,
    );
    assert!(
        cue_carom_sequence(&trace).starts_with(&[CueCaromStep::Object(BallType::Eight)]),
        "three-rail bank should contact the legal 8 first"
    );
    let rails = ball_rail_sequence(&trace, BallType::Eight);
    assert!(
        rails.starts_with(&[Rail::Bottom, Rail::Right, Rail::Top]),
        "8 should bank bottom-right-top before entering the side; got {rails:?}"
    );
    assert_eq!(
        final_pocket(&trace, BallType::Eight),
        Some(Pocket::CenterLeft)
    );
    assert!(!has_collision(&trace, BallType::Eight, BallType::Nine));
}

#[test]
fn advanced_rail_first_safety_hides_the_cue_behind_the_eight() {
    let (_, trace) = trace_scenario(
        "examples/scenarios/nine_ball_rail_first_hide_safety.billiards",
        0,
    );
    let route = cue_carom_sequence(&trace);
    assert!(
        route.starts_with(&[
            CueCaromStep::Rail(Rail::Right),
            CueCaromStep::Object(BallType::Six),
        ]),
        "safety should contact a cushion before the legal 6; got {route:?}"
    );
    for ball in [BallType::Cue, BallType::Six, BallType::Eight] {
        assert!(
            !has_any_pocket(&trace, ball.clone()),
            "{ball:?} should remain on the table"
        );
    }
    assert!(!has_collision(&trace, BallType::Cue, BallType::Eight));

    let cue = final_position_inches(&trace, BallType::Cue);
    let object = final_position_inches(&trace, BallType::Six);
    let blocker = final_position_inches(&trace, BallType::Eight);
    let line = (object.0 - cue.0, object.1 - cue.1);
    let cue_to_blocker = (blocker.0 - cue.0, blocker.1 - cue.1);
    let line_length_squared = line.0 * line.0 + line.1 * line.1;
    let blocker_projection =
        (cue_to_blocker.0 * line.0 + cue_to_blocker.1 * line.1) / line_length_squared;
    let closest = (
        cue.0 + blocker_projection * line.0,
        cue.1 + blocker_projection * line.1,
    );
    let blocker_distance = (blocker.0 - closest.0).hypot(blocker.1 - closest.1);
    assert!(
        (0.0..=1.0).contains(&blocker_projection)
            && blocker_distance < 2.0 * TYPICAL_BALL_RADIUS.as_f64(),
        "8 should geometrically block the final cue-to-6 line; projection={blocker_projection:.3}, distance={blocker_distance:.3} in"
    );
}

#[test]
fn advanced_z_route_pockets_the_eight_and_finishes_on_the_nine_line() {
    let (scenario, trace) = trace_scenario(
        "examples/scenarios/nine_ball_two_rail_z_position.billiards",
        0,
    );
    assert!(has_collision(&trace, BallType::Cue, BallType::Eight));
    assert!(has_pocket(&trace, BallType::Eight, Pocket::TopRight));
    let rails = cue_rail_sequence(&trace);
    assert!(
        rails.starts_with(&[Rail::Right, Rail::Left]),
        "Z route should cross the table from right to left; got {rails:?}"
    );
    let cue_final_state = &trace
        .ball_traces
        .iter()
        .find(|ball_trace| ball_trace.ball == BallType::Cue)
        .expect("cue trace should exist")
        .final_state;
    assert!(
        matches!(cue_final_state, NBallSystemState::OnTable(_)),
        "Z route cue should finish on the table, got {cue_final_state:?}"
    );

    let cue = final_position_inches(&trace, BallType::Cue);
    let nine = final_position_inches(&trace, BallType::Nine);
    let target = Pocket::TopLeft.aiming_center();
    let target = (
        scenario
            .game_state
            .table_spec
            .diamond_to_inches(target.x.clone())
            .as_f64(),
        scenario
            .game_state
            .table_spec
            .diamond_to_inches(target.y)
            .as_f64(),
    );
    let position_error = angle_between_degrees(
        (nine.0 - cue.0, nine.1 - cue.1),
        (target.0 - nine.0, target.1 - nine.1),
    );
    assert!(
        position_error < 2.0,
        "cue should finish on the 9-to-top-left position line; angular error={position_error:.3}°"
    );
}

#[test]
fn advanced_jump_clears_the_blocker_pockets_the_six_and_lands() {
    let (_, trace) = trace_scenario(
        "examples/scenarios/nine_ball_jump_over_blocker_top_right.billiards",
        0,
    );
    let airborne_contact = trace
        .event_log
        .iter()
        .position(|event| {
            matches!(
                &event.kind,
                ScenarioShotTraceEventKind::AirborneBallBallCollision {
                    first_ball,
                    second_ball,
                } if (first_ball == &BallType::Cue && second_ball == &BallType::Six)
                    || (first_ball == &BallType::Six && second_ball == &BallType::Cue)
            )
        })
        .expect("jump should contact the legal 6 while airborne");
    assert!(
        !trace.event_log.iter().any(|event| {
            matches!(
                &event.kind,
                ScenarioShotTraceEventKind::BallBallCollision {
                    first_ball,
                    second_ball,
                } | ScenarioShotTraceEventKind::AirborneBallBallCollision {
                    first_ball,
                    second_ball,
                } if (first_ball == &BallType::Cue && second_ball == &BallType::Eight)
                    || (first_ball == &BallType::Eight && second_ball == &BallType::Cue)
            )
        }),
        "jump should clear the blocking 8"
    );
    assert!(has_pocket(&trace, BallType::Six, Pocket::TopRight));
    assert!(
        trace
            .event_log
            .iter()
            .skip(airborne_contact + 1)
            .any(|event| matches!(
                &event.kind,
                ScenarioShotTraceEventKind::BallTableBounce { ball }
                    if ball == &BallType::Cue
            )),
        "cue should return to the cloth after the airborne object-ball contact"
    );
    let cue_final_state = &trace
        .ball_traces
        .iter()
        .find(|ball_trace| ball_trace.ball == BallType::Cue)
        .expect("cue trace should exist")
        .final_state;
    assert!(
        matches!(cue_final_state, NBallSystemState::OnTable(_)),
        "jump cue should finish on the table after landing, got {cue_final_state:?}"
    );
}

#[test]
fn advanced_stun_carom_contacts_one_then_nine_and_wins_the_rack() {
    let (_, trace) = trace_scenario(
        "examples/scenarios/nine_ball_stun_carom_nine_top_right.billiards",
        0,
    );
    let route = cue_carom_sequence(&trace);
    assert!(
        route.starts_with(&[
            CueCaromStep::Object(BallType::One),
            CueCaromStep::Object(BallType::Nine),
        ]),
        "stun carom should contact the legal 1 then the 9 without an intervening rail; got {route:?}"
    );
    assert!(has_pocket(&trace, BallType::Nine, Pocket::TopRight));
    assert!(!has_any_pocket(&trace, BallType::Cue));
}
