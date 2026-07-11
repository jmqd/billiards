use std::process::ExitCode;

use clap::Parser;
use simul_three_cushion::Cli;

fn main() -> ExitCode {
    match execute() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn execute() -> Result<(), String> {
    let config = Cli::parse().into_config()?;
    let report = simul_three_cushion::run(&config)?;
    report
        .write_to(std::io::stdout().lock())
        .map_err(|error| format!("could not write report: {error}"))
}
