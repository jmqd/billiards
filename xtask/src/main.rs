use std::env;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};
use std::thread;

use serde::Deserialize;

use billiards::dsl::{parse_dsl_to_scenario, ScenarioShotTrace, ScenarioTraceRenderOptions};
use billiards::visualization::{
    BallPathRenderOptions, PathColorMode, DEFAULT_BALL_PATH_MAX_TIME_STEP_SECONDS,
};
use billiards::{
    diagram::{DiagramOutputFormat, DiagramViewport},
    human_tuned_preview_motion_config, BallType, CollisionModel, DiagramBackground,
    DiagramRenderOptions, GameState, HumanShotSpeedBand, NBallSystemState, RailModel, Seconds,
    ShotSpeedPreset, TableSpec,
};

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask manifest directory should be inside the workspace root")
}

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
        CommandName::BaseSvg(options) => run_base_svg(&options),
        CommandName::WasmPreview(options) => run_wasm_preview(&options),
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
    BaseSvg(BaseSvgOptions),
    WasmPreview(WasmPreviewOptions),
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

#[derive(Debug)]
struct BaseSvgOptions {
    output_path: PathBuf,
    transparent_background: bool,
}

#[derive(Debug)]
struct WasmPreviewOptions {
    output_dir: PathBuf,
    host: String,
    port: u16,
    serve: bool,
}

impl Default for BaseSvgOptions {
    fn default() -> Self {
        Self {
            output_path: workspace_root().join("target/base-pocket-table.svg"),
            transparent_background: false,
        }
    }
}

impl Default for WasmPreviewOptions {
    fn default() -> Self {
        Self {
            output_dir: workspace_root().join("target/wasm-preview"),
            host: "127.0.0.1".to_string(),
            port: 8000,
            serve: true,
        }
    }
}

