mod adapter;
mod cli;
mod model;

pub use cli::Cli;
pub use model::{
    known_carom_fixture, Bounds, CandidateReport, Controls, ExperimentConfig, ExperimentReport,
    KnownFixture, Mode, NoiseWidths, Position, Shooter, TrialDisposition, TrialReport,
};

pub fn run(config: &ExperimentConfig) -> Result<ExperimentReport, String> {
    adapter::run(config)
}
