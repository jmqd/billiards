use std::cmp::Ordering;
use std::error::Error;
use std::fmt;
use std::num::{NonZeroU32, NonZeroUsize};
use std::str::FromStr;

use simul::experiment::{
    run_replicated, Candidate, ReplicationPlan, SampleContext, SampleStream, SamplingError,
    TrialContext, TrialError, TrialRecord, SEED_PROTOCOL,
};
pub use simul::experiment::{RandomDomain, ReplayKey};

use super::{
    execute_three_cushion_compact, CueStrikeConfig, PhysicsProfile, ShotControls, ShotLayout,
    ShotLimit, ThreeCushionAdjudication, ThreeCushionShooter, ThreeCushionShot,
};

pub const ROBUST_SEED_PROTOCOL: &str = SEED_PROTOCOL;

pub const ROBUST_SEARCH_PROPOSAL_DOMAIN: RandomDomain = RandomDomain::new(0x5345_4152_4348_0001);
pub const ROBUST_SEARCH_SCREENING_DOMAIN: RandomDomain = RandomDomain::new(0x5345_4152_4348_0002);
pub const ROBUST_SEARCH_VALIDATION_DOMAIN: RandomDomain = RandomDomain::new(0x5345_4152_4348_0003);
pub const ROBUST_SENSITIVITY_PROPOSAL_DOMAIN: RandomDomain =
    RandomDomain::new(0x5345_4e53_4954_0001);
pub const ROBUST_SENSITIVITY_TRIAL_DOMAIN: RandomDomain = RandomDomain::new(0x5345_4e53_4954_0002);

const HEADING_STREAM: SampleStream = SampleStream::new(0x4845_4144_494e_4701);
const SPEED_STREAM: SampleStream = SampleStream::new(0x5350_4545_4400_0001);
const TIP_SIDE_STREAM: SampleStream = SampleStream::new(0x5349_4445_0000_0001);
const TIP_HEIGHT_STREAM: SampleStream = SampleStream::new(0x4845_4947_4854_0001);
const ELEVATION_STREAM: SampleStream = SampleStream::new(0x454c_4556_4154_0001);

pub const ROBUST_NOISE_TRUNCATION_STANDARD_DEVIATIONS: f64 = 3.0;
pub const ROBUST_MAX_CANONICAL_TIP_OFFSET: f64 = 0.5;
pub const ROBUST_TIP_FEASIBILITY_MARGIN: f64 = 1e-12;
pub const ROBUST_MAX_CUE_ELEVATION_DEGREES: f64 = 85.0;
pub const ROBUST_ELEVATION_FEASIBILITY_MARGIN_DEGREES: f64 = 1e-12;
pub const ROBUST_MAX_PROPOSAL_ATTEMPTS_PER_CANDIDATE: u64 = 4_096;

