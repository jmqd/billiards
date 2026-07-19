use std::num::NonZeroUsize;

use std::{borrow::Cow, fmt, io};

use clap::ValueEnum;

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
pub struct NoiseWidths {
    pub heading: f64,
    pub speed: f64,
    pub tip_side: f64,
    pub tip_height: f64,
    pub elevation: f64,
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
    pub sensitivity_centers: Vec<Controls>,
    pub perturbations: NoiseWidths,
    pub execution_noise: NoiseWidths,
    /// Heading, speed, side, height, and elevation bounds, in that order.
    pub search_bounds: [Bounds; 5],
    pub master_seed: u64,
    pub candidate_budget: u64,
    pub replication_budget: u32,
    pub workers: NonZeroUsize,
    pub max_events: usize,
}

impl ExperimentConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.candidate_budget == 0 {
            return Err("candidate budget must be greater than zero".into());
        }
        if self.mode == Mode::Sensitivity && self.sensitivity_centers.is_empty() {
            return Err("sensitivity mode requires at least one center".into());
        }
        if self.mode == Mode::Sensitivity
            && self.candidate_budget < self.sensitivity_centers.len() as u64
        {
            return Err("candidate budget must cover every supplied sensitivity center".into());
        }
        if self.replication_budget == 0 {
            return Err("replication budget must be greater than zero".into());
        }
        if self.max_events == 0 {
            return Err("max events must be greater than zero".into());
        }
        validate_controls(self.nominal)?;
        for position in self.positions {
            if !position.x.is_finite() || !position.y.is_finite() {
                return Err("ball positions must be finite".into());
            }
        }
        for (label, widths) in [
            ("perturbation", self.perturbations),
            ("noise", self.execution_noise),
        ] {
            for width in widths.as_array() {
                if !width.is_finite() || width < 0.0 {
                    return Err(format!("{label} widths must be finite and non-negative"));
                }
            }
        }
        for bounds in self.search_bounds {
            if !bounds.minimum.is_finite()
                || !bounds.maximum.is_finite()
                || bounds.minimum > bounds.maximum
            {
                return Err("search bounds must be finite and ordered".into());
            }
        }
        for center in &self.sensitivity_centers {
            validate_controls(*center)?;
        }
        Ok(())
    }
}

impl NoiseWidths {
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

fn validate_controls(controls: Controls) -> Result<(), String> {
    for (name, value) in [
        ("heading", controls.heading),
        ("speed", controls.speed),
        ("tip side", controls.tip_side),
        ("tip height", controls.tip_height),
        ("elevation", controls.elevation),
    ] {
        if !value.is_finite() {
            return Err(format!("{name} must be finite"));
        }
    }
    if controls.speed <= 0.0 {
        return Err("launch speed must be greater than zero".into());
    }
    if controls.elevation < 0.0 || controls.elevation >= 90.0 {
        return Err("elevation must be in [0, 90) degrees".into());
    }
    validate_tip(controls)
}

pub fn validate_tip(controls: Controls) -> Result<(), String> {
    if !controls.tip_side.is_finite() {
        return Err("tip side must be finite".into());
    }
    if !controls.tip_height.is_finite() {
        return Err("tip height must be finite".into());
    }
    if controls.tip_side.hypot(controls.tip_height) > 1.0 + 1e-12 {
        return Err("tip side/height must lie within one ball radius".into());
    }
    Ok(())
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrialDisposition {
    Scored,
    Miss(String),
    Indeterminate(String),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrialReport {
    pub candidate_id: u64,
    pub replication_id: u32,
    pub replay_key: String,
    pub applied: Controls,
    pub disposition: TrialDisposition,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CandidateReport {
    pub rank: Option<usize>,
    pub candidate_id: u64,
    pub controls: Controls,
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
pub struct ExperimentReport {
    pub mode: Mode,
    pub shooter: Shooter,
    pub master_seed: u64,
    pub seed_protocol: &'static str,
    pub candidates: Vec<CandidateReport>,
    pub trials: Vec<TrialReport>,
}

impl ExperimentReport {
    pub fn write_to(&self, mut output: impl io::Write) -> io::Result<()> {
        writeln!(
            output,
            "META,mode={},shooter={},master_seed={},seed_protocol={},candidate_count={},trial_count={}",
            self.mode,
            self.shooter,
            self.master_seed,
            self.seed_protocol,
            self.candidates.len(),
            self.trials.len()
        )?;
        writeln!(output, "CANDIDATE,rank,id,heading_deg,speed_ips,tip_side_r,tip_height_r,elevation_deg,requested,scored,missed,indeterminate,failed,success_rate,confidence_low,confidence_high,eligible")?;
        for candidate in &self.candidates {
            writeln!(
                output,
                "CANDIDATE,{},{},{:.9},{:.9},{:.9},{:.9},{:.9},{},{},{},{},{},{},{},{},{}",
                display_option_usize(candidate.rank),
                candidate.candidate_id,
                candidate.controls.heading,
                candidate.controls.speed,
                candidate.controls.tip_side,
                candidate.controls.tip_height,
                candidate.controls.elevation,
                candidate.requested,
                candidate.scored,
                candidate.missed,
                candidate.indeterminate,
                candidate.failed,
                display_option_f64(candidate.success_rate),
                display_option_f64(candidate.confidence_low),
                display_option_f64(candidate.confidence_high),
                candidate.eligible
            )?;
        }
        writeln!(output, "TRIAL,candidate_id,replication_id,replay_key,heading_deg,speed_ips,tip_side_r,tip_height_r,elevation_deg,outcome,detail")?;
        for trial in &self.trials {
            let (outcome, detail) = match &trial.disposition {
                TrialDisposition::Scored => ("scored", ""),
                TrialDisposition::Miss(detail) => ("miss", detail.as_str()),
                TrialDisposition::Indeterminate(detail) => ("indeterminate", detail.as_str()),
                TrialDisposition::Failed(detail) => ("failed", detail.as_str()),
            };
            writeln!(
                output,
                "TRIAL,{},{},{},{:.9},{:.9},{:.9},{:.9},{:.9},{},{}",
                trial.candidate_id,
                trial.replication_id,
                csv_field(&trial.replay_key),
                trial.applied.heading,
                trial.applied.speed,
                trial.applied.tip_side,
                trial.applied.tip_height,
                trial.applied.elevation,
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

fn csv_field(value: &str) -> Cow<'_, str> {
    if value.contains([',', '"', '\n', '\r']) {
        Cow::Owned(format!("\"{}\"", value.replace('"', "\"\"")))
    } else {
        Cow::Borrowed(value)
    }
}