impl Default for ValidationSuiteOptions {
    fn default() -> Self {
        Self {
            scenario_dir: workspace_root().join("examples/scenarios"),
            output_dir: workspace_root().join("target/validation-suite"),
            trace_sample_step_seconds: DEFAULT_BALL_PATH_MAX_TIME_STEP_SECONDS,
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
            "base-svg" | "base-table-svg" => Ok(Self {
                command: CommandName::BaseSvg(BaseSvgOptions::parse(&raw_args[1..])?),
            }),
            "wasm-preview" | "wasm-web" => Ok(Self {
                command: CommandName::WasmPreview(WasmPreviewOptions::parse(&raw_args[1..])?),
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

impl BaseSvgOptions {
    fn parse(raw_args: &[String]) -> Result<Self, String> {
        let mut options = Self::default();
        let mut index = 0;
        while index < raw_args.len() {
            match raw_args[index].as_str() {
                "--output" => {
                    index += 1;
                    options.output_path = PathBuf::from(value_after(raw_args, index, "--output")?);
                }
                "--transparent" => {
                    options.transparent_background = true;
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                other => return Err(format!("unknown base-svg option `{other}`")),
            }
            index += 1;
        }

        Ok(options)
    }
}

impl WasmPreviewOptions {
    fn parse(raw_args: &[String]) -> Result<Self, String> {
        let mut options = Self::default();
        let mut index = 0;
        while index < raw_args.len() {
            match raw_args[index].as_str() {
                "--output-dir" => {
                    index += 1;
                    options.output_dir =
                        PathBuf::from(value_after(raw_args, index, "--output-dir")?);
                }
                "--host" => {
                    index += 1;
                    options.host = value_after(raw_args, index, "--host")?.to_string();
                }
                "--port" => {
                    index += 1;
                    options.port = value_after(raw_args, index, "--port")?
                        .parse::<u16>()
                        .map_err(|error| format!("invalid --port: {error}"))?;
                }
                "--no-serve" => {
                    options.serve = false;
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                other => return Err(format!("unknown wasm-preview option `{other}`")),
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

fn run_base_svg(options: &BaseSvgOptions) -> Result<(), String> {
    if let Some(parent) = options
        .output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| {
            format!("failed to create output dir {}: {error}", parent.display())
        })?;
    }

    let render_options = DiagramRenderOptions {
        scale_factor: 1,
        background: if options.transparent_background {
            DiagramBackground::Transparent
        } else {
            DiagramBackground::Table
        },
    };
    let svg = GameState::default()
        .render_2d_diagram_with_options(DiagramOutputFormat::Svg, &render_options);
    if svg.is_empty() {
        return Err("rendered empty base pocket table SVG".to_string());
    }

    fs::write(&options.output_path, &svg)
        .map_err(|error| format!("failed to write {}: {error}", options.output_path.display()))?;
    println!(
        "Generated base pocket table SVG: {}",
        options.output_path.display()
    );
    Ok(())
}

fn run_wasm_preview(options: &WasmPreviewOptions) -> Result<(), String> {
    fs::create_dir_all(&options.output_dir).map_err(|error| {
        format!(
            "failed to create Wasm preview dir {}: {error}",
            options.output_dir.display()
        )
    })?;

    let wasm_artifact = build_wasm_artifact()?;

    let package_dir = options.output_dir.join("pkg");
    fs::create_dir_all(&package_dir).map_err(|error| {
        format!(
            "failed to create Wasm package dir {}: {error}",
            package_dir.display()
        )
    })?;

    run_checked(
        Command::new("wasm-bindgen")
            .arg("--target")
            .arg("web")
            .arg("--out-dir")
            .arg(&package_dir)
            .arg("--no-typescript")
            .arg(&wasm_artifact),
        "failed to generate browser Wasm bindings",
    )?;

    copy_preview_asset(
        workspace_root().join("web/billiards-ui.css"),
        &options.output_dir,
    )?;
    copy_preview_asset(
        workspace_root().join("web/billiards-viewer.js"),
        &options.output_dir,
    )?;

    let index_path = options.output_dir.join("index.html");
    fs::write(&index_path, wasm_preview_index_html())
        .map_err(|error| format!("failed to write {}: {error}", index_path.display()))?;

    println!("Built Wasm preview: {}", index_path.display());
    if !options.serve {
        println!(
            "Serve it with: python -m http.server {} --bind {} --directory {}",
            options.port,
            options.host,
            options.output_dir.display()
        );
        return Ok(());
    }

    let url = format!(
        "http://{}:{}/index.html",
        host_for_url(&options.host),
        options.port
    );
    println!("Serving Wasm preview at {url}");
    println!("Press Ctrl-C to stop the server.");
    run_checked(
        Command::new("python")
            .arg("-m")
            .arg("http.server")
            .arg(options.port.to_string())
            .arg("--bind")
            .arg(&options.host)
            .arg("--directory")
            .arg(&options.output_dir),
        "failed to serve Wasm preview",
    )
}

#[derive(Deserialize)]
struct CargoMessage {
    reason: Option<String>,
    target: Option<CargoTarget>,
}

#[derive(Deserialize)]
struct CargoTarget {
    name: Option<String>,
}

#[derive(Deserialize)]
struct CargoArtifactMessage {
    filenames: Option<Vec<PathBuf>>,
}

fn build_wasm_artifact() -> Result<PathBuf, String> {
    let output = Command::new("cargo")
        .args([
            "build",
            "--package",
            "billiards",
            "--lib",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "--message-format=json-render-diagnostics",
        ])
        .current_dir(workspace_root())
        .output()
        .map_err(|error| format!("failed to build billiards Wasm library: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "failed to build billiards Wasm library: command exited with {}{}{}",
            output.status,
            if stderr.trim().is_empty() { "" } else { "\n" },
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("Cargo emitted non-UTF-8 JSON messages: {error}"))?;
    let mut wasm_artifacts = Vec::new();
    for line in stdout.lines() {
        let Ok(message) = serde_json::from_str::<CargoMessage>(line) else {
            continue;
        };
        if message.reason.as_deref() != Some("compiler-artifact")
            || message.target.as_ref().and_then(|target| target.name.as_deref())
                != Some("billiards")
        {
            continue;
        }

        let artifact = serde_json::from_str::<CargoArtifactMessage>(line).map_err(|error| {
            format!("failed to parse matching Cargo compiler-artifact JSON: {error}")
        })?;
        wasm_artifacts.extend(
            artifact
                .filenames
                .unwrap_or_default()
                .into_iter()
                .filter(|path| path.extension() == Some(OsStr::new("wasm"))),
        );
    }
    wasm_artifacts.sort();
    wasm_artifacts.dedup();

    match wasm_artifacts.as_slice() {
        [artifact] => Ok(artifact.clone()),
        [] => Err("Cargo did not report a billiards Wasm artifact".to_string()),
        artifacts => Err(format!(
            "Cargo reported multiple billiards Wasm artifacts: {}",
            artifacts
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn host_for_url(host: &str) -> String {
    if host.contains(':') && !(host.starts_with('[') && host.ends_with(']')) {
        format!("[{host}]")
    } else {
        host.to_string()
    }
}

fn run_checked(command: &mut Command, failure_message: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("{failure_message}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{failure_message}: command exited with {status}"))
    }
}

fn copy_preview_asset(source: impl AsRef<Path>, output_dir: &Path) -> Result<(), String> {
    let source = source.as_ref();
    let target = output_dir.join(
        source
            .file_name()
            .ok_or_else(|| format!("asset path {} has no file name", source.display()))?,
    );
    fs::copy(source, &target).map_err(|error| {
        format!(
            "failed to copy {} to {}: {error}",
            source.display(),
            target.display()
        )
    })?;
    Ok(())
}

fn wasm_preview_index_html() -> &'static str {
    r###"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Billiards Wasm Preview</title>
  <link rel="stylesheet" href="./billiards-ui.css">
  <style>
    .wasm-preview-actions{display:flex;gap:.5rem;flex-wrap:wrap}
    .wasm-preview-status{min-height:1.4rem}
    .wasm-preview-grid{display:grid;grid-template-columns:minmax(22rem,.65fr) minmax(0,1.45fr);gap:1rem;align-items:start}
    @media (max-width:1100px){.wasm-preview-grid{grid-template-columns:1fr}}
  </style>
</head>
<body class="wasm-app">
  <header>
    <h1>Billiards Wasm Preview</h1>
    <p class="subtitle">A minimal static page generated by <code>cargo xtask wasm-preview</code>. It imports the compiled Rust/Wasm renderer from <code>./pkg/billiards.js</code>.</p>
  </header>
  <main class="wasm-preview-grid">
    <section class="card editor-panel" aria-labelledby="editor-title">
      <div class="panel-header">
        <h2 id="editor-title">Scenario DSL</h2>
        <div class="wasm-preview-actions">
          <button id="render-button" type="button" disabled>Render</button>
          <button id="reset-button" type="button" disabled>Reset sample</button>
        </div>
      </div>
      <div class="panel-body">
        <textarea id="dsl-input" spellcheck="false" aria-label="Billiards DSL input"></textarea>
        <div id="status" class="status wasm-preview-status">Loading Wasm renderer…</div>
      </div>
    </section>
    <section class="card wasm-report-card" aria-labelledby="preview-title">
      <div class="panel-header">
        <h2 id="preview-title">Rendered SVG</h2>
      </div>
      <div id="preview">
        <p class="empty-preview">Waiting for the Wasm renderer.</p>
      </div>
    </section>
  </main>
  <script src="./billiards-viewer.js"></script>
  <script type="module">
    import init, { render_svg_report_from_dsl } from "./pkg/billiards.js";

    const sampleDsl = `table brunswick_gc4_9ft
ball cue at center
ball one at (2.18, 4.12)
cue_strike(default).mass_ratio(1.0).energy_loss(0.1)
shot(cue).heading(0deg).speed(medium-soft).tip(side: -0.35R, height: -0.35R).using(default)
trace(max_events: 8)
`;

    const input = document.querySelector("#dsl-input");
    const status = document.querySelector("#status");
    const preview = document.querySelector("#preview");
    const renderButton = document.querySelector("#render-button");
    const resetButton = document.querySelector("#reset-button");

    function setStatus(message, kind = "") {
      status.textContent = message;
      status.classList.toggle("ok", kind === "ok");
      status.classList.toggle("error", kind === "error");
    }

    function escapeHtml(value) {
      return String(value ?? "")
        .replaceAll("&", "&amp;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;")
        .replaceAll('"', "&quot;");
    }

    function normalizeEvents(events) {
      return Array.isArray(events)
        ? events.map((event) => Array.isArray(event)
          ? { label: String(event[0] ?? ""), time: Number(event[1]), summary: String(event[2] ?? ""), title: String(event[3] ?? "") }
          : event)
        : [];
    }

    function eventLogHtml(events) {
      if (events.length === 0) {
        return "";
      }
      const rows = events.map((event) => {
        const label = event.label ?? "";
        const time = Number(event.time ?? 0).toFixed(6);
        const summary = event.summary ?? "";
        const title = event.title ?? `t=${time} ${summary}`;
        return `<li data-event-label="${escapeHtml(label)}" data-event-title="${escapeHtml(title)}" data-event-time="${time}"><span class="event-badge">${escapeHtml(label)}</span> <span class="event-time">t=${time}</span> <span class="event-summary">${escapeHtml(summary)}</span></li>`;
      }).join("");
      return `<details class="event-log" open><summary>Event log</summary><ol class="event-list">${rows}</ol></details>`;
    }

    function render() {
      try {
        const start = performance.now();
        const report = JSON.parse(render_svg_report_from_dsl(input.value));
        const events = normalizeEvents(report.events);
        const elapsedMs = Math.round(performance.now() - start);
        const viewer = window.BilliardsReportViewer;
        const controls = viewer?.viewerControlsHtml?.({ tableDetailDefault: "full" }) ?? "";
        const playback = viewer?.playbackPanelHtml?.(report.playback) ?? "";
        preview.innerHTML = `<figure class="svg-viewer" data-viewer>${controls}<div class="svg-frame">${report.svg}</div>${playback}</figure>${eventLogHtml(events)}`;
        viewer?.initialize(preview);
        const svgSizeKiB = (new Blob([report.svg], { type: "image/svg+xml" }).size / 1024).toFixed(1);
        const frames = report.playback?.frames?.length ?? 0;
        setStatus(`Rendered ${svgSizeKiB} KiB SVG in ${elapsedMs} ms${frames ? ` with ${frames} playback frames` : ""}.`, "ok");
      } catch (error) {
        preview.innerHTML = `<p class="empty-preview">Render failed.</p>`;
        setStatus(error instanceof Error ? error.message : String(error), "error");
      }
    }

    input.value = sampleDsl;
    try {
      await init();
      renderButton.disabled = false;
      resetButton.disabled = false;
      renderButton.addEventListener("click", render);
      resetButton.addEventListener("click", () => {
        input.value = sampleDsl;
        render();
      });
      render();
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error), "error");
      preview.innerHTML = `<p class="empty-preview">The Wasm package did not load. Re-run <code>cargo xtask wasm-preview</code>.</p>`;
    }
  </script>
</body>
</html>
"###
}

const VALIDATION_SUITE_MAX_PARALLELISM: usize = 8;

fn validation_suite_worker_count(scenario_count: usize) -> usize {
    if scenario_count == 0 {
        return 0;
    }

    thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .min(VALIDATION_SUITE_MAX_PARALLELISM)
        .min(scenario_count)
}

fn render_scenarios(
    scenarios: &[PathBuf],
    options: &ValidationSuiteOptions,
) -> Result<Vec<ScenarioReport>, String> {
    let worker_count = validation_suite_worker_count(scenarios.len());
    if worker_count <= 1 {
        return scenarios
            .iter()
            .map(|scenario_path| render_scenario(scenario_path, options))
            .collect();
    }

    let next_index = AtomicUsize::new(0);
    let reports = Mutex::new((0..scenarios.len()).map(|_| None).collect::<Vec<_>>());

    thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| loop {
                let index = next_index.fetch_add(1, Ordering::Relaxed);
                if index >= scenarios.len() {
                    break;
                }

                let report = render_scenario(&scenarios[index], options);
                reports.lock().expect("reports mutex poisoned")[index] = Some(report);
            });
        }
    });

    reports
        .into_inner()
        .expect("reports mutex poisoned")
        .into_iter()
        .enumerate()
        .map(|(index, report)| {
            report.unwrap_or_else(|| {
                Err(format!(
                    "scenario {} was not rendered",
                    scenarios[index].display()
                ))
            })
        })
        .collect()
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

    let reports = render_scenarios(&scenarios, options)?;

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
    radius_inches: f64,
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
    height_inches: f64,
    vx_ips: f64,
    vy_ips: f64,
    vz_ips: f64,
    wx_rps: f64,
    wy_rps: f64,
    wz_rps: f64,
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
        spin_glyphs: true,
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
                radius_inches: ball_spec.radius.as_f64(),
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
                        let center =
                            viewport.position_to_scene_point(&state.projected_position(table_spec));
                        ScenarioPlaybackBallReport {
                            id: playback_ball_id(&ball.ball),
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
    let mut json = String::new();
    write!(
        &mut json,
        "{{\"duration\":{:.6},\"events\":[",
        playback.duration
    )
    .expect("writing JSON to string should not fail");

    for (index, event) in playback.events.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push('[');
        push_json_string(&mut json, &event.label);
        write!(&mut json, ",{:.6},", event.time).expect("writing JSON to string should not fail");
        push_json_string(&mut json, &event.summary);
        json.push(']');
    }

    json.push_str("],\"balls\":[");
    for (index, ball) in playback.balls.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push('[');
        push_json_string(&mut json, &ball.id);
        json.push(',');
        push_json_string(&mut json, ball.fill);
        json.push(',');
        if let Some(label) = ball.label {
            push_json_string(&mut json, label);
        } else {
            json.push_str("null");
        }
        write!(&mut json, ",{:.3},{:.6}]", ball.radius, ball.radius_inches)
            .expect("writing JSON to string should not fail");
    }

    json.push_str("],\"frames\":[");
    for (frame_index, frame) in playback.frames.iter().enumerate() {
        if frame_index > 0 {
            json.push(',');
        }
        write!(&mut json, "[{:.6},[", frame.time).expect("writing JSON to string should not fail");
        for (ball_index, ball) in frame.balls.iter().enumerate() {
            if ball_index > 0 {
                json.push(',');
            }
            json.push('[');
            push_json_string(&mut json, &ball.id);
            write!(
                &mut json,
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
    json
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
            '\u{0C}' => out.push_str("\\f"),
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

fn render_cue_tip_diagram_svg(
    side_offset: f64,
    height_offset: f64,
    miscue_offset_limit: f64,
    cue_ball_launch_speed_mph: f64,
) -> String {
    let ball_radius = 64.0;
    let ball_center = 90.0;
    let tip_x = ball_center + side_offset * ball_radius;
    let tip_y = ball_center - height_offset * ball_radius;
    let limit_radius = miscue_offset_limit.clamp(0.0, 1.0) * ball_radius;
    let offset_radius = side_offset.hypot(height_offset);
    let marker_radius = cue_tip_marker_radius(cue_ball_launch_speed_mph, ball_radius);
    let marker_outline_radius = marker_radius + 2.5;
    let limit_status = if offset_radius <= miscue_offset_limit + 1e-12 {
        "inside"
    } else {
        "outside"
    };

    format!(
        r##"<svg class="cue-tip-diagram" data-tip-side="{side_offset:.3}" data-tip-height="{height_offset:.3}" data-miscue-limit="{miscue_offset_limit:.3}" data-cue-ball-speed-mph="{cue_ball_launch_speed_mph:.3}" data-tip-marker-r="{marker_radius:.3}" data-tip-x="{tip_x:.3}" data-tip-y="{tip_y:.3}" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 180 184" role="img" aria-label="Cue ball tip contact: side {side_offset:+.2} ball radii, height {height_offset:+.2} ball radii, {limit_status} the {miscue_offset_limit:.2} ball-radius miscue limit; red marker radius scales with {cue_ball_launch_speed_mph:.2} mph cue-ball launch speed">
<ellipse class="cue-ball-shadow" cx="94" cy="157" rx="54" ry="14" fill="#000000" opacity=".28"/>
<circle class="cue-ball-body" cx="{ball_center:.0}" cy="{ball_center:.0}" r="{ball_radius:.0}" fill="#e9e0c9" stroke="#fff9e9" stroke-width="1.5"/>
<circle class="cue-ball-highlight" cx="67" cy="55" r="32" fill="#ffffff" opacity=".24"/>
<ellipse class="cue-ball-glare" cx="64" cy="49" rx="17" ry="10" fill="#ffffff" opacity=".72" transform="rotate(-25 64 49)"/>
<path d="M45 118C58 138 82 149 110 142" fill="none" stroke="#ffffff" stroke-opacity=".28" stroke-width="5" stroke-linecap="round"/>
<circle class="miscue-limit" cx="{ball_center:.0}" cy="{ball_center:.0}" r="{limit_radius:.3}" fill="none" stroke="#090909" stroke-width="2.75"/>
<circle class="cue-tip-marker" cx="{tip_x:.3}" cy="{tip_y:.3}" r="{marker_radius:.3}" fill="#d91919" stroke="#ffffff" stroke-width="2"/>
<circle class="cue-tip-marker-outline" cx="{tip_x:.3}" cy="{tip_y:.3}" r="{marker_outline_radius:.3}" fill="none" stroke="#7b0000" stroke-opacity=".65" stroke-width="1.5"/>
</svg>
"##
    )
}

fn cue_tip_marker_radius(cue_ball_launch_speed_mph: f64, ball_radius: f64) -> f64 {
    let equator_width = ball_radius * 2.0;
    let min_radius = equator_width / 15.0;
    let max_radius = equator_width / 8.0;
    let speed_ratio = (cue_ball_launch_speed_mph / 30.0).clamp(0.0, 1.0);

    min_radius + (max_radius - min_radius) * speed_ratio
}

fn render_power_meter_svg(
    cue_ball_launch_speed_mph: f64,
    speed_band: HumanShotSpeedBand,
) -> String {
    const MIN_MPH: f64 = 0.0;
    const GREEN_END_MPH: f64 = 20.0;
    const YELLOW_END_MPH: f64 = 30.0;
    const MAX_MPH: f64 = 35.0;

    let cx = 110.0;
    let cy = 106.0;
    let radius = 80.0;
    let needle_angle = power_meter_angle_for_mph(cue_ball_launch_speed_mph);
    let track_arc = svg_arc_path(
        cx,
        cy,
        radius,
        power_meter_angle_for_mph(MIN_MPH),
        power_meter_angle_for_mph(MAX_MPH),
    );
    let (needle_x, needle_y) = polar_point(cx, cy, radius - 6.0, needle_angle);

    let mut zone_arcs = String::new();
    for (class, zone, start_mph, end_mph, stroke) in [
        (
            "power-meter-zone power-meter-zone-green",
            "green",
            MIN_MPH,
            GREEN_END_MPH,
            "#5fd35f",
        ),
        (
            "power-meter-zone power-meter-zone-yellow",
            "yellow",
            GREEN_END_MPH,
            YELLOW_END_MPH,
            "#f3c742",
        ),
        (
            "power-meter-zone power-meter-zone-red power-meter-redline",
            "red",
            YELLOW_END_MPH,
            MAX_MPH,
            "#e04747",
        ),
    ] {
        let arc = svg_arc_path(
            cx,
            cy,
            radius,
            power_meter_angle_for_mph(start_mph),
            power_meter_angle_for_mph(end_mph),
        );
        zone_arcs.push_str(&format!(
            r##"<path class="{class}" data-zone="{zone}" data-zone-start-mph="{start_mph:.3}" data-zone-end-mph="{end_mph:.3}" d="{arc}" fill="none" stroke="{stroke}" stroke-width="14" stroke-linecap="butt"/>
"##
        ));
    }

    let mut ticks = String::new();
    for tick_mph in [0.0, 5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 35.0] {
        let angle = power_meter_angle_for_mph(tick_mph);
        let major_tick = matches!(tick_mph as i32, 0 | 10 | 20 | 30 | 35);
        let (outer_x, outer_y) = polar_point(cx, cy, radius + 5.0, angle);
        let (inner_x, inner_y) =
            polar_point(cx, cy, radius - if major_tick { 14.0 } else { 8.0 }, angle);
        let tick_class = if major_tick {
            "power-meter-tick power-meter-tick-major"
        } else {
            "power-meter-tick"
        };
        let tick_width = if major_tick { 2.75 } else { 1.75 };
        ticks.push_str(&format!(
            r##"<line class="{tick_class}" data-tick-mph="{tick_mph:.3}" x1="{outer_x:.3}" y1="{outer_y:.3}" x2="{inner_x:.3}" y2="{inner_y:.3}" stroke="#101410" stroke-opacity=".68" stroke-width="{tick_width:.2}" stroke-linecap="round"/>
"##
        ));

        if major_tick {
            let (label_x, label_y) = polar_point(cx, cy, radius - 31.0, angle);
            let label_zone = if tick_mph >= YELLOW_END_MPH {
                "red"
            } else if tick_mph >= GREEN_END_MPH {
                "yellow"
            } else {
                "green"
            };
            ticks.push_str(&format!(
                r##"<circle class="power-meter-label-backplate power-meter-label-backplate-{label_zone}" cx="{label_x:.3}" cy="{label_y:.3}" r="10.5" fill="#fffaf1" fill-opacity=".96" stroke="#101410" stroke-opacity=".35" stroke-width=".8"/>
<text class="power-meter-label power-meter-label-{label_zone}" x="{label_x:.3}" y="{label_y:.3}" fill="#111111" stroke="#fffaf1" stroke-width="2.25" paint-order="stroke fill" font-size="12" font-weight="800" font-family="Inter,system-ui,sans-serif" text-anchor="middle" dominant-baseline="middle">{tick_mph:.0}</text>
"##
            ));
        }
    }

    format!(
        r##"<svg class="power-meter" data-cue-ball-speed-mph="{cue_ball_launch_speed_mph:.3}" data-speed-band="{speed_band:?}" data-speedometer-scale="green-yellow-red" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 220 160" role="img" aria-label="Power level: cue-ball launch speed {cue_ball_launch_speed_mph:.2} miles per hour, {speed_band_label} band. Gauge increases monotonically from green 0 to 20 miles per hour, yellow 20 to 30 miles per hour, and red 30 to 35 miles per hour.">
<path class="power-meter-track" d="{track_arc}" fill="none" stroke="#304030" stroke-width="18" stroke-linecap="round"/>
{zone_arcs}{ticks}<line class="power-meter-needle-halo" x1="{cx:.0}" y1="{cy:.0}" x2="{needle_x:.3}" y2="{needle_y:.3}" stroke="#fffaf1" stroke-width="10" stroke-linecap="round"/>
<line class="power-meter-needle" x1="{cx:.0}" y1="{cy:.0}" x2="{needle_x:.3}" y2="{needle_y:.3}" stroke="#050505" stroke-width="7" stroke-linecap="round"/>
<circle class="power-meter-hub" cx="{cx:.0}" cy="{cy:.0}" r="10" fill="#050505" stroke="#fffaf1" stroke-width="3.5"/>
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

fn filter_token(input: &str) -> String {
    let mut token = String::with_capacity(input.len());
    let mut pending_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_dash && !token.is_empty() {
                token.push('-');
            }
            token.push(ch.to_ascii_lowercase());
            pending_dash = false;
        } else {
            pending_dash = true;
        }
    }
    token
}

fn normalized_search_text(input: &str) -> String {
    let mut text = String::with_capacity(input.len());
    let mut pending_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            pending_space = true;
        } else {
            if pending_space && !text.is_empty() {
                text.push(' ');
            }
            for lowercase in ch.to_lowercase() {
                text.push(lowercase);
            }
            pending_space = false;
        }
    }
    text
}

fn scenario_filter_text(report: &ScenarioReport) -> String {
    let mut text = String::new();
    text.push_str(&report.name);
    text.push(' ');
    text.push_str(&report.image_file_name);
    for note in &report.notes {
        text.push(' ');
        text.push_str(note);
    }
    for row in &report.info_rows {
        text.push(' ');
        text.push_str(&row.label);
        text.push(' ');
        text.push_str(&row.value);
    }
    for event in &report.events {
        text.push(' ');
        text.push_str(&event.label);
        text.push(' ');
        text.push_str(&event.time);
        text.push(' ');
        text.push_str(&event.summary);
        text.push(' ');
        text.push_str(&event.payload);
    }
    normalized_search_text(&text)
}

fn scenario_speed_band_token(report: &ScenarioReport) -> String {
    report
        .info_rows
        .iter()
        .find(|row| row.label == "Cue-ball launch")
        .and_then(|row| row.value.rsplit_once('·').map(|(_, band)| band))
        .map(str::trim)
        .and_then(|band| band.strip_suffix(" band").or(Some(band)))
        .map(filter_token)
        .filter(|token| !token.is_empty())
        .unwrap_or_else(|| "none".to_string())
}

fn scenario_event_bucket(report: &ScenarioReport) -> &'static str {
    match report.events.len() {
        0 => "none",
        1 => "single",
        _ => "multi",
    }
}

fn scenario_playback_bucket(report: &ScenarioReport) -> &'static str {
    if report.playback.is_some() {
        "with-playback"
    } else {
        "no-playback"
    }
}

fn push_gallery_controls(html: &mut String, report_count: usize) {
    html.push_str(
        "<section class=\"gallery-controls\" aria-label=\"Gallery controls\">\n\
         <div class=\"control-grid\">\n\
         <label>Search scenarios<input type=\"search\" data-scenario-filter-search placeholder=\"name, source, note, event\" autocomplete=\"off\"></label>\n\
         <label>Speed band<select data-scenario-filter-speed>\n\
         <option value=\"\">All speed bands</option>\n\
         <option value=\"touch\">Touch</option>\n\
         <option value=\"slow\">Slow</option>\n\
         <option value=\"medium-soft\">Medium-soft</option>\n\
         <option value=\"medium\">Medium</option>\n\
         <option value=\"medium-fast\">Medium-fast</option>\n\
         <option value=\"fast\">Fast</option>\n\
         <option value=\"power\">Power</option>\n\
         <option value=\"typical-power-break\">Typical power break</option>\n\
         <option value=\"exceptional-power-break\">Exceptional power break</option>\n\
         <option value=\"beyond-exceptional-power-break\">Beyond exceptional power break</option>\n\
         <option value=\"none\">No speed band</option>\n\
         </select></label>\n\
         <label>Events<select data-scenario-filter-events>\n\
         <option value=\"\">All event counts</option>\n\
         <option value=\"any\">Has events</option>\n\
         <option value=\"none\">No events</option>\n\
         <option value=\"single\">One event</option>\n\
         <option value=\"multi\">Multiple events</option>\n\
         </select></label>\n\
         <label>Playback<select data-scenario-filter-playback>\n\
         <option value=\"\">All playback states</option>\n\
         <option value=\"with-playback\">With playback</option>\n\
         <option value=\"no-playback\">No playback</option>\n\
         </select></label>\n\
         <label>Table detail<select data-global-table-detail>\n\
         <option value=\"full\">Full material</option>\n\
         <option value=\"flat\">Flat colors</option>\n\
         <option value=\"cloth\">Cloth only</option>\n\
         <option value=\"rail\">Rails and pockets only</option>\n\
         </select></label>\n\
         <button type=\"button\" data-scenario-filter-reset>Reset filters</button>\n\
         </div>\n",
    );
    html.push_str(&format!(
        "<p class=\"scenario-filter-count\" data-scenario-filter-count>Showing {report_count} of {report_count} scenarios</p>\n"
    ));
    html.push_str("</section>\n");
}

const SHOT_SUMMARY_INFO_LABELS: &[&str] = &[
    "Cue-ball launch",
    "Cue-stick impact",
    "Heading",
    "Tip side",
    "Tip height",
    "Clean-cuing limit",
];

fn is_shot_summary_info_row(row: &ReportInfoRow) -> bool {
    SHOT_SUMMARY_INFO_LABELS.contains(&row.label.as_str())
}

fn escaped_embedded_script(source: &str) -> String {
    source.replace("</script", "<\\/script")
}

fn render_html(reports: &[ScenarioReport], options: &ValidationSuiteOptions) -> String {
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str("<title>Billiards scenario validation suite</title>\n");
    html.push_str("<style>\n");
    html.push_str(include_str!("../../web/billiards-ui.css"));
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
    html.push_str("</header>\n<main>\n");
    push_gallery_controls(&mut html, reports.len());
    html.push_str("<nav class=\"toc\" aria-label=\"Scenario list\">\n");
    for report in reports {
        let anchor = anchor_id(&report.name);
        html.push_str(&format!(
            "<a data-scenario-toc-link href=\"#{}\">{}</a>\n",
            escape_html(&anchor),
            escape_html(&report.name)
        ));
    }
    html.push_str("</nav>\n");

    for report in reports {
        let anchor = anchor_id(&report.name);
        let search_text = scenario_filter_text(report);
        let speed_band = scenario_speed_band_token(report);
        let event_bucket = scenario_event_bucket(report);
        let playback_bucket = scenario_playback_bucket(report);
        html.push_str(&format!(
            "<section class=\"card\" id=\"{}\" data-scenario-card data-scenario-search=\"{}\" data-scenario-speed-band=\"{}\" data-scenario-events=\"{}\" data-scenario-event-count=\"{}\" data-scenario-playback=\"{}\">\n<h2>{}</h2>\n",
            escape_html(&anchor),
            escape_html(&search_text),
            escape_html(&speed_band),
            event_bucket,
            report.events.len(),
            playback_bucket,
            escape_html(&report.name)
        ));
        html.push_str("<div class=\"card-workspace\">\n");
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
                    "Needle: estimated cue-ball launch speed. Green arc: 0-20 mph ordinary range. Yellow arc: 20-30 mph power-break approach. Red arc: 30-35 mph exceptional break-speed band.",
                );
                html.push_str("</div>\n</div>\n");
            }
            html.push_str("</div>\n");
        }
        let show_compact_shot_info =
            has_visuals && report.info_rows.iter().any(is_shot_summary_info_row);
        html.push_str("<div class=\"info-panel shot-data-panel\">\n");
        if show_compact_shot_info {
            push_info_table(
                &mut html,
                report
                    .info_rows
                    .iter()
                    .filter(|row| is_shot_summary_info_row(row)),
            );
        } else {
            push_info_table(&mut html, &report.info_rows);
        }
        if !show_compact_shot_info && !report.notes.is_empty() {
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
            "<div class=\"viewer-controls\" data-viewer-controls data-table-detail-default=\"global\" aria-label=\"Diagram controls\"></div>\n\
             <div class=\"svg-frame\">\n",
        );
        html.push_str(&report.inline_svg);
        html.push_str("</div>\n");
        if let Some(playback) = &report.playback {
            html.push_str(&format!(
                "<div class=\"playback-panel\" data-playback>\n\
                 <script type=\"application/json\" data-playback-data>{}</script>\n\
                 </div>\n",
                playback_json(playback)
            ));
        }
        push_download_links(&mut html, report);
        html.push_str("</figure>\n");
        if show_compact_shot_info
            && (report
                .info_rows
                .iter()
                .any(|row| !is_shot_summary_info_row(row))
                || !report.notes.is_empty())
        {
            html.push_str(
                "<details class=\"scenario-context scenario-details\"><summary>Scenario details</summary>\n",
            );
            push_info_table(
                &mut html,
                report
                    .info_rows
                    .iter()
                    .filter(|row| !is_shot_summary_info_row(row)),
            );
            if !report.notes.is_empty() {
                html.push_str("<ul class=\"notes\">\n");
                for note in &report.notes {
                    html.push_str(&format!("<li>{}</li>\n", escape_html(note)));
                }
                html.push_str("</ul>\n");
            }
            html.push_str("</details>\n");
        }
        push_event_log(&mut html, report);
        html.push_str("</div>\n</section>\n");
    }

    html.push_str("<script>\n");
    html.push_str(&escaped_embedded_script(include_str!(
        "../../web/billiards-viewer.js"
    )));
    html.push_str("</script>\n");
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

fn push_info_table<'a>(html: &mut String, rows: impl IntoIterator<Item = &'a ReportInfoRow>) {
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
    "Usage:\n  cargo xtask validation-suite [options]\n  cargo xtask base-svg [options]\n  cargo xtask wasm-preview [options]\n\nValidation suite options:\n  --scenario-dir <dir>               Directory containing .billiards files [default: examples/scenarios]\n  --output-dir <dir>                 Output directory for SVG diagrams and index.html [default: target/validation-suite]\n  --trace-sample-step-seconds <sec>  Path sampling step for rendered traces [default: 0.0025]\n  --max-events <n>                   Override scenario trace/simulation event limits\n  --transparent                      Render diagrams on a transparent background\n  --open                             Open the generated index.html with the platform opener\n\nBase SVG options:\n  --output <path>                    Output SVG path [default: target/base-pocket-table.svg]\n  --transparent                      Render the base table with transparent background metadata\n\nWasm preview options:\n  --output-dir <dir>                 Output directory for index.html, assets, and pkg/ [default: target/wasm-preview]\n  --host <host>                      Static server bind host [default: 127.0.0.1]\n  --port <port>                      Static server port [default: 8000]\n  --no-serve                         Build the preview without starting the HTTP server\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64) {
        let delta = (actual - expected).abs();
        assert!(
            delta < 1e-9,
            "expected {actual} to be within 1e-9 of {expected}; delta={delta}"
        );
    }

