use std::num::NonZeroUsize;
use std::{borrow::Cow, fmt, io};

use billiards::shot_simulation::{
    robust_controls_within_bounds, validate_robust_controls, validate_robust_noise_envelope,
    validate_robust_noise_sigmas, validate_robust_perturbations, validate_robust_search_bounds,
    ReplayKey, RobustControlBounds, RobustNoiseSigmas, RobustPerturbationWidths,
    RobustShotControls,
};
use clap::ValueEnum;

pub(crate) const MAX_PROPOSAL_ATTEMPTS_PER_CANDIDATE: u64 =
    billiards::shot_simulation::ROBUST_MAX_PROPOSAL_ATTEMPTS_PER_CANDIDATE;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Shooter {
    White,
    Yellow,
}

impl fmt::Display for Shooter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::White => "white",
            Self::Yellow => "yellow",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Mode {
    Sensitivity,
    Search,
}

impl fmt::Display for Mode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Sensitivity => "sensitivity",
            Self::Search => "search",
        })
    }
}

/// A location in carom-table diamond coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Controls {
    pub heading: f64,
    pub speed: f64,
    pub tip_side: f64,
    pub tip_height: f64,
    pub elevation: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerturbationWidths {
    pub heading: f64,
    pub speed: f64,
    pub tip_side: f64,
    pub tip_height: f64,
    pub elevation: f64,
}

impl PerturbationWidths {
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
pub struct NoiseSigmas {
    pub heading: f64,
    pub speed: f64,
    pub tip_side: f64,
    pub tip_height: f64,
    pub elevation: f64,
}

impl NoiseSigmas {
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
pub struct Bounds {
    pub minimum: f64,
    pub maximum: f64,
}

impl Bounds {
    pub const fn new(minimum: f64, maximum: f64) -> Self {
        Self { minimum, maximum }
    }

