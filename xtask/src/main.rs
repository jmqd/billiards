use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use billiards::dsl::{parse_dsl_to_scenario, ScenarioShotTrace, ScenarioTraceRenderOptions};
use billiards::visualization::{BallPathRenderOptions, PathColorMode};
use billiards::{
    diagram::{DiagramOutputFormat, DiagramViewport},
    human_tuned_preview_motion_config, BallType, CollisionModel, DiagramBackground,
    DiagramRenderOptions, HumanShotSpeedBand, NBallSystemState, RailModel, Seconds,
    ShotSpeedPreset, TableSpec,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse(env::args().skip(1).collect())?;
    match args.command {
        CommandName::Help => {
            print_usage();
            Ok(())
        }
        CommandName::ValidationSuite(options) => run_validation_suite(&options),
    }
}

#[derive(Debug)]
struct Args {
    command: CommandName,
}

#[derive(Debug)]
enum CommandName {
    Help,
    ValidationSuite(ValidationSuiteOptions),
}

#[derive(Debug)]
struct ValidationSuiteOptions {
    scenario_dir: PathBuf,
    output_dir: PathBuf,
    trace_sample_step_seconds: f64,
    max_events_override: Option<usize>,
    transparent_background: bool,
    open: bool,
}

impl Default for ValidationSuiteOptions {
    fn default() -> Self {
        Self {
            scenario_dir: PathBuf::from("examples/scenarios"),
            output_dir: PathBuf::from("target/validation-suite"),
            trace_sample_step_seconds: 0.02,
            max_events_override: None,
            transparent_background: false,
            open: false,
        }
    }
}

impl Args {
    fn parse(raw_args: Vec<String>) -> Result<Self, String> {
        let Some(command) = raw_args.first().map(String::as_str) else {
            return Ok(Self {
                command: CommandName::ValidationSuite(ValidationSuiteOptions::default()),
            });
        };

        match command {
            "help" | "--help" | "-h" => Ok(Self {
                command: CommandName::Help,
            }),
            "validation-suite" | "validate-scenarios" => Ok(Self {
                command: CommandName::ValidationSuite(ValidationSuiteOptions::parse(
                    &raw_args[1..],
                )?),
            }),
            other => Err(format!(
                "unknown xtask command `{other}`\n\n{}",
                usage_text()
            )),
        }
    }
}

impl ValidationSuiteOptions {
    fn parse(raw_args: &[String]) -> Result<Self, String> {
        let mut options = Self::default();
        let mut index = 0;
        while index < raw_args.len() {
            match raw_args[index].as_str() {
                "--scenario-dir" => {
                    index += 1;
                    options.scenario_dir =
                        PathBuf::from(value_after(raw_args, index, "--scenario-dir")?);
                }
                "--output-dir" => {
                    index += 1;
                    options.output_dir =
                        PathBuf::from(value_after(raw_args, index, "--output-dir")?);
                }
                "--trace-sample-step-seconds" => {
                    index += 1;
                    options.trace_sample_step_seconds =
                        value_after(raw_args, index, "--trace-sample-step-seconds")?
                            .parse::<f64>()
                            .map_err(|error| {
                                format!("invalid --trace-sample-step-seconds: {error}")
                            })?;
                    if !options.trace_sample_step_seconds.is_finite()
                        || options.trace_sample_step_seconds <= 0.0
                    {
                        return Err(
                            "--trace-sample-step-seconds must be positive and finite".to_string()
                        );
                    }
                }
                "--max-events" => {
                    index += 1;
                    options.max_events_override = Some(
                        value_after(raw_args, index, "--max-events")?
                            .parse::<usize>()
                            .map_err(|error| format!("invalid --max-events: {error}"))?,
                    );
                }
                "--transparent" => {
                    options.transparent_background = true;
                }
                "--open" => {
                    options.open = true;
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                other => return Err(format!("unknown validation-suite option `{other}`")),
            }
            index += 1;
        }

        Ok(options)
    }
}

fn value_after<'a>(args: &'a [String], index: usize, name: &str) -> Result<&'a str, String> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("{name} requires a value"))
}

fn run_validation_suite(options: &ValidationSuiteOptions) -> Result<(), String> {
    let scenarios = scenario_paths(&options.scenario_dir)?;
    if scenarios.is_empty() {
        return Err(format!(
            "no .billiards scenarios found under {}",
            options.scenario_dir.display()
        ));
    }

    fs::create_dir_all(&options.output_dir).map_err(|error| {
        format!(
            "failed to create output dir {}: {error}",
            options.output_dir.display()
        )
    })?;

    let mut reports = Vec::with_capacity(scenarios.len());
    for scenario_path in scenarios {
        reports.push(render_scenario(&scenario_path, options)?);
    }

    let index_path = options.output_dir.join("index.html");
    fs::write(&index_path, render_html(&reports, options)).map_err(|error| {
        format!(
            "failed to write validation gallery {}: {error}",
            index_path.display()
        )
    })?;

    println!(
        "Generated {} scenario diagram(s) in {}",
        reports.len(),
        options.output_dir.display()
    );
    println!("Gallery: {}", index_path.display());
    println!("Preview: cargo xtask validation-suite --open");

    if options.open {
        open_path(&index_path)?;
    }

    Ok(())
}

