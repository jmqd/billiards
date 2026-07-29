use std::fmt::Write as _;

use crate::diagram::{ball_visual, BallStyle, DiagramViewport};
use crate::dsl::{
    parse_dsl_to_scenario, ScenarioBallTrace, ScenarioPlaybackBall, ScenarioPlaybackFrame,
    ScenarioShotTrace, ScenarioShotTraceEvent, ScenarioTraceRenderOptions,
};
use crate::visualization::{PathColorMode, DEFAULT_BALL_PATH_MAX_TIME_STEP_SECONDS};
use crate::{
    human_tuned_preview_motion_config, Angle, BallState, CollisionModel, CutAngle,
    DiagramBackground, DiagramRenderOptions, NBallSystemEvent, RailModel, Seconds, TableKind,
    TableSpec,
};

#[derive(Clone, Debug, PartialEq)]
pub struct SvgGeneratorOptions {
    pub background: DiagramBackground,
    pub trace_sample_step_seconds: f64,
    pub trace_max_events: Option<usize>,
    pub start_ghost_balls: bool,
    pub event_markers: bool,
    pub spin_glyphs: bool,
    pub path_color_mode: PathColorMode,
}

impl Default for SvgGeneratorOptions {
    fn default() -> Self {
        Self {
            background: DiagramBackground::Table,
            trace_sample_step_seconds: DEFAULT_BALL_PATH_MAX_TIME_STEP_SECONDS,
            trace_max_events: None,
            start_ghost_balls: true,
            event_markers: true,
            spin_glyphs: true,
            path_color_mode: PathColorMode::Solid,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScenarioPlaybackReport {
    pub duration: f64,
    pub events: Vec<ScenarioPlaybackEventReport>,
    pub balls: Vec<ScenarioPlaybackBallVisual>,
    pub frames: Vec<ScenarioPlaybackFrameReport>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScenarioPlaybackEventReport {
    pub label: String,
    pub time: f64,
    pub summary: String,
}

/// Ball artwork metadata serialized as the canonical compact tuple
/// `[id, fill, label, radius, radius_inches, style, paint, gradient]`.
/// Pool balls carry paint and gradient identities; carom balls use `None` for both.
#[derive(Clone, Debug, PartialEq)]
pub struct ScenarioPlaybackBallVisual {
    pub id: &'static str,
    pub fill: &'static str,
    pub label: Option<&'static str>,
    pub radius: f32,
    pub radius_inches: f64,
    pub style: BallStyle,
    pub paint: Option<&'static str>,
    pub gradient: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScenarioPlaybackFrameReport {
    pub time: f64,
    pub balls: Vec<ScenarioPlaybackBallReport>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScenarioPlaybackBallReport {
    pub id: &'static str,
    pub x: f32,
    pub y: f32,
    pub height_inches: f64,
    pub vx_ips: f64,
    pub vy_ips: f64,
    pub vz_ips: f64,
    pub wx_rps: f64,
    pub wy_rps: f64,
    pub wz_rps: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FirstObjectContactReport {
    pub time: f64,
    pub cue_ball: ScenarioPlaybackBallVisual,
    pub object_ball: ScenarioPlaybackBallVisual,
    pub cut_angle_degrees: f64,
    pub hit_fraction: f64,
    pub lateral_offset_diameters: f64,
    pub forward_offset_diameters: f64,
    pub vertical_offset_diameters: f64,
    pub airborne: bool,
}

struct RenderedSvgScenario {
    svg: String,
    trace: Option<ScenarioShotTrace>,
    table_spec: TableSpec,
}

pub fn render_svg_from_dsl(source: &str) -> Result<String, String> {
    render_svg_from_dsl_with_options(source, &SvgGeneratorOptions::default())
}

pub fn render_svg_from_dsl_with_options(
    source: &str,
    options: &SvgGeneratorOptions,
) -> Result<String, String> {
    Ok(rendered_svg_scenario(source, options)?.svg)
}

pub fn render_svg_report_from_dsl(source: &str) -> Result<String, String> {
    render_svg_report_from_dsl_with_options(source, &SvgGeneratorOptions::default())
}

pub fn render_svg_report_from_dsl_with_options(
    source: &str,
    options: &SvgGeneratorOptions,
) -> Result<String, String> {
    let rendered = rendered_svg_scenario(source, options)?;
    let mut json = String::new();
    serialize_prepared_svg_report(
        &mut json,
        &rendered.svg,
        rendered.trace.as_ref(),
        &rendered.table_spec,
        Seconds::new(options.trace_sample_step_seconds),
    );
    Ok(json)
}

pub fn serialize_prepared_svg_report(
    json: &mut String,
    svg: &str,
    trace: Option<&ScenarioShotTrace>,
    table_spec: &TableSpec,
    max_time_step: Seconds,
) {
    json.push_str("{\"svg\":");
    push_json_string(json, svg);
    json.push_str(",\"events\":[");
    if let Some(trace) = trace {
        for (index, event) in trace.event_log.iter().enumerate() {
            if index > 0 {
                json.push(',');
            }
            let label = format!("({})", index + 1);
            json.push('[');
            push_json_string(json, &label);
            write!(json, ",{:.6},", event.time.as_f64())
                .expect("writing JSON to string should not fail");
            push_json_string(json, &event.kind.format_human());
            json.push(',');
            push_json_string(json, &event.format_human());
            json.push(']');
        }
    }
    json.push_str("],\"firstObjectContact\":");
    if let Some(contact) =
        trace.and_then(|trace| build_first_object_contact_report(trace, table_spec))
    {
        serialize_first_object_contact_report(json, &contact);
    } else {
        json.push_str("null");
    }
    json.push_str(",\"playback\":");
    if let Some(trace) = trace {
        serialize_scenario_playback_trace(json, trace, table_spec, max_time_step);
    } else {
        json.push_str("null");
    }
    json.push('}');
}

pub fn build_first_object_contact_report(
    trace: &ScenarioShotTrace,
    table_spec: &TableSpec,
) -> Option<FirstObjectContactReport> {
    let ball_spec = table_spec.default_ball_spec();
    let ball_diameter_inches = 2.0 * ball_spec.radius.as_f64();
    if ball_diameter_inches <= f64::EPSILON {
        return None;
    }

    let viewport = DiagramViewport::default();
    let ball_radius = viewport.ball_radius_px(table_spec, &ball_spec);
    let pool_artwork = table_spec.kind == TableKind::Pool;
    let mut event_offset = 0;

    for execution in &trace.shot_executions {
        let cue_ball_index = trace
            .ball_traces
            .iter()
            .position(|ball_trace| ball_trace.ball == execution.shot.ball)?;
        let mut event_time = execution.start_time.as_f64();

        for (event_index, event) in execution.simulation.events.iter().enumerate() {
            event_time += event.time().as_f64();
            let Some((object_ball_index, cue_state, object_state, airborne)) =
                first_object_contact_states(
                    trace,
                    event,
                    event_offset + event_index,
                    cue_ball_index,
                )
            else {
                continue;
            };
            let cue_ball_trace = trace.ball_traces.get(cue_ball_index)?;
            let object_ball_trace = trace.ball_traces.get(object_ball_index)?;
            if object_ball_trace.ball == execution.shot.ball {
                continue;
            }

            let cue_speed = cue_state.velocity.speed().as_f64();
            if cue_speed <= f64::EPSILON {
                return None;
            }
            let forward_x = cue_state.velocity.x().as_f64() / cue_speed;
            let forward_y = cue_state.velocity.y().as_f64() / cue_speed;
            let right_x = forward_y;
            let right_y = -forward_x;
            let delta_x = object_state.position.x().as_f64() - cue_state.position.x().as_f64();
            let delta_y = object_state.position.y().as_f64() - cue_state.position.y().as_f64();
            if delta_x.hypot(delta_y) <= f64::EPSILON {
                return None;
            }
            let cue_heading = cue_state.velocity.angle_from_north()?;
            let line_of_centers_heading = Angle::from_north(delta_x, delta_y);
            let cut_angle = CutAngle::from_headings(cue_heading, line_of_centers_heading);
            let hit_fraction = 1.0 - cut_angle.as_degrees().to_radians().sin();

            return Some(FirstObjectContactReport {
                time: event_time,
                cue_ball: scenario_playback_ball_visual(
                    cue_ball_trace,
                    ball_radius,
                    ball_spec.radius.as_f64(),
                    pool_artwork,
                ),
                object_ball: scenario_playback_ball_visual(
                    object_ball_trace,
                    ball_radius,
                    ball_spec.radius.as_f64(),
                    pool_artwork,
                ),
                cut_angle_degrees: cut_angle.as_degrees(),
                hit_fraction: hit_fraction.clamp(0.0, 1.0),
                lateral_offset_diameters: (delta_x * right_x + delta_y * right_y)
                    / ball_diameter_inches,
                forward_offset_diameters: (delta_x * forward_x + delta_y * forward_y)
                    / ball_diameter_inches,
                vertical_offset_diameters: (object_state.height.as_f64()
                    - cue_state.height.as_f64())
                    / ball_diameter_inches,
                airborne,
            });
        }

        event_offset += execution.simulation.events.len();
    }

    None
}

fn first_object_contact_states<'a>(
    trace: &'a ScenarioShotTrace,
    event: &'a NBallSystemEvent,
    event_index: usize,
    cue_ball_index: usize,
) -> Option<(usize, &'a BallState, &'a BallState, bool)> {
    match event {
        NBallSystemEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } if *first_ball_index == cue_ball_index => Some((
            *second_ball_index,
            collision.a_at_impact.as_ball_state(),
            collision.b_at_impact.as_ball_state(),
            false,
        )),
        NBallSystemEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } if *second_ball_index == cue_ball_index => Some((
            *first_ball_index,
            collision.b_at_impact.as_ball_state(),
            collision.a_at_impact.as_ball_state(),
            false,
        )),
        NBallSystemEvent::AirborneBallBallCollision {
            first_ball_index,
            second_ball_index,
            contact,
        } if *first_ball_index == cue_ball_index => Some((
            *second_ball_index,
            &contact.first_at_contact,
            &contact.second_at_contact,
            true,
        )),
        NBallSystemEvent::AirborneBallBallCollision {
            first_ball_index,
            second_ball_index,
            contact,
        } if *second_ball_index == cue_ball_index => Some((
            *first_ball_index,
            &contact.second_at_contact,
            &contact.first_at_contact,
            true,
        )),
        NBallSystemEvent::SharedBallBallContact {
            ball_indices,
            ball_ball_pairs,
            ..
        } if ball_indices.contains(&cue_ball_index) => {
            let cue_ball = &trace.ball_traces.get(cue_ball_index)?.ball;
            let object_ball_index = ball_ball_pairs
                .iter()
                .filter_map(|(first_ball_index, second_ball_index)| {
                    if *first_ball_index == cue_ball_index {
                        Some(*second_ball_index)
                    } else if *second_ball_index == cue_ball_index {
                        Some(*first_ball_index)
                    } else {
                        None
                    }
                })
                .filter(|object_ball_index| {
                    trace
                        .ball_traces
                        .get(*object_ball_index)
                        .is_some_and(|ball_trace| &ball_trace.ball != cue_ball)
                })
                .min()?;
            let cue_state = &trace
                .ball_traces
                .get(cue_ball_index)?
                .timeline_segments
                .get(event_index)?
                .end;
            let object_state = &trace
                .ball_traces
                .get(object_ball_index)?
                .timeline_segments
                .get(event_index)?
                .end;
            Some((object_ball_index, cue_state, object_state, false))
        }
        _ => None,
    }
}

pub fn serialize_first_object_contact_report(
    json: &mut String,
    contact: &FirstObjectContactReport,
) {
    write!(json, "{{\"time\":{:.6},\"cueBall\":", contact.time)
        .expect("writing JSON to string should not fail");
    push_playback_ball_visual(json, &contact.cue_ball);
    json.push_str(",\"objectBall\":");
    push_playback_ball_visual(json, &contact.object_ball);
    write!(
        json,
        ",\"cutAngleDegrees\":{:.6},\"hitFraction\":{:.6},\"lateralOffsetDiameters\":{:.6},\"forwardOffsetDiameters\":{:.6},\"verticalOffsetDiameters\":{:.6},\"airborne\":{}}}",
        contact.cut_angle_degrees,
        contact.hit_fraction,
        contact.lateral_offset_diameters,
        contact.forward_offset_diameters,
        contact.vertical_offset_diameters,
        contact.airborne,
    )
    .expect("writing JSON to string should not fail");
}

fn rendered_svg_scenario(
    source: &str,
    options: &SvgGeneratorOptions,
) -> Result<RenderedSvgScenario, String> {
    if !options.trace_sample_step_seconds.is_finite() || options.trace_sample_step_seconds <= 0.0 {
        return Err("trace sample step seconds must be positive and finite".to_string());
    }

    let mut scenario = parse_dsl_to_scenario(source).map_err(|error| error.to_string())?;
    scenario.game_state.resolve_positions();

    let table_spec = scenario.game_state.table_spec.clone();
    let ball_set = scenario.ball_set_physics_spec();
    let motion = human_tuned_preview_motion_config();
    let trace_options = trace_render_options(options);

    let trace = if !scenario.shots.is_empty() {
        if let Some(max_events) = options.trace_max_events.or(scenario.trace_max_events) {
            scenario.simulate_shot_trace_with_preferred_physics_on_table_until_event_limit(
                &ball_set,
                &motion,
                CollisionModel::ThrowAware,
                RailModel::SpinAware,
                max_events,
            )
        } else {
            scenario.simulate_shot_trace_with_preferred_physics_on_table_until_rest(
                &ball_set,
                &motion,
                CollisionModel::ThrowAware,
                RailModel::SpinAware,
            )
        }
        .map_err(|error| error.to_string())?
    } else {
        None
    };

    let game_state = match &trace {
        Some(trace) => trace.rendered_final_layout_with_trace_options(&scenario, &trace_options),
        None => scenario.game_state.clone(),
    };

    let render_options = DiagramRenderOptions {
        background: options.background,
        ..DiagramRenderOptions::default()
    };

    Ok(RenderedSvgScenario {
        svg: game_state.draw_2d_svg_with_options(&render_options),
        trace,
        table_spec,
    })
}

fn trace_render_options(options: &SvgGeneratorOptions) -> ScenarioTraceRenderOptions {
    let mut trace_options = ScenarioTraceRenderOptions::default();
    trace_options.path_render.max_time_step = Seconds::new(options.trace_sample_step_seconds);
    trace_options.start_ghost_balls = options.start_ghost_balls;
    trace_options.event_markers = options.event_markers;
    trace_options.spin_glyphs = options.spin_glyphs;
    trace_options.path_color_mode = options.path_color_mode;
    trace_options
}

pub fn build_scenario_playback_report(
    trace: &ScenarioShotTrace,
    table_spec: &TableSpec,
    max_time_step: Seconds,
) -> ScenarioPlaybackReport {
    let viewport = DiagramViewport::default();
    let ball_spec = table_spec.default_ball_spec();
    let ball_radius = viewport.ball_radius_px(table_spec, &ball_spec);
    let ball_radius_inches = ball_spec.radius.as_f64();
    let pool_artwork = table_spec.kind == TableKind::Pool;
    let frames = trace
        .playback_frames_iter(max_time_step)
        .map(|frame| scenario_playback_frame_report(frame, &viewport, table_spec))
        .collect::<Vec<_>>();
    let duration = frames
        .last()
        .map_or(0.0, |frame| frame.time)
        .max(trace.simulation.elapsed.as_f64());

    ScenarioPlaybackReport {
        duration,
        events: trace
            .event_log
            .iter()
            .enumerate()
            .map(|(index, event)| scenario_playback_event_report(index, event))
            .collect(),
        balls: trace
            .ball_traces
            .iter()
            .map(|ball_trace| {
                scenario_playback_ball_visual(
                    ball_trace,
                    ball_radius,
                    ball_radius_inches,
                    pool_artwork,
                )
            })
            .collect(),
        frames,
    }
}

fn scenario_playback_event_report(
    index: usize,
    event: &ScenarioShotTraceEvent,
) -> ScenarioPlaybackEventReport {
    ScenarioPlaybackEventReport {
        label: format!("({})", index + 1),
        time: event.time.as_f64(),
        summary: event.kind.format_human(),
    }
}

fn scenario_playback_ball_visual(
    ball_trace: &ScenarioBallTrace,
    ball_radius: f32,
    ball_radius_inches: f64,
    pool_artwork: bool,
) -> ScenarioPlaybackBallVisual {
    let visual = ball_visual(&ball_trace.ball);
    ScenarioPlaybackBallVisual {
        id: visual.id,
        fill: visual.fill,
        label: visual.label,
        radius: ball_radius,
        radius_inches: ball_radius_inches,
        style: if pool_artwork {
            visual.style
        } else {
            BallStyle::Plain
        },
        paint: pool_artwork.then_some(visual.paint),
        gradient: pool_artwork.then_some(visual.gradient),
    }
}

fn scenario_playback_ball_report(
    ball: &ScenarioPlaybackBall,
    viewport: &DiagramViewport,
    table_spec: &TableSpec,
) -> ScenarioPlaybackBallReport {
    let state = &ball.state;
    let visual = ball_visual(&ball.ball);
    let center = viewport.position_to_scene_point(&state.projected_position(table_spec));
    ScenarioPlaybackBallReport {
        id: visual.id,
        x: center.x,
        y: center.y,
        height_inches: state.height.as_f64(),
        vx_ips: state.velocity.x().as_f64(),
        vy_ips: state.velocity.y().as_f64(),
        vz_ips: state.vertical_velocity.as_f64(),
        wx_rps: state.angular_velocity.x().as_f64(),
        wy_rps: state.angular_velocity.y().as_f64(),
        wz_rps: state.angular_velocity.z().as_f64(),
    }
}

fn scenario_playback_frame_report(
    frame: ScenarioPlaybackFrame,
    viewport: &DiagramViewport,
    table_spec: &TableSpec,
) -> ScenarioPlaybackFrameReport {
    ScenarioPlaybackFrameReport {
        time: frame.time.as_f64(),
        balls: frame
            .balls
            .iter()
            .map(|ball| scenario_playback_ball_report(ball, viewport, table_spec))
            .collect(),
    }
}

fn push_playback_event_report(json: &mut String, event: &ScenarioPlaybackEventReport) {
    json.push('[');
    push_json_string(json, &event.label);
    write!(json, ",{:.6},", event.time).expect("writing JSON to string should not fail");
    push_json_string(json, &event.summary);
    json.push(']');
}

fn push_playback_ball_visual(json: &mut String, ball: &ScenarioPlaybackBallVisual) {
    json.push('[');
    push_json_string(json, ball.id);
    json.push(',');
    push_json_string(json, ball.fill);
    json.push(',');
    if let Some(label) = ball.label {
        push_json_string(json, label);
    } else {
        json.push_str("null");
    }
    write!(json, ",{:.3},{:.6},", ball.radius, ball.radius_inches)
        .expect("writing JSON to string should not fail");
    push_json_string(json, ball.style.as_str());
    json.push(',');
    if let Some(paint) = ball.paint {
        push_json_string(json, paint);
    } else {
        json.push_str("null");
    }
    json.push(',');
    if let Some(gradient) = ball.gradient {
        push_json_string(json, gradient);
    } else {
        json.push_str("null");
    }
    json.push(']');
}

fn push_playback_ball_report(json: &mut String, ball: &ScenarioPlaybackBallReport) {
    json.push('[');
    push_json_string(json, ball.id);
    write!(
        json,
        ",{:.3},{:.3},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}]",
        ball.x,
        ball.y,
        ball.height_inches,
        ball.vx_ips,
        ball.vy_ips,
        ball.vz_ips,
        ball.wx_rps,
        ball.wy_rps,
        ball.wz_rps
    )
    .expect("writing JSON to string should not fail");
}

fn push_playback_frame_report(json: &mut String, frame: &ScenarioPlaybackFrameReport) {
    write!(json, "[{:.6},[", frame.time).expect("writing JSON to string should not fail");
    for (ball_index, ball) in frame.balls.iter().enumerate() {
        if ball_index > 0 {
            json.push(',');
        }
        push_playback_ball_report(json, ball);
    }
    json.push_str("]]");
}

fn push_scenario_playback_frame(
    json: &mut String,
    frame: &ScenarioPlaybackFrame,
    viewport: &DiagramViewport,
    table_spec: &TableSpec,
) {
    write!(json, "[{:.6},[", frame.time.as_f64()).expect("writing JSON to string should not fail");
    for (ball_index, ball) in frame.balls.iter().enumerate() {
        if ball_index > 0 {
            json.push(',');
        }
        let report = scenario_playback_ball_report(ball, viewport, table_spec);
        push_playback_ball_report(json, &report);
    }
    json.push_str("]]");
}

pub fn serialize_scenario_playback_trace(
    json: &mut String,
    trace: &ScenarioShotTrace,
    table_spec: &TableSpec,
    max_time_step: Seconds,
) {
    let max_time_step = max_time_step.as_f64();
    let duration = trace
        .playback_last_frame_time(Seconds::new(max_time_step))
        .unwrap_or(0.0)
        .max(trace.simulation.elapsed.as_f64());
    write!(json, "{{\"duration\":{duration:.6},\"events\":[")
        .expect("writing JSON to string should not fail");

    for (index, event) in trace.event_log.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let report = scenario_playback_event_report(index, event);
        push_playback_event_report(json, &report);
    }

    let viewport = DiagramViewport::default();
    let ball_spec = table_spec.default_ball_spec();
    let ball_radius = viewport.ball_radius_px(table_spec, &ball_spec);
    let ball_radius_inches = ball_spec.radius.as_f64();
    let pool_artwork = table_spec.kind == TableKind::Pool;
    json.push_str("],\"balls\":[");
    for (index, ball_trace) in trace.ball_traces.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let visual = scenario_playback_ball_visual(
            ball_trace,
            ball_radius,
            ball_radius_inches,
            pool_artwork,
        );
        push_playback_ball_visual(json, &visual);
    }

    json.push_str("],\"frames\":[");
    for (frame_index, frame) in trace
        .playback_frames_iter(Seconds::new(max_time_step))
        .enumerate()
    {
        if frame_index > 0 {
            json.push(',');
        }
        push_scenario_playback_frame(json, &frame, &viewport, table_spec);
    }
    json.push_str("]}");
}

pub fn serialize_scenario_playback_report(json: &mut String, playback: &ScenarioPlaybackReport) {
    write!(json, "{{\"duration\":{:.6},\"events\":[", playback.duration)
        .expect("writing JSON to string should not fail");

    for (index, event) in playback.events.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        push_playback_event_report(json, event);
    }

    json.push_str("],\"balls\":[");
    for (index, ball) in playback.balls.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        push_playback_ball_visual(json, ball);
    }

    json.push_str("],\"frames\":[");
    for (frame_index, frame) in playback.frames.iter().enumerate() {
        if frame_index > 0 {
            json.push(',');
        }
        push_playback_frame_report(json, frame);
    }

    json.push_str("]}");
}

fn push_json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            ch if ch.is_control() => {
                write!(out, "\\u{:04x}", ch as u32)
                    .expect("writing JSON to string should not fail");
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
}
