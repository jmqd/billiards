use std::cmp::Ordering;

use crate::{
    CandidateReport, Controls, ExperimentConfig, ExperimentReport, Mode, TrialDisposition,
    TrialReport,
};

mod physics;

const SEED_PROTOCOL: &str = "local-v1-splitmix64";
const GOLDEN_RATIO: u64 = 0x9e37_79b9_7f4a_7c15;
const DOMAIN_SEARCH: u64 = 0x5345_4152_4348;
const DOMAIN_SENSITIVITY: u64 = 0x5345_4e53_4954;
const DOMAIN_EXECUTION: u64 = 0x4558_4543_5554;

#[derive(Clone, Copy, Debug)]
struct Candidate {
    id: u64,
    controls: Controls,
}

#[derive(Clone, Copy, Debug)]
struct TrialKey {
    candidate_id: u64,
    replication_id: u32,
}

#[derive(Clone, Copy, Debug)]
struct ReplayKey {
    trial: TrialKey,
    common_random_group: u64,
}

#[derive(Clone, Copy, Debug)]
struct TrialSpec {
    candidate: Candidate,
    key: TrialKey,
    replay: ReplayKey,
}

#[derive(Clone, Copy, Debug)]
struct TrialContext {
    master_seed: u64,
    common_random_group: u64,
}

impl TrialContext {
    fn uniform(&self, tag: &str) -> f64 {
        let tag_seed = hash_tag(tag);
        let seed = derive_seed(
            self.master_seed,
            DOMAIN_EXECUTION,
            (self.common_random_group << 32) ^ tag_seed,
        );
        uniform_f64(seed)
    }
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

#[derive(Clone, Debug)]
struct TrialRecord {
    key: TrialKey,
    replay: ReplayKey,
    result: Result<EvaluatedTrial, FailedTrial>,
}

#[derive(Default)]
struct BernoulliReducer {
    successes: u32,
    observations: u32,
}

struct BernoulliStatistics {
    rate: f64,
    wilson_lower: f64,
    wilson_upper: f64,
}

impl BernoulliReducer {
    fn observe(&mut self, success: bool) {
        self.observations = self.observations.saturating_add(1);
        if success {
            self.successes = self.successes.saturating_add(1);
        }
    }

    fn finish(&self) -> Option<BernoulliStatistics> {
        if self.observations == 0 {
            return None;
        }
        let n = f64::from(self.observations);
        let rate = f64::from(self.successes) / n;
        let z = 1.959_963_984_540_054;
        let z_squared = z * z;
        let denominator = 1.0 + z_squared / n;
        let center = (rate + z_squared / (2.0 * n)) / denominator;
        let margin =
            z * ((rate * (1.0 - rate) / n + z_squared / (4.0 * n * n)).sqrt()) / denominator;
        Some(BernoulliStatistics {
            rate,
            wilson_lower: (center - margin).max(0.0),
            wilson_upper: (center + margin).min(1.0),
        })
    }
}

pub fn run(config: &ExperimentConfig) -> Result<ExperimentReport, String> {
    config.validate()?;
    let candidates = make_candidates(config)?;
    let records_per_candidate = usize::try_from(config.replication_budget)
        .map_err(|_| "replication budget does not fit this platform")?;
    let trial_capacity = candidates
        .len()
        .checked_mul(records_per_candidate)
        .ok_or("candidate × replication budget exceeds addressable memory")?;
    let mut specs = Vec::with_capacity(trial_capacity);
    for candidate in &candidates {
        for replication_id in 0..config.replication_budget {
            let key = TrialKey {
                candidate_id: candidate.id,
                replication_id,
            };
            let common_random_group = u64::from(replication_id);
            specs.push(TrialSpec {
                candidate: *candidate,
                key,
                replay: ReplayKey {
                    trial: key,
                    common_random_group,
                },
            });
        }
    }

    let records = run_trials(config, specs)?;
    let mut candidate_reports = Vec::with_capacity(candidates.len());
    let mut trial_reports = Vec::with_capacity(records.len());
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
            trial_reports.push(TrialReport {
                candidate_id: record.key.candidate_id,
                replication_id: record.key.replication_id,
                replay_key: replay_key(config.master_seed, record.replay),
                applied,
                disposition,
            });
        }
        let statistics = reducer.finish();
        candidate_reports.push(CandidateReport {
            rank: None,
            candidate_id: candidate.id,
            controls: candidate.controls,
            requested: config.replication_budget,
            scored,
            missed,
            indeterminate,
            failed,
            success_rate: statistics.as_ref().map(|value| value.rate),
            confidence_low: statistics.as_ref().map(|value| value.wilson_lower),
            confidence_high: statistics.as_ref().map(|value| value.wilson_upper),
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
        seed_protocol: SEED_PROTOCOL,
        candidates: candidate_reports,
        trials: trial_reports,
    })
}

