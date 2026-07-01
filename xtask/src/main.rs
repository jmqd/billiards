use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use billiards::dsl::{parse_dsl_to_scenario, ScenarioShotTrace, ScenarioTraceRenderOptions};
use billiards::visualization::{BallPathRenderOptions, PathColorMode};
use billiards::{
    diagram::DiagramOutputFormat, human_tuned_preview_motion_config, BallSetPhysicsSpec,
    CollisionModel, DiagramBackground, DiagramRenderOptions, HumanShotSpeedBand, NBallSystemState,
    RailModel, Seconds, ShotSpeedPreset,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValidationDiagramFormat {
    Svg,
    Png,
    Both,
}

impl ValidationDiagramFormat {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "svg" => Ok(Self::Svg),
            "png" => Ok(Self::Png),
            "both" => Ok(Self::Both),
            other => Err(format!(
                "invalid --format `{other}`; expected svg, png, or both"
            )),
        }
    }

    fn writes_svg(self) -> bool {
        matches!(self, Self::Svg | Self::Both)
    }

    fn writes_png(self) -> bool {
        matches!(self, Self::Png | Self::Both)
    }

    fn label(self) -> &'static str {
        match self {
            Self::Svg => "SVG",
            Self::Png => "PNG",
            Self::Both => "SVG + PNG",
        }
    }
}

#[derive(Debug)]
struct ValidationSuiteOptions {
    scenario_dir: PathBuf,
    output_dir: PathBuf,
    scale_factor: u32,
    trace_sample_step_seconds: f64,
    max_events_override: Option<usize>,
    transparent_background: bool,
    format: ValidationDiagramFormat,
    open: bool,
}

impl Default for ValidationSuiteOptions {
    fn default() -> Self {
        Self {
            scenario_dir: PathBuf::from("examples/scenarios"),
            output_dir: PathBuf::from("target/validation-suite"),
            scale_factor: 1,
            trace_sample_step_seconds: 0.02,
            max_events_override: None,
            transparent_background: false,
            format: ValidationDiagramFormat::Svg,
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
                "--scale-factor" => {
                    index += 1;
                    options.scale_factor = value_after(raw_args, index, "--scale-factor")?
                        .parse::<u32>()
                        .map_err(|error| format!("invalid --scale-factor: {error}"))?
                        .max(1);
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
                "--format" => {
                    index += 1;
                    options.format =
                        ValidationDiagramFormat::parse(value_after(raw_args, index, "--format")?)?;
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
    extra_file_names: Vec<String>,
    inline_svg: Option<String>,
    notes: Vec<String>,
    shot_line: Option<String>,
    speed_summary: Option<String>,
    shot_summary: Option<String>,
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

    let ball_set = BallSetPhysicsSpec::default();
    let motion = human_tuned_preview_motion_config();
    let trace_render = ScenarioTraceRenderOptions {
        path_render: BallPathRenderOptions {
            max_time_step: Seconds::new(options.trace_sample_step_seconds),
            ..BallPathRenderOptions::default()
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
        scale_factor: options.scale_factor,
        background: if options.transparent_background {
            DiagramBackground::Transparent
        } else {
            DiagramBackground::Table
        },
    };
    let mut image_file_name = String::new();
    let mut extra_file_names = Vec::new();
    let mut inline_svg = None;

    if options.format.writes_svg() {
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
        image_file_name = svg_file_name;
        inline_svg = Some(svg);
    }

    if options.format.writes_png() {
        let png =
            render_state.render_2d_diagram_with_options(DiagramOutputFormat::Png, &render_options);
        if png.is_empty() {
            return Err(format!(
                "rendered empty PNG for {}",
                scenario_path.display()
            ));
        }
        let png_file_name = format!("{stem}.png");
        let png_path = options.output_dir.join(&png_file_name);
        fs::write(&png_path, png)
            .map_err(|error| format!("failed to write {}: {error}", png_path.display()))?;
        if image_file_name.is_empty() {
            image_file_name = png_file_name;
        } else {
            extra_file_names.push(png_file_name);
        }
    }

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

    Ok(ScenarioReport {
        name: stem.replace('_', " "),
        source_path: scenario_path.to_path_buf(),
        extra_file_names,
        inline_svg,
        image_file_name,
        notes: scenario_notes(&source),
        shot_line: source
            .lines()
            .find(|line| line.trim_start().starts_with("shot("))
            .map(|line| line.trim().to_string()),
        speed_summary,
        shot_summary,
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
         .svg-frame svg{display:block;width:100%;height:auto;max-height:min(82vh,1100px);cursor:grab}\n\
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
    html.push_str("</style>\n</head>\n<body>\n");
    html.push_str("<header>\n<h1>Billiards scenario validation suite</h1>\n");
    html.push_str(&format!(
        "<p class=\"subtitle\">{} scenario diagram(s), generated from <code>{}</code> into <code>{}</code> as {}. Speeds are cue-ball launch estimates unless noted.</p>\n",
        reports.len(),
        escape_html(&options.scenario_dir.display().to_string()),
        escape_html(&options.output_dir.display().to_string()),
        options.format.label()
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
        if let Some(svg) = &report.inline_svg {
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
            html.push_str(svg);
            html.push_str("</div>\n");
            push_download_links(&mut html, report);
            html.push_str("</figure>\n");
        } else {
            html.push_str(&format!(
                "<figure><img src=\"{}\" alt=\"{} scenario diagram\">",
                escape_html(&report.image_file_name),
                escape_html(&report.name)
            ));
            push_download_links(&mut html, report);
            html.push_str("</figure>\n");
        }
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
    html.push_str("<div class=\"downloads\">Downloads: ");
    html.push_str(&format!(
        "<a href=\"{}\">{}</a>",
        escape_html(&report.image_file_name),
        escape_html(&report.image_file_name)
    ));
    for file_name in &report.extra_file_names {
        html.push_str(&format!(
            " <a href=\"{}\">{}</a>",
            escape_html(file_name),
            escape_html(file_name)
        ));
    }
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
    "Usage:\n  cargo xtask validation-suite [options]\n\nOptions:\n  --scenario-dir <dir>               Directory containing .billiards files [default: examples/scenarios]\n  --output-dir <dir>                 Output directory for diagrams and index.html [default: target/validation-suite]\n  --format <svg|png|both>            Diagram output format [default: svg]\n  --scale-factor <n>                 Positive integer render scale for PNG exports [default: 1]\n  --trace-sample-step-seconds <sec>  Path sampling step for rendered traces [default: 0.02]\n  --max-events <n>                   Override scenario trace/simulation event limits\n  --transparent                      Render diagrams on a transparent background\n  --open                             Open the generated index.html with the platform opener\n"
}
