use std::num::NonZeroUsize;
use std::{fmt, str::FromStr};

use clap::{Args, Parser};

use crate::{
    Bounds, Controls, ExperimentConfig, Mode, NoiseSigmas, PerturbationWidths, Position, Shooter,
};

#[derive(Clone, Debug, Parser)]
#[command(
    name = "simul-three-cushion",
    about = "Seeded direct-Rust three-cushion sensitivity and search experiments"
)]
pub struct Cli {
    /// Use a deterministic built-in legal-carom fixture.
    #[arg(long, conflicts_with_all = ["white", "yellow", "red"])]
    pub fixture: bool,

    /// Cue ball for this turn.
    #[arg(long, value_enum, default_value_t = Shooter::White)]
    pub shooter: Shooter,

    /// White ball position in carom-table diamond coordinates as X,Y.
    #[arg(long, value_name = "X,Y", required_unless_present = "fixture")]
    pub white: Option<PositionArg>,

    /// Yellow ball position in carom-table diamond coordinates as X,Y.
    #[arg(long, value_name = "X,Y", required_unless_present = "fixture")]
    pub yellow: Option<PositionArg>,

    /// Red ball position in carom-table diamond coordinates as X,Y.
    #[arg(long, value_name = "X,Y", required_unless_present = "fixture")]
    pub red: Option<PositionArg>,

    #[arg(long, value_enum)]
    pub mode: Mode,

    #[command(flatten)]
    pub controls: ControlArgs,

    #[command(flatten)]
    pub perturb: PerturbationArgs,

    #[command(flatten)]
    pub noise: NoiseArgs,

    #[command(flatten)]
    pub search_bounds: SearchBoundArgs,

    /// Master seed for proposals and named trial-noise streams.
    #[arg(long)]
    pub seed: u64,

    /// Number of nominal, explicit, and generated candidates.
    #[arg(long, default_value_t = 16)]
    pub candidates: usize,

    /// Number of screening executions per candidate.
    #[arg(long, default_value_t = 32)]
    pub screening_replications: u32,

    /// Maximum screening-eligible candidates evaluated on held-out trials.
    #[arg(long, default_value_t = 4)]
    pub finalists: usize,

    /// Number of held-out validation executions per finalist.
    #[arg(long, default_value_t = 256)]
    pub validation_replications: u32,

    /// Number of threads in the bounded outer trial pool.
    #[arg(long, default_value_t = NonZeroUsize::MIN)]
    pub workers: NonZeroUsize,

    /// Additional explicit candidates as heading,speed,side,height,elevation.
    #[arg(
        long = "good",
        value_name = "H,S,X,Y,E",
        value_parser = parse_controls
    )]
    pub good: Vec<Controls>,

    /// Maximum physics events before a trial becomes indeterminate.
    #[arg(long, default_value_t = 64)]
    pub max_events: usize,
}

#[derive(Clone, Copy, Debug, Args)]
pub struct ControlArgs {
    /// Heading in degrees clockwise from table north.
    #[arg(long, default_value_t = 196.391_792_039)]
    pub heading: f64,
    /// Requested cue-ball launch speed in inches per second.
    #[arg(long, default_value_t = 237.947_968_822)]
    pub speed: f64,
    /// Horizontal tip offset in ball-radius units.
    #[arg(long, default_value_t = -0.230_661_681)]
    pub tip_side: f64,
    /// Vertical tip offset in ball-radius units.
    #[arg(long, default_value_t = 0.365_060_077)]
    pub tip_height: f64,
    /// Cue elevation above the cloth in degrees.
    #[arg(long, default_value_t = 0.0)]
    pub elevation: f64,
}

#[derive(Clone, Copy, Debug, Args)]
pub struct PerturbationArgs {
    #[arg(long, default_value_t = 0.0)]
    pub heading_perturb: f64,
    #[arg(long, default_value_t = 0.0)]
    pub speed_perturb: f64,
    #[arg(long, default_value_t = 0.0)]
    pub tip_side_perturb: f64,
    #[arg(long, default_value_t = 0.0)]
    pub tip_height_perturb: f64,
    #[arg(long, default_value_t = 0.0)]
    pub elevation_perturb: f64,
}

#[derive(Clone, Copy, Debug, Args)]
pub struct NoiseArgs {
    /// Parent Gaussian sigma; draws are conditioned to [-3σ,+3σ].
    /// The realized standard deviation is about 0.9866σ.
    #[arg(long, default_value_t = 0.0)]
    pub heading_sigma: f64,
    /// Parent Gaussian sigma; draws are conditioned to [-3σ,+3σ].
    /// The realized standard deviation is about 0.9866σ.
    #[arg(long, default_value_t = 0.0)]
    pub speed_sigma: f64,
    /// Parent Gaussian sigma; draws are conditioned to [-3σ,+3σ].
    /// The realized standard deviation is about 0.9866σ.
    #[arg(long, default_value_t = 0.0)]
    pub tip_side_sigma: f64,
    /// Parent Gaussian sigma; draws are conditioned to [-3σ,+3σ].
    /// The realized standard deviation is about 0.9866σ.
    #[arg(long, default_value_t = 0.0)]
    pub tip_height_sigma: f64,
    /// Parent Gaussian sigma; draws are conditioned to [-3σ,+3σ].
    /// The realized standard deviation is about 0.9866σ.
    #[arg(long, default_value_t = 0.0)]
    pub elevation_sigma: f64,
}

