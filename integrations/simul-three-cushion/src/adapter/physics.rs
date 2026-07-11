use billiards::shot_simulation::{
    execute_three_cushion_compact, PhysicsProfile, ShotControls, ShotLayout, ShotLimit,
    ThreeCushionAdjudication, ThreeCushionShooter, ThreeCushionShot,
};

use crate::{Controls, ExperimentConfig, Shooter};

pub struct Evaluator {
    physics: PhysicsProfile,
    layout: ShotLayout,
}

#[derive(Clone, Debug)]
pub enum Outcome {
    Scored,
    Miss(String),
    Indeterminate(String),
}

impl Evaluator {
    pub fn new(config: &ExperimentConfig) -> Result<Self, String> {
        let [white, yellow, red] = config.positions;
        let layout = ShotLayout::three_cushion_from_diamonds(
            (white.x, white.y),
            (yellow.x, yellow.y),
            (red.x, red.y),
        )
        .map_err(|error| format!("invalid carom layout: {error}"))?;
        Ok(Self {
            physics: PhysicsProfile::three_cushion_default(),
            layout,
        })
    }

    pub fn evaluate(
        &self,
        shooter: Shooter,
        controls: Controls,
        max_events: usize,
    ) -> Result<Outcome, String> {
        let controls = ShotControls::new(
            controls.heading,
            controls.speed,
            controls.tip_side,
            controls.tip_height,
            controls.elevation,
        )
        .map_err(|error| format!("invalid noisy shot controls: {error}"))?;
        let shooter = match shooter {
            Shooter::White => ThreeCushionShooter::Cue,
            Shooter::Yellow => ThreeCushionShooter::YellowCue,
        };
        let shot = ThreeCushionShot::new(shooter, controls);
        let result = execute_three_cushion_compact(
            &self.physics,
            &self.layout,
            &shot,
            ShotLimit::EventCount(max_events),
        )
        .map_err(|error| format!("shot execution failed: {error}"))?;
        Ok(match result.completion.summary {
            ThreeCushionAdjudication::Scored(_) => Outcome::Scored,
            ThreeCushionAdjudication::Miss { reason, .. } => Outcome::Miss(format!("{reason:?}")),
            ThreeCushionAdjudication::Indeterminate { reason, .. } => {
                Outcome::Indeterminate(format!("{reason:?}"))
            }
        })
    }
}
