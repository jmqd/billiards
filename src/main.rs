use billiards::dsl::parse_dsl_to_scenario;
use billiards::{
    write_png_to_file, BallSetPhysicsSpec, CollisionModel, InchesPerSecondSq, MotionPhaseConfig,
    MotionTransitionConfig, OnTableMotionConfig, RadiansPerSecondSq, RailModel,
    RollingResistanceModel, Seconds, SlidingFrictionModel, SpinDecayModel,
};
use clap::Parser;
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

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Input .billiards file path
    #[arg(required = true)]
    input: PathBuf,

    /// Output .png file path (optional, defaults to input filename with .png extension)
    #[arg(short, long)]
    output: Option<PathBuf>,
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
        trace.rendered_final_layout_with_traces(&scenario, Seconds::new(0.02), &ball_set, &motion)
    } else {
        scenario.game_state
    };

    let img = render_state.draw_2d_diagram();

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
