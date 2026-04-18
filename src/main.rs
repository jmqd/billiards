use billiards::dsl::{parse_dsl_to_scenario, ScenarioTraceRenderOptions};
use billiards::{
    write_png_to_file,
    visualization::PathColorMode,
    BallSetPhysicsSpec, CollisionModel, DiagramBackground, DiagramRenderOptions,
    InchesPerSecondSq, MotionPhaseConfig, MotionTransitionConfig, OnTableMotionConfig,
    RadiansPerSecondSq, RailModel, RollingResistanceModel, Seconds, SlidingFrictionModel,
    SpinDecayModel,
};
use clap::{Parser, ValueEnum};
use std::fs;
use std::path::PathBuf;

fn shot_preview_motion_config() -> OnTableMotionConfig {
    MotionTransitionConfig {
        phase: MotionPhaseConfig::default(),
        sliding_friction: SlidingFrictionModel::ConstantAcceleration {
            acceleration_magnitude: InchesPerSecondSq::new("5"),
        },
        spin_decay: SpinDecayModel::ConstantAngularDeceleration {
            angular_deceleration: RadiansPerSecondSq::new(2.0),
        },
        rolling_resistance: RollingResistanceModel::ConstantDeceleration {
            linear_deceleration: InchesPerSecondSq::new("5"),
        },
    }
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

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Input .billiards file path
    #[arg(required = true)]
    input: PathBuf,

    /// Output .png file path (optional, defaults to input filename with .png extension)
    #[arg(short, long)]
    output: Option<PathBuf>,

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

    /// Scale the final rendered PNG by this positive integer factor.
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
        max_time_step: Seconds::new(args.trace_sample_step_seconds),
        line_width_px: 3.0,
        start_ghost_balls: args.trace_start_ghosts,
        event_markers: args.trace_event_markers,
        labels: args.trace_labels,
        path_color_mode: args.trace_color_mode.into(),
    };
    let render_state = if let Some(trace) = scenario
        .simulate_shot_trace_with_rails_and_pockets_on_table_until_rest(
            &ball_set,
            &motion,
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        )? {
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
        println!(
            "Simulated shot to rest: {} event(s), {} pocketed, {} on-table remaining",
            trace.simulation.events.len(),
            pocketed,
            remaining
        );
        trace.rendered_final_layout_with_trace_options(&scenario, &trace_render, &ball_set, &motion)
    } else {
        scenario.game_state
    };

    let img = render_state.draw_2d_diagram_with_options(&DiagramRenderOptions {
        scale_factor: args.scale_factor.max(1),
        background: if args.transparent_background {
            DiagramBackground::Transparent
        } else {
            DiagramBackground::Table
        },
    });

    let output_path = match args.output {
        Some(path) => path,
        None => {
            let mut path = args.input.clone();
            path.set_extension("png");
            path
        }
    };

    write_png_to_file(&img, Some(&output_path));
    println!("Diagram written to {:?}", output_path);

    Ok(())
}
