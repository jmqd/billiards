use billiards::dsl::{parse_dsl_to_scenario, ScenarioTraceRenderOptions};
use billiards::{
    diagram::DiagramOutputFormat,
    human_tuned_preview_motion_config,
    visualization::{BallPathRenderOptions, PathColorMode},
    BallSetPhysicsSpec, CollisionModel, DiagramBackground, DiagramRenderOptions,
    OnTableMotionConfig, RailModel, Seconds,
};
use clap::{Parser, ValueEnum};
use std::fs;
use std::path::PathBuf;

fn shot_preview_motion_config() -> OnTableMotionConfig {
    human_tuned_preview_motion_config()
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum TraceColorModeArg {
    Solid,
    FadeByTime,
    MotionPhase,
}

impl From<TraceColorModeArg> for PathColorMode {
    fn from(value: TraceColorModeArg) -> Self {
        match value {
            TraceColorModeArg::Solid => PathColorMode::Solid,
            TraceColorModeArg::FadeByTime => PathColorMode::FadeByTime,
            TraceColorModeArg::MotionPhase => PathColorMode::MotionPhase,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormatArg {
    Png,
    Svg,
}

impl From<OutputFormatArg> for DiagramOutputFormat {
    fn from(value: OutputFormatArg) -> Self {
        match value {
            OutputFormatArg::Png => DiagramOutputFormat::Png,
            OutputFormatArg::Svg => DiagramOutputFormat::Svg,
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Input .billiards file path
    #[arg(required = true)]
    input: PathBuf,

    /// Output diagram file path. The format is inferred from extension unless --format is set.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Output format. Defaults to SVG unless an output path extension says otherwise.
    #[arg(long, value_enum)]
    format: Option<OutputFormatArg>,

    /// Render translucent ghost balls at the start of each moving trace.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    trace_start_ghosts: bool,

    /// Render event markers at traced segment endpoints.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    trace_event_markers: bool,

    /// Render numeric labels next to traced segment endpoints.
    #[arg(long, default_value_t = false, action = clap::ArgAction::Set)]
    trace_labels: bool,

    /// Render traced paths with solid, fade-by-time, or motion-phase coloring.
    #[arg(long, value_enum, default_value_t = TraceColorModeArg::Solid)]
    trace_color_mode: TraceColorModeArg,

    /// Maximum trace sampling step in seconds for smooth path rendering.
    #[arg(long, default_value_t = 0.02)]
    trace_sample_step_seconds: f64,

    /// Maximum simulation events to include in the rendered trace; use 0 for scenario/default behavior.
    #[arg(long, default_value_t = 0)]
    trace_max_events: usize,

    /// Scale PNG exports by this positive integer factor. SVG keeps a scalable viewBox.
    #[arg(long, default_value_t = 1)]
    scale_factor: u32,

    /// Render onto a transparent background instead of the table image.
    #[arg(long, default_value_t = false, action = clap::ArgAction::SetTrue)]
    transparent_background: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let input_content = fs::read_to_string(&args.input)
        .map_err(|e| format!("Failed to read input file {:?}: {}", args.input, e))?;

    let mut scenario = parse_dsl_to_scenario(&input_content)?;

    // The DSL parser constructs the state, but we need to resolve any unresolved shifts
    // (though currently DSL might produce fully resolved coordinates, resolve_positions handles
    // any manual inches-based shifts if we added them. The current DSL implementation
    // does resolve aliases immediately, but if we add inches support in DSL later, this is good practice).
    scenario.game_state.resolve_positions();

    let ball_set = BallSetPhysicsSpec::default();
    let motion = shot_preview_motion_config();
    let trace_render = ScenarioTraceRenderOptions {
        path_render: BallPathRenderOptions {
            max_time_step: Seconds::new(args.trace_sample_step_seconds),
            ..BallPathRenderOptions::default()
        },
        start_ghost_balls: args.trace_start_ghosts,
        event_markers: args.trace_event_markers,
        labels: args.trace_labels,
        path_color_mode: args.trace_color_mode.into(),
    };
    let effective_trace_max_events = if args.trace_max_events == 0 {
        scenario.trace_max_events.or_else(|| {
            scenario
                .preferred_simulation_name()
                .and_then(|name| scenario.simulation_named(name).ok())
                .and_then(|simulation| simulation.max_events)
        })
    } else {
        Some(args.trace_max_events)
    };

    let trace = if let Some(max_events) = effective_trace_max_events {
        scenario.simulate_shot_trace_with_preferred_physics_on_table_until_event_limit(
            &ball_set,
            &motion,
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
            max_events,
        )?
    } else {
        scenario.simulate_shot_trace_with_preferred_physics_on_table_until_rest(
            &ball_set,
            &motion,
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        )?
    };

    let render_state = if let Some(trace) = trace {
        for line in trace.event_lines() {
            println!("{line}");
        }
        let pocketed = trace
            .simulation
            .states
            .iter()
            .filter(|state| matches!(state, billiards::NBallSystemState::Pocketed { .. }))
            .count();
        let remaining = trace
            .simulation
            .states
            .iter()
            .filter(|state| matches!(state, billiards::NBallSystemState::OnTable(_)))
            .count();
        let limit_status = if effective_trace_max_events
            .is_some_and(|max_events| trace.simulation.events.len() >= max_events)
        {
            "event limit"
        } else {
            "rest"
        };
        println!(
            "Simulated shot to {limit_status}: {} event(s), {} pocketed, {} on-table remaining",
            trace.simulation.events.len(),
            pocketed,
            remaining
        );
        trace.rendered_final_layout_with_trace_options(&scenario, &trace_render)
    } else {
        scenario.game_state
    };

    let output_format = args
        .format
        .map(Into::into)
        .or_else(|| {
            args.output
                .as_ref()
                .and_then(|path| path.extension())
                .and_then(|extension| extension.to_str())
                .and_then(DiagramOutputFormat::from_extension)
        })
        .unwrap_or(DiagramOutputFormat::Svg);
    let output_path = match args.output {
        Some(path) => path,
        None => {
            let mut path = args.input.clone();
            path.set_extension(output_format.extension());
            path
        }
    };

    let render_options = DiagramRenderOptions {
        scale_factor: args.scale_factor.max(1),
        background: if args.transparent_background {
            DiagramBackground::Transparent
        } else {
            DiagramBackground::Table
        },
    };
    let diagram = render_state.render_2d_diagram_with_options(output_format, &render_options);

    fs::write(&output_path, diagram)?;
    println!("Diagram written to {:?}", output_path);

    Ok(())
}
