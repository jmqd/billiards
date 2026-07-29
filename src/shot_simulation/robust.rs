use std::cmp::Ordering;
use std::error::Error;
use std::fmt;
use std::num::{NonZeroU32, NonZeroUsize};
use std::str::FromStr;

use simul::experiment::{
    run_replicated, Candidate, CrossEntropyConfig, CrossEntropyDimension, CrossEntropyOptimizer,
    CrossEntropySample, ReplicationPlan, SampleContext, SampleStream, SamplingError, TrialContext,
    TrialError, TrialRecord, SEED_PROTOCOL,
};
pub use simul::experiment::{RandomDomain, ReplayKey};

use super::{
    execute_three_cushion_compact, CaromBallRole, CueStrikeConfig, PhysicsProfile, ShotControls,
    ShotLayout, ShotLimit, ThreeCushionAdjudication, ThreeCushionShooter, ThreeCushionShot,
    THREE_CUSHION_MAX_CUE_BALL_HEIGHT_INCHES,
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
const CROSS_ENTROPY_POPULATION_SIZE: usize = 24;
const CROSS_ENTROPY_ISLAND_COUNT: usize = 2;
const CROSS_ENTROPY_ELITE_FRACTION: f64 = 0.25;
const CROSS_ENTROPY_LEARNING_RATE: f64 = 0.6;
const CROSS_ENTROPY_GLOBAL_IMMIGRANT_INTERVAL: usize = 6;
const CROSS_ENTROPY_NORMAL_TRUNCATION: f64 = 3.0;
const CROSS_ENTROPY_INITIAL_STANDARD_DEVIATION: [f64; 5] = [0.125, 0.18, 0.25, 0.25, 0.12];
const CROSS_ENTROPY_MINIMUM_STANDARD_DEVIATION: [f64; 5] = [0.01, 0.02, 0.025, 0.025, 0.01];
const CROSS_ENTROPY_STREAMS: [SampleStream; 5] = [
    HEADING_STREAM,
    SPEED_STREAM,
    TIP_SIDE_STREAM,
    TIP_HEIGHT_STREAM,
    ELEVATION_STREAM,
];
// Domain priors are fractions of validated search bounds, not fixture controls.
const GUIDED_SPEED_FRACTION: f64 = 0.36;
const GUIDED_SIDE_MAGNITUDE_FRACTION: f64 = 0.87;
const GUIDED_HEIGHT_FRACTION: f64 = 0.25;
const GUIDED_ELEVATION_FRACTION: f64 = 0.01;

pub const ROBUST_NOISE_TRUNCATION_STANDARD_DEVIATIONS: f64 = 3.0;
pub const ROBUST_MAX_CANONICAL_TIP_OFFSET: f64 = 0.5;
pub const ROBUST_TIP_FEASIBILITY_MARGIN: f64 = 1e-12;
pub const ROBUST_MAX_CUE_ELEVATION_DEGREES: f64 = 85.0;
pub const ROBUST_ELEVATION_FEASIBILITY_MARGIN_DEGREES: f64 = 1e-12;
pub const ROBUST_MAX_PROPOSAL_ATTEMPTS_PER_CANDIDATE: u64 = 4_096;

pub const MIN_ROBUST_SEARCH_EVALUATIONS: u32 = 16;
pub const MAX_ROBUST_SEARCH_EVALUATIONS: u32 = 10_000;
pub const DEFAULT_ROBUST_SEARCH_SEED: u64 = 0x524f_4255_5354_0001;
const MAX_PLAYER_SEARCH_CANDIDATES: usize = 512;
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

const PRO_PLAYER_SHOT_INACCURACY: RobustNoiseSigmas = RobustNoiseSigmas {
    heading: 0.25,
    speed: 1.5,
    tip_side: 0.008,
    tip_height: 0.008,
    elevation: 0.15,
};
const WORLD_CLASS_PRO_SIGMA_FACTOR: f64 = 0.7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThreeCushionPlayerLevel {
    B,
    A,
    Pro,
    WorldClassPro,
}

impl ThreeCushionPlayerLevel {
    pub const fn key(self) -> &'static str {
        match self {
            Self::B => "b",
            Self::A => "a",
            Self::Pro => "pro",
            Self::WorldClassPro => "world-class-pro",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::B => "B player",
            Self::A => "A player",
            Self::Pro => "Pro player",
            Self::WorldClassPro => "World Class Pro",
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
            Self::Pro => PRO_PLAYER_SHOT_INACCURACY,
            Self::WorldClassPro => RobustNoiseSigmas {
                heading: PRO_PLAYER_SHOT_INACCURACY.heading * WORLD_CLASS_PRO_SIGMA_FACTOR,
                speed: PRO_PLAYER_SHOT_INACCURACY.speed * WORLD_CLASS_PRO_SIGMA_FACTOR,
                tip_side: PRO_PLAYER_SHOT_INACCURACY.tip_side * WORLD_CLASS_PRO_SIGMA_FACTOR,
                tip_height: PRO_PLAYER_SHOT_INACCURACY.tip_height * WORLD_CLASS_PRO_SIGMA_FACTOR,
                elevation: PRO_PLAYER_SHOT_INACCURACY.elevation * WORLD_CLASS_PRO_SIGMA_FACTOR,
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
            "unknown player level '{}'; expected b, a, pro, or world-class-pro",
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
            "world-class-pro" | "world class pro" => Ok(Self::WorldClassPro),
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
    search_fitness: f64,
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
    search_fitness_total: f64,
    evaluated: u32,
}

struct StageResults {
    summaries: Vec<RobustOutcomeSummary>,
    search_fitnesses: Vec<f64>,
    trials: Vec<RobustTrialReport>,
}

impl Evaluator {
    fn evaluate(
        &self,
        shooter: ThreeCushionShooter,
        controls: RobustShotControls,
        max_events: usize,
    ) -> Result<(EvaluatedOutcome, f64), String> {
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
        let search_fitness = three_cushion_search_fitness(
            &result.completion.summary,
            2.0 * self.physics.ball.radius.as_f64(),
        );
        let outcome = match result.completion.summary {
            ThreeCushionAdjudication::Scored(_) => EvaluatedOutcome::Scored,
            ThreeCushionAdjudication::Miss { reason, .. } => {
                EvaluatedOutcome::Miss(format!("{reason:?}"))
            }
            ThreeCushionAdjudication::Indeterminate { reason, .. } => {
                EvaluatedOutcome::Indeterminate(format!("{reason:?}"))
            }
        };
        Ok((outcome, search_fitness))
    }
}

/// Bounded partial progress used only to steer search proposals.
fn three_cushion_search_fitness(
    adjudication: &ThreeCushionAdjudication,
    contact_distance: f64,
) -> f64 {
    let facts = adjudication.facts();
    if facts.maximum_cue_ball_height.as_f64() > THREE_CUSHION_MAX_CUE_BALL_HEIGHT_INCHES {
        return 0.0;
    }
    if adjudication.is_scored() {
        return 1.0;
    }

    let cushions = facts.cushion_contacts_before_completion.min(3);
    if facts.completion.is_some() {
        // Reaching the second object before three cushions is terminal, not useful progress.
        return 0.02 + 0.04 * f64::from(cushions);
    }

    let object_contacts = u8::from(facts.object_a_first_contact.is_some())
        + u8::from(facts.object_b_first_contact.is_some());
    let cushion_tier = 0.2 * f64::from(cushions);
    let first_object_bonus = if object_contacts == 1 { 0.1 } else { 0.0 };
    let clearance_bonus = if cushions == 3 && object_contacts == 1 {
        facts
            .estimated_closest_second_object_clearance
            .as_ref()
            .filter(|_| contact_distance.is_finite() && contact_distance > 0.0)
            .map_or(0.0, |clearance| {
                let clearance = clearance.as_f64().max(0.0);
                0.25 * contact_distance / (contact_distance + clearance)
            })
    } else {
        0.0
    };
    (cushion_tier + first_object_bonus + clearance_bonus).min(0.95)
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
            search_fitness_total: 0.0,
            evaluated: 0,
        }
    }

    fn record(&mut self, stage: RobustTrialStage, record: StageTrialRecord) -> RobustTrialReport {
        let (applied, disposition) = match record.result {
            Ok(evaluated) => {
                let EvaluatedTrial {
                    applied,
                    outcome,
                    search_fitness,
                } = evaluated;
                self.search_fitness_total += search_fitness;
                self.evaluated += 1;
                match outcome {
                    EvaluatedOutcome::Scored => {
                        self.scored += 1;
                        self.reducer.observe(true);
                        (Some(applied), RobustTrialDisposition::Scored)
                    }
                    EvaluatedOutcome::Miss(detail) => {
                        self.missed += 1;
                        self.reducer.observe(false);
                        (Some(applied), RobustTrialDisposition::Miss(detail))
                    }
                    EvaluatedOutcome::Indeterminate(detail) => {
                        self.indeterminate += 1;
                        (Some(applied), RobustTrialDisposition::Indeterminate(detail))
                    }
                }
            }
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

    fn mean_search_fitness(&self) -> f64 {
        if self.evaluated == 0 {
            0.0
        } else {
            self.search_fitness_total / f64::from(self.evaluated)
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

    // Preserve the exact minimum-budget schedule. Larger searches reserve one quarter for
    // held-out validation and use at least two CRN observations per screening proposal.
    let (screening_budget, minimum_screening_replications) = if requested_evaluations < 32 {
        (requested_evaluations.div_euclid(2), 1)
    } else {
        (
            requested_evaluations - requested_evaluations.div_euclid(4),
            2,
        )
    };
    let candidate_budget =
        usize::try_from(screening_budget.div_euclid(minimum_screening_replications))
            .map_err(|_| {
                RobustExperimentError::InvalidConfiguration(
                    "screening candidate budget does not fit platform size".to_string(),
                )
            })?
            .clamp(1, MAX_PLAYER_SEARCH_CANDIDATES);
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
    let screening_replications = screening_budget
        .div_euclid(candidate_count)
        .max(minimum_screening_replications);
    let screening_cost = candidate_count
        .checked_mul(screening_replications)
        .ok_or_else(|| invalid_configuration("screening evaluation count overflowed"))?;
    let remaining = requested_evaluations
        .checked_sub(screening_cost)
        .ok_or_else(|| invalid_configuration("screening exceeded requested evaluations"))?;
    let validation_replications = remaining.div_euclid(finalist_count).max(1);
    let validation_cost = finalist_count
        .checked_mul(validation_replications)
        .ok_or_else(|| invalid_configuration("validation evaluation count overflowed"))?;
    let planned_evaluations = screening_cost
        .checked_add(validation_cost)
        .ok_or_else(|| invalid_configuration("planned evaluation count overflowed"))?;
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
    let (candidates, screening) = match config.mode {
        RobustExperimentMode::Search => run_cross_entropy_search_screening(config)?,
        RobustExperimentMode::Sensitivity => {
            let candidates = make_candidates(config)?;
            let screening = run_stage(
                config,
                &candidates,
                RobustTrialStage::Screening,
                ROBUST_SENSITIVITY_TRIAL_DOMAIN,
                config.screening_replication_budget,
            )?;
            (candidates, screening)
        }
    };

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
        let finalists = select_finalists(
            &candidate_reports,
            &candidates,
            &screening.search_fitnesses,
            config.finalist_budget,
        )?;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CrossEntropyProposalKind {
    Center,
    Distribution,
    Global,
}

/// Evaluates proposal generations in batches, then updates two object-directed CEM islands.
fn run_cross_entropy_search_screening(
    config: &RobustThreeCushionExperimentConfig,
) -> Result<(Vec<Candidate<RobustShotControls>>, StageResults), RobustExperimentError> {
    let mut candidates = Vec::new();
    candidates
        .try_reserve_exact(config.candidate_budget)
        .map_err(|error| resource_error("cross-entropy candidate set", error))?;
    let mut summaries = Vec::new();
    summaries
        .try_reserve_exact(config.candidate_budget)
        .map_err(|error| resource_error("cross-entropy screening summaries", error))?;
    let mut search_fitnesses = Vec::new();
    search_fitnesses
        .try_reserve_exact(config.candidate_budget)
        .map_err(|error| resource_error("cross-entropy search fitnesses", error))?;
    let trial_capacity = config
        .candidate_budget
        .checked_mul(
            usize::try_from(config.screening_replication_budget).map_err(|_| {
                RobustExperimentError::InvalidConfiguration(
                    "screening replication budget does not fit platform size".to_string(),
                )
            })?,
        )
        .ok_or_else(|| {
            RobustExperimentError::InvalidConfiguration(
                "cross-entropy screening trial count overflowed platform size".to_string(),
            )
        })?;
    let mut trials = Vec::new();
    trials
        .try_reserve_exact(trial_capacity)
        .map_err(|error| resource_error("cross-entropy screening trials", error))?;

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
    let mut configured_results = run_stage(
        config,
        &candidates,
        RobustTrialStage::Screening,
        ROBUST_SEARCH_SCREENING_DOMAIN,
        config.screening_replication_budget,
    )?;
    summaries.append(&mut configured_results.summaries);
    search_fitnesses.append(&mut configured_results.search_fitnesses);
    trials.append(&mut configured_results.trials);

    let mut optimizers = cross_entropy_islands(config)?;
    let mut batch = Vec::new();
    batch
        .try_reserve_exact(CROSS_ENTROPY_POPULATION_SIZE)
        .map_err(|error| resource_error("cross-entropy generation", error))?;
    let mut island_by_offset = [0; CROSS_ENTROPY_POPULATION_SIZE];
    let mut point_by_offset = [[0.0; 5]; CROSS_ENTROPY_POPULATION_SIZE];
    let empty_sample = CrossEntropySample::new([0.0; 5], f64::NAN);
    let mut island_samples = [[empty_sample; CROSS_ENTROPY_POPULATION_SIZE / 2]; 2];
    let mut island_sample_counts = [0; CROSS_ENTROPY_ISLAND_COUNT];
    let mut generated_count = 0;

    while candidates.len() < config.candidate_budget {
        batch.clear();
        island_sample_counts.fill(0);
        let batch_size =
            CROSS_ENTROPY_POPULATION_SIZE.min(config.candidate_budget - candidates.len());
        for offset in 0..batch_size {
            let candidate_id = u64::try_from(candidates.len() + offset).map_err(|_| {
                RobustExperimentError::CandidateGeneration(
                    "candidate ID does not fit u64".to_string(),
                )
            })?;
            let island = generated_count % CROSS_ENTROPY_ISLAND_COUNT;
            let proposal_kind = cross_entropy_proposal_kind(generated_count);
            let (controls, point) = generate_cross_entropy_candidate(
                config,
                candidate_id,
                &optimizers[island],
                proposal_kind,
            )?;
            island_by_offset[offset] = island;
            point_by_offset[offset] = point;
            batch.push(Candidate {
                id: candidate_id,
                value: controls,
            });
            generated_count += 1;
        }

        let mut result = run_stage(
            config,
            &batch,
            RobustTrialStage::Screening,
            ROBUST_SEARCH_SCREENING_DOMAIN,
            config.screening_replication_budget,
        )?;
        if result.summaries.len() != batch.len() || result.search_fitnesses.len() != batch.len() {
            return Err(RobustExperimentError::ReportAssembly(
                "cross-entropy generation result count did not match its candidates".to_string(),
            ));
        }
        for offset in 0..batch.len() {
            let island = island_by_offset[offset];
            let sample_index = island_sample_counts[island];
            island_samples[island][sample_index] = CrossEntropySample::new(
                point_by_offset[offset],
                cross_entropy_score(&result.summaries[offset], result.search_fitnesses[offset]),
            );
            island_sample_counts[island] += 1;
        }
        for island in 0..CROSS_ENTROPY_ISLAND_COUNT {
            optimizers[island]
                .tell(&mut island_samples[island][..island_sample_counts[island]])
                .map_err(|error| {
                    RobustExperimentError::ReportAssembly(format!(
                        "cross-entropy island update failed: {error}"
                    ))
                })?;
        }

        candidates.append(&mut batch);
        summaries.append(&mut result.summaries);
        search_fitnesses.append(&mut result.search_fitnesses);
        trials.append(&mut result.trials);
    }

    Ok((
        candidates,
        StageResults {
            summaries,
            search_fitnesses,
            trials,
        },
    ))
}

/// Creates one proposal distribution aimed at each possible first object ball.
fn cross_entropy_islands(
    config: &RobustThreeCushionExperimentConfig,
) -> Result<[CrossEntropyOptimizer<5>; CROSS_ENTROPY_ISLAND_COUNT], RobustExperimentError> {
    Ok([
        cross_entropy_island(config, 1)?,
        cross_entropy_island(config, 2)?,
    ])
}

/// Creates one normalized diagonal Gaussian CEM island.
fn cross_entropy_island(
    config: &RobustThreeCushionExperimentConfig,
    object_selector: u64,
) -> Result<CrossEntropyOptimizer<5>, RobustExperimentError> {
    let samples = SampleContext::new(
        config.master_seed,
        ROBUST_SEARCH_PROPOSAL_DOMAIN,
        object_selector,
    );
    let center = sample_guided_search_candidate(config, object_selector, samples, true)?;
    let dimensions = cross_entropy_dimensions(config.search_bounds);
    CrossEntropyOptimizer::new(
        CrossEntropyConfig::new(
            normalize_search_controls(center, config.search_bounds),
            CROSS_ENTROPY_INITIAL_STANDARD_DEVIATION,
        )
        .with_dimensions(dimensions)
        .with_minimum_standard_deviation(CROSS_ENTROPY_MINIMUM_STANDARD_DEVIATION)
        .with_elite_fraction(CROSS_ENTROPY_ELITE_FRACTION)
        .with_learning_rate(CROSS_ENTROPY_LEARNING_RATE),
    )
    .map_err(|error| {
        RobustExperimentError::CandidateGeneration(format!(
            "could not initialize cross-entropy island: {error}"
        ))
    })
}

/// Chooses structured centers, global immigrants, or an island distribution.
fn cross_entropy_proposal_kind(generated_index: usize) -> CrossEntropyProposalKind {
    if generated_index < CROSS_ENTROPY_ISLAND_COUNT {
        CrossEntropyProposalKind::Center
    } else if (generated_index + 1).is_multiple_of(CROSS_ENTROPY_GLOBAL_IMMIGRANT_INTERVAL) {
        CrossEntropyProposalKind::Global
    } else {
        CrossEntropyProposalKind::Distribution
    }
}

/// Generates an executable proposal and its normalized representation.
fn generate_cross_entropy_candidate(
    config: &RobustThreeCushionExperimentConfig,
    candidate_id: u64,
    optimizer: &CrossEntropyOptimizer<5>,
    proposal_kind: CrossEntropyProposalKind,
) -> Result<(RobustShotControls, [f64; 5]), RobustExperimentError> {
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
        let samples =
            SampleContext::new(config.master_seed, ROBUST_SEARCH_PROPOSAL_DOMAIN, sample_id);
        let point = match proposal_kind {
            CrossEntropyProposalKind::Center if attempt == 0 => *optimizer.mean(),
            CrossEntropyProposalKind::Global => {
                std::array::from_fn(|dimension| samples.uniform(CROSS_ENTROPY_STREAMS[dimension]))
            }
            CrossEntropyProposalKind::Center | CrossEntropyProposalKind::Distribution => {
                sample_cross_entropy_point(optimizer, samples)?
            }
        };
        let controls = denormalize_search_controls(point, config.search_bounds);
        if candidate_is_executable(config, controls) {
            return Ok((controls, point));
        }
    }
    Err(RobustExperimentError::CandidateGeneration(format!(
        "could not generate cross-entropy candidate {candidate_id} with an executable ±3σ envelope after 4096 attempts"
    )))
}

/// Samples a CEM point from deterministic named standard-normal streams.
fn sample_cross_entropy_point(
    optimizer: &CrossEntropyOptimizer<5>,
    samples: SampleContext,
) -> Result<[f64; 5], RobustExperimentError> {
    let mut sampling_error = None;
    let point = optimizer
        .ask_with_standard_normal(|dimension| {
            match samples.truncated_standard_normal(
                CROSS_ENTROPY_STREAMS[dimension],
                CROSS_ENTROPY_NORMAL_TRUNCATION,
            ) {
                Ok(value) => value,
                Err(error) => {
                    sampling_error.get_or_insert(error);
                    0.0
                }
            }
        })
        .map_err(|error| {
            RobustExperimentError::CandidateGeneration(format!(
                "cross-entropy sampling failed: {error}"
            ))
        })?;
    if let Some(error) = sampling_error {
        Err(RobustExperimentError::CandidateGeneration(format!(
            "cross-entropy normal sampling failed: {error}"
        )))
    } else {
        Ok(point)
    }
}

/// Uses eligible soft progress for CEM fitting; validation still ranks legal-score probability.
fn cross_entropy_score(summary: &RobustOutcomeSummary, search_fitness: f64) -> f64 {
    if summary.eligible {
        search_fitness
    } else {
        f64::NAN
    }
}

/// Returns normalized geometry, treating only a full heading range as circular.
fn cross_entropy_dimensions(bounds: [RobustControlBounds; 5]) -> [CrossEntropyDimension; 5] {
    let heading = if bounds[0].minimum == 0.0 && bounds[0].maximum == 360.0 {
        CrossEntropyDimension::Circular
    } else {
        CrossEntropyDimension::Linear
    };
    [
        heading,
        CrossEntropyDimension::Linear,
        CrossEntropyDimension::Linear,
        CrossEntropyDimension::Linear,
        CrossEntropyDimension::Linear,
    ]
}

/// Maps physical controls into CEM's normalized search space.
fn normalize_search_controls(
    controls: RobustShotControls,
    bounds: [RobustControlBounds; 5],
) -> [f64; 5] {
    let values = controls.as_array();
    std::array::from_fn(|dimension| {
        let span = bounds[dimension].maximum - bounds[dimension].minimum;
        if span == 0.0 {
            0.5
        } else if dimension == 0
            && bounds[dimension].minimum == 0.0
            && bounds[dimension].maximum == 360.0
        {
            (values[dimension] - bounds[dimension].minimum).rem_euclid(span) / span
        } else {
            ((values[dimension] - bounds[dimension].minimum) / span).clamp(0.0, 1.0)
        }
    })
}

/// Maps normalized CEM coordinates into physical shot controls.
fn denormalize_search_controls(
    point: [f64; 5],
    bounds: [RobustControlBounds; 5],
) -> RobustShotControls {
    RobustShotControls {
        heading: interpolate(bounds[0], point[0]),
        speed: interpolate(bounds[1], point[1]),
        tip_side: interpolate(bounds[2], point[2]),
        tip_height: interpolate(bounds[3], point[3]),
        elevation: interpolate(bounds[4], point[4]),
    }
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
                .map(|(outcome, search_fitness)| EvaluatedTrial {
                    applied: prepared.applied,
                    outcome,
                    search_fitness,
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
    let mut search_fitnesses = Vec::new();
    search_fitnesses
        .try_reserve_exact(candidates.len())
        .map_err(|error| resource_error("stage search fitnesses", error))?;
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
        search_fitnesses.push(accumulator.mean_search_fitness());
        summaries.push(accumulator.finish()?);
    }
    if records.next().is_some() {
        return Err(RobustExperimentError::ReportAssembly(format!(
            "{stage} left unconsumed records"
        )));
    }
    Ok(StageResults {
        summaries,
        search_fitnesses,
        trials,
    })
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
fn sample_guided_search_candidate(
    config: &RobustThreeCushionExperimentConfig,
    candidate_id: u64,
    samples: SampleContext,
    structured_seed: bool,
) -> Result<RobustShotControls, RobustExperimentError> {
    let direct_heading = direct_object_heading(config, candidate_id)?;
    let bounded_heading = direct_heading.clamp(
        config.search_bounds[0].minimum,
        config.search_bounds[0].maximum,
    );
    if structured_seed && candidate_id <= 2 {
        let side_sample = if candidate_id.is_multiple_of(2) {
            (1.0 - GUIDED_SIDE_MAGNITUDE_FRACTION) / 2.0
        } else {
            (1.0 + GUIDED_SIDE_MAGNITUDE_FRACTION) / 2.0
        };
        return Ok(RobustShotControls {
            heading: bounded_heading,
            speed: interpolate(config.search_bounds[1], GUIDED_SPEED_FRACTION),
            tip_side: interpolate(config.search_bounds[2], side_sample),
            tip_height: interpolate(
                config.search_bounds[3],
                (1.0 + GUIDED_HEIGHT_FRACTION) / 2.0,
            ),
            elevation: interpolate(config.search_bounds[4], GUIDED_ELEVATION_FRACTION),
        });
    }

    let heading_deviation = if candidate_id <= 2 {
        0.0
    } else {
        45.0 * symmetric_uniform(samples, HEADING_STREAM)
    };
    let speed = if candidate_id <= 2 {
        interpolate(config.search_bounds[1], GUIDED_SPEED_FRACTION)
    } else {
        let practical_speed_sample = 0.15 + 0.55 * samples.uniform(SPEED_STREAM);
        interpolate(config.search_bounds[1], practical_speed_sample)
    };
    let elevation_sample = samples.uniform(ELEVATION_STREAM);
    Ok(RobustShotControls {
        heading: (direct_heading + heading_deviation)
            .rem_euclid(360.0)
            .clamp(
                config.search_bounds[0].minimum,
                config.search_bounds[0].maximum,
            ),
        speed,
        tip_side: interpolate(config.search_bounds[2], samples.uniform(TIP_SIDE_STREAM)),
        tip_height: interpolate(config.search_bounds[3], samples.uniform(TIP_HEIGHT_STREAM)),
        elevation: interpolate(config.search_bounds[4], elevation_sample.powi(3)),
    })
}

fn direct_object_heading(
    config: &RobustThreeCushionExperimentConfig,
    candidate_id: u64,
) -> Result<f64, RobustExperimentError> {
    let (shooter_role, first_object, second_object) = match config.shooter {
        ThreeCushionShooter::Cue => (
            CaromBallRole::Cue,
            CaromBallRole::YellowCue,
            CaromBallRole::Red,
        ),
        ThreeCushionShooter::YellowCue => (
            CaromBallRole::YellowCue,
            CaromBallRole::Cue,
            CaromBallRole::Red,
        ),
    };
    let target_role = if candidate_id.is_multiple_of(2) {
        second_object
    } else {
        first_object
    };
    let (shooter_x, shooter_y) = layout_position(&config.layout, shooter_role)?;
    let (target_x, target_y) = layout_position(&config.layout, target_role)?;
    Ok((target_x - shooter_x)
        .atan2(target_y - shooter_y)
        .to_degrees()
        .rem_euclid(360.0))
}

fn layout_position(
    layout: &ShotLayout,
    role: CaromBallRole,
) -> Result<(f64, f64), RobustExperimentError> {
    let ball = layout
        .balls()
        .iter()
        .find(|ball| ball.role == role)
        .ok_or_else(|| {
            RobustExperimentError::CandidateGeneration(format!(
                "validated layout is missing {role:?}"
            ))
        })?;
    let position = &ball.state.as_ball_state().position;
    Ok((position.x().as_f64(), position.y().as_f64()))
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
    search_fitnesses: &[f64],
    finalist_budget: usize,
) -> Result<Vec<Candidate<RobustShotControls>>, RobustExperimentError> {
    if reports.len() != search_fitnesses.len() {
        return Err(RobustExperimentError::ReportAssembly(format!(
            "screening returned {} fitnesses for {} candidates",
            search_fitnesses.len(),
            reports.len()
        )));
    }
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
        compare_summary_metrics(&reports[*left].screening, &reports[*right].screening)
            .then_with(|| search_fitnesses[*right].total_cmp(&search_fitnesses[*left]))
            .then_with(|| {
                reports[*left]
                    .candidate_id
                    .cmp(&reports[*right].candidate_id)
            })
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
            .is_some_and(|summary| summary.eligible && summary.scored > 0)
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
    compare_summary_metrics(left, right).then_with(|| left_id.cmp(&right_id))
}

fn compare_summary_metrics(left: &RobustOutcomeSummary, right: &RobustOutcomeSummary) -> Ordering {
    compare_optional_descending(left.confidence_low, right.confidence_low)
        .then_with(|| compare_optional_descending(left.success_rate, right.success_rate))
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
    use super::super::{ContactInstant, ThreeCushionFacts, ThreeCushionMiss};
    use super::*;
    use crate::{Inches, Seconds};

    fn progress_miss(
        cushions: u16,
        object_contacts: u8,
        clearance: Option<f64>,
    ) -> ThreeCushionAdjudication {
        let contact = ContactInstant {
            event_index: 0,
            at: Seconds::zero(),
        };
        ThreeCushionAdjudication::Miss {
            facts: ThreeCushionFacts {
                object_a_first_contact: (object_contacts >= 1).then_some(contact),
                object_b_first_contact: (object_contacts >= 2).then_some(contact),
                completion: (object_contacts >= 2).then_some(contact),
                cushion_contacts_before_completion: cushions,
                first_three_qualifying_cushions: [None; 3],
                maximum_cue_ball_height: Inches::zero(),
                estimated_closest_second_object_clearance: clearance.map(Inches::from_f64),
            },
            reason: if object_contacts >= 2 {
                ThreeCushionMiss::InsufficientCushions {
                    required: 3,
                    observed: cushions,
                }
            } else {
                ThreeCushionMiss::MissingObjectContact
            },
        }
    }

    #[test]
    fn soft_fitness_prioritizes_cushions_and_continuous_second_object_clearance() {
        let contact_distance = 2.25;
        let no_object_three_cushions =
            three_cushion_search_fitness(&progress_miss(3, 0, None), contact_distance);
        let one_object_two_cushions =
            three_cushion_search_fitness(&progress_miss(2, 1, None), contact_distance);
        let early_second_object =
            three_cushion_search_fitness(&progress_miss(2, 2, None), contact_distance);
        assert!(no_object_three_cushions > one_object_two_cushions);
        assert!(one_object_two_cushions > early_second_object);

        let no_clearance =
            three_cushion_search_fitness(&progress_miss(3, 1, None), contact_distance);
        let far = three_cushion_search_fitness(&progress_miss(3, 1, Some(9.0)), contact_distance);
        let near = three_cushion_search_fitness(&progress_miss(3, 1, Some(0.5)), contact_distance);
        let contact =
            three_cushion_search_fitness(&progress_miss(3, 1, Some(0.0)), contact_distance);
        assert!(no_clearance < far);
        assert!(far < near);
        assert!(near < contact);
        assert!(contact < 1.0);

        let mut jumping = progress_miss(3, 1, Some(0.0));
        let ThreeCushionAdjudication::Miss { facts, .. } = &mut jumping else {
            panic!("progress fixture must be a miss");
        };
        facts.maximum_cue_ball_height =
            Inches::from_f64(THREE_CUSHION_MAX_CUE_BALL_HEIGHT_INCHES + 0.001);
        assert!(three_cushion_search_fitness(&jumping, contact_distance).abs() <= f64::EPSILON);
    }

    #[test]
    fn player_profiles_are_monotonic_and_parse_stable_keys() {
        let b = ThreeCushionPlayerLevel::B.shot_inaccuracy().as_array();
        let a = ThreeCushionPlayerLevel::A.shot_inaccuracy().as_array();
        let pro = ThreeCushionPlayerLevel::Pro.shot_inaccuracy().as_array();
        let world_class_pro = ThreeCushionPlayerLevel::WorldClassPro
            .shot_inaccuracy()
            .as_array();
        for index in 0..5 {
            assert!(b[index] > a[index]);
            assert!(a[index] > pro[index]);
            assert!(pro[index] > world_class_pro[index]);
            assert_eq!(
                world_class_pro[index],
                pro[index] * WORLD_CLASS_PRO_SIGMA_FACTOR,
                "world-class sigma {index} must be exactly 70% of Pro",
            );
        }
        assert_eq!("B player".parse(), Ok(ThreeCushionPlayerLevel::B));
        assert_eq!("a".parse(), Ok(ThreeCushionPlayerLevel::A));
        assert_eq!("pro-player".parse(), Ok(ThreeCushionPlayerLevel::Pro));
        assert_eq!(
            ThreeCushionPlayerLevel::WorldClassPro.key(),
            "world-class-pro",
        );
        assert_eq!(
            ThreeCushionPlayerLevel::WorldClassPro.label(),
            "World Class Pro",
        );
        assert_eq!(
            "world-class-pro".parse(),
            Ok(ThreeCushionPlayerLevel::WorldClassPro),
        );
        assert_eq!(
            "World Class Pro".parse(),
            Ok(ThreeCushionPlayerLevel::WorldClassPro),
        );
        let Err(error) = "novice".parse::<ThreeCushionPlayerLevel>() else {
            panic!("novice must not parse as a player level");
        };
        assert_eq!(
            error.to_string(),
            "unknown player level 'novice'; expected b, a, pro, or world-class-pro",
        );
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
        assert_eq!(exact.candidate_budget, 8);
        assert_eq!(exact.screening_replications, 1);
        assert_eq!(exact.finalist_budget, 4);
        assert_eq!(exact.validation_replications, 2);
        assert_eq!(exact.planned_evaluations, 16);

        let repeated = allocate_robust_search_budget(32)
            .unwrap_or_else(|error| panic!("32-evaluation budget failed: {error}"));
        assert_eq!(repeated.candidate_budget, 12);
        assert_eq!(repeated.screening_replications, 2);
        assert_eq!(repeated.validation_replications, 2);
        assert_eq!(repeated.planned_evaluations, 32);

        let default = allocate_robust_search_budget(256)
            .unwrap_or_else(|error| panic!("256-evaluation budget failed: {error}"));
        assert_eq!(default.candidate_budget, 96);
        assert_eq!(default.screening_replications, 2);
        assert_eq!(default.validation_replications, 16);
        assert_eq!(default.planned_evaluations, 256);

        let maximum = allocate_robust_search_budget(MAX_ROBUST_SEARCH_EVALUATIONS)
            .unwrap_or_else(|error| panic!("maximum budget failed: {error}"));
        assert_eq!(maximum.candidate_budget, MAX_PLAYER_SEARCH_CANDIDATES);
        assert_eq!(maximum.screening_replications, 14);
        assert_eq!(maximum.validation_replications, 708);
        assert_eq!(maximum.planned_evaluations, MAX_ROBUST_SEARCH_EVALUATIONS);
    }

    #[test]
    fn cross_entropy_score_is_normalized_and_ignores_ineligible_trials() {
        let one_replication = RobustOutcomeSummary {
            requested: 1,
            scored: 0,
            missed: 1,
            indeterminate: 0,
            failed: 0,
            success_rate: Some(0.0),
            confidence_low: Some(0.0),
            confidence_high: Some(1.0),
            eligible: true,
        };
        let many_replications = RobustOutcomeSummary {
            requested: 32,
            scored: 17,
            missed: 15,
            indeterminate: 0,
            failed: 0,
            success_rate: Some(17.0 / 32.0),
            confidence_low: Some(0.0),
            confidence_high: Some(1.0),
            eligible: true,
        };
        let fitness = 0.73;
        assert!(
            (cross_entropy_score(&one_replication, fitness)
                - cross_entropy_score(&many_replications, fitness))
            .abs()
                <= f64::EPSILON
        );

        let mut ineligible = many_replications;
        ineligible.eligible = false;
        assert!(cross_entropy_score(&ineligible, fitness).is_nan());
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
        if let Some(winner) = first.winner() {
            assert!(winner
                .validation
                .as_ref()
                .is_some_and(|summary| summary.scored > 0));
        }
    }

    #[test]
    fn cross_entropy_schedule_and_normalized_mapping_preserve_domain_geometry() {
        assert_eq!(
            cross_entropy_proposal_kind(0),
            CrossEntropyProposalKind::Center
        );
        assert_eq!(
            cross_entropy_proposal_kind(1),
            CrossEntropyProposalKind::Center
        );
        assert_eq!(
            cross_entropy_proposal_kind(2),
            CrossEntropyProposalKind::Distribution
        );
        assert_eq!(
            cross_entropy_proposal_kind(5),
            CrossEntropyProposalKind::Global
        );
        assert_eq!(
            cross_entropy_proposal_kind(6),
            CrossEntropyProposalKind::Distribution
        );
        assert_eq!(
            cross_entropy_proposal_kind(11),
            CrossEntropyProposalKind::Global
        );

        let bounds = [
            RobustControlBounds::new(0.0, 360.0),
            RobustControlBounds::new(1.0, 300.0),
            RobustControlBounds::new(-0.45, 0.45),
            RobustControlBounds::new(-0.45, 0.45),
            RobustControlBounds::new(10.0, 10.0),
        ];
        assert_eq!(
            cross_entropy_dimensions(bounds)[0],
            CrossEntropyDimension::Circular
        );
        let controls = RobustShotControls {
            heading: 90.0,
            speed: 100.0,
            tip_side: -0.1,
            tip_height: 0.1,
            elevation: 10.0,
        };
        let normalized = normalize_search_controls(controls, bounds);
        assert!((normalized[0] - 0.25).abs() <= f64::EPSILON);
        assert!((normalized[4] - 0.5).abs() <= f64::EPSILON);
        assert!(normalized
            .into_iter()
            .all(|coordinate| (0.0..=1.0).contains(&coordinate)));

        let reconstructed = denormalize_search_controls(normalized, bounds);
        for (actual, expected) in reconstructed
            .as_array()
            .into_iter()
            .zip(controls.as_array())
        {
            assert!((actual - expected).abs() <= 1.0e-12);
        }
        let wrapped = RobustShotControls {
            heading: 360.0,
            ..controls
        };
        assert!(normalize_search_controls(wrapped, bounds)[0].abs() <= f64::EPSILON);
    }

    #[test]
    fn shotless_search_discovers_and_validates_a_scoring_stroke() {
        let source = concat!(
            "table three_cushion_carom_10ft\n",
            "game three_cushion\n",
            "ball cue at (3.354, 3.309)\n",
            "ball yellow at (2.491, 5.838)\n",
            "ball red at (2.762, 3.888)\n",
            "cue_strike(default).mass_ratio(1.0).energy_loss(0.08)\n",
            "ball_ball(carom).normal_restitution(0.98).tangential_friction(0.05)\n",
            "rail_response(lively).normal_restitution(0.82).tangential_friction(0.82)\n",
            "rails(carom).default(lively)\n",
            "simulation(default)\n",
            " .collision_model(throw_aware)\n",
            " .ball_ball(carom)\n",
            " .rail_model(spin_aware)\n",
            " .rails(carom)\n",
            " .conditions(heated_carom)\n",
            " .max_events(24)\n",
            "trace(max_events: 24)\n",
        );
        let (scenario, controls, preferred_cue) =
            match crate::dsl::scenario_controls_and_preferred_cue_from_dsl(source) {
                Ok(parsed) => parsed,
                Err(error) => panic!("shotless fixture DSL failed: {error}"),
            };
        assert_eq!(controls, None);
        let simulation = match scenario.preferred_simulation_physics(
            &crate::human_tuned_preview_motion_config(),
            crate::CollisionModel::ThrowAware,
            crate::RailModel::SpinAware,
        ) {
            Ok(simulation) => simulation,
            Err(error) => panic!("shotless fixture physics failed: {error}"),
        };
        let physics = match PhysicsProfile::new(
            scenario.game_state.table_spec.clone(),
            scenario.ball_set_physics_spec(),
            simulation.motion,
            simulation.collision_model,
            simulation.collision_config,
            simulation.rail_model,
            simulation.rail_profile,
        ) {
            Ok(physics) => physics,
            Err(error) => panic!("shotless fixture profile failed: {error}"),
        };
        let Some(cue) = preferred_cue else {
            panic!("shotless fixture lost its default cue");
        };
        let layout = match ShotLayout::three_cushion_from_diamonds(
            (3.354, 3.309),
            (2.491, 5.838),
            (2.762, 3.888),
        ) {
            Ok(layout) => layout,
            Err(error) => panic!("shotless fixture layout failed: {error}"),
        };
        let report = match run_player_robust_search(&PlayerRobustSearchRequest {
            physics,
            layout,
            cue,
            shooter: ThreeCushionShooter::Cue,
            current_controls: RobustShotControls {
                heading: 0.0,
                speed: 150.0,
                tip_side: 0.0,
                tip_height: 0.0,
                elevation: 0.0,
            },
            requested_evaluations: 256,
            player_level: ThreeCushionPlayerLevel::Pro,
            max_events: 24,
        }) {
            Ok(report) => report,
            Err(error) => panic!("shotless adaptive search failed: {error}"),
        };
        let Some(winner) = report.winner() else {
            panic!("shotless adaptive search found no scoring winner");
        };
        let Some(validation) = &winner.validation else {
            panic!("shotless adaptive search winner was not validated");
        };

        assert_eq!(report.actual_evaluations(), 256);
        assert!(validation.scored > 0);
    }
}