fn run_trials(
    config: &ExperimentConfig,
    specs: Vec<TrialSpec>,
) -> Result<Vec<TrialRecord>, String> {
    if specs.is_empty() {
        return Ok(Vec::new());
    }
    let worker_count = config.workers.get().min(specs.len());
    if worker_count == 1 {
        let evaluator = physics::Evaluator::new(config)?;
        return Ok(specs
            .iter()
            .map(|spec| execute_spec(&evaluator, config, spec))
            .collect());
    }

    // Validate once before launching workers so invalid layouts remain run-level errors.
    physics::Evaluator::new(config)?;
    // Each worker owns its evaluator. Chunks are joined in their original order,
    // so worker scheduling cannot change report or trial ordering.
    std::thread::scope(|scope| {
        let chunk_size = specs.len().div_ceil(worker_count);
        let mut handles = Vec::with_capacity(worker_count);
        for chunk in specs.chunks(chunk_size) {
            handles.push(scope.spawn(move || {
                let evaluator = physics::Evaluator::new(config);
                chunk
                    .iter()
                    .map(|spec| match &evaluator {
                        Ok(evaluator) => execute_spec(evaluator, config, spec),
                        Err(detail) => failed_record(config, spec, detail),
                    })
                    .collect::<Vec<_>>()
            }));
        }
        let mut records = Vec::with_capacity(specs.len());
        for handle in handles {
            records.extend(
                handle
                    .join()
                    .map_err(|_| "parallel trial worker panicked".to_string())?,
            );
        }
        Ok(records)
    })
}

fn execute_spec(
    evaluator: &physics::Evaluator,
    config: &ExperimentConfig,
    spec: &TrialSpec,
) -> TrialRecord {
    let context = TrialContext {
        master_seed: config.master_seed,
        common_random_group: spec.replay.common_random_group,
    };
    let result = evaluate_trial(evaluator, &spec.candidate, config, &context);
    TrialRecord {
        key: spec.key,
        replay: spec.replay,
        result,
    }
}

