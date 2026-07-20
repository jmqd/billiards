mod adapter;
mod cli;
mod model;

pub use billiards::shot_simulation::ReplayKey;
pub use cli::Cli;
pub use model::{
    known_carom_fixture, Bounds, CandidateReport, Controls, ExperimentConfig, ExperimentReport,
    KnownFixture, Mode, NoiseSigmas, OutcomeSummary, PerturbationWidths, Position, Shooter,
    TrialDisposition, TrialReport, TrialStage,
};

pub fn run(config: &ExperimentConfig) -> Result<ExperimentReport, String> {
    adapter::run(config)
}
