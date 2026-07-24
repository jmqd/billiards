use std::fmt::Write as _;

use crate::diagram::{ball_visual, BallStyle, DiagramViewport};
use crate::dsl::{parse_dsl_to_scenario, ScenarioShotTrace, ScenarioTraceRenderOptions};
use crate::visualization::{PathColorMode, DEFAULT_BALL_PATH_MAX_TIME_STEP_SECONDS};
use crate::{
    human_tuned_preview_motion_config, CollisionModel, DiagramBackground, DiagramRenderOptions,
    RailModel, Seconds, TableKind, TableSpec,
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
    json.push_str("{\"svg\":");
    push_json_string(&mut json, &rendered.svg);
    json.push_str(",\"events\":[");
    if let Some(trace) = &rendered.trace {
        for (index, event) in trace.event_log.iter().enumerate() {
            if index > 0 {
                json.push(',');
            }
            let label = format!("({})", index + 1);
            json.push('[');
            push_json_string(&mut json, &label);
            write!(&mut json, ",{:.6},", event.time.as_f64())
                .expect("writing JSON to string should not fail");
            push_json_string(&mut json, &event.kind.format_human());
            json.push(',');
            push_json_string(&mut json, &event.format_human());
            json.push(']');
        }
    }
    json.push_str("],\"playback\":");
    if let Some(trace) = &rendered.trace {
        let playback = build_scenario_playback_report(
            trace,
            &rendered.table_spec,
            Seconds::new(options.trace_sample_step_seconds),
        );
        serialize_scenario_playback_report(&mut json, &playback);
    } else {
        json.push_str("null");
    }
    json.push('}');
    Ok(json)
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
    let frames = trace.playback_frames(max_time_step);
    let duration = frames
        .last()
        .map_or(0.0, |frame| frame.time.as_f64())
        .max(trace.simulation.elapsed.as_f64());

    ScenarioPlaybackReport {
        duration,
        events: trace
            .event_log
            .iter()
            .enumerate()
            .map(|(index, event)| ScenarioPlaybackEventReport {
                label: format!("({})", index + 1),
                time: event.time.as_f64(),
                summary: event.kind.format_human(),
            })
            .collect(),
        balls: trace
            .ball_traces
            .iter()
            .map(|ball_trace| {
                let visual = ball_visual(&ball_trace.ball);
                let pool_artwork = table_spec.kind == TableKind::Pool;
                ScenarioPlaybackBallVisual {
                    id: visual.id,
                    fill: visual.fill,
                    label: visual.label,
                    radius: ball_radius,
                    radius_inches: ball_spec.radius.as_f64(),
                    style: if pool_artwork {
                        visual.style
                    } else {
                        BallStyle::Plain
                    },
                    paint: pool_artwork.then_some(visual.paint),
                    gradient: pool_artwork.then_some(visual.gradient),
                }
            })
            .collect(),
        frames: frames
            .into_iter()
            .map(|frame| ScenarioPlaybackFrameReport {
                time: frame.time.as_f64(),
                balls: frame
                    .balls
                    .into_iter()
                    .map(|ball| {
                        let state = &ball.state;
                        let visual = ball_visual(&ball.ball);
                        let center =
                            viewport.position_to_scene_point(&state.projected_position(table_spec));
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
                    })
                    .collect(),
            })
            .collect(),
    }
}

pub fn serialize_scenario_playback_report(json: &mut String, playback: &ScenarioPlaybackReport) {
    write!(json, "{{\"duration\":{:.6},\"events\":[", playback.duration)
        .expect("writing JSON to string should not fail");

    for (index, event) in playback.events.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push('[');
        push_json_string(json, &event.label);
        write!(json, ",{:.6},", event.time).expect("writing JSON to string should not fail");
        push_json_string(json, &event.summary);
        json.push(']');
    }

    json.push_str("],\"balls\":[");
    for (index, ball) in playback.balls.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
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

    json.push_str("],\"frames\":[");
    for (frame_index, frame) in playback.frames.iter().enumerate() {
        if frame_index > 0 {
            json.push(',');
        }
        write!(json, "[{:.6},[", frame.time).expect("writing JSON to string should not fail");
        for (ball_index, ball) in frame.balls.iter().enumerate() {
            if ball_index > 0 {
                json.push(',');
            }
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
        json.push_str("]]");
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
