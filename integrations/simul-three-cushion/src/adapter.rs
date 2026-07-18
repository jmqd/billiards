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
    replay: ReplayKey,
}

impl TrialSpec {
    fn new(candidate: Candidate, replication_id: u32) -> Self {
        Self {
            candidate,
            replay: ReplayKey {
                trial: TrialKey {
                    candidate_id: candidate.id,
                    replication_id,
                },
                common_random_group: u64::from(replication_id),
            },
        }
    }

    fn key(&self) -> TrialKey {
        self.replay.trial
    }
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

#[derive(Clone, Copy, Debug)]
struct PreparedTrial {
    key: TrialKey,
    replay: ReplayKey,
    applied: Controls,
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

impl PreparedTrial {
    fn record(self, result: Result<physics::Outcome, String>) -> TrialRecord {
        let result = result
            .map(|outcome| EvaluatedTrial {
                applied: self.applied,
                outcome,
            })
            .map_err(|detail| FailedTrial {
                applied: self.applied,
                detail,
            });
        TrialRecord {
            key: self.key,
            replay: self.replay,
            result,
        }
    }
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

struct CandidateAccumulator {
    candidate: Candidate,
    requested: u32,
    scored: u32,
    missed: u32,
    indeterminate: u32,
    failed: u32,
    reducer: BernoulliReducer,
}

impl CandidateAccumulator {
    fn new(candidate: Candidate, requested: u32) -> Self {
        Self {
            candidate,
            requested,
            scored: 0,
            missed: 0,
            indeterminate: 0,
            failed: 0,
            reducer: BernoulliReducer::default(),
        }
    }

    fn record(
        &mut self,
        result: &Result<EvaluatedTrial, FailedTrial>,
    ) -> (Controls, TrialDisposition) {
        match result {
            Ok(evaluated) => match &evaluated.outcome {
                physics::Outcome::Scored => {
                    self.scored += 1;
                    self.reducer.observe(true);
                    (evaluated.applied, TrialDisposition::Scored)
                }
                physics::Outcome::Miss(detail) => {
                    self.missed += 1;
                    self.reducer.observe(false);
                    (evaluated.applied, TrialDisposition::Miss(detail.clone()))
                }
                physics::Outcome::Indeterminate(detail) => {
                    self.indeterminate += 1;
                    (
                        evaluated.applied,
                        TrialDisposition::Indeterminate(detail.clone()),
                    )
                }
            },
            Err(error) => {
                self.failed += 1;
                (
                    error.applied,
                    TrialDisposition::Failed(error.detail.clone()),
                )
            }
        }
    }

    fn finish(self) -> CandidateReport {
        let observed = self.scored + self.missed + self.indeterminate + self.failed;
        assert_eq!(
            observed, self.requested,
            "candidate disposition count must equal requested trial count"
        );
        let (success_rate, confidence_low, confidence_high) = match self.reducer.finish() {
            Some(statistics) => (
                Some(statistics.rate),
                Some(statistics.wilson_lower),
                Some(statistics.wilson_upper),
            ),
            None => (None, None, None),
        };
        CandidateReport {
            rank: None,
            candidate_id: self.candidate.id,
            controls: self.candidate.controls,
            requested: self.requested,
            scored: self.scored,
            missed: self.missed,
            indeterminate: self.indeterminate,
            failed: self.failed,
            success_rate,
            confidence_low,
            confidence_high,
            eligible: self.indeterminate == 0 && self.failed == 0,
        }
    }
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
            specs.push(TrialSpec::new(*candidate, replication_id));
        }
    }