    pub fn around_or(center: f64, width: f64, default_minimum: f64, default_maximum: f64) -> Self {
        if width == 0.0 {
            Self::new(default_minimum, default_maximum)
        } else {
            Self::new(center - width, center + width)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExperimentConfig {
    pub mode: Mode,
    pub shooter: Shooter,
    /// White, yellow, and red diamond-coordinate positions, in that order.
    pub positions: [Position; 3],
    pub nominal: Controls,
    pub additional_candidates: Vec<Controls>,
    pub perturbations: PerturbationWidths,
    pub shot_inaccuracy: NoiseSigmas,
    /// Heading, speed, side, height, and elevation bounds, in that order.
    pub search_bounds: [Bounds; 5],
    pub master_seed: u64,
    pub candidate_budget: usize,
    pub screening_replication_budget: u32,
    pub finalist_budget: usize,
    pub validation_replication_budget: u32,
    pub workers: NonZeroUsize,
    pub max_events: usize,
}

impl ExperimentConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.candidate_budget == 0 {
            return Err("candidate budget must be greater than zero".into());
        }
        if self.screening_replication_budget == 0 {
            return Err("screening replication budget must be greater than zero".into());
        }
        if self.finalist_budget == 0 {
            return Err("finalist budget must be greater than zero".into());
        }
        if self.validation_replication_budget == 0 {
            return Err("validation replication budget must be greater than zero".into());
        }
        if self.max_events == 0 {
            return Err("max events must be greater than zero".into());
        }

        let required_candidates = self
            .additional_candidates
            .len()
            .checked_add(1)
            .ok_or_else(|| "additional candidate count overflowed platform size".to_owned())?;
        if self.candidate_budget < required_candidates {
            return Err(
                "candidate budget must cover the nominal shot and every supplied candidate".into(),
            );
        }
        let greatest_candidate_index = self.candidate_budget - 1;
        let greatest_candidate_id = u64::try_from(greatest_candidate_index)
            .map_err(|_| "candidate budget exceeds the u64 candidate-ID space".to_owned())?;
        greatest_candidate_id
            .checked_mul(MAX_PROPOSAL_ATTEMPTS_PER_CANDIDATE)
            .ok_or_else(|| "candidate proposal sample ID would overflow u64".to_owned())?;

        for position in self.positions {
            if !position.x.is_finite() || !position.y.is_finite() {
                return Err("ball positions must be finite".into());
            }
        }
        validate_robust_perturbations(robust_perturbations(self.perturbations))
            .map_err(|error| error.to_string())?;
        validate_robust_noise_sigmas(robust_sigmas(self.shot_inaccuracy))
            .map_err(|error| error.to_string())?;
        validate_robust_search_bounds(self.search_bounds.map(robust_bounds))
            .map_err(|error| error.to_string())?;

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

fn validate_configured_candidate(
    label: &str,
    controls: Controls,
    mode: Mode,
    search_bounds: [Bounds; 5],
    sigmas: NoiseSigmas,
) -> Result<(), String> {
    let controls = robust_controls(controls);
    validate_robust_controls(controls).map_err(|error| format!("{label}: {error}"))?;
    if mode == Mode::Search
        && !robust_controls_within_bounds(controls, search_bounds.map(robust_bounds))
    {
        return Err(format!("{label} must lie within every search bound"));
    }
    validate_robust_noise_envelope(controls, robust_sigmas(sigmas))
        .map_err(|error| format!("{label} has a non-executable ±3σ envelope: {error}"))
}

pub(crate) const fn robust_controls(controls: Controls) -> RobustShotControls {
    RobustShotControls {
        heading: controls.heading,
        speed: controls.speed,
        tip_side: controls.tip_side,
        tip_height: controls.tip_height,
        elevation: controls.elevation,
    }
}

pub(crate) const fn robust_perturbations(widths: PerturbationWidths) -> RobustPerturbationWidths {
    RobustPerturbationWidths {
        heading: widths.heading,
        speed: widths.speed,
        tip_side: widths.tip_side,
        tip_height: widths.tip_height,
        elevation: widths.elevation,
    }
}

pub(crate) const fn robust_sigmas(sigmas: NoiseSigmas) -> RobustNoiseSigmas {
    RobustNoiseSigmas {
        heading: sigmas.heading,
        speed: sigmas.speed,
        tip_side: sigmas.tip_side,
        tip_height: sigmas.tip_height,
        elevation: sigmas.elevation,
    }
}

pub(crate) const fn robust_bounds(bounds: Bounds) -> RobustControlBounds {
    RobustControlBounds::new(bounds.minimum, bounds.maximum)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnownFixture {
    pub positions: [Position; 3],
    pub controls: Controls,
}

pub const fn known_carom_fixture() -> KnownFixture {
    KnownFixture {
        positions: [
            Position { x: 0.700, y: 1.000 },
            Position { x: 1.200, y: 2.100 },
            Position { x: 0.850, y: 6.550 },
        ],
        controls: Controls {
            heading: 196.391_792_039,
            speed: 237.947_968_822,
            tip_side: -0.230_661_681,
            tip_height: 0.365_060_077,
            elevation: 0.0,
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrialStage {
    Screening,
    Validation,
}

impl fmt::Display for TrialStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Screening => "screening",
            Self::Validation => "validation",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrialDisposition {
    Scored,
    Miss(String),
    Indeterminate(String),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct OutcomeSummary {
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
pub struct TrialReport {
    pub stage: TrialStage,
    pub candidate_id: u64,
    pub replication_id: u32,
    pub replay_key: ReplayKey,
    pub applied: Option<Controls>,
    pub disposition: TrialDisposition,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CandidateReport {
    pub rank: Option<usize>,
    pub candidate_id: u64,
    pub controls: Controls,
    pub screening: OutcomeSummary,
    pub validation: Option<OutcomeSummary>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExperimentReport {
    pub mode: Mode,
    pub shooter: Shooter,
    /// White, yellow, and red diamond-coordinate positions, in that order.
    pub positions: [Position; 3],
    pub perturbations: PerturbationWidths,
    pub shot_inaccuracy: NoiseSigmas,
    /// Heading, speed, side, height, and elevation bounds, in that order.
    pub search_bounds: [Bounds; 5],
    pub candidate_budget: usize,
    pub screening_replication_budget: u32,
    pub finalist_budget: usize,
    pub validation_replication_budget: u32,
    pub requested_workers: NonZeroUsize,
    pub max_events: usize,
    pub master_seed: u64,
    pub seed_protocol: &'static str,
    pub physics_profile: &'static str,
    pub noise_model: &'static str,
    pub noise_parent_sigma_limit: f64,
    pub selection_policy: &'static str,
    pub winner_id: Option<u64>,
    pub candidates: Vec<CandidateReport>,
    pub trials: Vec<TrialReport>,
}

impl ExperimentReport {
    pub fn winner(&self) -> Option<&CandidateReport> {
        self.candidates
            .iter()
            .find(|candidate| candidate.rank == Some(1))
    }

    pub fn write_to(&self, mut output: impl io::Write) -> io::Result<()> {
        writeln!(
            output,
            "META,mode={},shooter={},physics_profile={},master_seed={},seed_protocol={},search_proposal_domain=5345415243480001,search_screening_domain=5345415243480002,search_validation_domain=5345415243480003,sensitivity_proposal_domain=53454e5349540001,sensitivity_trial_domain=53454e5349540002,noise_model={},noise_parent_sigma_limit={:.9},selection_policy={},winner_id={},candidate_count={},trial_count={}",
            self.mode,
            self.shooter,
            self.physics_profile,
            self.master_seed,
            self.seed_protocol,
            self.noise_model,
            self.noise_parent_sigma_limit,
            self.selection_policy,
            display_option_u64(self.winner_id),
            self.candidates.len(),
            self.trials.len()
        )?;
        writeln!(
            output,
            "CONFIG,white_diamonds={:.9}:{:.9},yellow_diamonds={:.9}:{:.9},red_diamonds={:.9}:{:.9},proposal_widths={:.9}:{:.9}:{:.9}:{:.9}:{:.9},noise_sigmas={:.9}:{:.9}:{:.9}:{:.9}:{:.9},search_bounds={:.9}:{:.9};{:.9}:{:.9};{:.9}:{:.9};{:.9}:{:.9};{:.9}:{:.9},candidate_budget={},screening_replications={},finalist_budget={},validation_replications={},workers={},max_events={},proposal_attempt_limit={}",
            self.positions[0].x,
            self.positions[0].y,
            self.positions[1].x,
            self.positions[1].y,
            self.positions[2].x,
            self.positions[2].y,
            self.perturbations.heading,
            self.perturbations.speed,
            self.perturbations.tip_side,
            self.perturbations.tip_height,
            self.perturbations.elevation,
            self.shot_inaccuracy.heading,
            self.shot_inaccuracy.speed,
            self.shot_inaccuracy.tip_side,
            self.shot_inaccuracy.tip_height,
            self.shot_inaccuracy.elevation,
            self.search_bounds[0].minimum,
            self.search_bounds[0].maximum,
            self.search_bounds[1].minimum,
            self.search_bounds[1].maximum,
            self.search_bounds[2].minimum,
            self.search_bounds[2].maximum,
            self.search_bounds[3].minimum,
            self.search_bounds[3].maximum,
            self.search_bounds[4].minimum,
            self.search_bounds[4].maximum,
            self.candidate_budget,
            self.screening_replication_budget,
            self.finalist_budget,
            self.validation_replication_budget,
            self.requested_workers,
            self.max_events,
            MAX_PROPOSAL_ATTEMPTS_PER_CANDIDATE
        )?;
        writeln!(output, "CANDIDATE,rank,id,heading_deg,speed_ips,tip_side_r,tip_height_r,elevation_deg,screening_requested,screening_scored,screening_missed,screening_indeterminate,screening_failed,screening_success_rate,screening_confidence_low,screening_confidence_high,screening_eligible,validation_requested,validation_scored,validation_missed,validation_indeterminate,validation_failed,validation_success_rate,validation_confidence_low,validation_confidence_high,validation_eligible")?;
        for candidate in &self.candidates {
            let validation = candidate.validation.as_ref();
            writeln!(
                output,
                "CANDIDATE,{},{},{:.9},{:.9},{:.9},{:.9},{:.9},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                display_option_usize(candidate.rank),
                candidate.candidate_id,
                candidate.controls.heading,
                candidate.controls.speed,
                candidate.controls.tip_side,
                candidate.controls.tip_height,
                candidate.controls.elevation,
                candidate.screening.requested,
                candidate.screening.scored,
                candidate.screening.missed,
                candidate.screening.indeterminate,
                candidate.screening.failed,
                display_option_f64(candidate.screening.success_rate),
                display_option_f64(candidate.screening.confidence_low),
                display_option_f64(candidate.screening.confidence_high),
                candidate.screening.eligible,
                display_option_u32(validation.map(|summary| summary.requested)),
                display_option_u32(validation.map(|summary| summary.scored)),
                display_option_u32(validation.map(|summary| summary.missed)),
                display_option_u32(validation.map(|summary| summary.indeterminate)),
                display_option_u32(validation.map(|summary| summary.failed)),
                display_option_f64(validation.and_then(|summary| summary.success_rate)),
                display_option_f64(validation.and_then(|summary| summary.confidence_low)),
                display_option_f64(validation.and_then(|summary| summary.confidence_high)),
                display_option_bool(validation.map(|summary| summary.eligible))
            )?;
        }
        writeln!(output, "TRIAL,stage,candidate_id,replication_id,replay_key,heading_deg,speed_ips,tip_side_r,tip_height_r,elevation_deg,outcome,detail")?;
        for trial in &self.trials {
            let (outcome, detail) = match &trial.disposition {
                TrialDisposition::Scored => ("scored", ""),
                TrialDisposition::Miss(detail) => ("miss", detail.as_str()),
                TrialDisposition::Indeterminate(detail) => ("indeterminate", detail.as_str()),
                TrialDisposition::Failed(detail) => ("failed", detail.as_str()),
            };
            let applied = trial.applied;
            writeln!(
                output,
                "TRIAL,{},{},{},{},{},{},{},{},{},{},{}",
                trial.stage,
                trial.candidate_id,
                trial.replication_id,
                trial.replay_key,
                display_option_f64(applied.map(|controls| controls.heading)),
                display_option_f64(applied.map(|controls| controls.speed)),
                display_option_f64(applied.map(|controls| controls.tip_side)),
                display_option_f64(applied.map(|controls| controls.tip_height)),
                display_option_f64(applied.map(|controls| controls.elevation)),
                outcome,
                csv_field(detail)
            )?;
        }
        Ok(())
    }
}

fn display_option_f64(value: Option<f64>) -> String {
    value.map_or_else(String::new, |number| format!("{number:.9}"))
}

fn display_option_usize(value: Option<usize>) -> String {
    value.map_or_else(String::new, |number| number.to_string())
}

fn display_option_u64(value: Option<u64>) -> String {
    value.map_or_else(String::new, |number| number.to_string())
}

fn display_option_u32(value: Option<u32>) -> String {
    value.map_or_else(String::new, |number| number.to_string())
}

fn display_option_bool(value: Option<bool>) -> String {
    value.map_or_else(String::new, |boolean| boolean.to_string())
}

fn csv_field(value: &str) -> Cow<'_, str> {
    if value.contains([',', '"', '\n', '\r']) {
        Cow::Owned(format!("\"{}\"", value.replace('"', "\"\"")))
    } else {
        Cow::Borrowed(value)
    }
}
