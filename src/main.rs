use billiards::dsl::parse_dsl_to_game_state;
use billiards::write_png_to_file;
use clap::Parser;
use std::fs;
use std::path::PathBuf;

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

    let mut game_state = parse_dsl_to_game_state(&input_content)?;

    // The DSL parser constructs the state, but we need to resolve any unresolved shifts
    // (though currently DSL might produce fully resolved coordinates, resolve_positions handles
    // any manual inches-based shifts if we added them. The current DSL implementation
    // does resolve aliases immediately, but if we add inches support in DSL later, this is good practice).
    game_state.resolve_positions();

    let img = game_state.draw_2d_diagram();

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