    let records = run_trials(config, specs)?;
    let mut candidate_reports = Vec::with_capacity(candidates.len());
    let mut trial_reports = Vec::with_capacity(records.len());
    for (candidate_index, candidate) in candidates.iter().enumerate() {
        let start = candidate_index * records_per_candidate;
        let candidate_records = &records[start..start + records_per_candidate];
        let mut accumulator = CandidateAccumulator::new(*candidate, config.replication_budget);
        for record in candidate_records {
            let (applied, disposition) = accumulator.record(&record.result);
            trial_reports.push(TrialReport {
                candidate_id: record.key.candidate_id,
                replication_id: record.key.replication_id,
                replay_key: replay_key(config.master_seed, record.replay),
                applied,
                disposition,
            });
        }
        candidate_reports.push(accumulator.finish());
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

fn bounded_worker_count(requested: usize, specs_len: usize, available: usize) -> usize {
    requested.min(specs_len).min(available.max(1))
}

fn run_trials(
    config: &ExperimentConfig,
    specs: Vec<TrialSpec>,
) -> Result<Vec<TrialRecord>, String> {
    if specs.is_empty() {
        return Ok(Vec::new());
    }
    let available = std::thread::available_parallelism().map_or(1, |count| count.get());
    let worker_count = bounded_worker_count(config.workers.get(), specs.len(), available);
    if worker_count == 1 {
        let evaluator = physics::Evaluator::new(config)?;
        return Ok(specs
            .iter()
            .map(|spec| execute_trial(&evaluator, config, prepare_trial(config, spec)))
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
            handles.push(
                std::thread::Builder::new()
                    .spawn_scoped(scope, move || {
                        let evaluator = physics::Evaluator::new(config);
                        chunk
                            .iter()
                            .map(|spec| {
                                let prepared = prepare_trial(config, spec);
                                match &evaluator {
                                    Ok(evaluator) => execute_trial(evaluator, config, prepared),
                                    Err(detail) => prepared.record(Err(detail.to_string())),
                                }
                            })
                            .collect::<Vec<_>>()
                    })
                    .map_err(|error| format!("failed to spawn parallel trial worker: {error}"))?,
            );
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

fn prepare_trial(config: &ExperimentConfig, spec: &TrialSpec) -> PreparedTrial {
    let context = TrialContext {
        master_seed: config.master_seed,
        common_random_group: spec.replay.common_random_group,
    };
    PreparedTrial {
        key: spec.key(),
        replay: spec.replay,
        applied: apply_execution_noise(spec.candidate.controls, config, &context),
    }
}

fn execute_trial(
    evaluator: &physics::Evaluator,
    config: &ExperimentConfig,
    prepared: PreparedTrial,
) -> TrialRecord {
    let result = evaluate_trial(evaluator, config, &prepared);
    prepared.record(result)
}

fn evaluate_trial(
    evaluator: &physics::Evaluator,
    config: &ExperimentConfig,
    prepared: &PreparedTrial,
) -> Result<physics::Outcome, String> {
    evaluator.evaluate(config.shooter, prepared.applied, config.max_events)
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
        let applied = |candidate_id, replication_id| {
            let spec = TrialSpec::new(
                Candidate {
                    id: candidate_id,
                    controls,
                },
                replication_id,
            );
            prepare_trial(&config, &spec).applied
        };

        let first_candidate = applied(u64::MAX, u32::MAX);
        let distinct_candidate = applied(0, u32::MAX);
        let next_replication = applied(u64::MAX, u32::MAX - 1);

        assert_eq!(first_candidate, distinct_candidate);
        assert_ne!(first_candidate, next_replication);
    }

    #[test]
    fn normal_and_evaluator_construction_failure_share_prepared_trial() {
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
            "--max-events",
            "1",
        ])
        .expect("fixed test arguments should parse")
        .into_config()
        .expect("fixed test configuration should validate");
        let spec = TrialSpec::new(
            Candidate {
                id: 41,
                controls: Controls {
                    heading: 196.391_792_039,
                    speed: 237.947_968_822,
                    tip_side: -0.230_661_681,
                    tip_height: 0.365_060_077,
                    elevation: 5.0,
                },
            },
            29,
        );
        let evaluator = physics::Evaluator::new(&config).expect("fixture evaluator should build");

        let prepared = prepare_trial(&config, &spec);
        let executed = execute_trial(&evaluator, &config, prepared);
        let failed = prepared.record(Err("forced evaluator construction failure".to_string()));
        let executed_applied = match &executed.result {
            Ok(evaluated) => evaluated.applied,
            Err(error) => panic!("ordinary evaluation unexpectedly failed: {}", error.detail),
        };
        let failed_error = failed
            .result
            .as_ref()
            .expect_err("forced evaluator construction failure should remain failed");

        assert_eq!(
            (
                executed.key.candidate_id,
                executed.key.replication_id,
                executed.replay.trial.candidate_id,
                executed.replay.trial.replication_id,
                executed.replay.common_random_group,
                replay_key(config.master_seed, executed.replay),
            ),
            (
                failed.key.candidate_id,
                failed.key.replication_id,
                failed.replay.trial.candidate_id,
                failed.replay.trial.replication_id,
                failed.replay.common_random_group,
                replay_key(config.master_seed, failed.replay),
            )
        );
        assert_eq!(
            replay_key(config.master_seed, failed.replay),
            "v1:918273:41:29:29"
        );
        assert_eq!(failed_error.detail, "forced evaluator construction failure");
        assert_eq!(executed_applied, failed_error.applied);
        assert_eq!(
            executed_applied,
            Controls {
                heading: 196.473_634_909_604_16,
                speed: 238.114_336_421_040_92,
                tip_side: -0.232_619_970_111_715_95,
                tip_height: 0.354_032_810_918_413_65,
                elevation: 5.250_516_891_614_714,
            }
        );
    }

    #[test]
    fn candidate_accumulator_preserves_four_way_accounting_and_bernoulli_scope() {
        let candidate = Candidate {
            id: 17,
            controls: Controls {
                heading: 20.0,
                speed: 150.0,
                tip_side: 0.1,
                tip_height: 0.2,
                elevation: 5.0,
            },
        };
        let scored_applied = Controls {
            heading: 21.0,
            ..candidate.controls
        };
        let missed_applied = Controls {
            heading: 22.0,
            ..candidate.controls
        };
        let indeterminate_applied = Controls {
            heading: 23.0,
            ..candidate.controls
        };
        let failed_applied = Controls {
            heading: 24.0,
            ..candidate.controls
        };
        let cases = [
            (
                "scored",
                Ok(EvaluatedTrial {
                    applied: scored_applied,
                    outcome: physics::Outcome::Scored,
                }),
                scored_applied,
                TrialDisposition::Scored,
            ),
            (
                "miss",
                Ok(EvaluatedTrial {
                    applied: missed_applied,
                    outcome: physics::Outcome::Miss("object ball first".into()),
                }),
                missed_applied,
                TrialDisposition::Miss("object ball first".into()),
            ),
            (
                "indeterminate",
                Ok(EvaluatedTrial {
                    applied: indeterminate_applied,
                    outcome: physics::Outcome::Indeterminate("event limit".into()),
                }),
                indeterminate_applied,
                TrialDisposition::Indeterminate("event limit".into()),
            ),
            (
                "failed",
                Err(FailedTrial {
                    applied: failed_applied,
                    detail: "invalid noisy controls".into(),
                }),
                failed_applied,
                TrialDisposition::Failed("invalid noisy controls".into()),
            ),
        ];
        let mut accumulator = CandidateAccumulator::new(candidate, cases.len() as u32);
        for (name, result, expected_applied, expected_disposition) in cases {
            assert_eq!(
                accumulator.record(&result),
                (expected_applied, expected_disposition),
                "{name}"
            );
        }

        let mut expected_reducer = BernoulliReducer::default();
        expected_reducer.observe(true);
        expected_reducer.observe(false);
        let expected_statistics = expected_reducer
            .finish()
            .expect("scored and missed trials are observations");
        let report = accumulator.finish();
        assert_eq!(
            (report.candidate_id, report.controls),
            (candidate.id, candidate.controls)
        );
        assert_eq!(
            (
                report.requested,
                report.scored,
                report.missed,
                report.indeterminate,
                report.failed,
            ),
            (4, 1, 1, 1, 1)
        );
        assert_eq!(
            report.scored + report.missed + report.indeterminate + report.failed,
            report.requested
        );
        assert_eq!(
            (
                report.success_rate,
                report.confidence_low,
                report.confidence_high,
            ),
            (
                Some(expected_statistics.rate),
                Some(expected_statistics.wilson_lower),
                Some(expected_statistics.wilson_upper),
            )
        );
        assert!(!report.eligible);

        let eligibility_cases = [
            (
                "observed outcomes only",
                vec![
                    Ok(EvaluatedTrial {
                        applied: scored_applied,
                        outcome: physics::Outcome::Scored,
                    }),
                    Ok(EvaluatedTrial {
                        applied: missed_applied,
                        outcome: physics::Outcome::Miss("miss".into()),
                    }),
                ],
                true,
            ),
            (
                "indeterminate outcome",
                vec![Ok(EvaluatedTrial {
                    applied: indeterminate_applied,
                    outcome: physics::Outcome::Indeterminate("event limit".into()),
                })],
                false,
            ),
            (
                "failed outcome",
                vec![Err(FailedTrial {
                    applied: failed_applied,
                    detail: "invalid noisy controls".into(),
                })],
                false,
            ),
        ];
        for (name, results, expected_eligible) in eligibility_cases {
            let mut accumulator = CandidateAccumulator::new(candidate, results.len() as u32);
            for result in &results {
                accumulator.record(result);
            }
            assert_eq!(accumulator.finish().eligible, expected_eligible, "{name}");
        }
    }

    #[test]
    fn bounded_worker_count_respects_all_limits() {
        let cases = [
            (
                "huge request capped by availability",
                usize::MAX,
                1_024,
                8,
                8,
            ),
            ("request capped by fewer specs", 12, 3, 8, 3),
            ("ordinary request below both limits", 4, 12, 8, 4),
            ("zero availability normalized to one", 8, 12, 0, 1),
        ];

        for (name, requested, specs_len, available, expected) in cases {
            assert_eq!(
                bounded_worker_count(requested, specs_len, available),
                expected,
                "{name}"
            );
        }
    }
}