fn scenario_paths(scenario_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = fs::read_dir(scenario_dir)
        .map_err(|error| format!("failed to read {}: {error}", scenario_dir.display()))?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read directory entry: {error}"))?;
        let path = entry.path();
        if path.extension() == Some(OsStr::new("billiards")) {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

#[derive(Debug)]
struct ScenarioReport {
    name: String,
    image_file_name: String,
    inline_svg: String,
    notes: Vec<String>,
    info_rows: Vec<ReportInfoRow>,
    cue_tip_diagram_svg: Option<String>,
    power_meter_svg: Option<String>,
    playback: Option<ScenarioPlaybackReport>,
    events: Vec<ScenarioEventReport>,
}

#[derive(Debug)]
struct ReportInfoRow {
    label: String,
    value: String,
}

impl ReportInfoRow {
    fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug)]
struct ScenarioEventReport {
    label: String,
    time: String,
    time_seconds: f64,
    summary: String,
    payload: String,
}

#[derive(Debug)]
struct ScenarioPlaybackReport {
    duration: f64,
    events: Vec<ScenarioPlaybackEventReport>,
    balls: Vec<ScenarioPlaybackBallVisual>,
    frames: Vec<ScenarioPlaybackFrameReport>,
}

#[derive(Debug)]
struct ScenarioPlaybackEventReport {
    label: String,
    time: f64,
    summary: String,
}

#[derive(Debug)]
struct ScenarioPlaybackBallVisual {
    id: String,
    fill: &'static str,
    label: Option<&'static str>,
    radius: f32,
}

#[derive(Debug)]
struct ScenarioPlaybackFrameReport {
    time: f64,
    balls: Vec<ScenarioPlaybackBallReport>,
}

#[derive(Debug)]
struct ScenarioPlaybackBallReport {
    id: String,
    x: f32,
    y: f32,
    speed_ips: f64,
}

fn render_scenario(
    scenario_path: &Path,
    options: &ValidationSuiteOptions,
) -> Result<ScenarioReport, String> {
    let source = fs::read_to_string(scenario_path)
        .map_err(|error| format!("failed to read {}: {error}", scenario_path.display()))?;
    let mut scenario = parse_dsl_to_scenario(&source)
        .map_err(|error| format!("failed to parse {}: {error}", scenario_path.display()))?;
    scenario.game_state.resolve_positions();

    let ball_set = scenario.ball_set_physics_spec();
    let motion = human_tuned_preview_motion_config();
    let trace_render = ScenarioTraceRenderOptions {
        path_render: BallPathRenderOptions {
            max_time_step: Seconds::new(options.trace_sample_step_seconds),
            ..ScenarioTraceRenderOptions::default().path_render
        },
        start_ghost_balls: true,
        event_markers: true,
        labels: false,
        path_color_mode: PathColorMode::MotionPhase,
    };

    let effective_trace_max_events = options.max_events_override.or_else(|| {
        scenario.trace_max_events.or_else(|| {
            scenario
                .preferred_simulation_name()
                .and_then(|name| scenario.simulation_named(name).ok())
                .and_then(|simulation| simulation.max_events)
        })
    });

    let trace = if let Some(max_events) = effective_trace_max_events {
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
    .map_err(|error| format!("failed to simulate {}: {error}", scenario_path.display()))?;

    let (render_state, simulation_summary, events, playback) = if let Some(trace) = trace {
        let pocketed = trace
            .simulation
            .states
            .iter()
            .filter(|state| matches!(state, NBallSystemState::Pocketed { .. }))
            .count();
        let remaining = trace
            .simulation
            .states
            .iter()
            .filter(|state| matches!(state, NBallSystemState::OnTable(_)))
            .count();
        let limit_status = if effective_trace_max_events
            .is_some_and(|max_events| trace.simulation.events.len() >= max_events)
        {
            "event limit"
        } else {
            "rest"
        };
        let summary = format!(
            "Simulated to {limit_status}: {} event(s), {:.3}s elapsed, {} pocketed, {} on-table remaining",
            trace.simulation.events.len(),
            trace.simulation.elapsed.as_f64(),
            pocketed,
            remaining
        );
        let events = scenario_event_reports(&trace);
        let playback = scenario_playback_report(
            &trace,
            &scenario.game_state.table_spec,
            Seconds::new(options.trace_sample_step_seconds),
        );
        (
            trace.rendered_final_layout_with_trace_options(&scenario, &trace_render),
            summary,
            events,
            Some(playback),
        )
    } else {
        (
            scenario.game_state.clone(),
            "No shot defined; rendered initial layout only".to_string(),
            Vec::new(),
            None,
        )
    };

    let stem = scenario_path
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| format!("invalid scenario file name {}", scenario_path.display()))?;
    let render_options = DiagramRenderOptions {
        scale_factor: 1,
        background: if options.transparent_background {
            DiagramBackground::Transparent
        } else {
            DiagramBackground::Table
        },
    };
    let svg =
        render_state.render_2d_diagram_with_options(DiagramOutputFormat::Svg, &render_options);
    if svg.is_empty() {
        return Err(format!(
            "rendered empty SVG for {}",
            scenario_path.display()
        ));
    }
    let svg = String::from_utf8(svg).map_err(|error| {
        format!(
            "rendered invalid UTF-8 SVG for {}: {error}",
            scenario_path.display()
        )
    })?;
    let svg_file_name = format!("{stem}.svg");
    let svg_path = options.output_dir.join(&svg_file_name);
    fs::write(&svg_path, svg.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", svg_path.display()))?;

    let speed_validation = scenario.validate_shot_human_speed().map_err(|error| {
        format!(
            "failed to validate shot speed for {}: {error}",
            scenario_path.display()
        )
    })?;
    let shot_line = source
        .lines()
        .find(|line| line.trim_start().starts_with("shot("))
        .map(|line| line.trim().to_string());
    let cue_ball_launch_speed_mph = speed_validation
        .as_ref()
        .map(|validation| validation.estimated_cue_ball_speed_after_impact.as_mph());

    let mut info_rows = Vec::new();
    info_rows.push(ReportInfoRow::new(
        "Source",
        scenario_path.display().to_string(),
    ));
    info_rows.push(ReportInfoRow::new(
        "Simulation",
        simulation_summary.as_str(),
    ));

    if let Some(validation) = &speed_validation {
        let nearest =
            ShotSpeedPreset::nearest_to_speed(&validation.estimated_cue_ball_speed_after_impact);
        info_rows.push(ReportInfoRow::new(
            "Cue-ball launch",
            format!(
                "{:.2} mph · {} · {} band",
                validation.estimated_cue_ball_speed_after_impact.as_mph(),
                nearest.human_label(),
                speed_band_label(validation.cue_ball_speed_band)
            ),
        ));
        info_rows.push(ReportInfoRow::new(
            "Cue-stick impact",
            format!(
                "{:.2} mph · {} band",
                validation.cue_speed_at_impact.as_mph(),
                speed_band_label(validation.cue_speed_band)
            ),
        ));
    }

    if let Some(shot) = scenario.shot.as_ref() {
        info_rows.push(ReportInfoRow::new(
            "Shot target",
            format!("{:?}", shot.ball),
        ));
        info_rows.push(ReportInfoRow::new(
            "Heading",
            format!("{:.2}°", shot.shot.heading().as_degrees()),
        ));
        info_rows.push(ReportInfoRow::new(
            "Tip side",
            format!("{:+.2} R", shot.shot.tip_contact().side_offset().as_f64()),
        ));
        info_rows.push(ReportInfoRow::new(
            "Tip height",
            format!("{:+.2} R", shot.shot.tip_contact().height_offset().as_f64()),
        ));
        info_rows.push(ReportInfoRow::new(
            "Clean-cuing limit",
            format!("{:.2} R", shot.cue_strike.miscue_offset_limit().as_f64()),
        ));
    }

    if let Some(shot_line) = &shot_line {
        info_rows.push(ReportInfoRow::new("DSL shot", shot_line.as_str()));
    }

    let cue_tip_diagram_svg = scenario.shot.as_ref().map(|shot| {
        render_cue_tip_diagram_svg(
            shot.shot.tip_contact().side_offset().as_f64(),
            shot.shot.tip_contact().height_offset().as_f64(),
            shot.cue_strike.miscue_offset_limit().as_f64(),
            cue_ball_launch_speed_mph.unwrap_or_else(|| shot.shot.cue_speed().as_mph()),
        )
    });
    let power_meter_svg = speed_validation.as_ref().map(|validation| {
        render_power_meter_svg(
            validation.estimated_cue_ball_speed_after_impact.as_mph(),
            validation.cue_ball_speed_band,
        )
    });

    Ok(ScenarioReport {
        name: stem.replace('_', " "),
        image_file_name: svg_file_name,
        inline_svg: svg,
        notes: scenario_notes(&source),
        info_rows,
        cue_tip_diagram_svg,
        power_meter_svg,
        playback,
        events,
    })
}

fn speed_band_label(band: HumanShotSpeedBand) -> &'static str {
    match band {
        HumanShotSpeedBand::Touch => "touch",
        HumanShotSpeedBand::Slow => "slow",
        HumanShotSpeedBand::MediumSoft => "medium-soft",
        HumanShotSpeedBand::Medium => "medium",
        HumanShotSpeedBand::MediumFast => "medium-fast",
        HumanShotSpeedBand::Fast => "fast",
        HumanShotSpeedBand::Power => "power",
        HumanShotSpeedBand::TypicalPowerBreak => "typical power break",
        HumanShotSpeedBand::ExceptionalPowerBreak => "exceptional power break",
        HumanShotSpeedBand::BeyondExceptionalPowerBreak => "beyond exceptional power break",
    }
}

fn scenario_notes(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            trimmed
                .strip_prefix('#')
                .map(str::trim)
                .filter(|note| !note.is_empty())
                .map(str::to_string)
        })
        .collect()
}

fn scenario_event_reports(trace: &ScenarioShotTrace) -> Vec<ScenarioEventReport> {
    trace
        .event_log
        .iter()
        .enumerate()
        .map(|(index, event)| {
            let time_seconds = event.time.as_f64();
            ScenarioEventReport {
                label: format!("({})", index + 1),
                time: format!("{time_seconds:.6}"),
                time_seconds,
                summary: event.kind.format_human(),
                payload: format!("{:#?}", event.kind),
            }
        })
        .collect()
}