pub const MIN_ROBUST_SEARCH_EVALUATIONS: u32 = 16;
pub const MAX_ROBUST_SEARCH_EVALUATIONS: u32 = 10_000;
pub const DEFAULT_ROBUST_SEARCH_SEED: u64 = 0x524f_4255_5354_0001;
const MAX_PLAYER_SEARCH_CANDIDATES: usize = 128;
const PLAYER_SEARCH_FINALISTS: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RobustExperimentMode {
    Sensitivity,
    Search,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RobustShotControls {
    pub heading: f64,
    pub speed: f64,
    pub tip_side: f64,
    pub tip_height: f64,
    pub elevation: f64,
}

impl RobustShotControls {
    pub const fn as_array(self) -> [f64; 5] {
        [
            self.heading,
            self.speed,
            self.tip_side,
            self.tip_height,
            self.elevation,
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RobustPerturbationWidths {
    pub heading: f64,
    pub speed: f64,
    pub tip_side: f64,
    pub tip_height: f64,
    pub elevation: f64,
}

impl RobustPerturbationWidths {
    pub const fn as_array(self) -> [f64; 5] {
        [
            self.heading,
            self.speed,
            self.tip_side,
            self.tip_height,
            self.elevation,
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RobustNoiseSigmas {
    pub heading: f64,
    pub speed: f64,
    pub tip_side: f64,
    pub tip_height: f64,
    pub elevation: f64,
}

impl RobustNoiseSigmas {
    pub const fn as_array(self) -> [f64; 5] {
        [
            self.heading,
            self.speed,
            self.tip_side,
            self.tip_height,
            self.elevation,
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RobustControlBounds {
    pub minimum: f64,
    pub maximum: f64,
}

impl RobustControlBounds {
    pub const fn new(minimum: f64, maximum: f64) -> Self {
        Self { minimum, maximum }
    }

    pub fn contains(self, value: f64) -> bool {
        value >= self.minimum && value <= self.maximum
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RobustThreeCushionExperimentConfig {
    pub mode: RobustExperimentMode,
    pub physics: PhysicsProfile,
    pub layout: ShotLayout,
    pub cue: CueStrikeConfig,
    pub shooter: ThreeCushionShooter,
    pub nominal: RobustShotControls,
    pub additional_candidates: Vec<RobustShotControls>,
    pub perturbations: RobustPerturbationWidths,
    pub shot_inaccuracy: RobustNoiseSigmas,
    pub search_bounds: [RobustControlBounds; 5],
    pub master_seed: u64,
    pub candidate_budget: usize,
    pub screening_replication_budget: u32,
    pub finalist_budget: usize,
    pub validation_replication_budget: u32,
    pub workers: NonZeroUsize,
    pub max_events: usize,
}

impl RobustThreeCushionExperimentConfig {
    pub fn validate(&self) -> Result<(), RobustExperimentError> {
        validate_budget(self)?;
        validate_robust_perturbations(self.perturbations)
            .map_err(|error| invalid_configuration(error.to_string()))?;
        validate_robust_noise_sigmas(self.shot_inaccuracy)
            .map_err(|error| invalid_configuration(error.to_string()))?;
        validate_robust_search_bounds(self.search_bounds)
            .map_err(|error| invalid_configuration(error.to_string()))?;
        validate_configured_candidate(
            "nominal controls",
            self.nominal,
            self.mode,
            self.search_bounds,
            self.shot_inaccuracy,
        )?;
        for (index, controls) in self.additional_candidates.iter().copied().enumerate() {
            validate_configured_candidate(
                &format!("additional candidate {index}"),
                controls,
                self.mode,
                self.search_bounds,
                self.shot_inaccuracy,
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RobustTrialStage {
    Screening,
    Validation,
}

impl fmt::Display for RobustTrialStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Screening => "screening",
            Self::Validation => "validation",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RobustTrialDisposition {
    Scored,
    Miss(String),
    Indeterminate(String),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RobustOutcomeSummary {
    pub requested: u32,
    pub scored: u32,
    pub missed: u32,
    pub indeterminate: u32,
    pub failed: u32,
    pub success_rate: Option<f64>,
    pub confidence_low: Option<f64>,
    pub confidence_high: Option<f64>,
    pub eligible: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RobustTrialReport {
    pub stage: RobustTrialStage,
    pub candidate_id: u64,
    pub replication_id: u32,
    pub replay_key: ReplayKey,
    pub applied: Option<RobustShotControls>,
    pub disposition: RobustTrialDisposition,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RobustCandidateReport {
    pub rank: Option<usize>,
    pub candidate_id: u64,
    pub controls: RobustShotControls,
    pub screening: RobustOutcomeSummary,
    pub validation: Option<RobustOutcomeSummary>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RobustThreeCushionExperimentReport {
    pub candidates: Vec<RobustCandidateReport>,
    pub trials: Vec<RobustTrialReport>,
    pub winner_id: Option<u64>,
}

impl RobustThreeCushionExperimentReport {
    pub fn winner(&self) -> Option<&RobustCandidateReport> {
        self.candidates
            .iter()
            .find(|candidate| candidate.rank == Some(1))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThreeCushionPlayerLevel {
    B,
    A,
    Pro,
}

impl ThreeCushionPlayerLevel {
    pub const fn key(self) -> &'static str {
        match self {
            Self::B => "b",
            Self::A => "a",
            Self::Pro => "pro",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::B => "B player",
            Self::A => "A player",
            Self::Pro => "Pro player",
        }
    }

    pub const fn shot_inaccuracy(self) -> RobustNoiseSigmas {
        match self {
            Self::B => RobustNoiseSigmas {
                heading: 1.2,
                speed: 7.0,
                tip_side: 0.035,
                tip_height: 0.035,
                elevation: 0.75,
            },
            Self::A => RobustNoiseSigmas {
                heading: 0.6,
                speed: 3.5,
                tip_side: 0.018,
                tip_height: 0.018,
                elevation: 0.35,
            },
            Self::Pro => RobustNoiseSigmas {
                heading: 0.25,
                speed: 1.5,
                tip_side: 0.008,
                tip_height: 0.008,
                elevation: 0.15,
            },
        }
    }
}

impl fmt::Display for ThreeCushionPlayerLevel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.key())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsePlayerLevelError(String);

impl fmt::Display for ParsePlayerLevelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unknown player level '{}'; expected b, a, or pro",
            self.0
        )
    }
}

impl Error for ParsePlayerLevelError {}

impl FromStr for ThreeCushionPlayerLevel {
    type Err = ParsePlayerLevelError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "b" | "b-player" | "b player" => Ok(Self::B),
            "a" | "a-player" | "a player" => Ok(Self::A),
            "pro" | "pro-player" | "pro player" => Ok(Self::Pro),
            _ => Err(ParsePlayerLevelError(value.to_string())),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RobustSearchBudget {
    pub requested_evaluations: u32,
    pub evaluation_cap: u32,
    pub candidate_budget: usize,
    pub screening_replications: u32,
    pub finalist_budget: usize,
    pub validation_replications: u32,
    pub planned_evaluations: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerRobustSearchRequest {
    pub physics: PhysicsProfile,
    pub layout: ShotLayout,
    pub cue: CueStrikeConfig,
    pub shooter: ThreeCushionShooter,
    pub current_controls: RobustShotControls,
    pub requested_evaluations: u32,
    pub player_level: ThreeCushionPlayerLevel,
    pub max_events: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerRobustSearchReport {
    pub player_level: ThreeCushionPlayerLevel,
    pub shot_inaccuracy: RobustNoiseSigmas,
    pub budget: RobustSearchBudget,
    pub search_seed_controls: RobustShotControls,
    pub master_seed: u64,
    pub experiment: RobustThreeCushionExperimentReport,
}

impl PlayerRobustSearchReport {
    pub fn actual_evaluations(&self) -> usize {
        self.experiment.trials.len()
    }

    pub fn winner(&self) -> Option<&RobustCandidateReport> {
        self.experiment.winner()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RobustControlValidationError(String);

impl fmt::Display for RobustControlValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for RobustControlValidationError {}

#[derive(Debug)]
pub enum RobustExperimentError {
    InvalidConfiguration(String),
    CandidateGeneration(String),
    Replication(String),
    ReportAssembly(String),
}

impl fmt::Display for RobustExperimentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(detail) => write!(formatter, "invalid experiment: {detail}"),
            Self::CandidateGeneration(detail) => {
                write!(formatter, "candidate generation failed: {detail}")
            }
            Self::Replication(detail) => write!(formatter, "replication failed: {detail}"),
            Self::ReportAssembly(detail) => write!(formatter, "report assembly failed: {detail}"),
        }
    }
}

impl Error for RobustExperimentError {}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PreparedTrial {
    applied: RobustShotControls,
}

#[derive(Clone, Debug)]
struct EvaluatedTrial {
    applied: RobustShotControls,
    outcome: EvaluatedOutcome,
}

#[derive(Clone, Debug)]
struct FailedTrial {
    applied: RobustShotControls,
    detail: String,
}

#[derive(Clone, Debug)]
enum EvaluatedOutcome {
    Scored,
    Miss(String),
    Indeterminate(String),
}

type StageTrialRecord = TrialRecord<EvaluatedTrial, SamplingError, FailedTrial>;

struct Evaluator {
    physics: PhysicsProfile,
    layout: ShotLayout,
    cue: CueStrikeConfig,
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

struct OutcomeAccumulator {
    requested: u32,
    scored: u32,
    missed: u32,
    indeterminate: u32,
    failed: u32,
    reducer: BernoulliReducer,
}

struct StageResults {
    summaries: Vec<RobustOutcomeSummary>,
    trials: Vec<RobustTrialReport>,
}

impl Evaluator {
    fn evaluate(
        &self,
        shooter: ThreeCushionShooter,
        controls: RobustShotControls,
        max_events: usize,
    ) -> Result<EvaluatedOutcome, String> {
        let controls = ShotControls::new(
            controls.heading,
            controls.speed,
            controls.tip_side,
            controls.tip_height,
            controls.elevation,
        )
        .map_err(|error| format!("invalid noisy shot controls: {error}"))?;
        let shot = ThreeCushionShot::new(shooter, controls).with_cue_config(self.cue.clone());
        let result = execute_three_cushion_compact(
            &self.physics,
            &self.layout,
            &shot,
            ShotLimit::EventCount(max_events),
        )
        .map_err(|error| format!("shot execution failed: {error}"))?;
        Ok(match result.completion.summary {
            ThreeCushionAdjudication::Scored(_) => EvaluatedOutcome::Scored,
            ThreeCushionAdjudication::Miss { reason, .. } => {
                EvaluatedOutcome::Miss(format!("{reason:?}"))
            }
            ThreeCushionAdjudication::Indeterminate { reason, .. } => {
                EvaluatedOutcome::Indeterminate(format!("{reason:?}"))
            }
        })
    }
}

impl OutcomeAccumulator {
    const fn new(requested: u32) -> Self {
        Self {
            requested,
            scored: 0,
            missed: 0,
            indeterminate: 0,
            failed: 0,
            reducer: BernoulliReducer {
                successes: 0,
                observations: 0,
            },
        }
    }

    fn record(&mut self, stage: RobustTrialStage, record: StageTrialRecord) -> RobustTrialReport {
        let (applied, disposition) = match record.result {
            Ok(evaluated) => match evaluated.outcome {
                EvaluatedOutcome::Scored => {
                    self.scored += 1;
                    self.reducer.observe(true);
                    (Some(evaluated.applied), RobustTrialDisposition::Scored)
                }
                EvaluatedOutcome::Miss(detail) => {
                    self.missed += 1;
                    self.reducer.observe(false);
                    (
                        Some(evaluated.applied),
                        RobustTrialDisposition::Miss(detail),
                    )
                }
                EvaluatedOutcome::Indeterminate(detail) => {
                    self.indeterminate += 1;
                    (
                        Some(evaluated.applied),
                        RobustTrialDisposition::Indeterminate(detail),
                    )
                }
            },
            Err(TrialError::Prepare(error)) => {
                self.failed += 1;
                (
                    None,
                    RobustTrialDisposition::Failed(format!(
                        "shot-inaccuracy sampling failed: {error}"
                    )),
                )
            }
            Err(TrialError::Evaluate(error)) => {
                self.failed += 1;
                (
                    Some(error.applied),
                    RobustTrialDisposition::Failed(error.detail),
                )
            }
        };
        RobustTrialReport {
            stage,
            candidate_id: record.key.candidate_id,
            replication_id: record.key.replication_id,
            replay_key: record.replay_key,
            applied,
            disposition,
        }
    }

    fn finish(self) -> Result<RobustOutcomeSummary, RobustExperimentError> {
        let observed = self.scored + self.missed + self.indeterminate + self.failed;
        if observed != self.requested {
            return Err(RobustExperimentError::ReportAssembly(format!(
                "counted {observed} dispositions for {} requested trials",
                self.requested
            )));
        }
        let (success_rate, confidence_low, confidence_high) = match self.reducer.finish() {
            Some(statistics) => (
                Some(statistics.rate),
                Some(statistics.wilson_lower),
                Some(statistics.wilson_upper),
            ),
            None => (None, None, None),
        };
        Ok(RobustOutcomeSummary {
            requested: self.requested,
            scored: self.scored,
            missed: self.missed,
            indeterminate: self.indeterminate,
            failed: self.failed,
            success_rate,
            confidence_low,
            confidence_high,
            eligible: self.indeterminate == 0 && self.failed == 0,
        })
    }
}

impl BernoulliReducer {
    fn observe(&mut self, success: bool) {
        self.observations += 1;
        if success {
            self.successes += 1;
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
        let margin = z * (rate * (1.0 - rate) / n + z_squared / (4.0 * n * n)).sqrt() / denominator;
        Some(BernoulliStatistics {
            rate,
            wilson_lower: (center - margin).max(0.0),
            wilson_upper: (center + margin).min(1.0),
        })
    }
}

pub fn allocate_robust_search_budget(
    requested_evaluations: u32,
) -> Result<RobustSearchBudget, RobustExperimentError> {
    if !(MIN_ROBUST_SEARCH_EVALUATIONS..=MAX_ROBUST_SEARCH_EVALUATIONS)
        .contains(&requested_evaluations)
    {
        return Err(RobustExperimentError::InvalidConfiguration(format!(
            "iterations must be in [{MIN_ROBUST_SEARCH_EVALUATIONS}, {MAX_ROBUST_SEARCH_EVALUATIONS}]"
        )));
    }

    let candidate_budget = usize::try_from(requested_evaluations.isqrt())
        .map_err(|_| {
            RobustExperimentError::InvalidConfiguration(
                "iteration square root does not fit platform size".to_string(),
            )
        })?
        .min(MAX_PLAYER_SEARCH_CANDIDATES);
    let finalist_budget = candidate_budget.min(PLAYER_SEARCH_FINALISTS);
    let candidate_count = u32::try_from(candidate_budget).map_err(|_| {
        RobustExperimentError::InvalidConfiguration(
            "candidate budget does not fit evaluation counter".to_string(),
        )
    })?;
    let finalist_count = u32::try_from(finalist_budget).map_err(|_| {
        RobustExperimentError::InvalidConfiguration(
            "finalist budget does not fit evaluation counter".to_string(),
        )
    })?;
    let screening_replications = requested_evaluations
        .div_euclid(2)
        .div_euclid(candidate_count)
        .max(1);
    let screening_cost = candidate_count * screening_replications;
    let validation_replications = (requested_evaluations - screening_cost)
        .div_euclid(finalist_count)
        .max(1);
    let planned_evaluations = screening_cost + finalist_count * validation_replications;
    if planned_evaluations > requested_evaluations {
        return Err(RobustExperimentError::InvalidConfiguration(
            "iteration allocation exceeded requested maximum".to_string(),
        ));
    }

    Ok(RobustSearchBudget {
        requested_evaluations,
        evaluation_cap: MAX_ROBUST_SEARCH_EVALUATIONS,
        candidate_budget,
        screening_replications,
        finalist_budget,
        validation_replications,
        planned_evaluations,
    })
}

pub fn run_player_robust_search(
    request: &PlayerRobustSearchRequest,
) -> Result<PlayerRobustSearchReport, RobustExperimentError> {
    let budget = allocate_robust_search_budget(request.requested_evaluations)?;
    let shot_inaccuracy = request.player_level.shot_inaccuracy();
    let search_bounds = player_search_bounds(shot_inaccuracy);
    let search_seed_controls =
        player_search_seed(request.current_controls, search_bounds, shot_inaccuracy);
    let experiment = run_robust_three_cushion_experiment(&RobustThreeCushionExperimentConfig {
        mode: RobustExperimentMode::Search,
        physics: request.physics.clone(),
        layout: request.layout.clone(),
        cue: request.cue.clone(),
        shooter: request.shooter,
        nominal: search_seed_controls,
        additional_candidates: Vec::new(),
        perturbations: RobustPerturbationWidths {
            heading: 0.0,
            speed: 0.0,
            tip_side: 0.0,
            tip_height: 0.0,
            elevation: 0.0,
        },
        shot_inaccuracy,
        search_bounds,
        master_seed: DEFAULT_ROBUST_SEARCH_SEED,
        candidate_budget: budget.candidate_budget,
        screening_replication_budget: budget.screening_replications,
        finalist_budget: budget.finalist_budget,
        validation_replication_budget: budget.validation_replications,
        workers: NonZeroUsize::MIN,
        max_events: request.max_events,
    })?;
    Ok(PlayerRobustSearchReport {
        player_level: request.player_level,
        shot_inaccuracy,
        budget,
        search_seed_controls,
        master_seed: DEFAULT_ROBUST_SEARCH_SEED,
        experiment,
    })
}

pub fn run_robust_three_cushion_experiment(
    config: &RobustThreeCushionExperimentConfig,
) -> Result<RobustThreeCushionExperimentReport, RobustExperimentError> {
    config.validate()?;
    let candidates = make_candidates(config)?;
    let screening_domain = match config.mode {
        RobustExperimentMode::Search => ROBUST_SEARCH_SCREENING_DOMAIN,
        RobustExperimentMode::Sensitivity => ROBUST_SENSITIVITY_TRIAL_DOMAIN,
    };
    let screening = run_stage(
        config,
        &candidates,
        RobustTrialStage::Screening,
        screening_domain,
        config.screening_replication_budget,
    )?;

    let mut candidate_reports = Vec::new();
    candidate_reports
        .try_reserve_exact(candidates.len())
        .map_err(|error| resource_error("candidate reports", error))?;
    for (candidate, summary) in candidates.iter().zip(screening.summaries) {
        candidate_reports.push(RobustCandidateReport {
            rank: None,
            candidate_id: candidate.id,
            controls: candidate.value,
            screening: summary,
            validation: None,
        });
    }

    let mut trials = screening.trials;
    if config.mode == RobustExperimentMode::Search {
        let finalists = select_finalists(&candidate_reports, &candidates, config.finalist_budget)?;
        if !finalists.is_empty() {
            let validation = run_stage(
                config,
                &finalists,
                RobustTrialStage::Validation,
                ROBUST_SEARCH_VALIDATION_DOMAIN,
                config.validation_replication_budget,
            )?;
            for (finalist, summary) in finalists.iter().zip(validation.summaries) {
                let index = candidate_reports
                    .binary_search_by_key(&finalist.id, |candidate| candidate.candidate_id)
                    .map_err(|_| {
                        RobustExperimentError::ReportAssembly(format!(
                            "could not find validation finalist {}",
                            finalist.id
                        ))
                    })?;
                candidate_reports[index].validation = Some(summary);
            }
            trials
                .try_reserve(validation.trials.len())
                .map_err(|error| resource_error("validation trial reports", error))?;
            trials.extend(validation.trials);
        }
        rank_validated_candidates(&mut candidate_reports)?;
        sort_candidate_rows(&mut candidate_reports);
    }

    let winner_id = candidate_reports
        .iter()
        .find(|candidate| candidate.rank == Some(1))
        .map(|candidate| candidate.candidate_id);
    Ok(RobustThreeCushionExperimentReport {
        candidates: candidate_reports,
        trials,
        winner_id,
    })
}

fn run_stage(
    config: &RobustThreeCushionExperimentConfig,
    candidates: &[Candidate<RobustShotControls>],
    stage: RobustTrialStage,
    random_domain: RandomDomain,
    replications: u32,
) -> Result<StageResults, RobustExperimentError> {
    let replications = NonZeroU32::new(replications).ok_or_else(|| {
        RobustExperimentError::InvalidConfiguration(format!(
            "{stage} replication budget must be greater than zero"
        ))
    })?;
    let records = run_replicated(
        candidates,
        ReplicationPlan {
            master_seed: config.master_seed,
            random_domain,
            replications,
            workers: config.workers,
        },
        |_| {
            Ok::<_, String>(Evaluator {
                physics: config.physics.clone(),
                layout: config.layout.clone(),
                cue: config.cue.clone(),
            })
        },
        |candidate, context| prepare_trial(*candidate, config.shot_inaccuracy, context),
        |evaluator, prepared| {
            evaluator
                .evaluate(config.shooter, prepared.applied, config.max_events)
                .map(|outcome| EvaluatedTrial {
                    applied: prepared.applied,
                    outcome,
                })
                .map_err(|detail| FailedTrial {
                    applied: prepared.applied,
                    detail,
                })
        },
    )
    .map_err(|error| RobustExperimentError::Replication(format!("{stage}: {error}")))?;
    assemble_stage(stage, candidates, replications.get(), records)
}

fn assemble_stage(
    stage: RobustTrialStage,
    candidates: &[Candidate<RobustShotControls>],
    replications: u32,
    records: Vec<StageTrialRecord>,
) -> Result<StageResults, RobustExperimentError> {
    let records_per_candidate = usize::try_from(replications).map_err(|_| {
        RobustExperimentError::ReportAssembly(format!(
            "{stage} replication budget does not fit platform size"
        ))
    })?;
    let expected_records = candidates
        .len()
        .checked_mul(records_per_candidate)
        .ok_or_else(|| {
            RobustExperimentError::ReportAssembly(format!(
                "{stage} candidate × replication count overflowed"
            ))
        })?;
    if records.len() != expected_records {
        return Err(RobustExperimentError::ReportAssembly(format!(
            "{stage} runner returned {} records; expected {expected_records}",
            records.len()
        )));
    }

    let mut summaries = Vec::new();
    summaries
        .try_reserve_exact(candidates.len())
        .map_err(|error| resource_error("stage summaries", error))?;
    let mut trials = Vec::new();
    trials
        .try_reserve_exact(records.len())
        .map_err(|error| resource_error("trial reports", error))?;
    let mut records = records.into_iter();
    for candidate in candidates {
        let mut accumulator = OutcomeAccumulator::new(replications);
        for replication_id in 0..replications {
            let record = records.next().ok_or_else(|| {
                RobustExperimentError::ReportAssembly(format!(
                    "{stage} ran out of records for candidate {}",
                    candidate.id
                ))
            })?;
            if record.key.candidate_id != candidate.id
                || record.key.replication_id != replication_id
            {
                return Err(RobustExperimentError::ReportAssembly(format!(
                    "{stage} record order mismatch for candidate {} replication {replication_id}",
                    candidate.id
                )));
            }
            trials.push(accumulator.record(stage, record));
        }
        summaries.push(accumulator.finish()?);
    }
    if records.next().is_some() {
        return Err(RobustExperimentError::ReportAssembly(format!(
            "{stage} left unconsumed records"
        )));
    }
    Ok(StageResults { summaries, trials })
}

fn prepare_trial(
    candidate: RobustShotControls,
    sigmas: RobustNoiseSigmas,
    context: TrialContext,
) -> Result<PreparedTrial, SamplingError> {
    let samples = context.samples();
    Ok(PreparedTrial {
        applied: RobustShotControls {
            heading: (candidate.heading + sample_delta(samples, HEADING_STREAM, sigmas.heading)?)
                .rem_euclid(360.0),
            speed: candidate.speed + sample_delta(samples, SPEED_STREAM, sigmas.speed)?,
            tip_side: candidate.tip_side + sample_delta(samples, TIP_SIDE_STREAM, sigmas.tip_side)?,
            tip_height: candidate.tip_height
                + sample_delta(samples, TIP_HEIGHT_STREAM, sigmas.tip_height)?,
            elevation: candidate.elevation
                + sample_delta(samples, ELEVATION_STREAM, sigmas.elevation)?,
        },
    })
}

fn sample_delta(
    samples: SampleContext,
    stream: SampleStream,
    sigma: f64,
) -> Result<f64, SamplingError> {
    if sigma == 0.0 {
        Ok(0.0)
    } else {
        samples
            .truncated_standard_normal(stream, ROBUST_NOISE_TRUNCATION_STANDARD_DEVIATIONS)
            .map(|standard_normal| sigma * standard_normal)
    }
}

fn make_candidates(
    config: &RobustThreeCushionExperimentConfig,
) -> Result<Vec<Candidate<RobustShotControls>>, RobustExperimentError> {
    let mut candidates = Vec::new();
    candidates
        .try_reserve_exact(config.candidate_budget)
        .map_err(|error| resource_error("candidate set", error))?;
    candidates.push(Candidate {
        id: 0,
        value: config.nominal,
    });
    for controls in &config.additional_candidates {
        let id = u64::try_from(candidates.len()).map_err(|_| {
            RobustExperimentError::CandidateGeneration("candidate ID does not fit u64".to_string())
        })?;
        candidates.push(Candidate {
            id,
            value: *controls,
        });
    }
    while candidates.len() < config.candidate_budget {
        let id = u64::try_from(candidates.len()).map_err(|_| {
            RobustExperimentError::CandidateGeneration("candidate ID does not fit u64".to_string())
        })?;
        candidates.push(Candidate {
            id,
            value: generate_candidate(config, id)?,
        });
    }
    Ok(candidates)
}

fn generate_candidate(
    config: &RobustThreeCushionExperimentConfig,
    candidate_id: u64,
) -> Result<RobustShotControls, RobustExperimentError> {
    let proposal_domain = match config.mode {
        RobustExperimentMode::Search => ROBUST_SEARCH_PROPOSAL_DOMAIN,
        RobustExperimentMode::Sensitivity => ROBUST_SENSITIVITY_PROPOSAL_DOMAIN,
    };
    let base_sample_id = candidate_id
        .checked_mul(ROBUST_MAX_PROPOSAL_ATTEMPTS_PER_CANDIDATE)
        .ok_or_else(|| {
            RobustExperimentError::CandidateGeneration(
                "candidate proposal sample ID overflowed u64".to_string(),
            )
        })?;
    for attempt in 0..ROBUST_MAX_PROPOSAL_ATTEMPTS_PER_CANDIDATE {
        let sample_id = base_sample_id.checked_add(attempt).ok_or_else(|| {
            RobustExperimentError::CandidateGeneration(
                "candidate proposal sample ID overflowed u64".to_string(),
            )
        })?;
        let samples = SampleContext::new(config.master_seed, proposal_domain, sample_id);
        let controls = match config.mode {
            RobustExperimentMode::Search => sample_search_candidate(config.search_bounds, samples),
            RobustExperimentMode::Sensitivity => sample_sensitivity_candidate(
                sensitivity_center(config, candidate_id)?,
                config.perturbations,
                samples,
            ),
        };
        if candidate_is_executable(config, controls) {
            return Ok(controls);
        }
    }
    Err(RobustExperimentError::CandidateGeneration(format!(
        "could not generate candidate {candidate_id} with an executable ±3σ envelope after 4096 attempts; tighten bounds/perturbations or shot-inaccuracy sigmas"
    )))
}

fn sensitivity_center(
    config: &RobustThreeCushionExperimentConfig,
    candidate_id: u64,
) -> Result<RobustShotControls, RobustExperimentError> {
    let center_count = config
        .additional_candidates
        .len()
        .checked_add(1)
        .ok_or_else(|| {
            RobustExperimentError::CandidateGeneration(
                "sensitivity center count overflowed platform size".to_string(),
            )
        })?;
    let candidate_index = usize::try_from(candidate_id).map_err(|_| {
        RobustExperimentError::CandidateGeneration(
            "candidate ID does not fit platform size".to_string(),
        )
    })?;
    let center_index = candidate_index % center_count;
    if center_index == 0 {
        Ok(config.nominal)
    } else {
        Ok(config.additional_candidates[center_index - 1])
    }
}

fn sample_search_candidate(
    bounds: [RobustControlBounds; 5],
    samples: SampleContext,
) -> RobustShotControls {
    RobustShotControls {
        heading: interpolate(bounds[0], samples.uniform(HEADING_STREAM)),
        speed: interpolate(bounds[1], samples.uniform(SPEED_STREAM)),
        tip_side: interpolate(bounds[2], samples.uniform(TIP_SIDE_STREAM)),
        tip_height: interpolate(bounds[3], samples.uniform(TIP_HEIGHT_STREAM)),
        elevation: interpolate(bounds[4], samples.uniform(ELEVATION_STREAM)),
    }
}

fn sample_sensitivity_candidate(
    center: RobustShotControls,
    widths: RobustPerturbationWidths,
    samples: SampleContext,
) -> RobustShotControls {
    RobustShotControls {
        heading: center.heading + widths.heading * symmetric_uniform(samples, HEADING_STREAM),
        speed: center.speed + widths.speed * symmetric_uniform(samples, SPEED_STREAM),
        tip_side: center.tip_side + widths.tip_side * symmetric_uniform(samples, TIP_SIDE_STREAM),
        tip_height: center.tip_height
            + widths.tip_height * symmetric_uniform(samples, TIP_HEIGHT_STREAM),
        elevation: center.elevation
            + widths.elevation * symmetric_uniform(samples, ELEVATION_STREAM),
    }
}

fn symmetric_uniform(samples: SampleContext, stream: SampleStream) -> f64 {
    2.0 * samples.uniform(stream) - 1.0
}

fn interpolate(bounds: RobustControlBounds, sample: f64) -> f64 {
    if bounds.minimum == bounds.maximum {
        bounds.minimum
    } else {
        bounds.minimum + (bounds.maximum - bounds.minimum) * sample
    }
}

fn candidate_is_executable(
    config: &RobustThreeCushionExperimentConfig,
    controls: RobustShotControls,
) -> bool {
    validate_robust_controls(controls).is_ok()
        && (config.mode != RobustExperimentMode::Search
            || robust_controls_within_bounds(controls, config.search_bounds))
        && validate_robust_noise_envelope(controls, config.shot_inaccuracy).is_ok()
}

fn select_finalists(
    reports: &[RobustCandidateReport],
    candidates: &[Candidate<RobustShotControls>],
    finalist_budget: usize,
) -> Result<Vec<Candidate<RobustShotControls>>, RobustExperimentError> {
    let mut eligible_indices = Vec::new();
    eligible_indices
        .try_reserve_exact(reports.len())
        .map_err(|error| resource_error("screening finalist indices", error))?;
    eligible_indices.extend(
        reports
            .iter()
            .enumerate()
            .filter_map(|(index, report)| report.screening.eligible.then_some(index)),
    );
    eligible_indices.sort_by(|left, right| {
        compare_summary(
            reports[*left].candidate_id,
            &reports[*left].screening,
            reports[*right].candidate_id,
            &reports[*right].screening,
        )
    });
    eligible_indices.truncate(finalist_budget.min(eligible_indices.len()));

    let mut finalists = Vec::new();
    finalists
        .try_reserve_exact(eligible_indices.len())
        .map_err(|error| resource_error("validation finalists", error))?;
    for index in eligible_indices {
        let report = &reports[index];
        let candidate = candidates
            .binary_search_by_key(&report.candidate_id, |candidate| candidate.id)
            .map(|candidate_index| candidates[candidate_index])
            .map_err(|_| {
                RobustExperimentError::ReportAssembly(format!(
                    "screening finalist {} was absent from candidate set",
                    report.candidate_id
                ))
            })?;
        finalists.push(candidate);
    }
    finalists.sort_by_key(|candidate| candidate.id);
    Ok(finalists)
}

fn rank_validated_candidates(
    reports: &mut [RobustCandidateReport],
) -> Result<(), RobustExperimentError> {
    let mut eligible_indices = Vec::new();
    eligible_indices
        .try_reserve_exact(reports.len())
        .map_err(|error| resource_error("validation rank indices", error))?;
    eligible_indices.extend(reports.iter().enumerate().filter_map(|(index, report)| {
        report
            .validation
            .as_ref()
            .is_some_and(|summary| summary.eligible)
            .then_some(index)
    }));
    eligible_indices.sort_by(|left, right| {
        let left_report = &reports[*left];
        let right_report = &reports[*right];
        match (&left_report.validation, &right_report.validation) {
            (Some(left_summary), Some(right_summary)) => compare_summary(
                left_report.candidate_id,
                left_summary,
                right_report.candidate_id,
                right_summary,
            ),
            _ => left_report.candidate_id.cmp(&right_report.candidate_id),
        }
    });
    for (index, candidate_index) in eligible_indices.into_iter().enumerate() {
        reports[candidate_index].rank = Some(index + 1);
    }
    Ok(())
}

fn compare_summary(
    left_id: u64,
    left: &RobustOutcomeSummary,
    right_id: u64,
    right: &RobustOutcomeSummary,
) -> Ordering {
    compare_optional_descending(left.confidence_low, right.confidence_low)
        .then_with(|| compare_optional_descending(left.success_rate, right.success_rate))
        .then_with(|| left_id.cmp(&right_id))
}

fn compare_optional_descending(left: Option<f64>, right: Option<f64>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => right.total_cmp(&left),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn sort_candidate_rows(reports: &mut [RobustCandidateReport]) {
    reports.sort_by(|left, right| match (left.rank, right.rank) {
        (Some(left_rank), Some(right_rank)) => left_rank
            .cmp(&right_rank)
            .then_with(|| left.candidate_id.cmp(&right.candidate_id)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => left.candidate_id.cmp(&right.candidate_id),
    });
}

fn validate_budget(
    config: &RobustThreeCushionExperimentConfig,
) -> Result<(), RobustExperimentError> {
    for (value, label) in [
        (config.candidate_budget, "candidate budget"),
        (config.finalist_budget, "finalist budget"),
        (config.max_events, "max events"),
    ] {
        if value == 0 {
            return Err(invalid_configuration(format!(
                "{label} must be greater than zero"
            )));
        }
    }
    if config.screening_replication_budget == 0 {
        return Err(invalid_configuration(
            "screening replication budget must be greater than zero",
        ));
    }
    if config.validation_replication_budget == 0 {
        return Err(invalid_configuration(
            "validation replication budget must be greater than zero",
        ));
    }
    let required = config
        .additional_candidates
        .len()
        .checked_add(1)
        .ok_or_else(|| invalid_configuration("additional candidate count overflowed"))?;
    if config.candidate_budget < required {
        return Err(invalid_configuration(
            "candidate budget must cover nominal and additional candidates",
        ));
    }
    let greatest_id = u64::try_from(config.candidate_budget - 1)
        .map_err(|_| invalid_configuration("candidate budget exceeds u64 ID space"))?;
    greatest_id
        .checked_mul(ROBUST_MAX_PROPOSAL_ATTEMPTS_PER_CANDIDATE)
        .ok_or_else(|| invalid_configuration("candidate proposal sample ID would overflow u64"))?;
    Ok(())
}

fn validate_configured_candidate(
    label: &str,
    controls: RobustShotControls,
    mode: RobustExperimentMode,
    search_bounds: [RobustControlBounds; 5],
    sigmas: RobustNoiseSigmas,
) -> Result<(), RobustExperimentError> {
    validate_robust_controls(controls)
        .map_err(|error| invalid_configuration(format!("{label}: {error}")))?;
    if mode == RobustExperimentMode::Search
        && !robust_controls_within_bounds(controls, search_bounds)
    {
        return Err(invalid_configuration(format!(
            "{label} must lie within every search bound"
        )));
    }
    validate_robust_noise_envelope(controls, sigmas).map_err(|error| {
        invalid_configuration(format!(
            "{label} has a non-executable ±3σ envelope: {error}"
        ))
    })
}

fn validate_robust_non_negative_finite(
    label: &str,
    values: [f64; 5],
) -> Result<(), RobustControlValidationError> {
    if values
        .into_iter()
        .any(|value| !value.is_finite() || value < 0.0)
    {
        return Err(RobustControlValidationError(format!(
            "{label} must be finite and non-negative"
        )));
    }
    Ok(())
}

pub fn validate_robust_perturbations(
    widths: RobustPerturbationWidths,
) -> Result<(), RobustControlValidationError> {
    validate_robust_non_negative_finite("perturbation widths", widths.as_array())
}

pub fn validate_robust_noise_sigmas(
    sigmas: RobustNoiseSigmas,
) -> Result<(), RobustControlValidationError> {
    validate_robust_non_negative_finite("shot-inaccuracy sigmas", sigmas.as_array())
}

pub fn validate_robust_controls(
    controls: RobustShotControls,
) -> Result<(), RobustControlValidationError> {
    for (name, value) in [
        ("heading", controls.heading),
        ("speed", controls.speed),
        ("tip side", controls.tip_side),
        ("tip height", controls.tip_height),
        ("elevation", controls.elevation),
    ] {
        if !value.is_finite() {
            return Err(RobustControlValidationError(format!(
                "{name} must be finite"
            )));
        }
    }
    if !(0.0..360.0).contains(&controls.heading) {
        return Err(RobustControlValidationError(
            "heading must be in [0, 360) degrees".to_string(),
        ));
    }
    if controls.speed <= 0.0 {
        return Err(RobustControlValidationError(
            "launch speed must be greater than zero".to_string(),
        ));
    }
    let maximum_elevation =
        ROBUST_MAX_CUE_ELEVATION_DEGREES - ROBUST_ELEVATION_FEASIBILITY_MARGIN_DEGREES;
    if controls.elevation < 0.0 || controls.elevation > maximum_elevation {
        return Err(RobustControlValidationError(format!(
            "elevation must be in [0, {maximum_elevation}] degrees"
        )));
    }
    let maximum_tip = ROBUST_MAX_CANONICAL_TIP_OFFSET - ROBUST_TIP_FEASIBILITY_MARGIN;
    if controls.tip_side.hypot(controls.tip_height) > maximum_tip {
        return Err(RobustControlValidationError(format!(
            "tip side/height radius must not exceed {maximum_tip} ball radii"
        )));
    }
    Ok(())
}

pub fn robust_controls_within_bounds(
    controls: RobustShotControls,
    bounds: [RobustControlBounds; 5],
) -> bool {
    controls
        .as_array()
        .into_iter()
        .zip(bounds)
        .all(|(value, bound)| bound.contains(value))
}

pub fn validate_robust_noise_envelope(
    controls: RobustShotControls,
    sigmas: RobustNoiseSigmas,
) -> Result<(), RobustControlValidationError> {
    if sigmas
        .as_array()
        .into_iter()
        .any(|sigma| !sigma.is_finite() || sigma < 0.0)
    {
        return Err(RobustControlValidationError(
            "shot-inaccuracy sigmas must be finite and non-negative".to_string(),
        ));
    }
    let mut endpoints = [(0.0, 0.0); 5];
    for (index, (control, sigma)) in controls
        .as_array()
        .into_iter()
        .zip(sigmas.as_array())
        .enumerate()
    {
        let delta = sigma * ROBUST_NOISE_TRUNCATION_STANDARD_DEVIATIONS;
        let lower = control - delta;
        let upper = control + delta;
        if !delta.is_finite() || !lower.is_finite() || !upper.is_finite() {
            return Err(RobustControlValidationError(
                "control ± 3σ arithmetic must remain finite".to_string(),
            ));
        }
        endpoints[index] = (lower, upper);
    }
    if endpoints[1].0 <= 0.0 {
        return Err(RobustControlValidationError(
            "speed's lower 3σ endpoint must be greater than zero".to_string(),
        ));
    }
    let maximum_elevation =
        ROBUST_MAX_CUE_ELEVATION_DEGREES - ROBUST_ELEVATION_FEASIBILITY_MARGIN_DEGREES;
    if endpoints[4].0 < 0.0 || endpoints[4].1 > maximum_elevation {
        return Err(RobustControlValidationError(format!(
            "elevation's 3σ endpoints must stay in [0, {maximum_elevation}] degrees"
        )));
    }
    let maximum_tip = ROBUST_MAX_CANONICAL_TIP_OFFSET - ROBUST_TIP_FEASIBILITY_MARGIN;
    for side in [endpoints[2].0, endpoints[2].1] {
        for height in [endpoints[3].0, endpoints[3].1] {
            if side.hypot(height) > maximum_tip {
                return Err(RobustControlValidationError(format!(
                    "every tip side/height 3σ corner must stay within radius {maximum_tip}"
                )));
            }
        }
    }
    Ok(())
}

pub fn validate_robust_search_bounds(
    bounds: [RobustControlBounds; 5],
) -> Result<(), RobustControlValidationError> {
    for bound in bounds {
        if !bound.minimum.is_finite() || !bound.maximum.is_finite() || bound.minimum > bound.maximum
        {
            return Err(RobustControlValidationError(
                "search bounds must be finite and ordered".to_string(),
            ));
        }
        if !(bound.maximum - bound.minimum).is_finite() {
            return Err(RobustControlValidationError(
                "every search-bound span must be finite".to_string(),
            ));
        }
    }
    let [heading, speed, side, height, elevation] = bounds;
    if heading.minimum < 0.0
        || heading.maximum > 360.0
        || (heading.minimum == 360.0 && heading.maximum == 360.0)
    {
        return Err(RobustControlValidationError(
            "heading bounds must lie in [0, 360] and cannot be [360, 360]".to_string(),
        ));
    }
    if speed.minimum <= 0.0 {
        return Err(RobustControlValidationError(
            "speed bound minimum must be greater than zero".to_string(),
        ));
    }
    for (label, bound) in [("tip-side", side), ("tip-height", height)] {
        if bound.minimum < -ROBUST_MAX_CANONICAL_TIP_OFFSET
            || bound.maximum > ROBUST_MAX_CANONICAL_TIP_OFFSET
        {
            return Err(RobustControlValidationError(format!(
                "{label} bounds must lie in [-{ROBUST_MAX_CANONICAL_TIP_OFFSET}, {ROBUST_MAX_CANONICAL_TIP_OFFSET}]"
            )));
        }
    }
    let maximum_elevation =
        ROBUST_MAX_CUE_ELEVATION_DEGREES - ROBUST_ELEVATION_FEASIBILITY_MARGIN_DEGREES;
    if elevation.minimum < 0.0 || elevation.maximum > maximum_elevation {
        return Err(RobustControlValidationError(format!(
            "elevation bounds must lie in [0, {maximum_elevation}] degrees"
        )));
    }
    Ok(())
}

fn player_search_bounds(sigmas: RobustNoiseSigmas) -> [RobustControlBounds; 5] {
    let truncation = ROBUST_NOISE_TRUNCATION_STANDARD_DEVIATIONS;
    [
        RobustControlBounds::new(0.0, 360.0),
        RobustControlBounds::new(
            1.0 + truncation * sigmas.speed,
            300.0 - truncation * sigmas.speed,
        ),
        RobustControlBounds::new(-0.45, 0.45),
        RobustControlBounds::new(-0.45, 0.45),
        RobustControlBounds::new(
            truncation * sigmas.elevation,
            ROBUST_MAX_CUE_ELEVATION_DEGREES
                - ROBUST_ELEVATION_FEASIBILITY_MARGIN_DEGREES
                - truncation * sigmas.elevation,
        ),
    ]
}

fn player_search_seed(
    current: RobustShotControls,
    bounds: [RobustControlBounds; 5],
    sigmas: RobustNoiseSigmas,
) -> RobustShotControls {
    if validate_robust_controls(current).is_ok()
        && robust_controls_within_bounds(current, bounds)
        && validate_robust_noise_envelope(current, sigmas).is_ok()
    {
        current
    } else {
        RobustShotControls {
            heading: if current.heading.is_finite() {
                current.heading.rem_euclid(360.0)
            } else {
                0.0
            },
            speed: if current.speed.is_finite() {
                current.speed.clamp(bounds[1].minimum, bounds[1].maximum)
            } else {
                150.0
            },
            tip_side: 0.0,
            tip_height: 0.0,
            elevation: if current.elevation.is_finite() {
                current
                    .elevation
                    .clamp(bounds[4].minimum, bounds[4].maximum)
            } else {
                bounds[4].minimum
            },
        }
    }
}

fn invalid_configuration(detail: impl Into<String>) -> RobustExperimentError {
    RobustExperimentError::InvalidConfiguration(detail.into())
}

fn resource_error(resource: &str, error: impl fmt::Display) -> RobustExperimentError {
    RobustExperimentError::ReportAssembly(format!("could not reserve {resource}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_profiles_are_monotonic_and_parse_stable_keys() {
        let b = ThreeCushionPlayerLevel::B.shot_inaccuracy().as_array();
        let a = ThreeCushionPlayerLevel::A.shot_inaccuracy().as_array();
        let pro = ThreeCushionPlayerLevel::Pro.shot_inaccuracy().as_array();
        for index in 0..5 {
            assert!(b[index] > a[index]);
            assert!(a[index] > pro[index]);
        }
        assert_eq!("B player".parse(), Ok(ThreeCushionPlayerLevel::B));
        assert_eq!("a".parse(), Ok(ThreeCushionPlayerLevel::A));
        assert_eq!("pro-player".parse(), Ok(ThreeCushionPlayerLevel::Pro));
        assert!("novice".parse::<ThreeCushionPlayerLevel>().is_err());
    }

    #[test]
    fn evaluation_budget_is_bounded_and_explicit() {
        for iterations in [16, 100, 1_000, MAX_ROBUST_SEARCH_EVALUATIONS] {
            let budget = allocate_robust_search_budget(iterations)
                .unwrap_or_else(|error| panic!("budget {iterations} failed: {error}"));
            assert!(budget.planned_evaluations <= iterations);
            assert!(budget.candidate_budget >= budget.finalist_budget);
            assert!(budget.screening_replications > 0);
            assert!(budget.validation_replications > 0);
        }
        assert!(allocate_robust_search_budget(0).is_err());
        assert!(allocate_robust_search_budget(MIN_ROBUST_SEARCH_EVALUATIONS - 1).is_err());
        assert!(allocate_robust_search_budget(MAX_ROBUST_SEARCH_EVALUATIONS + 1).is_err());

        let exact = allocate_robust_search_budget(16)
            .unwrap_or_else(|error| panic!("minimum budget failed: {error}"));
        assert_eq!(exact.candidate_budget, 4);
        assert_eq!(exact.screening_replications, 2);
        assert_eq!(exact.finalist_budget, 4);
        assert_eq!(exact.validation_replications, 2);
        assert_eq!(exact.planned_evaluations, 16);
    }

    #[test]
    fn fixed_player_search_is_deterministic_and_honors_budget() {
        let layout = match ShotLayout::three_cushion_from_diamonds(
            (0.700, 1.000),
            (1.200, 2.100),
            (0.850, 6.550),
        ) {
            Ok(layout) => layout,
            Err(error) => panic!("fixture layout failed: {error}"),
        };
        let request = PlayerRobustSearchRequest {
            physics: PhysicsProfile::three_cushion_default(),
            layout,
            cue: crate::canonical_three_cushion_cue_config(),
            shooter: ThreeCushionShooter::Cue,
            current_controls: RobustShotControls {
                heading: 196.391_792_039,
                speed: 237.947_968_822,
                tip_side: -0.230_661_681,
                tip_height: 0.365_060_077,
                elevation: 0.0,
            },
            requested_evaluations: MIN_ROBUST_SEARCH_EVALUATIONS,
            player_level: ThreeCushionPlayerLevel::Pro,
            max_events: 64,
        };
        let first = match run_player_robust_search(&request) {
            Ok(report) => report,
            Err(error) => panic!("first fixed search failed: {error}"),
        };
        let second = match run_player_robust_search(&request) {
            Ok(report) => report,
            Err(error) => panic!("second fixed search failed: {error}"),
        };

        assert_eq!(first, second);
        assert_eq!(first.budget.planned_evaluations, 16);
        assert_eq!(first.actual_evaluations(), 16);
        assert!(first.winner().is_some());
    }
}
