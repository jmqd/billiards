use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use billiards::dsl::{parse_dsl_to_scenario, ScenarioTraceRenderOptions};
use billiards::visualization::{BallPathRenderOptions, PathColorMode};
use billiards::{
    human_tuned_preview_motion_config, BallSetPhysicsSpec, CollisionModel, DiagramBackground,
    DiagramRenderOptions, HumanShotSpeedBand, NBallSystemState, RailModel, Seconds,
    ShotSpeedPreset,
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
    scale_factor: u32,
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
            scale_factor: 1,
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
    notes: Vec<String>,
    shot_line: Option<String>,
    speed_summary: Option<String>,
    shot_summary: Option<String>,
    simulation_summary: String,
    event_lines: Vec<String>,
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

    let (render_state, simulation_summary, event_lines) = if let Some(trace) = trace {
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
        let event_lines = trace.event_lines();
        (
            trace.rendered_final_layout_with_trace_options(&scenario, &trace_render),
            summary,
            event_lines,
        )
    } else {
        (
            scenario.game_state.clone(),
            "No shot defined; rendered initial layout only".to_string(),
            Vec::new(),
        )
    };

    let image = render_state.draw_2d_diagram_with_options(&DiagramRenderOptions {
        scale_factor: options.scale_factor,
        background: if options.transparent_background {
            DiagramBackground::Transparent
        } else {
            DiagramBackground::Table
        },
    });
    if image.is_empty() {
        return Err(format!(
            "rendered empty PNG for {}",
            scenario_path.display()
        ));
    }

    let stem = scenario_path
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| format!("invalid scenario file name {}", scenario_path.display()))?;
    let image_file_name = format!("{stem}.png");
    let image_path = options.output_dir.join(&image_file_name);
    fs::write(&image_path, image)
        .map_err(|error| format!("failed to write {}: {error}", image_path.display()))?;

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
        image_file_name,
        notes: scenario_notes(&source),
        shot_line: source
            .lines()
            .find(|line| line.trim_start().starts_with("shot("))
            .map(|line| line.trim().to_string()),
        speed_summary,
        shot_summary,
        simulation_summary,
        event_lines,
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

fn render_html(reports: &[ScenarioReport], options: &ValidationSuiteOptions) -> String {
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str("<title>Billiards scenario validation suite</title>\n");
    html.push_str("<style>\n");
    html.push_str(
        ":root{color-scheme:light dark;font-family:Inter,system-ui,-apple-system,BlinkMacSystemFont,Segoe UI,sans-serif;line-height:1.45}\n\
         body{margin:0;background:#101410;color:#f1f5ef}\n\
         header{position:sticky;top:0;z-index:2;background:rgba(16,20,16,.94);backdrop-filter:blur(8px);border-bottom:1px solid #314131;padding:1rem 1.25rem}\n\
         h1{margin:.1rem 0 .35rem;font-size:1.55rem}\n\
         .subtitle{color:#bfd0bb;margin:0}\n\
         main{max-width:1220px;margin:0 auto;padding:1.25rem}\n\
         .toc{display:flex;flex-wrap:wrap;gap:.45rem;margin:1rem 0 1.25rem}\n\
         .toc a{color:#dff5d7;background:#263326;border:1px solid #405440;border-radius:999px;padding:.3rem .65rem;text-decoration:none;font-size:.9rem}\n\
         .card{background:#182018;border:1px solid #354635;border-radius:16px;margin:0 0 1.25rem;overflow:hidden;box-shadow:0 12px 30px rgba(0,0,0,.24)}\n\
         .card h2{margin:0;padding:1rem 1rem .35rem;font-size:1.25rem}\n\
         .meta{display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:.55rem;padding:0 1rem 1rem;color:#d5e4d0}\n\
         .meta div{background:#111811;border:1px solid #293829;border-radius:10px;padding:.55rem .65rem}\n\
         .meta strong{display:block;color:#f7fff2;margin-bottom:.2rem;font-size:.8rem;text-transform:uppercase;letter-spacing:.04em}\n\
         figure{margin:0;background:#253025;padding:1rem;border-top:1px solid #354635;border-bottom:1px solid #354635}\n\
         img{display:block;max-width:100%;height:auto;margin:0 auto;border-radius:10px;background:#0a0d0a}\n\
         details{padding:.85rem 1rem 1rem}\n\
         summary{cursor:pointer;color:#f7fff2;font-weight:700}\n\
         pre{white-space:pre-wrap;overflow:auto;background:#0c110c;border:1px solid #2a382a;border-radius:10px;padding:.85rem;color:#dcead8}\n\
         code{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}\n\
         .notes{margin:.25rem 0 0;padding-left:1.1rem}\n\
         .notes li{margin:.2rem 0}\n\
         a{color:#a8e89a}\n",
    );
    html.push_str("</style>\n</head>\n<body>\n");
    html.push_str("<header>\n<h1>Billiards scenario validation suite</h1>\n");
    html.push_str(&format!(
        "<p class=\"subtitle\">{} scenario diagram(s), generated from <code>{}</code> into <code>{}</code>. Speeds are cue-ball launch estimates unless noted.</p>\n",
        reports.len(),
        escape_html(&options.scenario_dir.display().to_string()),
        escape_html(&options.output_dir.display().to_string())
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
        html.push_str(&format!(
            "<figure><img src=\"{}\" alt=\"{} scenario diagram\"></figure>\n",
            escape_html(&report.image_file_name),
            escape_html(&report.name)
        ));
        if !report.event_lines.is_empty() {
            html.push_str("<details><summary>Event log</summary><pre><code>");
            html.push_str(&escape_html(&report.event_lines.join("\n")));
            html.push_str("</code></pre></details>\n");
        }
        html.push_str("</section>\n");
    }

    html.push_str("</main>\n</body>\n</html>\n");
    html
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
    "Usage:\n  cargo xtask validation-suite [options]\n\nOptions:\n  --scenario-dir <dir>               Directory containing .billiards files [default: examples/scenarios]\n  --output-dir <dir>                 Output directory for PNGs and index.html [default: target/validation-suite]\n  --scale-factor <n>                 Positive integer render scale [default: 1]\n  --trace-sample-step-seconds <sec>  Path sampling step for rendered traces [default: 0.02]\n  --max-events <n>                   Override scenario trace/simulation event limits\n  --transparent                      Render diagrams on a transparent background\n  --open                             Open the generated index.html with the platform opener\n"
}
