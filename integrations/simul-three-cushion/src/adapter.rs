use std::{cmp::Ordering, num::NonZeroUsize};

use simul::experiment::search::sample_bounded;
use simul::experiment::{
    run_parallel, run_serial, BernoulliReducer, CandidateId, FixedReplications, MasterSeed,
    ParallelConfig, SeedDeriver, TrialContext, TrialStream,
};

use crate::{
    CandidateReport, Controls, ExperimentConfig, ExperimentReport, Mode, TrialDisposition,
    TrialReport,
};

mod physics;

#[derive(Clone, Copy, Debug)]
struct Candidate {
    id: CandidateId,
    controls: Controls,
}

#[derive(Clone, Debug)]
struct EvaluatedTrial {
    applied: Controls,
    outcome: physics::Outcome,
}

#[derive(Clone, Debug)]
struct FailedTrial {
    applied: Controls,
    detail: String,
}

pub fn run(config: &ExperimentConfig) -> Result<ExperimentReport, String> {
    config.validate()?;
    let master_seed = MasterSeed::from_u64(config.master_seed);
    let candidates = make_candidates(config, master_seed)?;
    let trial_capacity = candidates
        .len()
        .checked_mul(config.replication_budget as usize)
        .ok_or("candidate × replication budget exceeds addressable memory")?;
    let schedule = FixedReplications::new(config.replication_budget, 0);
    let mut specs = Vec::with_capacity(trial_capacity);
    for candidate in &candidates {
        specs.extend(schedule.trials(candidate.id, *candidate));
    }

    let records = if config.workers == NonZeroUsize::MIN {
        // Preserve the original serial path exactly for the default configuration.
        let evaluator = physics::Evaluator::new(config)?;
        run_serial(master_seed, specs, |candidate, context| {
            evaluate_trial(&evaluator, candidate, config, context)
        })
        .map_err(|error| format!("trial scheduling failed: {error:?}"))?
    } else {
        // Validate construction before starting the pool so layout/configuration failures
        // remain run-level failures rather than becoming one failure per trial.
        physics::Evaluator::new(config)?;
        let batch_limit = NonZeroUsize::new(trial_capacity)
            .ok_or("trial batch must contain at least one record")?;
        run_parallel(
            master_seed,
            specs,
            ParallelConfig::new(config.workers, batch_limit),
            || physics::Evaluator::new(config),
            |evaluator, candidate, context| match evaluator {
                Ok(evaluator) => evaluate_trial(evaluator, candidate, config, context),
                Err(detail) => {
                    let applied = apply_execution_noise(candidate.controls, config, context);
                    Err(FailedTrial {
                        applied,
                        detail: detail.clone(),
                    })
                }
            },
        )
        .map_err(|error| format!("parallel trial scheduling failed: {error:?}"))?
    };

    let mut candidate_reports = Vec::with_capacity(candidates.len());
    let mut trial_reports = Vec::with_capacity(records.len());
    let records_per_candidate = config.replication_budget as usize;
    for (candidate_index, candidate) in candidates.iter().enumerate() {
        let start = candidate_index * records_per_candidate;
        let candidate_records = &records[start..start + records_per_candidate];
        let mut scored = 0_u32;
        let mut missed = 0_u32;
        let mut indeterminate = 0_u32;
        let mut failed = 0_u32;
        let mut reducer = BernoulliReducer::default();
        for record in candidate_records {
            let (applied, disposition) = match &record.result {
                Ok(evaluated) => match &evaluated.outcome {
                    physics::Outcome::Scored => {
                        scored += 1;
                        reducer.observe(true);
                        (evaluated.applied, TrialDisposition::Scored)
                    }
                    physics::Outcome::Miss(detail) => {
                        missed += 1;
                        reducer.observe(false);
                        (evaluated.applied, TrialDisposition::Miss(detail.clone()))
                    }
                    physics::Outcome::Indeterminate(detail) => {
                        indeterminate += 1;
                        (
                            evaluated.applied,
                            TrialDisposition::Indeterminate(detail.clone()),
                        )
                    }
                },
                Err(error) => {
                    failed += 1;
                    (
                        error.applied,
                        TrialDisposition::Failed(error.detail.clone()),
                    )
                }
            };
            let key = record.key;
            trial_reports.push(TrialReport {
                candidate_id: key.candidate().value(),
                replication_id: key.replication().value(),
                replay_key: replay_key(config.master_seed, record.replay),
                applied,
                disposition,
            });
        }
        let statistics = reducer.finish().ok();
        candidate_reports.push(CandidateReport {
            rank: None,
            candidate_id: candidate.id.value(),
            controls: candidate.controls,
            requested: config.replication_budget,
            scored,
            missed,
            indeterminate,
            failed,
            success_rate: statistics.map(|value| value.rate),
            confidence_low: statistics.map(|value| value.wilson_lower),
            confidence_high: statistics.map(|value| value.wilson_upper),
            eligible: indeterminate == 0 && failed == 0,
        });
    }

    if config.mode == Mode::Search {
        rank_search_candidates(&mut candidate_reports);
    }
    Ok(ExperimentReport {
        mode: config.mode,
        shooter: config.shooter,
        master_seed: config.master_seed,
        seed_protocol: "simul-v1-blake3-chacha12",
        candidates: candidate_reports,
        trials: trial_reports,
    })
}