#[derive(Clone, Copy, Debug, Default, Args)]
pub struct SearchBoundArgs {
    #[arg(long, value_name = "MIN,MAX")]
    pub heading_bounds: Option<BoundsArg>,
    #[arg(long, value_name = "MIN,MAX")]
    pub speed_bounds: Option<BoundsArg>,
    #[arg(long, value_name = "MIN,MAX")]
    pub tip_side_bounds: Option<BoundsArg>,
    #[arg(long, value_name = "MIN,MAX")]
    pub tip_height_bounds: Option<BoundsArg>,
    #[arg(long, value_name = "MIN,MAX")]
    pub elevation_bounds: Option<BoundsArg>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PositionArg(pub Position);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundsArg(pub Bounds);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseTupleError(&'static str);

impl fmt::Display for ParseTupleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for ParseTupleError {}

fn parse_finite_fields<const N: usize>(value: &str) -> Result<[f64; N], ParseTupleError> {
    let fields = value.split(',').map(str::trim);
    if fields.clone().count() != N {
        return Err(ParseTupleError("wrong number of comma-separated values"));
    }
    let mut parsed = [0.0_f64; N];
    for (target, field) in parsed.iter_mut().zip(fields) {
        *target = field
            .parse()
            .map_err(|_| ParseTupleError("values must be numbers"))?;
        if !(*target).is_finite() {
            return Err(ParseTupleError("values must be finite"));
        }
    }
    Ok(parsed)
}

impl FromStr for PositionArg {
    type Err = ParseTupleError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let [x, y] = parse_finite_fields(value)?;
        Ok(Self(Position { x, y }))
    }
}

fn parse_controls(value: &str) -> Result<Controls, ParseTupleError> {
    let [heading, speed, tip_side, tip_height, elevation] = parse_finite_fields(value)?;
    Ok(Controls {
        heading,
        speed,
        tip_side,
        tip_height,
        elevation,
    })
}

impl FromStr for BoundsArg {
    type Err = ParseTupleError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let [minimum, maximum] = parse_finite_fields(value)?;
        if minimum > maximum {
            return Err(ParseTupleError("bound minimum must not exceed maximum"));
        }
        Ok(Self(Bounds { minimum, maximum }))
    }
}

impl Cli {
    pub fn into_config(self) -> Result<ExperimentConfig, String> {
        let fixture = crate::known_carom_fixture();
        let positions = if self.fixture {
            fixture.positions
        } else {
            [
                self.white.ok_or("--white is required")?.0,
                self.yellow.ok_or("--yellow is required")?.0,
                self.red.ok_or("--red is required")?.0,
            ]
        };
        let nominal = Controls {
            heading: self.controls.heading,
            speed: self.controls.speed,
            tip_side: self.controls.tip_side,
            tip_height: self.controls.tip_height,
            elevation: self.controls.elevation,
        };
        let perturbations = PerturbationWidths {
            heading: self.perturb.heading_perturb,
            speed: self.perturb.speed_perturb,
            tip_side: self.perturb.tip_side_perturb,
            tip_height: self.perturb.tip_height_perturb,
            elevation: self.perturb.elevation_perturb,
        };
        let shot_inaccuracy = NoiseSigmas {
            heading: self.noise.heading_sigma,
            speed: self.noise.speed_sigma,
            tip_side: self.noise.tip_side_sigma,
            tip_height: self.noise.tip_height_sigma,
            elevation: self.noise.elevation_sigma,
        };
        let search_bounds = self.search_bounds.resolve(nominal, perturbations)?;
        let config = ExperimentConfig {
            mode: self.mode,
            shooter: self.shooter,
            positions,
            nominal,
            additional_candidates: self.good,
            perturbations,
            shot_inaccuracy,
            search_bounds,
            master_seed: self.seed,
            candidate_budget: self.candidates,
            screening_replication_budget: self.screening_replications,
            finalist_budget: self.finalists,
            validation_replication_budget: self.validation_replications,
            workers: self.workers,
            max_events: self.max_events,
        };
        config.validate()?;
        Ok(config)
    }
}

impl SearchBoundArgs {
    fn resolve(
        self,
        nominal: Controls,
        perturb: PerturbationWidths,
    ) -> Result<[Bounds; 5], String> {
        if perturb
            .as_array()
            .into_iter()
            .any(|width| !width.is_finite() || width < 0.0)
        {
            return Err("perturbation widths must be finite and non-negative".into());
        }

        let bounds = [
            self.heading_bounds.map_or_else(
                || Bounds::around_or(nominal.heading, perturb.heading, 0.0, 360.0),
                |value| value.0,
            ),
            self.speed_bounds.map_or_else(
                || Bounds::around_or(nominal.speed, perturb.speed, 1.0, 300.0),
                |value| value.0,
            ),
            self.tip_side_bounds
                .map_or(Bounds::new(-0.3, 0.3), |value| value.0),
            self.tip_height_bounds
                .map_or(Bounds::new(-0.05, 0.4), |value| value.0),
            self.elevation_bounds.map_or_else(
                || Bounds::around_or(nominal.elevation, perturb.elevation, 0.0, 45.0),
                |value| value.0,
            ),
        ];
        if bounds.iter().any(|bound| {
            !bound.minimum.is_finite()
                || !bound.maximum.is_finite()
                || bound.minimum > bound.maximum
                || !(bound.maximum - bound.minimum).is_finite()
        }) {
            return Err("search bounds and spans must be finite and ordered".into());
        }
        Ok(bounds)
    }
}
