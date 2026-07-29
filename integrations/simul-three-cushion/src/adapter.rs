use billiards::shot_simulation::{
    run_robust_three_cushion_experiment, RobustCandidateReport, RobustExperimentMode,
    RobustOutcomeSummary, RobustShotControls, RobustThreeCushionExperimentConfig,
    RobustTrialDisposition, RobustTrialReport, RobustTrialStage,
};

use crate::model::{robust_bounds, robust_controls, robust_perturbations, robust_sigmas};
use crate::{
    CandidateReport, Controls, ExperimentConfig, ExperimentReport, Mode, OutcomeSummary, Shooter,
    TrialDisposition, TrialReport, TrialStage,
};

mod physics;

const PHYSICS_PROFILE: &str = "three-cushion-default";
const NOISE_MODEL: &str = "independent-truncated-normal";
const SEARCH_SELECTION_POLICY: &str = "wilson95-lower-then-rate-then-object-first-then-id";
const NO_SELECTION_POLICY: &str = "none";

pub fn run(config: &ExperimentConfig) -> Result<ExperimentReport, String> {
    config.validate()?;
    let robust = run_robust_three_cushion_experiment(&RobustThreeCushionExperimentConfig {
        mode: match config.mode {
            Mode::Sensitivity => RobustExperimentMode::Sensitivity,
            Mode::Search => RobustExperimentMode::Search,
        },
        physics: billiards::shot_simulation::PhysicsProfile::three_cushion_default(),
        layout: physics::layout(config)?,
        cue: billiards::shot_simulation::canonical_three_cushion_cue_config(),
        shooter: match config.shooter {
            Shooter::White => billiards::shot_simulation::ThreeCushionShooter::Cue,
            Shooter::Yellow => billiards::shot_simulation::ThreeCushionShooter::YellowCue,
        },
        nominal: robust_controls(config.nominal),
        additional_candidates: config
            .additional_candidates
            .iter()
            .copied()
            .map(robust_controls)
            .collect(),
        perturbations: robust_perturbations(config.perturbations),
        shot_inaccuracy: robust_sigmas(config.shot_inaccuracy),
        search_bounds: config.search_bounds.map(robust_bounds),
        master_seed: config.master_seed,
        candidate_budget: config.candidate_budget,
        screening_replication_budget: config.screening_replication_budget,
        finalist_budget: config.finalist_budget,
        validation_replication_budget: config.validation_replication_budget,
        workers: config.workers,
        max_events: config.max_events,
    })
    .map_err(|error| error.to_string())?;

    let candidates = robust
        .candidates
        .into_iter()
        .map(candidate_report)
        .collect();
    let trials = robust.trials.into_iter().map(trial_report).collect();
    Ok(ExperimentReport {
        mode: config.mode,
        shooter: config.shooter,
        positions: config.positions,
        perturbations: config.perturbations,
        shot_inaccuracy: config.shot_inaccuracy,
        search_bounds: config.search_bounds,
        candidate_budget: config.candidate_budget,
        screening_replication_budget: config.screening_replication_budget,
        finalist_budget: config.finalist_budget,
        validation_replication_budget: config.validation_replication_budget,
        requested_workers: config.workers,
        max_events: config.max_events,
        master_seed: config.master_seed,
        seed_protocol: billiards::shot_simulation::ROBUST_SEED_PROTOCOL,
        physics_profile: PHYSICS_PROFILE,
        noise_model: NOISE_MODEL,
        noise_parent_sigma_limit:
            billiards::shot_simulation::ROBUST_NOISE_TRUNCATION_STANDARD_DEVIATIONS,
        selection_policy: match config.mode {
            Mode::Search => SEARCH_SELECTION_POLICY,
            Mode::Sensitivity => NO_SELECTION_POLICY,
        },
        winner_id: robust.winner_id,
        candidates,
        trials,
    })
}

const fn controls(controls: RobustShotControls) -> Controls {
    Controls {
        heading: controls.heading,
        speed: controls.speed,
        tip_side: controls.tip_side,
        tip_height: controls.tip_height,
        elevation: controls.elevation,
    }
}

fn outcome_summary(summary: RobustOutcomeSummary) -> OutcomeSummary {
    OutcomeSummary {
        requested: summary.requested,
        scored: summary.scored,
        scored_object_first: summary.scored_object_first,
        missed: summary.missed,
        indeterminate: summary.indeterminate,
        failed: summary.failed,
        success_rate: summary.success_rate,
        confidence_low: summary.confidence_low,
        confidence_high: summary.confidence_high,
        eligible: summary.eligible,
    }
}

fn candidate_report(candidate: RobustCandidateReport) -> CandidateReport {
    CandidateReport {
        rank: candidate.rank,
        candidate_id: candidate.candidate_id,
        controls: controls(candidate.controls),
        screening: outcome_summary(candidate.screening),
        validation: candidate.validation.map(outcome_summary),
    }
}

fn trial_report(trial: RobustTrialReport) -> TrialReport {
    TrialReport {
        stage: match trial.stage {
            RobustTrialStage::Screening => TrialStage::Screening,
            RobustTrialStage::Validation => TrialStage::Validation,
        },
        candidate_id: trial.candidate_id,
        replication_id: trial.replication_id,
        replay_key: trial.replay_key,
        applied: trial.applied.map(controls),
        disposition: match trial.disposition {
            RobustTrialDisposition::Scored => TrialDisposition::Scored,
            RobustTrialDisposition::Miss(detail) => TrialDisposition::Miss(detail),
            RobustTrialDisposition::Indeterminate(detail) => {
                TrialDisposition::Indeterminate(detail)
            }
            RobustTrialDisposition::Failed(detail) => TrialDisposition::Failed(detail),
        },
    }
}