    #[derive(Clone, Copy)]
    enum CargoArtifactExpectation {
        Artifact(&'static str),
        Error(&'static str),
        MalformedRecord,
    }

    struct CargoMessageCase {
        name: &'static str,
        stdout: &'static str,
        expected: CargoArtifactExpectation,
    }

    const CARGO_MESSAGE_CASES: &[CargoMessageCase] = &[
        CargoMessageCase {
            name: "reordered fields and whitespace",
            stdout: r#"{ "filenames" : ["target/wasm32-unknown-unknown/release/billiards.wasm"], "target" : { "name" : "billiards" }, "reason" : "compiler-artifact" }"#,
            expected: CargoArtifactExpectation::Artifact(
                "target/wasm32-unknown-unknown/release/billiards.wasm",
            ),
        },
        CargoMessageCase {
            name: "escaped filename",
            stdout: r#"{"reason":"compiler-artifact","target":{"name":"billiards"},"filenames":["target\/billiards-\ud83c\udfaf.wasm"]}"#,
            expected: CargoArtifactExpectation::Artifact("target/billiards-🎯.wasm"),
        },
        CargoMessageCase {
            name: "unrelated output, messages, and targets",
            stdout: r#"not JSON
{"reason":"compiler-artifact","target":{"name":"helper"},"metadata":{"name":"billiards"},"filenames":["target/helper.wasm"]}
{"reason":"build-script-executed","metadata":{"reason":"compiler-artifact"},"target":{"name":"billiards"},"filenames":["target/build-script.wasm"]}
{"reason":"compiler-artifact","target":{"name":"billiards"},"filenames":["target/billiards.wasm"]}"#,
            expected: CargoArtifactExpectation::Artifact("target/billiards.wasm"),
        },
        CargoMessageCase {
            name: "matching record without filenames",
            stdout: r#"{"reason":"compiler-artifact","target":{"name":"billiards"}}"#,
            expected: CargoArtifactExpectation::Error(
                "Cargo did not report a billiards Wasm artifact",
            ),
        },
        CargoMessageCase {
            name: "malformed matching record",
            stdout: r#"{"reason":"compiler-artifact","target":{"name":"billiards"},"filenames":42}"#,
            expected: CargoArtifactExpectation::MalformedRecord,
        },
        CargoMessageCase {
            name: "zero wasm artifacts",
            stdout: r#"{"reason":"compiler-artifact","target":{"name":"billiards"},"filenames":["target/libbilliards.rlib"]}
{"reason":"compiler-artifact","target":{"name":"helper"},"filenames":["target/helper.wasm"]}"#,
            expected: CargoArtifactExpectation::Error(
                "Cargo did not report a billiards Wasm artifact",
            ),
        },
        CargoMessageCase {
            name: "one deduplicated wasm artifact",
            stdout: r#"{"reason":"compiler-artifact","target":{"name":"billiards"},"filenames":["target/billiards.wasm","target/libbilliards.rlib"]}
{"reason":"compiler-artifact","target":{"name":"billiards"},"filenames":["target/billiards.wasm"]}"#,
            expected: CargoArtifactExpectation::Artifact("target/billiards.wasm"),
        },
        CargoMessageCase {
            name: "multiple distinct wasm artifacts",
            stdout: r#"{"reason":"compiler-artifact","target":{"name":"billiards"},"filenames":["target/z-billiards.wasm","target/a-billiards.wasm"]}"#,
            expected: CargoArtifactExpectation::Error(
                "Cargo reported multiple billiards Wasm artifacts: target/a-billiards.wasm, target/z-billiards.wasm",
            ),
        },
    ];

    fn assert_cargo_message_case(case: &CargoMessageCase) {
        match (build_wasm_artifact(), case.expected) {
            (Ok(actual), CargoArtifactExpectation::Artifact(expected)) => {
                assert_eq!(actual, PathBuf::from(expected), "case: {}", case.name);
            }
            (Err(actual), CargoArtifactExpectation::Error(expected)) => {
                assert_eq!(actual, expected, "case: {}", case.name);
            }
            (Err(actual), CargoArtifactExpectation::MalformedRecord) => {
                let describes_parse_failure = ["json", "parse", "malformed", "invalid"]
                    .iter()
                    .any(|term| actual.to_ascii_lowercase().contains(term));
                assert!(
                    describes_parse_failure,
                    "case `{}` silently became an artifact-selection error: {actual}",
                    case.name
                );
            }
            (actual, _) => panic!("case `{}` produced an unexpected result: {actual:?}", case.name),
        }
    }

    #[cfg(unix)]
    struct FakeCargoDir(PathBuf);

    #[cfg(unix)]
    impl FakeCargoDir {
        fn new() -> Self {
            use std::os::unix::fs::PermissionsExt as _;

            let base = env::temp_dir();
            let directory = (0..100)
                .map(|attempt| {
                    base.join(format!(
                        "billiards-xtask-cargo-messages-{}-{attempt}",
                        std::process::id()
                    ))
                })
                .find(|candidate| fs::create_dir(candidate).is_ok())
                .expect("create isolated fake Cargo directory");
            let cargo = directory.join("cargo");
            fs::write(
                &cargo,
                "#!/bin/sh\nprintf '%s' \"$XTASK_FAKE_CARGO_STDOUT\"\n",
            )
            .expect("write fake Cargo executable");
            let mut permissions = fs::metadata(&cargo)
                .expect("read fake Cargo metadata")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&cargo, permissions).expect("make fake Cargo executable");
            Self(directory)
        }
    }

