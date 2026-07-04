use std::fmt::Write as _;

use crate::diagram::DiagramViewport;
use crate::dsl::{parse_dsl_to_scenario, ScenarioShotTrace, ScenarioTraceRenderOptions};
use crate::visualization::{PathColorMode, DEFAULT_BALL_PATH_MAX_TIME_STEP_SECONDS};
use crate::{
    human_tuned_preview_motion_config, BallType, CollisionModel, DiagramBackground,
    DiagramRenderOptions, RailModel, Seconds, TableSpec,
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
        push_playback_json(
            &mut json,
            trace,
            &rendered.table_spec,
            Seconds::new(options.trace_sample_step_seconds),
        );
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

    let trace = if scenario.shot.is_some() {
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

fn push_playback_json(
    json: &mut String,
    trace: &ScenarioShotTrace,
    table_spec: &TableSpec,
    max_time_step: Seconds,
) {
    let viewport = DiagramViewport::default();
    let ball_spec = table_spec.default_ball_spec();
    let ball_radius = viewport.ball_radius_px(table_spec, &ball_spec);
    let frames = trace.playback_frames(max_time_step);
    let duration = frames
        .last()
        .map_or(0.0, |frame| frame.time.as_f64())
        .max(trace.simulation.elapsed.as_f64());

    write!(json, "{{\"duration\":{duration:.6},\"events\":[")
        .expect("writing JSON to string should not fail");
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
        json.push(']');
    }

    json.push_str("],\"balls\":[");
    for (index, ball_trace) in trace.ball_traces.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push('[');
        push_json_string(json, playback_ball_id(&ball_trace.ball));
        json.push(',');
        push_json_string(json, playback_ball_fill(&ball_trace.ball));
        json.push(',');
        if let Some(label) = playback_ball_label(&ball_trace.ball) {
            push_json_string(json, label);
        } else {
            json.push_str("null");
        }
        write!(
            json,
            ",{:.3},{:.6}]",
            ball_radius,
            ball_spec.radius.as_f64()
        )
        .expect("writing JSON to string should not fail");
    }

    json.push_str("],\"frames\":[");
    for (frame_index, frame) in frames.iter().enumerate() {
        if frame_index > 0 {
            json.push(',');
        }
        write!(json, "[{:.6},[", frame.time.as_f64())
            .expect("writing JSON to string should not fail");
        for (ball_index, ball) in frame.balls.iter().enumerate() {
            if ball_index > 0 {
                json.push(',');
            }
            let state = &ball.state;
            let center = viewport.position_to_scene_point(&state.projected_position(table_spec));
            json.push('[');
            push_json_string(json, playback_ball_id(&ball.ball));
            write!(
                json,
                ",{:.3},{:.3},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}]",
                center.x,
                center.y,
                state.height.as_f64(),
                state.velocity.x().as_f64(),
                state.velocity.y().as_f64(),
                state.vertical_velocity.as_f64(),
                state.angular_velocity.x().as_f64(),
                state.angular_velocity.y().as_f64(),
                state.angular_velocity.z().as_f64()
            )
            .expect("writing JSON to string should not fail");
        }
        json.push_str("]]");
    }
    json.push_str("]}");
}

fn playback_ball_id(ball_type: &BallType) -> &'static str {
    match ball_type {
        BallType::Cue => "cue",
        BallType::One => "one",
        BallType::Two => "two",
        BallType::Three => "three",
        BallType::Four => "four",
        BallType::Five => "five",
        BallType::Six => "six",
        BallType::Seven => "seven",
        BallType::Eight => "eight",
        BallType::Nine => "nine",
        BallType::YellowCue => "yellow",
        BallType::Red => "red",
    }
}

fn playback_ball_fill(ball_type: &BallType) -> &'static str {
    match ball_type {
        BallType::Cue => "#f8f4e8",
        BallType::One | BallType::Nine | BallType::YellowCue => "#f1c232",
        BallType::Two => "#2458c8",
        BallType::Three | BallType::Red => "#c82828",
        BallType::Four => "#6f3fa8",
        BallType::Five => "#e27a22",
        BallType::Six => "#25834b",
        BallType::Seven => "#8f2d20",
        BallType::Eight => "#111111",
    }
}

fn playback_ball_label(ball_type: &BallType) -> Option<&'static str> {
    match ball_type {
        BallType::Cue | BallType::YellowCue | BallType::Red => None,
        BallType::One => Some("1"),
        BallType::Two => Some("2"),
        BallType::Three => Some("3"),
        BallType::Four => Some("4"),
        BallType::Five => Some("5"),
        BallType::Six => Some("6"),
        BallType::Seven => Some("7"),
        BallType::Eight => Some("8"),
        BallType::Nine => Some("9"),
    }
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