fn evaluate_trial(
    evaluator: &physics::Evaluator,
    candidate: &Candidate,
    config: &ExperimentConfig,
    context: &TrialContext,
) -> Result<EvaluatedTrial, FailedTrial> {
    let applied = apply_execution_noise(candidate.controls, config, context);
    evaluator
        .evaluate(config.shooter, applied, config.max_events)
        .map(|outcome| EvaluatedTrial { applied, outcome })
        .map_err(|detail| FailedTrial { applied, detail })
}

fn make_candidates(
    config: &ExperimentConfig,
    master_seed: MasterSeed,
) -> Result<Vec<Candidate>, String> {
    let budget = usize::try_from(config.candidate_budget)
        .map_err(|_| "candidate budget does not fit this platform")?;
    match config.mode {
        Mode::Sensitivity => Ok(make_sensitivity_candidates(config, master_seed, budget)),
        Mode::Search => {
            let bounds: Vec<_> = config
                .search_bounds
                .iter()
                .map(|bounds| (bounds.minimum, bounds.maximum))
                .collect();
            let sampled = sample_bounded(master_seed, budget, &bounds)
                .map_err(|error| format!("invalid search bounds: {error:?}"))?;
            sampled
                .into_iter()
                .enumerate()
                .map(|(index, values)| {
                    let [heading, speed, tip_side, tip_height, elevation]: [f64; 5] = values
                        .try_into()
                        .map_err(|_| "simul search returned the wrong control dimension")?;
                    Ok(Candidate {
                        id: CandidateId::new(index as u64),
                        controls: Controls {
                            heading,
                            speed,
                            tip_side,
                            tip_height,
                            elevation,
                        },
                    })
                })
                .collect()
        }
    }
}

fn make_sensitivity_candidates(
    config: &ExperimentConfig,
    master_seed: MasterSeed,
    budget: usize,
) -> Vec<Candidate> {
    let deriver = SeedDeriver::new(master_seed);
    let center_count = config.sensitivity_centers.len();
    (0..budget)
        .map(|index| {
            let center = config.sensitivity_centers[index % center_count];
            let controls = if index < center_count {
                center
            } else {
                perturb_candidate(center, config, &deriver, index as u64)
            };
            Candidate {
                id: CandidateId::new(index as u64),
                controls,
            }
        })
        .collect()
}

fn perturb_candidate(
    center: Controls,
    config: &ExperimentConfig,
    deriver: &SeedDeriver,
    token: u64,
) -> Controls {
    let sample = |dimension: u64| {
        let stream_token = token.wrapping_mul(8).wrapping_add(dimension);
        2.0 * deriver.proposal_seed(stream_token).rng().uniform_f64() - 1.0
    };
    Controls {
        heading: center.heading + config.perturbations.heading * sample(0),
        speed: center.speed + config.perturbations.speed * sample(1),
        tip_side: center.tip_side + config.perturbations.tip_side * sample(2),
        tip_height: center.tip_height + config.perturbations.tip_height * sample(3),
        elevation: center.elevation + config.perturbations.elevation * sample(4),
    }
}

fn apply_execution_noise(
    candidate: Controls,
    config: &ExperimentConfig,
    context: &TrialContext,
) -> Controls {
    let sample = |tag| {
        let mut rng = context.common_rng(TrialStream::ExecutionNoise, tag);
        2.0 * rng.uniform_f64() - 1.0
    };
    Controls {
        heading: candidate.heading + config.execution_noise.heading * sample("heading-degrees"),
        speed: candidate.speed + config.execution_noise.speed * sample("launch-speed-ips"),
        tip_side: candidate.tip_side + config.execution_noise.tip_side * sample("tip-side-radii"),
        tip_height: candidate.tip_height
            + config.execution_noise.tip_height * sample("tip-height-radii"),
        elevation: candidate.elevation
            + config.execution_noise.elevation * sample("elevation-degrees"),
    }
}

fn replay_key(master_seed: u64, replay: simul::experiment::ReplayKey) -> String {
    let trial = replay.trial();
    format!(
        "v1:{master_seed}:{}:{}:{}",
        trial.candidate().value(),
        trial.replication().value(),
        replay.common_random_group()
    )
}

fn rank_search_candidates(candidates: &mut [CandidateReport]) {
    candidates.sort_by(|left, right| {
        right
            .eligible
            .cmp(&left.eligible)
            .then_with(|| compare_descending(right.confidence_low, left.confidence_low))
            .then_with(|| compare_descending(right.success_rate, left.success_rate))
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    let mut rank = 0_usize;
    for candidate in candidates {
        if candidate.eligible {
            rank += 1;
            candidate.rank = Some(rank);
        }
    }
}

fn compare_descending(left: Option<f64>, right: Option<f64>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.total_cmp(&right),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}