fn scenario_playback_report(
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
            .map(|ball_trace| ScenarioPlaybackBallVisual {
                id: playback_ball_id(&ball_trace.ball),
                fill: playback_ball_fill(&ball_trace.ball),
                label: playback_ball_label(&ball_trace.ball),
                radius: ball_radius,
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
                        let state = ball.state.as_ball_state();
                        let center =
                            viewport.position_to_scene_point(&state.projected_position(table_spec));
                        ScenarioPlaybackBallReport {
                            id: playback_ball_id(&ball.ball),
                            x: center.x,
                            y: center.y,
                            speed_ips: state.speed().as_f64(),
                        }
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn playback_ball_id(ball_type: &BallType) -> String {
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
    .to_string()
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

fn playback_json(playback: &ScenarioPlaybackReport) -> String {
    let events = playback
        .events
        .iter()
        .map(|event| {
            format!(
                "{{\"label\":{},\"time\":{:.6},\"summary\":{}}}",
                json_string(&event.label),
                event.time,
                json_string(&event.summary)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let balls = playback
        .balls
        .iter()
        .map(|ball| {
            let label = ball.label.map_or_else(|| "null".to_string(), json_string);
            format!(
                "{{\"id\":{},\"fill\":{},\"label\":{},\"radius\":{:.3}}}",
                json_string(&ball.id),
                json_string(ball.fill),
                label,
                ball.radius
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let frames = playback
        .frames
        .iter()
        .map(|frame| {
            let balls = frame
                .balls
                .iter()
                .map(|ball| {
                    format!(
                        "{{\"id\":{},\"x\":{:.3},\"y\":{:.3},\"speed\":{:.6}}}",
                        json_string(&ball.id),
                        ball.x,
                        ball.y,
                        ball.speed_ips
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{\"time\":{:.6},\"balls\":[{}]}}", frame.time, balls)
        })
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "{{\"duration\":{:.6},\"events\":[{}],\"balls\":[{}],\"frames\":[{}]}}",
        playback.duration, events, balls, frames
    )
}

fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0C}' => escaped.push_str("\\f"),
            '<' => escaped.push_str("\\u003c"),
            '>' => escaped.push_str("\\u003e"),
            '&' => escaped.push_str("\\u0026"),
            '\u{2028}' => escaped.push_str("\\u2028"),
            '\u{2029}' => escaped.push_str("\\u2029"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

fn render_cue_tip_diagram_svg(
    side_offset: f64,
    height_offset: f64,
    miscue_offset_limit: f64,
    cue_ball_launch_speed_mph: f64,
) -> String {
    let ball_radius = 72.0;
    let ball_center = 90.0;
    let tip_x = ball_center + side_offset * ball_radius;
    let tip_y = ball_center - height_offset * ball_radius;
    let limit_radius = miscue_offset_limit.clamp(0.0, 1.0) * ball_radius;
    let offset_radius = side_offset.hypot(height_offset);
    let marker_radius = cue_tip_marker_radius(cue_ball_launch_speed_mph, ball_radius);
    let marker_outline_radius = marker_radius + 3.0;
    let limit_status = if offset_radius <= miscue_offset_limit + 1e-12 {
        "inside"
    } else {
        "outside"
    };

    format!(
        r##"<svg class="cue-tip-diagram" data-tip-side="{side_offset:.3}" data-tip-height="{height_offset:.3}" data-miscue-limit="{miscue_offset_limit:.3}" data-cue-ball-speed-mph="{cue_ball_launch_speed_mph:.3}" data-tip-marker-r="{marker_radius:.3}" data-tip-x="{tip_x:.3}" data-tip-y="{tip_y:.3}" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 180 184" role="img" aria-label="Cue ball tip contact: side {side_offset:+.2} ball radii, height {height_offset:+.2} ball radii, {limit_status} the {miscue_offset_limit:.2} ball-radius miscue limit; red marker radius scales with {cue_ball_launch_speed_mph:.2} mph cue-ball launch speed">
<ellipse class="cue-ball-shadow" cx="94" cy="165" rx="62" ry="16" fill="#000000" opacity=".35"/>
<circle class="cue-ball-body" cx="{ball_center:.0}" cy="{ball_center:.0}" r="{ball_radius:.0}" fill="#e9e0c9" stroke="#fff9e9" stroke-width="1.5"/>
<circle class="cue-ball-shade" cx="116" cy="118" r="48" fill="#000000" opacity=".10"/>
<circle class="cue-ball-highlight" cx="64" cy="50" r="38" fill="#ffffff" opacity=".24"/>
<ellipse class="cue-ball-glare" cx="61" cy="43" rx="20" ry="12" fill="#ffffff" opacity=".72" transform="rotate(-25 61 43)"/>
<path d="M39 121C53 145 81 158 113 150" fill="none" stroke="#ffffff" stroke-opacity=".28" stroke-width="6" stroke-linecap="round"/>
<circle class="miscue-limit" cx="{ball_center:.0}" cy="{ball_center:.0}" r="{limit_radius:.3}" fill="none" stroke="#090909" stroke-width="2.75"/>
<circle class="cue-tip-marker" cx="{tip_x:.3}" cy="{tip_y:.3}" r="{marker_radius:.3}" fill="#d91919" stroke="#ffffff" stroke-width="2"/>
<circle class="cue-tip-marker-outline" cx="{tip_x:.3}" cy="{tip_y:.3}" r="{marker_outline_radius:.3}" fill="none" stroke="#7b0000" stroke-opacity=".65" stroke-width="1.5"/>
</svg>
"##
    )
}

fn cue_tip_marker_radius(cue_ball_launch_speed_mph: f64, ball_radius: f64) -> f64 {
    let equator_width = ball_radius * 2.0;
    let min_radius = equator_width / 16.0;
    let max_radius = equator_width / 8.0;
    let speed_ratio = (cue_ball_launch_speed_mph / 30.0).clamp(0.0, 1.0);

    min_radius + (max_radius - min_radius) * speed_ratio
}

fn render_power_meter_svg(
    cue_ball_launch_speed_mph: f64,
    speed_band: HumanShotSpeedBand,
) -> String {
    let cx = 110.0;
    let cy = 106.0;
    let radius = 80.0;
    let normal_end_angle = power_meter_angle_for_mph(30.0);
    let max_angle = power_meter_angle_for_mph(35.0);
    let needle_angle = power_meter_angle_for_mph(cue_ball_launch_speed_mph);
    let normal_arc = svg_arc_path(
        cx,
        cy,
        radius,
        power_meter_angle_for_mph(0.0),
        normal_end_angle,
    );
    let redline_arc = svg_arc_path(cx, cy, radius, normal_end_angle, max_angle);
    let track_arc = svg_arc_path(cx, cy, radius, power_meter_angle_for_mph(0.0), max_angle);
    let (needle_x, needle_y) = polar_point(cx, cy, radius - 12.0, needle_angle);
    let mut ticks = String::new();

    for tick_mph in [0.0, 10.0, 20.0, 30.0, 35.0] {
        let angle = power_meter_angle_for_mph(tick_mph);
        let (outer_x, outer_y) = polar_point(cx, cy, radius + 4.0, angle);
        let (inner_x, inner_y) = polar_point(cx, cy, radius - 9.0, angle);
        let tick_label = match tick_mph as i32 {
            0 => Some("0"),
            30 => Some("30"),
            35 => Some("35"),
            _ => None,
        };
        ticks.push_str(&format!(
            r##"<line class="power-meter-tick" x1="{outer_x:.3}" y1="{outer_y:.3}" x2="{inner_x:.3}" y2="{inner_y:.3}" stroke="#d5e4d0" stroke-width="2" stroke-linecap="round"/>
"##
        ));
        if let Some(tick_label) = tick_label {
            let (label_x, label_y) = polar_point(cx, cy, radius - 28.0, angle);
            let label_class = if tick_mph >= 30.0 {
                "power-meter-label power-meter-label-redline"
            } else {
                "power-meter-label"
            };
            ticks.push_str(&format!(
                r##"<text class="{label_class}" x="{label_x:.3}" y="{label_y:.3}" fill="#d5e4d0" font-size="10" font-family="Inter,system-ui,sans-serif" text-anchor="middle" dominant-baseline="middle">{tick_label}</text>
"##
            ));
        }
    }

    format!(
        r##"<svg class="power-meter" data-cue-ball-speed-mph="{cue_ball_launch_speed_mph:.3}" data-speed-band="{speed_band:?}" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 220 154" role="img" aria-label="Power level: cue-ball launch speed {cue_ball_launch_speed_mph:.2} miles per hour, {speed_band_label} band. Gauge spans 0 to 35 miles per hour with a red-line zone from 30 to 35.">
<path class="power-meter-track" d="{track_arc}" fill="none" stroke="#304030" stroke-width="14" stroke-linecap="round"/>
<path class="power-meter-normal" d="{normal_arc}" fill="none" stroke="#78d66b" stroke-width="14" stroke-linecap="round"/>
<path class="power-meter-redline" d="{redline_arc}" fill="none" stroke="#e04747" stroke-width="14" stroke-linecap="round"/>
{ticks}<line class="power-meter-needle" x1="{cx:.0}" y1="{cy:.0}" x2="{needle_x:.3}" y2="{needle_y:.3}" stroke="#f7fff2" stroke-width="4" stroke-linecap="round"/>
<circle class="power-meter-hub" cx="{cx:.0}" cy="{cy:.0}" r="8" fill="#f7fff2" stroke="#101410" stroke-width="3"/>
</svg>
"##,
        speed_band_label = speed_band_label(speed_band)
    )
}

fn power_meter_angle_for_mph(mph: f64) -> f64 {
    150.0 + (mph / 35.0).clamp(0.0, 1.0) * 240.0
}

fn svg_arc_path(
    cx: f64,
    cy: f64,
    radius: f64,
    start_angle_degrees: f64,
    end_angle_degrees: f64,
) -> String {
    let (start_x, start_y) = polar_point(cx, cy, radius, start_angle_degrees);
    let (end_x, end_y) = polar_point(cx, cy, radius, end_angle_degrees);
    let large_arc = if (end_angle_degrees - start_angle_degrees).abs() > 180.0 {
        1
    } else {
        0
    };

    format!("M {start_x:.3} {start_y:.3} A {radius:.3} {radius:.3} 0 {large_arc} 1 {end_x:.3} {end_y:.3}")
}

fn polar_point(cx: f64, cy: f64, radius: f64, angle_degrees: f64) -> (f64, f64) {
    let angle_radians = angle_degrees.to_radians();

    (
        cx + radius * angle_radians.cos(),
        cy + radius * angle_radians.sin(),
    )
}

fn push_event_log(html: &mut String, report: &ScenarioReport) {
    if report.events.is_empty() {
        return;
    }

    html.push_str(
        "<details class=\"event-log\"><summary>Event log</summary><ol class=\"event-list\">\n",
    );
    for event in &report.events {
        let title = format!("{} @ t={}s\n{}", event.summary, event.time, event.payload);
        html.push_str(&format!(
            "<li data-event-label=\"{}\" data-event-time=\"{:.6}\" data-event-title=\"{}\"><span class=\"event-badge\" title=\"{}\">{}</span><code class=\"event-time\">t={}s</code><span class=\"event-summary\">{}</span><pre class=\"event-payload\"><code>{}</code></pre></li>\n",
            escape_html(&event.label),
            event.time_seconds,
            escape_html(&title),
            escape_html(&title),
            escape_html(&event.label),
            escape_html(&event.time),
            escape_html(&event.summary),
            escape_html(&event.payload)
        ));
    }
    html.push_str("</ol></details>\n");
}

fn render_html(reports: &[ScenarioReport], options: &ValidationSuiteOptions) -> String {
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str("<title>Billiards scenario validation suite</title>\n");
    html.push_str("<style>\n");
    html.push_str(r#":root{
  color-scheme:light;
  --paper:#f8f5ed;
  --paper-warm:#fffaf2;
  --panel:#fffefd;
  --panel-tint:#fbf6eb;
  --ink:#1d1914;
  --muted:#6f6557;
  --faint:#8a8070;
  --rule:#d8d0c0;
  --rule-strong:#b9aa8f;
  --accent:#7b1f1f;
  --accent-2:#164c78;
  --code:#f5efe2;
  --shadow:rgba(70,52,33,.12);
  font-family:Georgia,"Times New Roman",serif;
  line-height:1.55;
  font-variant-numeric:tabular-nums;
}
*{box-sizing:border-box}
body{margin:0;background:var(--paper);color:var(--ink);font-size:16px}
body::before{content:"";position:fixed;inset:0;pointer-events:none;background:radial-gradient(circle at 50% 0,rgba(255,255,255,.7),transparent 22rem),linear-gradient(90deg,rgba(120,90,40,.035),transparent 12%,transparent 88%,rgba(120,90,40,.035));z-index:-1}
header{max-width:min(100%,1240px);margin:0 auto;padding:1.45rem 1.25rem 1rem;border-bottom:1px solid var(--rule)}
h1{margin:.1rem 0 .35rem;font-size:clamp(1.6rem,3vw,2.35rem);font-weight:400;letter-spacing:-.035em;line-height:1.05}
.subtitle{color:var(--muted);margin:0;overflow-wrap:anywhere}
main{width:min(100%,1240px);margin:0 auto;padding:1.25rem}
.toc{display:flex;flex-wrap:wrap;gap:.38rem;margin:1rem 0 1.35rem}
.toc a{color:var(--accent);background:var(--paper-warm);border:1px solid var(--rule);border-radius:3px;padding:.2rem .48rem;text-decoration:none;font-size:.9rem;overflow-wrap:anywhere;box-shadow:0 1px 0 rgba(255,255,255,.8) inset}
.toc a:hover,.toc a:focus-visible{border-color:var(--accent);background:#fff4df;outline:none}
.card{background:var(--panel);border:1px solid var(--rule);border-radius:3px;margin:0 0 1.35rem;overflow:visible;box-shadow:0 2px 14px var(--shadow)}
.card h2{margin:0;padding:.8rem 1rem .65rem;border-bottom:1px solid var(--rule);font-size:1.35rem;font-weight:400;letter-spacing:-.018em;overflow-wrap:anywhere}
.card-overview{display:grid;grid-template-columns:minmax(12rem,16rem) minmax(0,1fr);gap:.9rem;padding:1rem;align-items:start}
.card-overview-full{grid-template-columns:minmax(0,1fr)}
.visual-stack{display:grid;gap:.65rem;min-width:0}
.visual-panel{min-width:0;background:var(--panel-tint);border:1px solid var(--rule);border-radius:3px;padding:.65rem;color:var(--ink)}
.visual-panel svg{display:block;width:100%;height:auto;max-width:15rem;margin:0 auto}
.visual-caption{display:flex;align-items:center;justify-content:center;gap:.35rem;margin:.55rem 0 0;font-size:.88rem;line-height:1.25;color:var(--muted)}
.visual-caption strong{color:var(--ink);font-size:.9rem;font-variant:small-caps;font-weight:600;letter-spacing:.035em}
.tooltip{position:relative;display:inline-flex;align-items:center;justify-content:center;vertical-align:baseline;color:var(--accent);cursor:help;text-decoration:none;border-bottom:1px dotted currentColor;outline-offset:3px}
.tooltip-mark{display:inline-flex;align-items:center;justify-content:center;min-width:1.15em;height:1.15em;border:1px solid var(--rule-strong);border-radius:50%;background:#fff7d7;color:var(--accent);font-family:ui-sans-serif,system-ui,sans-serif;font-size:.75em;font-weight:700;line-height:1}
.tooltip::after{content:attr(data-tooltip);position:absolute;left:50%;bottom:calc(100% + .55rem);transform:translate(-50%,.25rem);z-index:20;width:min(24rem,80vw);padding:.65rem .75rem;background:#fffdf7;border:1px solid var(--rule-strong);box-shadow:0 8px 24px rgba(72,52,28,.18);color:var(--ink);font:400 .88rem/1.35 ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,Segoe UI,sans-serif;text-align:left;white-space:normal;opacity:0;pointer-events:none;transition:opacity .12s ease,transform .12s ease}
.tooltip::before{content:"";position:absolute;left:50%;bottom:calc(100% + .28rem);z-index:21;width:.55rem;height:.55rem;background:#fffdf7;border-left:1px solid var(--rule-strong);border-bottom:1px solid var(--rule-strong);transform:translate(-50%,.25rem) rotate(-45deg);opacity:0;pointer-events:none;transition:opacity .12s ease,transform .12s ease}
.tooltip:hover::after,.tooltip:focus-visible::after,.tooltip:hover::before,.tooltip:focus-visible::before{opacity:1;transform:translate(-50%,0)}
.info-panel{min-width:0;background:var(--panel-tint);border:1px solid var(--rule);border-radius:3px;padding:.65rem .85rem;color:var(--ink)}
.info-table{display:grid;margin:0;gap:0}
.info-row{display:grid;grid-template-columns:minmax(7.5rem,.34fr) minmax(0,1fr);gap:.8rem;padding:.34rem 0;border-bottom:1px solid var(--rule);align-items:start}
.info-row:first-child{padding-top:0}
.info-row:last-child{padding-bottom:0;border-bottom:0}
.info-row dt{color:var(--faint);font-size:.72rem;font-weight:700;text-transform:uppercase;letter-spacing:.06em;font-family:ui-sans-serif,system-ui,sans-serif}
.info-row dd{margin:0;color:var(--ink);text-align:right;overflow-wrap:anywhere}
.scenario-context{margin:.65rem 0 0;padding:.55rem 0 0;border-top:1px solid var(--rule)}
.scenario-context summary{font-size:.82rem}
figure{min-width:0;margin:0;background:#f3ead8;padding:1rem;border-top:1px solid var(--rule);border-bottom:1px solid var(--rule)}
img{display:block;max-width:100%;height:auto;margin:0 auto;border-radius:2px;background:#fdfbf6}
.svg-viewer{display:grid;gap:.7rem;min-width:0}
.viewer-controls{display:flex;flex-wrap:wrap;align-items:center;gap:.4rem;color:var(--muted);font-size:.9rem;font-family:ui-sans-serif,system-ui,sans-serif}
.viewer-controls button{background:var(--paper-warm);color:var(--ink);border:1px solid var(--rule-strong);border-radius:3px;padding:.25rem .5rem;cursor:pointer}
.viewer-controls button:hover,.viewer-controls button:focus-visible{border-color:var(--accent);outline:none}
.viewer-controls label{display:inline-flex;align-items:center;gap:.25rem;background:var(--paper-warm);border:1px solid var(--rule);border-radius:3px;padding:.22rem .5rem;max-width:100%;overflow-wrap:anywhere}
.svg-frame{overflow:hidden;border-radius:2px;background:#fdfbf6;touch-action:none;min-width:0;border:1px solid var(--rule)}
.svg-frame svg{display:block;max-width:100%;width:auto;height:auto;margin:0 auto;cursor:grab}
.svg-frame .event-marker[data-event-label]{cursor:help}
.svg-frame svg.dragging{cursor:grabbing}
.playback-panel{display:grid;gap:.55rem;background:var(--paper-warm);border:1px solid var(--rule);border-radius:3px;padding:.6rem;color:var(--muted);font-family:ui-sans-serif,system-ui,sans-serif;font-size:.9rem}
.playback-controls{display:flex;flex-wrap:wrap;align-items:center;gap:.45rem}
.playback-controls button{background:var(--panel);color:var(--ink);border:1px solid var(--rule-strong);border-radius:3px;padding:.25rem .55rem;cursor:pointer}
.playback-controls button:hover,.playback-controls button:focus-visible{border-color:var(--accent);outline:none}
.playback-controls input[type="range"]{flex:1 1 16rem;accent-color:var(--accent)}
.playback-time{min-width:9rem;color:var(--ink);font-variant-numeric:tabular-nums}
.playback-event{flex:1 1 18rem;min-width:min(100%,18rem);color:var(--ink);overflow-wrap:anywhere}
.playback-event[data-event-state="hit"]{color:var(--accent);font-weight:700}
.playback-help{margin:0;color:var(--muted);font-size:.84rem;line-height:1.35}
.playback-balls{pointer-events:none}
.playback-ball-label{font-family:ui-sans-serif,system-ui,sans-serif;font-size:10px;font-weight:800;text-anchor:middle;dominant-baseline:central;pointer-events:none}
.playback-heading{stroke:#111;stroke-linecap:round;stroke-linejoin:round;pointer-events:none}
.downloads{display:flex;flex-wrap:wrap;gap:.5rem;margin:.55rem 0 0;font-size:.9rem;color:var(--muted)}
.downloads a{overflow-wrap:anywhere}
details{padding:.85rem 1rem 1rem;min-width:0}
summary{cursor:pointer;color:var(--accent);font-weight:600}
pre{white-space:pre-wrap;overflow:auto;max-height:28rem;background:var(--code);border:1px solid var(--rule);border-radius:3px;padding:.85rem;color:var(--ink);overflow-wrap:anywhere}
code{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;overflow-wrap:anywhere;background:rgba(122,31,31,.06);border-radius:2px;padding:0 .12em}
pre code{background:transparent;padding:0}
.notes{margin:.25rem 0 0;padding-left:1.1rem;overflow-wrap:anywhere}
.notes li{margin:.16rem 0}
.event-log{padding-top:1rem}
.event-list{list-style:none;margin:.75rem 0 0;padding:0;display:grid;gap:.65rem}
.event-list li{min-width:0;background:#fffaf1;border:1px solid var(--rule);border-radius:3px;padding:.65rem;display:grid;grid-template-columns:auto auto minmax(0,1fr);gap:.35rem .6rem;align-items:start}
.event-list li.event-current{border-color:var(--accent);box-shadow:inset .22rem 0 0 var(--accent)}
.event-badge{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-weight:800;color:#fffaf1;background:var(--accent);border-radius:999px;padding:.05rem .45rem;cursor:help}
.event-time{color:var(--muted);white-space:nowrap}
.event-summary{min-width:0;overflow-wrap:anywhere}
.event-payload{grid-column:1 / -1;margin:.2rem 0 0;max-height:9rem;font-size:.82rem}
.cue-tip-marker{filter:drop-shadow(0 1px 2px rgba(0,0,0,.35))}
.power-meter-needle,.power-meter-hub{filter:drop-shadow(0 1px 2px rgba(0,0,0,.35))}
.power-meter-label-redline{fill:#ffd0d0}
a{color:var(--accent)}
a:hover,a:focus-visible{color:#501212}
@media (max-width:720px){header{padding:.95rem .85rem}main{padding:.75rem}.toc{gap:.35rem}.card{border-radius:2px}.card h2{padding:.75rem}.card-overview{grid-template-columns:1fr;padding:.65rem}.info-row{grid-template-columns:1fr;gap:.12rem}.info-row dd{text-align:left}figure{padding:.65rem}.viewer-controls{align-items:stretch}.viewer-controls button,.viewer-controls label{flex:1 1 auto;justify-content:center}.tooltip::after{left:auto;right:0;transform:translate(0,.25rem)}.tooltip::before{left:50%}.tooltip:hover::after,.tooltip:focus-visible::after{transform:translate(0,0)}}
"#);
    html.push_str("</style>\n</head>\n<body>\n");
    html.push_str("<header>\n<h1>Billiards scenario validation suite</h1>\n");
    html.push_str(&format!(
        "<p class=\"subtitle\">{} scenario diagram(s) · <code>{}</code> → <code>{}</code> ",
        reports.len(),
        escape_html(&options.scenario_dir.display().to_string()),
        escape_html(&options.output_dir.display().to_string()),
    ));
    push_tooltip(
        &mut html,
        "?",
        "Inline SVG report. Speed fields are cue-ball launch estimates unless an individual scenario says otherwise.",
    );
    html.push_str("</p>\n");
    html.push_str("</header>\n<main>\n<nav class=\"toc\" aria-label=\"Scenario list\">\n");
    for report in reports {
        html.push_str(&format!(
            "<a href=\"#{}\">{}</a>\n",
            escape_html(&anchor_id(&report.name)),
            escape_html(&report.name)
        ));
    }
    html.push_str("</nav>\n");

    for report in reports {
        html.push_str(&format!(
            "<section class=\"card\" id=\"{}\">\n<h2>{}</h2>\n",
            escape_html(&anchor_id(&report.name)),
            escape_html(&report.name)
        ));
        let has_visuals = report.cue_tip_diagram_svg.is_some() || report.power_meter_svg.is_some();
        let overview_class = if has_visuals {
            "card-overview"
        } else {
            "card-overview card-overview-full"
        };
        html.push_str(&format!("<div class=\"{overview_class}\">\n"));
        if has_visuals {
            html.push_str("<div class=\"visual-stack\" aria-label=\"Shot visual aids\">\n");
            if let Some(cue_tip_diagram_svg) = &report.cue_tip_diagram_svg {
                html.push_str("<div class=\"visual-panel\">\n");
                html.push_str(cue_tip_diagram_svg);
                html.push_str("<div class=\"visual-caption\"><strong>Cue tip</strong>");
                push_tooltip(
                    &mut html,
                    "?",
                    "Red marker: tip-contact offset in cue-ball radii. Marker size follows cue-ball launch speed. Ring: configured clean-cuing limit.",
                );
                html.push_str("</div>\n</div>\n");
            }
            if let Some(power_meter_svg) = &report.power_meter_svg {
                html.push_str("<div class=\"visual-panel\">\n");
                html.push_str(power_meter_svg);
                html.push_str("<div class=\"visual-caption\"><strong>Power</strong>");
                push_tooltip(
                    &mut html,
                    "?",
                    "Needle: estimated cue-ball launch speed. Green arc: ordinary range through 30 mph. Red arc: 30-35 mph break-speed band.",
                );
                html.push_str("</div>\n</div>\n");
            }
            html.push_str("</div>\n");
        }
        html.push_str("<div class=\"info-panel\">\n");
        push_info_table(&mut html, &report.info_rows);
        if !report.notes.is_empty() {
            html.push_str(
                "<details class=\"scenario-context\" open><summary>Scenario context</summary><ul class=\"notes\">\n",
            );
            for note in &report.notes {
                html.push_str(&format!("<li>{}</li>\n", escape_html(note)));
            }
            html.push_str("</ul></details>\n");
        }
        html.push_str("</div>\n</div>\n");
        html.push_str("<figure class=\"svg-viewer\" data-viewer>\n");
        html.push_str(
            "<div class=\"viewer-controls\" aria-label=\"Diagram controls\">\n\
             <button type=\"button\" data-zoom=\"in\">Zoom in</button>\n\
             <button type=\"button\" data-zoom=\"out\">Zoom out</button>\n\
             <button type=\"button\" data-zoom=\"reset\">Reset</button>\n\
             <label><input type=\"checkbox\" data-layer-toggle=\"table\" checked>Table</label>\n\
             <label><input type=\"checkbox\" data-layer-toggle=\"overlays-below-balls\" checked>Below-ball overlays</label>\n\
             <label><input type=\"checkbox\" data-layer-toggle=\"balls\" checked>Balls</label>\n\
             <label><input type=\"checkbox\" data-layer-toggle=\"overlays-above-balls\" checked>Above-ball overlays</label>\n\
             </div>\n\
             <div class=\"svg-frame\">\n",
        );
        html.push_str(&report.inline_svg);
        html.push_str("</div>\n");
        if let Some(playback) = &report.playback {
            let max_frame = playback.frames.len().saturating_sub(1);
            html.push_str(&format!(
                "<div class=\"playback-panel\" data-playback>\n\
                 <script type=\"application/json\" data-playback-data>{}</script>\n\
                 <div class=\"playback-controls\" aria-label=\"Playback controls\">\n\
                 <button type=\"button\" data-playback-step=\"-1\">Step back</button>\n\
                 <button type=\"button\" data-playback-play>Play</button>\n\
                 <button type=\"button\" data-playback-step=\"1\">Step forward</button>\n\
                 <button type=\"button\" data-playback-next-event>Next event</button>\n\
                 <input type=\"range\" data-playback-slider min=\"0\" max=\"{}\" value=\"{}\" step=\"1\" aria-label=\"Trace frame\">\n\
                 <span class=\"playback-time\" data-playback-time>t=0.000s</span>\n\
                 <span class=\"playback-event\" data-playback-event>No events</span>\n\
                 </div>\n\
                 <p class=\"playback-help\">Scrub the physics frames in either direction, or play to the next logged event. Balls are sampled by the Rust physics solver; black ticks show instantaneous travel direction and fade with speed.</p>\n\
                 </div>\n",
                playback_json(playback),
                max_frame,
                max_frame
            ));
        }
        push_download_links(&mut html, report);
        html.push_str("</figure>\n");
        push_event_log(&mut html, report);
        html.push_str("</section>\n");
    }

    html.push_str(
        r#"<script>
document.querySelectorAll('[data-viewer]').forEach((viewer) => {
  const svg = viewer.querySelector('svg');
  if (!svg || !svg.viewBox || !svg.viewBox.baseVal) return;
  const base = svg.viewBox.baseVal;
  let box = { x: base.x, y: base.y, width: base.width, height: base.height };
  const apply = () => svg.setAttribute('viewBox', `${box.x} ${box.y} ${box.width} ${box.height}`);
  const zoom = (factor, cx = box.x + box.width / 2, cy = box.y + box.height / 2) => {
    const nextWidth = box.width * factor;
    const nextHeight = box.height * factor;
    const rx = (cx - box.x) / box.width;
    const ry = (cy - box.y) / box.height;
    box = { x: cx - nextWidth * rx, y: cy - nextHeight * ry, width: nextWidth, height: nextHeight };
    apply();
  };
  viewer.querySelectorAll('[data-zoom]').forEach((button) => {
    button.addEventListener('click', () => {
      const action = button.dataset.zoom;
      if (action === 'in') zoom(0.8);
      if (action === 'out') zoom(1.25);
      if (action === 'reset') { box = { x: base.x, y: base.y, width: base.width, height: base.height }; apply(); }
    });
  });
  viewer.querySelectorAll('[data-layer-toggle]').forEach((input) => {
    input.addEventListener('change', () => {
      svg.querySelectorAll(`[data-layer="${input.dataset.layerToggle}"]`).forEach((layer) => {
        layer.style.display = input.checked ? '' : 'none';
      });
    });
  });
  const card = viewer.closest('.card');
  const eventTitles = new Map(Array.from(card?.querySelectorAll('[data-event-label]') ?? []).map((row) => [row.getAttribute('data-event-label'), row.getAttribute('data-event-title')]));
  svg.querySelectorAll('.event-marker[data-event-label]').forEach((marker) => {
    const eventLabel = marker.getAttribute('data-event-label');
    const titleText = eventTitles.get(eventLabel);
    if (!titleText) return;
    marker.setAttribute('tabindex', '0');
    marker.setAttribute('aria-label', titleText);
    marker.classList.add('event-marker-tooltip');
    if (!marker.querySelector('title')) {
      const title = document.createElementNS('http://www.w3.org/2000/svg', 'title');
      title.textContent = titleText;
      marker.appendChild(title);
    } else {
      marker.querySelector('title').textContent = titleText;
    }
  });
  const playbackPanel = viewer.querySelector('[data-playback]');
  if (playbackPanel) {
    const playbackDataElement = playbackPanel.querySelector('[data-playback-data]');
    const slider = playbackPanel.querySelector('[data-playback-slider]');
    const timeLabel = playbackPanel.querySelector('[data-playback-time]');
    const eventTicker = playbackPanel.querySelector('[data-playback-event]');
    const playButton = playbackPanel.querySelector('[data-playback-play]');
    const nextEventButton = playbackPanel.querySelector('[data-playback-next-event]');
    let playback = null;
    try {
      playback = JSON.parse(playbackDataElement?.textContent ?? '');
    } catch (_) {
      playback = null;
    }
    if (playback && Array.isArray(playback.frames) && playback.frames.length > 0 && slider) {
      const ns = 'http://www.w3.org/2000/svg';
      const ballLayer = svg.querySelector('[data-layer="balls"]');
      if (ballLayer) {
        ballLayer.style.display = 'none';
        const ballToggle = viewer.querySelector('[data-layer-toggle="balls"]');
        if (ballToggle) ballToggle.checked = false;
      }
      const playbackLayer = document.createElementNS(ns, 'g');
      playbackLayer.setAttribute('class', 'playback-layer');
      playbackLayer.setAttribute('data-layer', 'playback-balls');
      if (ballLayer && ballLayer.parentNode) {
        ballLayer.parentNode.insertBefore(playbackLayer, ballLayer.nextSibling);
      } else {
        svg.appendChild(playbackLayer);
      }
      const visuals = new Map((playback.balls ?? []).map((ball) => [ball.id, ball]));
      const events = Array.isArray(playback.events)
        ? playback.events
            .map((event) => ({
              label: String(event.label ?? ''),
              time: Number(event.time),
              summary: String(event.summary ?? ''),
            }))
            .filter((event) => event.label && Number.isFinite(event.time))
            .sort((a, b) => a.time - b.time)
        : [];
      const eventRows = new Map(Array.from(card?.querySelectorAll('.event-list [data-event-label]') ?? []).map((row) => [row.getAttribute('data-event-label'), row]));
      const eventHitWindow = 0.005;
      const clampFrame = (value) => Math.max(0, Math.min(playback.frames.length - 1, Number(value) || 0));
      const frameTime = (frameIndex) => Number(playback.frames[clampFrame(frameIndex)]?.time) || 0;
      const formatPlaybackTime = (time) => (Number(time) || 0).toFixed(3);
      const nextEventAfter = (time) => events.find((event) => event.time > time + eventHitWindow);
      const updateEventTicker = (time) => {
        eventRows.forEach((row) => row.classList.remove('event-current'));
        if (!eventTicker) return;
        if (events.length === 0) {
          eventTicker.textContent = 'no logged events';
          eventTicker.dataset.eventState = 'none';
          return;
        }
        const hit = events.find((event) => Math.abs(event.time - time) <= eventHitWindow);
        if (hit) {
          eventTicker.textContent = `event ${hit.label} @ t=${formatPlaybackTime(hit.time)}s: ${hit.summary}`;
          eventTicker.dataset.eventState = 'hit';
          eventRows.get(hit.label)?.classList.add('event-current');
          return;
        }
        const next = nextEventAfter(time);
        if (next) {
          eventTicker.textContent = `next ${next.label} in ${Math.max(0, next.time - time).toFixed(3)}s: ${next.summary}`;
          eventTicker.dataset.eventState = 'next';
          return;
        }
        const last = events.slice().reverse().find((event) => event.time <= time + eventHitWindow);
        if (last) {
          eventTicker.textContent = `last ${last.label} @ t=${formatPlaybackTime(last.time)}s: ${last.summary}`;
          eventTicker.dataset.eventState = 'done';
          eventRows.get(last.label)?.classList.add('event-current');
        } else {
          eventTicker.textContent = 'before first event';
          eventTicker.dataset.eventState = 'before';
        }
      };
      const frameBallMap = (frame) => new Map((frame?.balls ?? []).map((ball) => [ball.id, ball]));
      const headingForBall = (index, ball) => {
        const previous = frameBallMap(playback.frames[Math.max(0, index - 1)]).get(ball.id);
        const next = frameBallMap(playback.frames[Math.min(playback.frames.length - 1, index + 1)]).get(ball.id);
        const dx = next && (Math.abs(next.x - ball.x) > 0.01 || Math.abs(next.y - ball.y) > 0.01)
          ? next.x - ball.x
          : previous ? ball.x - previous.x : 0;
        const dy = next && (Math.abs(next.x - ball.x) > 0.01 || Math.abs(next.y - ball.y) > 0.01)
          ? next.y - ball.y
          : previous ? ball.y - previous.y : 0;
        const length = Math.hypot(dx, dy);
        if (length <= 0.01) return null;
        return { dx: dx / length, dy: dy / length };
      };
      const appendCircle = (className, cx, cy, radius, fill, opacity, stroke = 'none', strokeWidth = '0') => {
        const circle = document.createElementNS(ns, 'circle');
        circle.setAttribute('class', className);
        circle.setAttribute('cx', cx.toFixed(3));
        circle.setAttribute('cy', cy.toFixed(3));
        circle.setAttribute('r', radius.toFixed(3));
        circle.setAttribute('fill', fill);
        circle.setAttribute('fill-opacity', opacity.toFixed(3));
        circle.setAttribute('stroke', stroke);
        circle.setAttribute('stroke-width', strokeWidth);
        playbackLayer.appendChild(circle);
      };
      const paintPlayback = (frameIndex) => {
        const index = clampFrame(frameIndex);
        const frame = playback.frames[index];
        const time = Number(frame.time) || 0;
        playbackLayer.replaceChildren();
        for (const ball of frame.balls ?? []) {
          const visual = visuals.get(ball.id) ?? {};
          const radius = Number(visual.radius) || 12;
          appendCircle('playback-ball-shadow', ball.x + radius * 0.12, ball.y + radius * 0.18, radius * 1.02, '#000', 0.25);
          appendCircle('playback-ball', ball.x, ball.y, radius, visual.fill || '#ffffff', 1, '#111', '1.25');
          const heading = headingForBall(index, ball);
          const speed = Math.max(0, Number(ball.speed) || 0);
          if (heading && speed > 0.05) {
            const lineLength = radius * (1.18 + Math.min(speed, 160) / 220);
            const halfLength = lineLength / 2;
            const x1 = ball.x - heading.dx * halfLength;
            const y1 = ball.y - heading.dy * halfLength;
            const x2 = ball.x + heading.dx * halfLength;
            const y2 = ball.y + heading.dy * halfLength;
            const width = Math.max(0.5, Math.min(4.8, 0.5 + speed / 55));
            const opacity = Math.max(0.18, Math.min(1, speed / 80));
            const line = document.createElementNS(ns, 'line');
            line.setAttribute('class', 'playback-heading');
            line.setAttribute('x1', x1.toFixed(3));
            line.setAttribute('y1', y1.toFixed(3));
            line.setAttribute('x2', x2.toFixed(3));
            line.setAttribute('y2', y2.toFixed(3));
            line.setAttribute('stroke-width', width.toFixed(3));
            line.setAttribute('stroke-opacity', opacity.toFixed(3));
            playbackLayer.appendChild(line);
          }
          if (visual.label) {
            const label = document.createElementNS(ns, 'text');
            label.setAttribute('class', 'playback-ball-label');
            label.setAttribute('x', Number(ball.x).toFixed(3));
            label.setAttribute('y', Number(ball.y).toFixed(3));
            label.textContent = visual.label;
            playbackLayer.appendChild(label);
          }
        }
        slider.value = String(index);
        if (timeLabel) timeLabel.textContent = `t=${formatPlaybackTime(time)}s`;
        updateEventTicker(time);
      };
      let playing = false;
      let animationId = null;
      let playStartedAt = 0;
      let playStartTime = 0;
      let playTargetTime = null;
      const stopPlayback = () => {
        playing = false;
        if (animationId !== null) cancelAnimationFrame(animationId);
        animationId = null;
        playTargetTime = null;
        if (playButton) playButton.textContent = 'Play';
      };
      const nearestFrameForTime = (time) => {
        let bestIndex = 0;
        let bestDistance = Infinity;
        playback.frames.forEach((frame, index) => {
          const distance = Math.abs((Number(frame.time) || 0) - time);
          if (distance < bestDistance) {
            bestIndex = index;
            bestDistance = distance;
          }
        });
        return bestIndex;
      };
      const startPlayback = (startIndex, targetTime = null) => {
        stopPlayback();
        const duration = Math.max(0, Number(playback.duration) || 0);
        const boundedTarget = Number.isFinite(targetTime) ? Math.max(0, Math.min(duration, targetTime)) : null;
        playStartTime = frameTime(startIndex);
        if (boundedTarget !== null && boundedTarget <= playStartTime + eventHitWindow) {
          paintPlayback(nearestFrameForTime(boundedTarget));
          return;
        }
        playing = true;
        playTargetTime = boundedTarget;
        if (playButton) playButton.textContent = 'Pause';
        playStartedAt = performance.now();
        paintPlayback(startIndex);
        animationId = requestAnimationFrame(tick);
      };
      const tick = (now) => {
        if (!playing) return;
        const duration = Math.max(0, Number(playback.duration) || 0);
        const targetTime = playTargetTime === null ? duration : playTargetTime;
        const elapsed = (now - playStartedAt) / 1000;
        const time = playStartTime + elapsed;
        if (duration > 0 && time >= targetTime - eventHitWindow) {
          paintPlayback(nearestFrameForTime(targetTime));
          stopPlayback();
          return;
        }
        paintPlayback(nearestFrameForTime(time));
        animationId = requestAnimationFrame(tick);
      };
      slider.addEventListener('input', () => {
        stopPlayback();
        paintPlayback(slider.value);
      });
      viewer.querySelectorAll('[data-playback-step]').forEach((button) => {
        button.addEventListener('click', () => {
          stopPlayback();
          paintPlayback(clampFrame(slider.value) + Number(button.dataset.playbackStep));
        });
      });
      if (nextEventButton) {
        nextEventButton.addEventListener('click', () => {
          const next = nextEventAfter(frameTime(slider.value));
          if (!next) {
            stopPlayback();
            paintPlayback(playback.frames.length - 1);
            return;
          }
          startPlayback(clampFrame(slider.value), next.time);
        });
      }
      if (playButton) {
        playButton.addEventListener('click', () => {
          if (playing) {
            stopPlayback();
            return;
          }
          let startIndex = clampFrame(slider.value);
          if (startIndex >= playback.frames.length - 1) startIndex = 0;
          startPlayback(startIndex);
        });
      }
      paintPlayback(playback.frames.length - 1);
    }
  }
  svg.addEventListener('wheel', (event) => {
    event.preventDefault();
    const rect = svg.getBoundingClientRect();
    const cx = box.x + ((event.clientX - rect.left) / rect.width) * box.width;
    const cy = box.y + ((event.clientY - rect.top) / rect.height) * box.height;
    zoom(event.deltaY < 0 ? 0.9 : 1.1, cx, cy);
  }, { passive: false });
  let drag = null;
  svg.addEventListener('pointerdown', (event) => {
    svg.setPointerCapture(event.pointerId);
    svg.classList.add('dragging');
    drag = { x: event.clientX, y: event.clientY, box: { ...box } };
  });
  svg.addEventListener('pointermove', (event) => {
    if (!drag) return;
    const rect = svg.getBoundingClientRect();
    box.x = drag.box.x - ((event.clientX - drag.x) / rect.width) * drag.box.width;
    box.y = drag.box.y - ((event.clientY - drag.y) / rect.height) * drag.box.height;
    apply();
  });
  const stopDrag = () => { drag = null; svg.classList.remove('dragging'); };
  svg.addEventListener('pointerup', stopDrag);
  svg.addEventListener('pointercancel', stopDrag);
});
</script>
"#,
    );
    html.push_str("</main>\n</body>\n</html>\n");
    html
}

fn push_download_links(html: &mut String, report: &ScenarioReport) {
    html.push_str("<div class=\"downloads\">Download: ");
    html.push_str(&format!(
        "<a href=\"{}\">{}</a>",
        escape_html(&report.image_file_name),
        escape_html(&report.image_file_name)
    ));
    html.push_str("</div>\n");
}

fn push_tooltip(html: &mut String, label: &str, tooltip: &str) {
    html.push_str(&format!(
        "<span class=\"tooltip\" tabindex=\"0\" data-tooltip=\"{}\" aria-label=\"{}\"><span class=\"tooltip-mark\" aria-hidden=\"true\">{}</span></span>",
        escape_html(tooltip),
        escape_html(tooltip),
        escape_html(label)
    ));
}

fn push_info_table(html: &mut String, rows: &[ReportInfoRow]) {
    html.push_str("<dl class=\"info-table\">\n");
    for row in rows {
        html.push_str(&format!(
            "<div class=\"info-row\"><dt>{}</dt><dd>{}</dd></div>\n",
            escape_html(&row.label),
            escape_html(&row.value)
        ));
    }
    html.push_str("</dl>\n");
}

fn anchor_id(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

fn escape_html(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn open_path(path: &Path) -> Result<(), String> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "xdg-open"
    };

    let status = if cfg!(target_os = "windows") {
        Command::new(opener)
            .args(["/C", "start", "", &path.display().to_string()])
            .status()
    } else {
        Command::new(opener).arg(path).status()
    }
    .map_err(|error| format!("failed to launch opener for {}: {error}", path.display()))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "opener exited with status {status}; open {} manually",
            path.display()
        ))
    }
}

fn print_usage() {
    println!("{}", usage_text());
}

fn usage_text() -> &'static str {
    "Usage:\n  cargo xtask validation-suite [options]\n\nOptions:\n  --scenario-dir <dir>               Directory containing .billiards files [default: examples/scenarios]\n  --output-dir <dir>                 Output directory for SVG diagrams and index.html [default: target/validation-suite]\n  --trace-sample-step-seconds <sec>  Path sampling step for rendered traces [default: 0.02]\n  --max-events <n>                   Override scenario trace/simulation event limits\n  --transparent                      Render diagrams on a transparent background\n  --open                             Open the generated index.html with the platform opener\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cue_tip_diagram_places_marker_from_shot_offsets_and_speed() {
        let svg = render_cue_tip_diagram_svg(0.25, -0.5, 0.5, 15.0);

        assert!(svg.contains("class=\"cue-tip-diagram\""));
        assert!(svg.contains("data-tip-side=\"0.250\""));
        assert!(svg.contains("data-tip-height=\"-0.500\""));
        assert!(svg.contains("data-miscue-limit=\"0.500\""));
        assert!(svg.contains("data-cue-ball-speed-mph=\"15.000\""));
        assert!(svg.contains("data-tip-marker-r=\"13.500\""));
        assert!(svg.contains("data-tip-x=\"108.000\""));
        assert!(svg.contains("data-tip-y=\"126.000\""));
        assert!(svg.contains("class=\"miscue-limit\" cx=\"90\" cy=\"90\" r=\"36.000\""));
        assert!(svg.contains("class=\"cue-tip-marker\" cx=\"108.000\" cy=\"126.000\" r=\"13.500\""));
        assert!(svg.contains("fill=\"#d91919\""));
    }

    #[test]
    fn cue_tip_marker_radius_scales_across_requested_speed_range() {
        assert_eq!(cue_tip_marker_radius(0.0, 72.0), 9.0);
        assert_eq!(cue_tip_marker_radius(30.0, 72.0), 18.0);
        assert_eq!(cue_tip_marker_radius(35.0, 72.0), 18.0);
    }

    #[test]
    fn power_meter_renders_redline_and_needle_data() {
        let svg = render_power_meter_svg(30.0, HumanShotSpeedBand::TypicalPowerBreak);

        assert!(svg.contains("class=\"power-meter\""));
        assert!(svg.contains("data-cue-ball-speed-mph=\"30.000\""));
        assert!(svg.contains("data-speed-band=\"TypicalPowerBreak\""));
        assert!(svg.contains("class=\"power-meter-redline\""));
        assert!(svg.contains("Gauge spans 0 to 35 miles per hour"));
    }

    #[test]
    fn validation_report_embeds_tabular_info_and_visual_stack() {
        let report = ScenarioReport {
            name: "cue tip test".to_string(),
            image_file_name: "cue_tip_test.svg".to_string(),
            inline_svg: "<svg></svg>".to_string(),
            notes: Vec::new(),
            info_rows: vec![
                ReportInfoRow::new("Source", "examples/scenarios/cue_tip_test.billiards"),
                ReportInfoRow::new("Heading", "90.00°"),
                ReportInfoRow::new("Cue-ball launch", "7.27 mph · medium speed · medium band"),
            ],
            cue_tip_diagram_svg: Some(render_cue_tip_diagram_svg(0.25, -0.5, 0.5, 7.27)),
            power_meter_svg: Some(render_power_meter_svg(7.27, HumanShotSpeedBand::Medium)),
            playback: None,
            events: Vec::new(),
        };

        let html = render_html(&[report], &ValidationSuiteOptions::default());

        assert!(html.contains("class=\"card-overview\""));
        assert!(html.contains("class=\"visual-stack\""));
        assert!(html.contains("<strong>Cue tip</strong>"));
        assert!(html.contains("<strong>Power</strong>"));
        assert!(html.contains("class=\"tooltip\""));
        assert!(html.contains("data-tooltip=\"Red marker: tip-contact offset"));
        assert!(!html.contains("Red dot position is the tip contact"));
        assert!(html.contains("<dl class=\"info-table\""));
        assert!(html.contains("<dt>Heading</dt><dd>90.00°</dd>"));
        assert!(html.contains("<svg class=\"cue-tip-diagram\""));
        assert!(html.contains("<svg class=\"power-meter\""));
    }

    #[test]
    fn validation_report_embeds_playback_ticker_and_event_times() {
        let report = ScenarioReport {
            name: "playback ticker test".to_string(),
            image_file_name: "playback_ticker_test.svg".to_string(),
            inline_svg: "<svg viewBox=\"0 0 100 100\"><g data-layer=\"balls\"></g></svg>"
                .to_string(),
            notes: Vec::new(),
            info_rows: Vec::new(),
            cue_tip_diagram_svg: None,
            power_meter_svg: None,
            playback: Some(ScenarioPlaybackReport {
                duration: 0.5,
                events: vec![ScenarioPlaybackEventReport {
                    label: "(1)".to_string(),
                    time: 0.125,
                    summary: "cue -> one collision".to_string(),
                }],
                balls: vec![ScenarioPlaybackBallVisual {
                    id: "cue".to_string(),
                    fill: "#f8f4e8",
                    label: Some("C"),
                    radius: 10.0,
                }],
                frames: vec![
                    ScenarioPlaybackFrameReport {
                        time: 0.0,
                        balls: vec![ScenarioPlaybackBallReport {
                            id: "cue".to_string(),
                            x: 10.0,
                            y: 20.0,
                            speed_ips: 5.0,
                        }],
                    },
                    ScenarioPlaybackFrameReport {
                        time: 0.125,
                        balls: vec![ScenarioPlaybackBallReport {
                            id: "cue".to_string(),
                            x: 20.0,
                            y: 20.0,
                            speed_ips: 0.0,
                        }],
                    },
                ],
            }),
            events: vec![ScenarioEventReport {
                label: "(1)".to_string(),
                time: "0.125000".to_string(),
                time_seconds: 0.125,
                summary: "cue -> one collision".to_string(),
                payload: "BallBallCollision".to_string(),
            }],
        };

        let html = render_html(&[report], &ValidationSuiteOptions::default());

        assert!(html.contains("data-playback-next-event"));
        assert!(html.contains("data-playback-event"));
        assert!(html.contains("play to the next logged event"));
        assert!(html.contains("\"events\":[{\"label\":\"(1)\",\"time\":0.125000,\"summary\":\"cue -\\u003e one collision\"}]"));
        assert!(html.contains("data-event-time=\"0.125000\""));
        assert!(html.contains("nextEventAfter"));
        assert!(html.contains("event-current"));
    }

    #[test]
    fn validation_suite_options_reject_removed_format_option() {
        let args = ["--format".to_string(), "svg".to_string()];

        let error = ValidationSuiteOptions::parse(&args).expect_err("format option is gone");

        assert!(error.contains("unknown validation-suite option `--format`"));
        assert!(!usage_text().contains("--format"));
    }
}
