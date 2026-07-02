use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use billiards::dsl::{parse_dsl_to_scenario, ScenarioShotTrace, ScenarioTraceRenderOptions};
use billiards::visualization::{BallPathRenderOptions, PathColorMode};
use billiards::{
    diagram::DiagramOutputFormat, human_tuned_preview_motion_config, CollisionModel,
    DiagramBackground, DiagramRenderOptions, HumanShotSpeedBand, NBallSystemState, RailModel,
    Seconds, ShotSpeedPreset,
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
    source_path: PathBuf,
    image_file_name: String,
    inline_svg: String,
    notes: Vec<String>,
    shot_line: Option<String>,
    speed_summary: Option<String>,
    shot_summary: Option<String>,
    cue_tip_diagram_svg: Option<String>,
    simulation_summary: String,
    events: Vec<ScenarioEventReport>,
}

#[derive(Debug)]
struct ScenarioEventReport {
    label: String,
    time: String,
    summary: String,
    payload: String,
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
        labels: true,
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

    let (render_state, simulation_summary, events) = if let Some(trace) = trace {
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
        (
            trace.rendered_final_layout_with_trace_options(&scenario, &trace_render),
            summary,
            events,
        )
    } else {
        (
            scenario.game_state.clone(),
            "No shot defined; rendered initial layout only".to_string(),
            Vec::new(),
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

    let speed_summary = scenario
        .validate_shot_human_speed()
        .map_err(|error| {
            format!(
                "failed to validate shot speed for {}: {error}",
                scenario_path.display()
            )
        })?
        .map(|validation| {
            let nearest = ShotSpeedPreset::nearest_to_speed(
                &validation.estimated_cue_ball_speed_after_impact,
            );
            format!(
                "cue-ball launch {:.2} mph ({}, {} band); cue stick {:.2} mph at impact ({})",
                validation.estimated_cue_ball_speed_after_impact.as_mph(),
                nearest.human_label(),
                speed_band_label(validation.cue_ball_speed_band),
                validation.cue_speed_at_impact.as_mph(),
                speed_band_label(validation.cue_speed_band)
            )
        });

    let shot_summary = scenario.shot.as_ref().map(|shot| {
        format!(
            "{:?} shot, heading {:.2}°, tip side {:+.2}R, height {:+.2}R",
            shot.ball,
            shot.shot.heading().as_degrees(),
            shot.shot.tip_contact().side_offset().as_f64(),
            shot.shot.tip_contact().height_offset().as_f64()
        )
    });
    let cue_tip_diagram_svg = scenario.shot.as_ref().map(|shot| {
        render_cue_tip_diagram_svg(
            shot.shot.tip_contact().side_offset().as_f64(),
            shot.shot.tip_contact().height_offset().as_f64(),
            shot.cue_strike.miscue_offset_limit().as_f64(),
        )
    });

    Ok(ScenarioReport {
        name: stem.replace('_', " "),
        source_path: scenario_path.to_path_buf(),
        image_file_name: svg_file_name,
        inline_svg: svg,
        notes: scenario_notes(&source),
        shot_line: source
            .lines()
            .find(|line| line.trim_start().starts_with("shot("))
            .map(|line| line.trim().to_string()),
        speed_summary,
        shot_summary,
        cue_tip_diagram_svg,
        simulation_summary,
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
        .map(|(index, event)| ScenarioEventReport {
            label: format!("({})", index + 1),
            time: format!("{:.6}", event.time.as_f64()),
            summary: event.kind.format_human(),
            payload: format!("{:#?}", event.kind),
        })
        .collect()
}

fn render_cue_tip_diagram_svg(
    side_offset: f64,
    height_offset: f64,
    miscue_offset_limit: f64,
) -> String {
    let ball_radius = 72.0;
    let ball_center = 90.0;
    let tip_x = ball_center + side_offset * ball_radius;
    let tip_y = ball_center - height_offset * ball_radius;
    let limit_radius = miscue_offset_limit.clamp(0.0, 1.0) * ball_radius;
    let offset_radius = side_offset.hypot(height_offset);
    let limit_status = if offset_radius <= miscue_offset_limit + 1e-12 {
        "inside"
    } else {
        "outside"
    };

    format!(
        r##"<svg class="cue-tip-diagram" data-tip-side="{side_offset:.3}" data-tip-height="{height_offset:.3}" data-miscue-limit="{miscue_offset_limit:.3}" data-tip-x="{tip_x:.3}" data-tip-y="{tip_y:.3}" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 180 184" role="img" aria-label="Cue ball tip contact: side {side_offset:+.2} ball radii, height {height_offset:+.2} ball radii, {limit_status} the {miscue_offset_limit:.2} ball-radius miscue limit">
<ellipse class="cue-ball-shadow" cx="94" cy="165" rx="62" ry="16" fill="#000000" opacity=".35"/>
<circle class="cue-ball-body" cx="{ball_center:.0}" cy="{ball_center:.0}" r="{ball_radius:.0}" fill="#e9e0c9" stroke="#fff9e9" stroke-width="1.5"/>
<circle class="cue-ball-shade" cx="116" cy="118" r="48" fill="#000000" opacity=".10"/>
<circle class="cue-ball-highlight" cx="64" cy="50" r="38" fill="#ffffff" opacity=".24"/>
<ellipse class="cue-ball-glare" cx="61" cy="43" rx="20" ry="12" fill="#ffffff" opacity=".72" transform="rotate(-25 61 43)"/>
<path d="M39 121C53 145 81 158 113 150" fill="none" stroke="#ffffff" stroke-opacity=".28" stroke-width="6" stroke-linecap="round"/>
<circle class="miscue-limit" cx="{ball_center:.0}" cy="{ball_center:.0}" r="{limit_radius:.3}" fill="none" stroke="#090909" stroke-width="2.75"/>
<circle class="cue-tip-marker" cx="{tip_x:.3}" cy="{tip_y:.3}" r="7" fill="#d91919" stroke="#ffffff" stroke-width="2"/>
<circle class="cue-tip-marker-outline" cx="{tip_x:.3}" cy="{tip_y:.3}" r="10" fill="none" stroke="#7b0000" stroke-opacity=".65" stroke-width="1.5"/>
</svg>
"##
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
            "<li data-event-label=\"{}\" data-event-title=\"{}\"><span class=\"event-badge\" title=\"{}\">{}</span><code class=\"event-time\">t={}s</code><span class=\"event-summary\">{}</span><pre class=\"event-payload\"><code>{}</code></pre></li>\n",
            escape_html(&event.label),
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
    html.push_str(
        ":root{color-scheme:light dark;font-family:Inter,system-ui,-apple-system,BlinkMacSystemFont,Segoe UI,sans-serif;line-height:1.45}\n\
         *{box-sizing:border-box}\n\
         body{margin:0;background:#101410;color:#f1f5ef}\n\
         header{position:sticky;top:0;z-index:2;background:rgba(16,20,16,.94);backdrop-filter:blur(8px);border-bottom:1px solid #314131;padding:1rem 1.25rem}\n\
         h1{margin:.1rem 0 .35rem;font-size:clamp(1.25rem,2.4vw,1.65rem)}\n\
         .subtitle{color:#bfd0bb;margin:0;overflow-wrap:anywhere}\n\
         main{width:min(100%,1220px);margin:0 auto;padding:1.25rem}\n\
         .toc{display:flex;flex-wrap:wrap;gap:.45rem;margin:1rem 0 1.25rem}\n\
         .toc a{color:#dff5d7;background:#263326;border:1px solid #405440;border-radius:999px;padding:.3rem .65rem;text-decoration:none;font-size:.9rem;overflow-wrap:anywhere}\n\
         .card{background:#182018;border:1px solid #354635;border-radius:16px;margin:0 0 1.25rem;overflow:hidden;box-shadow:0 12px 30px rgba(0,0,0,.24)}\n\
         .card h2{margin:0;padding:1rem 1rem .35rem;font-size:1.25rem;overflow-wrap:anywhere}\n\
         .meta{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,240px),1fr));gap:.55rem;padding:0 1rem 1rem;color:#d5e4d0}\n\
         .meta div{min-width:0;background:#111811;border:1px solid #293829;border-radius:10px;padding:.55rem .65rem;overflow-wrap:anywhere}\n\
         .meta strong{display:block;color:#f7fff2;margin-bottom:.2rem;font-size:.8rem;text-transform:uppercase;letter-spacing:.04em}\n\
         figure{min-width:0;margin:0;background:#253025;padding:1rem;border-top:1px solid #354635;border-bottom:1px solid #354635}\n\
         img{display:block;max-width:100%;height:auto;margin:0 auto;border-radius:10px;background:#0a0d0a}\n\
         .svg-viewer{display:grid;gap:.7rem;min-width:0}\n\
         .viewer-controls{display:flex;flex-wrap:wrap;align-items:center;gap:.45rem;color:#d5e4d0;font-size:.9rem}\n\
         .viewer-controls button{background:#111811;color:#f1f5ef;border:1px solid #405440;border-radius:8px;padding:.3rem .55rem;cursor:pointer}\n\
         .viewer-controls label{display:inline-flex;align-items:center;gap:.25rem;background:#182018;border:1px solid #405440;border-radius:999px;padding:.25rem .55rem;max-width:100%;overflow-wrap:anywhere}\n\
         .svg-frame{overflow:hidden;border-radius:10px;background:#0a0d0a;touch-action:none;min-width:0}\n\
         .svg-frame svg{display:block;max-width:100%;width:auto;height:auto;margin:0 auto;cursor:grab}\n\
         .svg-frame svg.dragging{cursor:grabbing}\n\
         .downloads{display:flex;flex-wrap:wrap;gap:.5rem;margin:.55rem 0 0;font-size:.9rem}\n\
         .downloads a{overflow-wrap:anywhere}\n\
         details{padding:.85rem 1rem 1rem;min-width:0}\n\
         summary{cursor:pointer;color:#f7fff2;font-weight:700}\n\
         pre{white-space:pre-wrap;overflow:auto;max-height:28rem;background:#0c110c;border:1px solid #2a382a;border-radius:10px;padding:.85rem;color:#dcead8;overflow-wrap:anywhere}\n\
         code{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;overflow-wrap:anywhere}\n\
         .notes{margin:.25rem 0 0;padding-left:1.1rem;overflow-wrap:anywhere}\n\
         .notes li{margin:.2rem 0}\n\
         .event-log{padding-top:1rem}\n\
         .event-list{list-style:none;margin:.75rem 0 0;padding:0;display:grid;gap:.65rem}\n\
         .event-list li{min-width:0;background:#111811;border:1px solid #293829;border-radius:10px;padding:.65rem;display:grid;grid-template-columns:auto auto minmax(0,1fr);gap:.35rem .6rem;align-items:start}\n\
         .event-badge{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-weight:800;color:#101410;background:#a8e89a;border-radius:999px;padding:.05rem .45rem;cursor:help}\n\
         .event-time{color:#bfd0bb;white-space:nowrap}\n\
         .event-summary{min-width:0;overflow-wrap:anywhere}\n\
         .event-payload{grid-column:1 / -1;margin:.2rem 0 0;max-height:9rem;font-size:.82rem}\n\
         a{color:#a8e89a}\n\
         @media (max-width:720px){header{position:static;padding:.85rem}main{padding:.75rem}.toc{gap:.35rem}.card{border-radius:12px}figure{padding:.65rem}.viewer-controls{align-items:stretch}.viewer-controls button,.viewer-controls label{flex:1 1 auto;justify-content:center}}\n",
    );
    html.push_str(
        ".cue-tip-card{display:grid;grid-template-columns:minmax(11rem,15rem) minmax(0,1fr);gap:1rem;align-items:center;margin:0 1rem 1rem;padding:1rem;background:#111811;border:1px solid #293829;border-radius:14px;color:#d5e4d0}\n\
         .cue-tip-card h3{margin:0 0 .25rem;color:#f7fff2;font-size:1rem}\n\
         .cue-tip-card p{margin:.25rem 0 0;overflow-wrap:anywhere}\n\
         .cue-tip-card svg{width:100%;height:auto;max-width:15rem;justify-self:center}\n\
         .cue-tip-marker{filter:drop-shadow(0 1px 2px rgba(0,0,0,.45))}\n\
         @media (max-width:720px){.cue-tip-card{grid-template-columns:1fr;margin:.65rem;padding:.75rem}}\n",
    );
    html.push_str("</style>\n</head>\n<body>\n");
    html.push_str("<header>\n<h1>Billiards scenario validation suite</h1>\n");
    html.push_str(&format!(
        "<p class=\"subtitle\">{} scenario diagram(s), generated from <code>{}</code> into <code>{}</code> as inline SVG. Speeds are cue-ball launch estimates unless noted.</p>\n",
        reports.len(),
        escape_html(&options.scenario_dir.display().to_string()),
        escape_html(&options.output_dir.display().to_string()),
    ));
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
        html.push_str("<div class=\"meta\">\n");
        html.push_str(&meta_block(
            "Source",
            &report.source_path.display().to_string(),
        ));
        html.push_str(&meta_block("Simulation", &report.simulation_summary));
        if let Some(summary) = &report.speed_summary {
            html.push_str(&meta_block("Speed", summary));
        }
        if let Some(summary) = &report.shot_summary {
            html.push_str(&meta_block("Shot", summary));
        }
        if let Some(shot_line) = &report.shot_line {
            html.push_str(&meta_block("DSL shot", shot_line));
        }
        html.push_str("</div>\n");
        if !report.notes.is_empty() {
            html.push_str(
                "<details open><summary>Scenario context</summary><ul class=\"notes\">\n",
            );
            for note in &report.notes {
                html.push_str(&format!("<li>{}</li>\n", escape_html(note)));
            }
            html.push_str("</ul></details>\n");
        }
        if let Some(cue_tip_diagram_svg) = &report.cue_tip_diagram_svg {
            html.push_str(
                "<aside class=\"cue-tip-card\" aria-label=\"Cue-tip placement diagram\">\n",
            );
            html.push_str(cue_tip_diagram_svg);
            html.push_str(
                "<div><h3>Cue-tip placement</h3><p>The red dot is the shot tip contact in cue-ball-radius units. The black circle is the configured maximum clean cuing offset before a miscue.</p></div>\n</aside>\n",
            );
        }
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
  const eventTitles = new Map(Array.from(card?.querySelectorAll('[data-event-label]') ?? []).map((row) => [row.dataset.eventLabel, row.dataset.eventTitle]));
  svg.querySelectorAll('text.overlay-label').forEach((label) => {
    const marker = (label.childNodes[0]?.nodeValue ?? label.textContent).trim();
    const titleText = eventTitles.get(marker);
    if (!titleText) return;
    label.setAttribute('tabindex', '0');
    label.setAttribute('aria-label', titleText);
    label.classList.add('event-label-tooltip');
    if (!label.querySelector('title')) {
      const title = document.createElementNS('http://www.w3.org/2000/svg', 'title');
      title.textContent = titleText;
      label.appendChild(title);
    }
  });
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

fn meta_block(label: &str, value: &str) -> String {
    format!(
        "<div><strong>{}</strong>{}</div>\n",
        escape_html(label),
        escape_html(value)
    )
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
    fn cue_tip_diagram_places_marker_from_shot_offsets() {
        let svg = render_cue_tip_diagram_svg(0.25, -0.5, 0.5);

        assert!(svg.contains("class=\"cue-tip-diagram\""));
        assert!(svg.contains("data-tip-side=\"0.250\""));
        assert!(svg.contains("data-tip-height=\"-0.500\""));
        assert!(svg.contains("data-miscue-limit=\"0.500\""));
        assert!(svg.contains("data-tip-x=\"108.000\""));
        assert!(svg.contains("data-tip-y=\"126.000\""));
        assert!(svg.contains("class=\"miscue-limit\" cx=\"90\" cy=\"90\" r=\"36.000\""));
        assert!(svg.contains("class=\"cue-tip-marker\" cx=\"108.000\" cy=\"126.000\""));
        assert!(svg.contains("fill=\"#d91919\""));
    }

    #[test]
    fn validation_report_embeds_cue_tip_diagram_panel() {
        let report = ScenarioReport {
            name: "cue tip test".to_string(),
            source_path: PathBuf::from("examples/scenarios/cue_tip_test.billiards"),
            image_file_name: "cue_tip_test.svg".to_string(),
            inline_svg: "<svg></svg>".to_string(),
            notes: Vec::new(),
            shot_line: None,
            speed_summary: None,
            shot_summary: Some(
                "cue shot, heading 90.00°, tip side +0.25R, height -0.50R".to_string(),
            ),
            cue_tip_diagram_svg: Some(render_cue_tip_diagram_svg(0.25, -0.5, 0.5)),
            simulation_summary: "Simulated shot to rest: 1 event(s)".to_string(),
            events: Vec::new(),
        };

        let html = render_html(&[report], &ValidationSuiteOptions::default());

        assert!(html.contains("Cue-tip placement"));
        assert!(html.contains("<aside class=\"cue-tip-card\""));
        assert!(html.contains("<svg class=\"cue-tip-diagram\""));
        assert!(html.contains("data-tip-side=\"0.250\""));
        assert!(html.contains("data-tip-height=\"-0.500\""));
    }

    #[test]
    fn validation_suite_options_reject_removed_format_option() {
        let args = ["--format".to_string(), "svg".to_string()];

        let error = ValidationSuiteOptions::parse(&args).expect_err("format option is gone");

        assert!(error.contains("unknown validation-suite option `--format`"));
        assert!(!usage_text().contains("--format"));
    }
}
