use billiards::shot_simulation::{ShotLayout, ShotSimulationError};

use crate::ExperimentConfig;

pub fn layout(config: &ExperimentConfig) -> Result<ShotLayout, String> {
    let [white, yellow, red] = config.positions;
    ShotLayout::three_cushion_from_diamonds(
        (white.x, white.y),
        (yellow.x, yellow.y),
        (red.x, red.y),
    )
    .map_err(|error: ShotSimulationError| format!("invalid carom layout: {error}"))
}