    #[cfg(unix)]
    impl Drop for FakeCargoDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated fake Cargo directory");
        }
    }

    #[cfg(unix)]
    #[test]
    fn cargo_messages_select_wasm_artifact_by_semantic_fields() {
        const CHILD_CASE: &str = "XTASK_CARGO_MESSAGE_CHILD_CASE";

        if let Some(case_index) = env::var_os(CHILD_CASE) {
            let case_index = case_index
                .to_str()
                .expect("child case index should be UTF-8")
                .parse::<usize>()
                .expect("child case index should be numeric");
            assert_cargo_message_case(&CARGO_MESSAGE_CASES[case_index]);
            return;
        }

        let fake_cargo = FakeCargoDir::new();
        let mut paths = vec![fake_cargo.0.clone()];
        paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
        let path = env::join_paths(paths).expect("prepend fake Cargo to PATH");
        let test_binary = env::current_exe().expect("locate xtask test binary");
        let mut failures = Vec::new();

        for (case_index, case) in CARGO_MESSAGE_CASES.iter().enumerate() {
            let output = Command::new(&test_binary)
                .args([
                    "tests::cargo_messages_select_wasm_artifact_by_semantic_fields",
                    "--exact",
                    "--nocapture",
                ])
                .env("PATH", &path)
                .env("XTASK_FAKE_CARGO_STDOUT", case.stdout)
                .env(CHILD_CASE, case_index.to_string())
                .output()
                .expect("run isolated Cargo-message test case");
            if !output.status.success() {
                failures.push(format!(
                    "{}:\n{}{}",
                    case.name,
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "Cargo message cases failed:\n{}",
            failures.join("\n")
        );
    }

    #[test]
    fn cue_tip_diagram_places_marker_from_shot_offsets_and_speed() {
        let svg = render_cue_tip_diagram_svg(0.25, -0.5, 0.5, 15.0);

        assert!(svg.contains("class=\"cue-tip-diagram\""));
        assert!(!svg.contains("class=\"cue-ball-shade\""));
        assert!(svg.contains("data-tip-side=\"0.250\""));
        assert!(svg.contains("data-tip-height=\"-0.500\""));
        assert!(svg.contains("data-miscue-limit=\"0.500\""));
        assert!(svg.contains("data-cue-ball-speed-mph=\"15.000\""));
        assert!(svg.contains("data-tip-marker-r=\"12.267\""));
        assert!(svg.contains("data-tip-x=\"106.000\""));
        assert!(svg.contains("data-tip-y=\"122.000\""));
        assert!(svg.contains("class=\"miscue-limit\" cx=\"90\" cy=\"90\" r=\"32.000\""));
        assert!(svg.contains("class=\"cue-tip-marker\" cx=\"106.000\" cy=\"122.000\" r=\"12.267\""));
        assert!(svg.contains("fill=\"#d91919\""));
    }

    #[test]
    fn cue_tip_marker_radius_scales_across_requested_speed_range() {
        assert_close(cue_tip_marker_radius(0.0, 64.0), 8.533333333333333);
        assert_eq!(cue_tip_marker_radius(30.0, 64.0), 16.0);
        assert_eq!(cue_tip_marker_radius(35.0, 64.0), 16.0);
    }

    #[test]
    fn power_meter_renders_monotonic_green_yellow_red_scale_and_black_needle_data() {
        let svg = render_power_meter_svg(30.0, HumanShotSpeedBand::TypicalPowerBreak);

        assert!(svg.contains("class=\"power-meter\""));
        assert!(svg.contains("data-cue-ball-speed-mph=\"30.000\""));
        assert!(svg.contains("data-speed-band=\"TypicalPowerBreak\""));
        assert!(svg.contains("data-speedometer-scale=\"green-yellow-red\""));
        assert!(svg.contains(
            "data-zone=\"green\" data-zone-start-mph=\"0.000\" data-zone-end-mph=\"20.000\""
        ));
        assert!(svg.contains(
            "data-zone=\"yellow\" data-zone-start-mph=\"20.000\" data-zone-end-mph=\"30.000\""
        ));
        assert!(svg.contains(
            "data-zone=\"red\" data-zone-start-mph=\"30.000\" data-zone-end-mph=\"35.000\""
        ));
        assert!(svg.contains("class=\"power-meter-zone power-meter-zone-yellow\""));
        assert!(svg.contains("stroke=\"#f3c742\""));
        assert!(svg.contains("class=\"power-meter-zone power-meter-zone-red power-meter-redline\""));
        assert!(
            svg.contains("class=\"power-meter-label-backplate power-meter-label-backplate-red\"")
        );
        assert!(svg.contains("font-size=\"12\""));
        assert!(svg.contains("class=\"power-meter-needle-halo\""));
        assert!(svg.contains("class=\"power-meter-needle\""));
        assert!(svg.contains("stroke=\"#050505\" stroke-width=\"7\""));
        assert!(svg.contains("r=\"10\" fill=\"#050505\""));
        assert!(svg.contains("Gauge increases monotonically from green 0 to 20 miles per hour, yellow 20 to 30 miles per hour, and red 30 to 35 miles per hour."));
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
                ReportInfoRow::new("Tip side", "+0.25 R"),
                ReportInfoRow::new("Tip height", "-0.50 R"),
            ],
            cue_tip_diagram_svg: Some(render_cue_tip_diagram_svg(0.25, -0.5, 0.5, 7.27)),
            power_meter_svg: Some(render_power_meter_svg(7.27, HumanShotSpeedBand::Medium)),
            playback: None,
            events: Vec::new(),
        };

        let html = render_html(&[report], &ValidationSuiteOptions::default());

        assert!(html.contains("class=\"card-overview\""));
        assert!(html.contains("class=\"visual-stack\""));
        assert!(html.contains(".card-workspace{display:grid;grid-template-columns:1fr;gap:.85rem"));
        assert!(html.contains(".visual-stack{display:grid;grid-template-columns:repeat(2,6.8rem)"));
        assert!(html.contains(".visual-panel svg{display:block;width:6.35rem;height:6.35rem"));
        assert!(html.contains(
            ".svg-frame svg[data-orientation=\"clockwise\"]{width:100%;max-height:min(90vh,72rem)}"
        ));
        assert!(html.contains("<strong>Cue tip</strong>"));
        assert!(html.contains("<strong>Power</strong>"));
        assert!(html.contains("class=\"tooltip\""));
        assert!(html.contains("data-tooltip=\"Red marker: tip-contact offset"));
        assert!(!html.contains("Red dot position is the tip contact"));
        assert!(html.contains("<dl class=\"info-table\""));
        assert!(html.contains("<dt>Tip side</dt><dd>+0.25 R</dd>"));
        assert!(html.contains("<dt>Tip height</dt><dd>-0.50 R</dd>"));
        assert!(html.contains("class=\"info-panel shot-data-panel\""));
        assert!(html.contains("class=\"scenario-context scenario-details\""));
        assert!(html.contains("<summary>Scenario details</summary>"));
        assert!(html.contains("<dt>Source</dt><dd>examples/scenarios/cue_tip_test.billiards</dd>"));
        assert!(html.contains("<svg class=\"cue-tip-diagram\""));
        assert!(html.contains("<svg class=\"power-meter\""));
        assert!(html.contains("class=\"gallery-controls\""));
        assert!(html.contains("data-scenario-filter-search"));
        assert!(html.contains("data-scenario-filter-speed"));
        assert!(html.contains("data-scenario-filter-events"));
        assert!(html.contains("data-scenario-filter-playback"));
        assert!(html.contains("data-global-table-detail"));
        assert!(html.contains("data-scenario-toc-link"));
        assert!(html.contains("data-scenario-card"));
        assert!(html.contains("data-scenario-search=\"cue tip test cue_tip_test.svg source examples/scenarios/cue_tip_test.billiards"));
        assert!(html.contains("data-scenario-speed-band=\"medium\""));
        assert!(html.contains("data-scenario-events=\"none\""));
        assert!(html.contains("data-scenario-event-count=\"0\""));
        assert!(html.contains("data-scenario-playback=\"no-playback\""));
        assert!(html.contains("Showing 1 of 1 scenarios"));
        assert!(html.contains("data-table-detail"));
        assert!(html.contains("Page setting"));
        assert!(html.contains("Full material"));
        assert!(html.contains("Flat colors"));
        assert!(html.contains("Cloth only"));
        assert!(html.contains("Rails and pockets only"));
        assert!(html.contains("applyScenarioFilters"));
        assert!(html.contains("tableDetailGroups"));
        assert!(html.contains("applyTableDetailMode"));
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
                    radius_inches: 1.0,
                }],
                frames: vec![
                    ScenarioPlaybackFrameReport {
                        time: 0.0,
                        balls: vec![ScenarioPlaybackBallReport {
                            id: "cue".to_string(),
                            x: 10.0,
                            y: 20.0,
                            height_inches: 0.0,
                            vx_ips: 5.0,
                            vy_ips: 0.0,
                            vz_ips: 0.0,
                            wx_rps: 3.0,
                            wy_rps: 4.0,
                            wz_rps: 5.0,
                        }],
                    },
                    ScenarioPlaybackFrameReport {
                        time: 0.125,
                        balls: vec![ScenarioPlaybackBallReport {
                            id: "cue".to_string(),
                            x: 20.0,
                            y: 20.0,
                            height_inches: 0.0,
                            vx_ips: 0.0,
                            vy_ips: 0.0,
                            vz_ips: 0.0,
                            wx_rps: 0.0,
                            wy_rps: 0.0,
                            wz_rps: 0.0,
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

        assert!(html.contains("data-playback-reset"));
        assert!(html.contains("aria-label=\"Rewind to beginning\""));
        assert!(html.contains(">⏮</button>"));
        assert!(html.contains("aria-label=\"Step back one frame\""));
        assert!(html.contains(">⏪</button>"));
        assert!(html.contains("aria-label=\"Play\" title=\"Play\">▶</button>"));
        assert!(html.contains("aria-label=\"Step forward one frame\""));
        assert!(html.contains(">⏩</button>"));
        assert!(html.contains("aria-label=\"Play to next event\""));
        assert!(html.contains(">⏭</button>"));
        assert!(html.contains("data-playback-speed"));
        assert!(html.contains("min=\"0.0625\" max=\"1\" value=\"1\""));
        assert!(html.contains("data-playback-speed-label>1x"));
        assert!(html.contains("down to 1/16x for slow motion"));
        assert!(html.contains("data-playback-trace"));
        assert!(html.contains("Trace paths"));
        assert!(html.contains("toggle Trace paths to hide static trajectory lines"));
        assert!(html.contains("default 2.5 ms physics frames"));
        assert!(html.contains("playbackSpeed()"));
        assert!(html.contains("Spin badges use green arrows for natural roll"));
        assert!(html.contains("data-playback-event"));
        assert!(html.contains("play to the next logged event"));
        assert!(html.contains("\"events\":[[\"(1)\",0.125000,\"cue -\\u003e one collision\"]]"));
        assert!(html.contains("data-event-time=\"0.125000\""));
        assert!(html.contains("nextEventAfter"));
        assert!(html.contains(
            "[\"cue\",10.000,20.000,0.000000,5.000000,0.000000,0.000000,3.000000,4.000000,5.000000]"
        ));
        assert!(html.contains("appendSpinGlyph"));
        assert!(html.contains("playback-spin-glyph"));
        assert!(html.contains("setTracePathsVisible"));
        assert!(html.contains(".smooth-polyline, .heading-chevron"));
        assert!(html.contains("event-current"));
        assert!(html.contains("data-scenario-events=\"single\""));
        assert!(html.contains("data-scenario-event-count=\"1\""));
        assert!(html.contains("data-scenario-playback=\"with-playback\""));
        assert!(html.contains("data-playback-data>${escapeJsonScript(playback)}<\\/script>"));
        assert!(!html.contains("data-playback-data>${escapeJsonScript(playback)}</script>"));
    }

    #[test]
    fn validation_suite_defaults_use_smooth_slow_motion_sampling() {
        let options = ValidationSuiteOptions::default();

        assert_eq!(
            options.trace_sample_step_seconds,
            DEFAULT_BALL_PATH_MAX_TIME_STEP_SECONDS
        );
        assert_eq!(options.trace_sample_step_seconds, 0.0025);
        assert!(usage_text().contains("[default: 0.0025]"));
    }

    #[test]
    fn validation_suite_options_reject_removed_format_option() {
        let args = ["--format".to_string(), "svg".to_string()];

        let error = ValidationSuiteOptions::parse(&args).expect_err("format option is gone");

        assert!(error.contains("unknown validation-suite option `--format`"));
        assert!(!usage_text().contains("--format"));
    }

    #[test]
    fn base_svg_options_parse_output_and_transparent() {
        let args = [
            "--output".to_string(),
            "target/custom-base.svg".to_string(),
            "--transparent".to_string(),
        ];

        let options = BaseSvgOptions::parse(&args).expect("base-svg options should parse");

        assert_eq!(options.output_path, PathBuf::from("target/custom-base.svg"));
        assert!(options.transparent_background);
    }

    #[test]
    fn base_svg_options_reject_unknown_option() {
        let args = ["--format".to_string(), "svg".to_string()];

        let error = BaseSvgOptions::parse(&args).expect_err("format option is not supported");

        assert!(error.contains("unknown base-svg option `--format`"));
    }

    #[test]
    fn wasm_preview_options_parse_output_host_port_and_no_serve() {
        let args = [
            "--output-dir".to_string(),
            "target/custom-wasm".to_string(),
            "--host".to_string(),
            "0.0.0.0".to_string(),
            "--port".to_string(),
            "9090".to_string(),
            "--no-serve".to_string(),
        ];

        let options = WasmPreviewOptions::parse(&args).expect("wasm-preview options should parse");

        assert_eq!(options.output_dir, PathBuf::from("target/custom-wasm"));
        assert_eq!(options.host, "0.0.0.0");
        assert_eq!(options.port, 9090);
        assert!(!options.serve);
    }

    #[test]
    fn wasm_preview_options_reject_unknown_option() {
        let args = ["--format".to_string(), "svg".to_string()];

        let error = WasmPreviewOptions::parse(&args).expect_err("format option is not supported");

        assert!(error.contains("unknown wasm-preview option `--format`"));
    }

    #[test]
    fn usage_lists_base_svg_command() {
        let usage = usage_text();

        assert!(usage.contains("cargo xtask base-svg [options]"));
        assert!(usage.contains("cargo xtask wasm-preview [options]"));
        assert!(usage.contains("target/base-pocket-table.svg"));
        assert!(usage.contains("target/wasm-preview"));
    }
}