fn failed_record(config: &ExperimentConfig, spec: &TrialSpec, detail: &str) -> TrialRecord {
    let context = TrialContext {
        master_seed: config.master_seed,
        common_random_group: spec.replay.common_random_group,
    };
    let applied = apply_execution_noise(spec.candidate.controls, config, &context);
    TrialRecord {
        key: spec.key,
        replay: spec.replay,
        result: Err(FailedTrial {
            applied,
            detail: detail.to_string(),
        }),
    }
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

fn make_candidates(config: &ExperimentConfig) -> Result<Vec<Candidate>, String> {
    let budget = usize::try_from(config.candidate_budget)
        .map_err(|_| "candidate budget does not fit this platform")?;
    match config.mode {
        Mode::Sensitivity => make_sensitivity_candidates(config, budget),
        Mode::Search => {
            let sampled = sample_bounded(config.master_seed, budget, &config.search_bounds);
            Ok(sampled
                .into_iter()
                .enumerate()
                .map(
                    |(index, [heading, speed, tip_side, tip_height, elevation])| Candidate {
                        id: index as u64,
                        controls: Controls {
                            heading,
                            speed,
                            tip_side,
                            tip_height,
                            elevation,
                        },
                    },
                )
                .collect())
        }
    }
}

fn make_sensitivity_candidates(
    config: &ExperimentConfig,
    budget: usize,
) -> Result<Vec<Candidate>, String> {
    let center_count = config.sensitivity_centers.len();
    if center_count == 0 {
        return Err("sensitivity mode requires at least one center".into());
    }
    Ok((0..budget)
        .map(|index| {
            let center = config.sensitivity_centers[index % center_count];
            let controls = if index < center_count {
                center
            } else {
                perturb_candidate(center, config, index as u64)
            };
            Candidate {
                id: index as u64,
                controls,
            }
        })
        .collect())
}

fn sample_bounded(master_seed: u64, budget: usize, bounds: &[crate::Bounds; 5]) -> Vec<[f64; 5]> {
    (0..budget)
        .map(|index| {
            let candidate = index as u64;
            std::array::from_fn(|dimension| {
                let bound = bounds[dimension];
                let seed = derive_seed(
                    master_seed,
                    DOMAIN_SEARCH,
                    candidate.wrapping_mul(8).wrapping_add(dimension as u64),
                );
                if bound.minimum == bound.maximum {
                    bound.minimum
                } else {
                    let sample = uniform_f64(seed);
                    bound.minimum * (1.0 - sample) + bound.maximum * sample
                }
            })
        })
        .collect()
}

fn perturb_candidate(center: Controls, config: &ExperimentConfig, token: u64) -> Controls {
    let sample = |dimension: u64| {
        let seed = derive_seed(
            config.master_seed,
            DOMAIN_SENSITIVITY,
            token.wrapping_mul(8).wrapping_add(dimension),
        );
        2.0 * uniform_f64(seed) - 1.0
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
    let sample = |tag| 2.0 * context.uniform(tag) - 1.0;
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

fn replay_key(master_seed: u64, replay: ReplayKey) -> String {
    format!(
        "v1:{master_seed}:{}:{}:{}",
        replay.trial.candidate_id, replay.trial.replication_id, replay.common_random_group,
    )
}

fn hash_tag(tag: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for byte in tag.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

fn derive_seed(master_seed: u64, domain: u64, token: u64) -> u64 {
    mix_seed(master_seed ^ mix_seed(domain) ^ token.wrapping_mul(GOLDEN_RATIO))
}

fn mix_seed(mut value: u64) -> u64 {
    value = value.wrapping_add(GOLDEN_RATIO);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn uniform_f64(seed: u64) -> f64 {
    (mix_seed(seed) >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
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

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn common_random_group_keys_execution_noise_across_candidates_and_replications() {
        let config = crate::Cli::try_parse_from([
            "simul-three-cushion",
            "--fixture",
            "--mode",
            "sensitivity",
            "--seed",
            "918273",
            "--candidates",
            "1",
            "--replications",
            "1",
            "--heading-noise",
            "0.25",
            "--speed-noise",
            "1.5",
            "--tip-side-noise",
            "0.01",
            "--tip-height-noise",
            "0.015",
            "--elevation-noise",
            "0.5",
        ])
        .expect("fixed test arguments should parse")
        .into_config()
        .expect("fixed test configuration should validate");
        let controls = config.nominal;
        let applied = |candidate_id, replication_id, common_random_group| {
            let key = TrialKey {
                candidate_id,
                replication_id,
            };
            let spec = TrialSpec {
                candidate: Candidate {
                    id: candidate_id,
                    controls,
                },
                key,
                replay: ReplayKey {
                    trial: key,
                    common_random_group,
                },
            };
            failed_record(&config, &spec, "forced failure for noise-only test")
                .result
                .expect_err("failed_record should retain its failure")
                .applied
        };

        let first_candidate = applied(7, 11, 11);
        let distinct_candidate = applied(99, 11, 11);
        let next_replication = applied(7, 12, 12);

        assert_eq!(first_candidate, distinct_candidate);
        assert_ne!(first_candidate, next_replication);
    }
}
