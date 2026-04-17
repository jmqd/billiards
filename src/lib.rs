mod assets;
mod drawing;
pub mod dsl;

use crate::assets::diamond_to_pixel;
use assets::ideal_ball_size_px;
use core::fmt;
use image::imageops::{resize, FilterType};
use image::Rgba;
use lazy_static::lazy_static;
use std::fs::File;
use std::io::Write;
use std::ops::{Add, Div, Mul, Neg, Sub};
use std::path::Path;
use std::str::FromStr;

use bigdecimal::BigDecimal;
use bigdecimal::FromPrimitive;
use bigdecimal::ToPrimitive;

lazy_static! {
    pub static ref DIAMOND_SIGHT_NOSE_OFFSET: Inches = Inches {
        magnitude: BigDecimal::from_str("3.6875").unwrap()
    };
    pub static ref OFFICIAL_DIAMOND_SIGHT_NOSE_OFFSET: Inches = Inches {
        magnitude: BigDecimal::from_str("3.6875").unwrap()
    };
    /// When optimally packing pool balls into a "frozen" configuration, each
    /// set of three balls forms an equilateral triangle from center <-> center
    /// <-> center with each side being 2R. From this, we know that for any 2
    /// adjacent pairs in the triple, drawing a line between their centers, the
    /// distance we must shift that line to go through the center of the third
    /// ball is a dimensionless factor of sqrt(3), applied to the ball radius.
    pub static ref OPTIMAL_PACKING_RADIUS_SHIFT: Scale = Scale {
        magnitude: BigDecimal::from_usize(3).unwrap().sqrt().unwrap()
    };
    pub static ref GC4_POCKET_DEPTH: Inches = Inches {
        magnitude: BigDecimal::from_str("1.4").unwrap()
    };
    pub static ref GC4_CORNER_POCKET_WIDTH: Inches = Inches {
        magnitude: BigDecimal::from_str("4.5").unwrap()
    };
    pub static ref GC4_SIDE_POCKET_WIDTH: Inches = Inches {
        magnitude: BigDecimal::from_str("5").unwrap()
    };
    pub static ref TYPICAL_BALL_RADIUS: Inches = Inches {
        magnitude: BigDecimal::from_str("1.125").unwrap()
    };
    pub static ref CENTER_SPOT: Position = Position {
        x: Diamond::from("2"),
        y: Diamond::from("4"),
        ..Default::default()
    };
    pub static ref TOP_RIGHT_DIAMOND: Position = Position {
        x: Diamond::from("4"),
        y: Diamond::from("8"),
        ..Default::default()
    };
    pub static ref CENTER_RIGHT_DIAMOND: Position = Position {
        x: Diamond::from("4"),
        y: Diamond::from("4"),
        ..Default::default()
    };
    pub static ref RACK_SPOT: Position = Position {
        x: Diamond::from("2"),
        y: Diamond::from("2"),
        ..Default::default()
    };
    pub static ref BOTTOM_RIGHT_DIAMOND: Position = Position {
        x: Diamond::from("4"),
        y: Diamond::from("0"),
        ..Default::default()
    };
    pub static ref BOTTOM_LEFT_DIAMOND: Position = Position {
        x: Diamond::from("0"),
        y: Diamond::from("0"),
        ..Default::default()
    };
    pub static ref CENTER_LEFT_DIAMOND: Position = Position {
        x: Diamond::from("0"),
        y: Diamond::from("4"),
        ..Default::default()
    };
    pub static ref TOP_LEFT_DIAMOND: Position = Position {
        x: Diamond::from("0"),
        y: Diamond::from("8"),
        ..Default::default()
    };
    pub static ref CORNER_AIMING_CENTER_DX: Diamond = Diamond::from(".07");
    pub static ref CORNER_AIMING_CENTER_DY: Diamond = Diamond::from(".07");
    pub static ref AIM_TOP_RIGHT_POCKET: Position =
        translate_inwards(&TOP_RIGHT_DIAMOND, CORNER_AIMING_CENTER_DX.clone(), CORNER_AIMING_CENTER_DY.clone());
    pub static ref AIM_BOTTOM_RIGHT_POCKET: Position =
        translate_inwards(&BOTTOM_RIGHT_DIAMOND, CORNER_AIMING_CENTER_DX.clone(), CORNER_AIMING_CENTER_DY.clone());
    pub static ref AIM_BOTTOM_LEFT_POCKET: Position =
        translate_inwards(&BOTTOM_LEFT_DIAMOND, CORNER_AIMING_CENTER_DX.clone(), CORNER_AIMING_CENTER_DY.clone());
    pub static ref AIM_TOP_LEFT_POCKET: Position =
        translate_inwards(&TOP_LEFT_DIAMOND, CORNER_AIMING_CENTER_DX.clone(), CORNER_AIMING_CENTER_DY.clone());
}

pub fn translate_inwards(origin: &Position, dx: Diamond, dy: Diamond) -> Position {
    let (x_direction, y_direction) = origin.direction_from_center();

    Position {
        x: match x_direction {
            PolarDirection::Positive => origin.x.clone() - dx,
            PolarDirection::Negative => origin.x.clone() + dx,
        },
        y: match y_direction {
            PolarDirection::Positive => origin.y.clone() - dy,
            PolarDirection::Negative => origin.y.clone() + dy,
        },
        ..Default::default()
    }
}

/// This is all normalized to a headstring-at-the-top top-down view.
/// 0°   = "up"
/// 90°  = "right"
/// 180° = "down"
/// 270° = "left"
///
/// This represents an absolute table-heading direction, not a cut-angle magnitude.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Angle(f64);

impl Angle {
    /// Return the angle measured clockwise from the positive-Y axis.
    pub fn from_north(dx: f64, dy: f64) -> Angle {
        let deg = dx.atan2(dy).to_degrees();
        Angle(if deg < 0.0 { deg + 360.0 } else { deg })
    }

    pub fn as_degrees(&self) -> f64 {
        self.0
    }

    /// Return the direction that points 180° opposite this one.
    pub fn flipped(&self) -> Self {
        Angle((self.0 + 180.0).rem_euclid(360.0))
    }
}

impl fmt::Display for Angle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The unsigned cut-angle magnitude `φ` at ball-ball impact, in degrees.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct CutAngle(f64);

impl CutAngle {
    /// Construct a cut-angle magnitude in degrees.
    pub fn new(degrees: f64) -> Self {
        assert!(
            (0.0..=90.0).contains(&degrees),
            "cut-angle magnitude must be in [0°, 90°], got {degrees}"
        );
        Self(degrees)
    }

    /// Derive the cut-angle magnitude from the cue-ball heading and the object-ball heading.
    ///
    /// The object-ball heading is the line-of-centers direction at impact; equivalently, for an
    /// ideal equal-mass collision, it is the object ball's immediate post-impact travel direction.
    pub fn from_headings(cue_ball_heading: Angle, object_ball_heading: Angle) -> Self {
        let difference = (cue_ball_heading.0 - object_ball_heading.0)
            .abs()
            .rem_euclid(360.0);
        let unsigned_between_rays = difference.min(360.0 - difference);
        let acute_line_angle = unsigned_between_rays.min(180.0 - unsigned_between_rays);

        Self(acute_line_angle)
    }

    pub fn as_degrees(&self) -> f64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
/// Represents the unit of distance on a pool table of "a diamond".
/// Going left-to-right, a diamond is 25% of the pool tables width.
/// Going top-down, a diamond is 1/8 (12.5%) of the tables length.
pub struct Diamond {
    pub magnitude: BigDecimal,
}

impl Diamond {
    pub fn zero() -> Self {
        Diamond {
            magnitude: BigDecimal::from_usize(0).unwrap(),
        }
    }

    pub fn one() -> Self {
        Diamond {
            magnitude: BigDecimal::from_usize(1).unwrap(),
        }
    }

    pub fn two() -> Self {
        Diamond {
            magnitude: BigDecimal::from_usize(2).unwrap(),
        }
    }

    pub fn three() -> Self {
        Diamond {
            magnitude: BigDecimal::from_usize(3).unwrap(),
        }
    }

    pub fn four() -> Self {
        Diamond {
            magnitude: BigDecimal::from_usize(4).unwrap(),
        }
    }

    pub fn five() -> Self {
        Diamond {
            magnitude: BigDecimal::from_usize(5).unwrap(),
        }
    }

    pub fn six() -> Self {
        Diamond {
            magnitude: BigDecimal::from_usize(6).unwrap(),
        }
    }

    pub fn seven() -> Self {
        Diamond {
            magnitude: BigDecimal::from_usize(7).unwrap(),
        }
    }

    pub fn eight() -> Self {
        Diamond {
            magnitude: BigDecimal::from_usize(8).unwrap(),
        }
    }

    pub fn double(self) -> Self {
        Self {
            magnitude: self.magnitude.double(),
        }
    }

    pub fn half(self) -> Self {
        Self {
            magnitude: self.magnitude.half(),
        }
    }

    pub fn inverse(self) -> Self {
        Self {
            magnitude: self.magnitude.inverse(),
        }
    }
}

impl Default for Diamond {
    fn default() -> Self {
        Self::zero()
    }
}

#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
/// A dimensionless scale factor.
pub struct Scale {
    pub magnitude: BigDecimal,
}

impl Scale {
    pub fn zero() -> Self {
        Self::from(0u8)
    }

    pub fn from_f64(magnitude: f64) -> Self {
        assert!(magnitude.is_finite(), "scale magnitude must be finite");
        Self {
            magnitude: BigDecimal::from_f64(magnitude).unwrap(),
        }
    }

    pub fn as_f64(&self) -> f64 {
        self.magnitude.to_f64().unwrap()
    }
}

#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
/// Our representation for converting to inches.
pub struct Inches {
    pub magnitude: BigDecimal,
}

impl Inches {
    pub fn zero() -> Self {
        Self::from(0u8)
    }

    pub fn from_f64(magnitude: f64) -> Self {
        assert!(magnitude.is_finite(), "inch magnitude must be finite");
        Self {
            magnitude: BigDecimal::from_f64(magnitude).unwrap(),
        }
    }

    pub fn as_f64(&self) -> f64 {
        self.magnitude.to_f64().unwrap()
    }

    pub fn double(self) -> Self {
        Self {
            magnitude: self.magnitude.double(),
        }
    }

    pub fn half(self) -> Self {
        Self {
            magnitude: self.magnitude.half(),
        }
    }
}

impl Neg for Inches {
    type Output = Inches;

    fn neg(self) -> Self {
        Self {
            magnitude: self.magnitude.neg(),
        }
    }
}

/// A measure of speed in terms of inches per second.
#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct InchesPerSecond {
    inches: Inches,
}

impl InchesPerSecond {
    pub fn new<I: Into<Inches>>(inches: I) -> Self {
        Self {
            inches: inches.into(),
        }
    }

    pub fn zero() -> Self {
        Self::new(Inches::zero())
    }

    pub fn from_mph(mph: f64) -> Self {
        Self::new(Inches::from_f64(mph * 17.6))
    }

    pub fn as_inches(&self) -> &Inches {
        &self.inches
    }

    pub fn as_f64(&self) -> f64 {
        self.inches.as_f64()
    }

    pub fn as_mph(&self) -> f64 {
        self.as_f64() / 17.6
    }
}

/// A measure of acceleration in terms of inches per second squared.
#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct InchesPerSecondSq {
    inches: Inches,
}

impl InchesPerSecondSq {
    pub fn new<I: Into<Inches>>(inches: I) -> Self {
        Self {
            inches: inches.into(),
        }
    }

    pub fn zero() -> Self {
        Self::new(Inches::zero())
    }

    pub fn as_inches(&self) -> &Inches {
        &self.inches
    }

    pub fn as_f64(&self) -> f64 {
        self.inches.as_f64()
    }
}

/// A measure of elapsed time in seconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Seconds(f64);

impl Seconds {
    pub fn new(seconds: f64) -> Self {
        assert!(seconds.is_finite(), "seconds must be finite");
        Self(seconds)
    }

    pub fn zero() -> Self {
        Self(0.0)
    }

    pub fn as_f64(&self) -> f64 {
        self.0
    }
}

impl From<f64> for Seconds {
    fn from(value: f64) -> Self {
        Self::new(value)
    }
}

/// A measure of angular velocity in terms of radians per second.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct RadiansPerSecond(f64);

impl RadiansPerSecond {
    pub fn new(radians_per_second: f64) -> Self {
        assert!(
            radians_per_second.is_finite(),
            "angular velocity must be finite"
        );
        Self(radians_per_second)
    }

    pub fn zero() -> Self {
        Self(0.0)
    }

    pub fn as_f64(&self) -> f64 {
        self.0
    }
}

impl From<f64> for RadiansPerSecond {
    fn from(value: f64) -> Self {
        Self::new(value)
    }
}

/// A measure of angular acceleration in terms of radians per second squared.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct RadiansPerSecondSq(f64);

impl RadiansPerSecondSq {
    pub fn new(radians_per_second_sq: f64) -> Self {
        assert!(
            radians_per_second_sq.is_finite(),
            "angular acceleration must be finite"
        );
        Self(radians_per_second_sq)
    }

    pub fn zero() -> Self {
        Self(0.0)
    }

    pub fn as_f64(&self) -> f64 {
        self.0
    }
}

impl From<f64> for RadiansPerSecondSq {
    fn from(value: f64) -> Self {
        Self::new(value)
    }
}

/// A 2D vector whose components are measured in inches.
#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct Inches2 {
    x: Inches,
    y: Inches,
}

impl Inches2 {
    pub fn new<X: Into<Inches>, Y: Into<Inches>>(x: X, y: Y) -> Self {
        Self {
            x: x.into(),
            y: y.into(),
        }
    }

    pub fn zero() -> Self {
        Self::new(Inches::zero(), Inches::zero())
    }

    pub fn x(&self) -> &Inches {
        &self.x
    }

    pub fn y(&self) -> &Inches {
        &self.y
    }

    pub fn magnitude(&self) -> Inches {
        Inches::from_f64((self.x.as_f64().powi(2) + self.y.as_f64().powi(2)).sqrt())
    }

    pub fn angle_from_north(&self) -> Option<Angle> {
        let x = self.x.as_f64();
        let y = self.y.as_f64();
        if x == 0.0 && y == 0.0 {
            None
        } else {
            Some(Angle::from_north(x, y))
        }
    }
}

/// A 2D linear velocity vector measured in inches per second.
#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct Velocity2 {
    x: InchesPerSecond,
    y: InchesPerSecond,
}

impl Velocity2 {
    pub fn new<X: Into<Inches>, Y: Into<Inches>>(x: X, y: Y) -> Self {
        Self::from_components(InchesPerSecond::new(x), InchesPerSecond::new(y))
    }

    pub fn from_components(x: InchesPerSecond, y: InchesPerSecond) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self::new(Inches::zero(), Inches::zero())
    }

    pub fn from_polar(speed: InchesPerSecond, angle: Angle) -> Self {
        let radians = angle.as_degrees().to_radians();
        Self::new(
            Inches::from_f64(speed.as_f64() * radians.sin()),
            Inches::from_f64(speed.as_f64() * radians.cos()),
        )
    }

    pub fn x(&self) -> &InchesPerSecond {
        &self.x
    }

    pub fn y(&self) -> &InchesPerSecond {
        &self.y
    }

    pub fn speed(&self) -> InchesPerSecond {
        InchesPerSecond::new(Inches::from_f64(
            (self.x.as_f64().powi(2) + self.y.as_f64().powi(2)).sqrt(),
        ))
    }

    pub fn angle_from_north(&self) -> Option<Angle> {
        let x = self.x.as_f64();
        let y = self.y.as_f64();
        if x == 0.0 && y == 0.0 {
            None
        } else {
            Some(Angle::from_north(x, y))
        }
    }

    pub fn displacement_over(&self, duration: Seconds) -> Inches2 {
        self.clone() * duration
    }
}

/// A 3-axis angular velocity vector measured in radians per second.
#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct AngularVelocity3 {
    x: RadiansPerSecond,
    y: RadiansPerSecond,
    z: RadiansPerSecond,
}

impl AngularVelocity3 {
    pub fn new<X: Into<RadiansPerSecond>, Y: Into<RadiansPerSecond>, Z: Into<RadiansPerSecond>>(
        x: X,
        y: Y,
        z: Z,
    ) -> Self {
        Self {
            x: x.into(),
            y: y.into(),
            z: z.into(),
        }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    pub fn x(&self) -> RadiansPerSecond {
        self.x
    }

    pub fn y(&self) -> RadiansPerSecond {
        self.y
    }

    pub fn z(&self) -> RadiansPerSecond {
        self.z
    }
}

/// The qualitative motion phase of a ball.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MotionPhase {
    Airborne,
    Sliding,
    Rolling,
    Spinning,
    Rest,
}

/// Shared physical parameters for a set of billiard balls.
#[derive(Clone, Debug, PartialEq)]
pub struct BallSetPhysicsSpec {
    pub radius: Inches,
}

impl Default for BallSetPhysicsSpec {
    fn default() -> Self {
        Self {
            radius: TYPICAL_BALL_RADIUS.clone(),
        }
    }
}

/// The currently supported ball-ball collision approximations.
///
/// `Ideal` is the equal-mass, perfectly elastic limit described by the local references:
///
/// - `whitepapers/art_of_billiards_play_files/bil_praa.html` notes that for two balls of the same
///   mass in a perfectly elastic collision, the velocity component normal to the point of contact
///   is exchanged.
/// - The same reference also states that when the struck ball is initially stationary, the moving
///   ball departs along the tangent line at contact.
///
/// `ThrowAware` adds a first-pass non-ideal tangential-slip model for cut-induced and spin-induced
/// throw while preserving `Ideal` as the zero-throw limiting case. It currently models only the
/// immediate post-impact translational deflection, not transferred spin or the later post-contact
/// cue-ball bend caused by residual top / bottom / side spin on the cloth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollisionModel {
    Ideal,
    ThrowAware,
    SpinFriction,
}

/// The currently supported ball-rail / cushion collision approximations.
///
/// `Mirror` is the ideal no-friction, perfectly elastic limit: the velocity component normal to the
/// rail reverses sign while the tangential component is preserved.
///
/// `RestitutionOnly` keeps the tangential component unchanged but scales the rebound in the rail-
/// normal direction by a configurable coefficient of restitution.
///
/// `SpinAware` combines configurable normal restitution with a tunable tangential friction model:
/// tangential rebound and z-spin (`ωz`, running / reverse english) are coupled through the in-plane
/// rail-contact slip, while top / draw (`ωx`, `ωy`) enter through the vertical component of the
/// rail-face contact slip. Partial-slip vs no-slip behavior is determined by the friction
/// coefficient.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailModel {
    Mirror,
    RestitutionOnly,
    SpinAware,
}

const DEFAULT_RAIL_NORMAL_RESTITUTION: f64 = 0.80;

/// Configurable coefficients for the current ball-rail response helpers.
///
/// `human_tuned()` is the current default: it keeps the cushion a little less lively than the
/// earlier placeholder default so multi-rail rebounds feel closer to ordinary table play. The
/// `tangential_friction_coefficient` is still the current first-pass coefficient `fi` from
/// `whitepapers/art_of_billiards_play_files/bil_praa.html`, §7.1.
#[derive(Clone, Debug, PartialEq)]
pub struct RailCollisionConfig {
    pub normal_restitution: Scale,
    pub tangential_friction_coefficient: Scale,
}

impl RailCollisionConfig {
    pub fn human_tuned() -> Self {
        Self {
            normal_restitution: Scale::from_f64(DEFAULT_RAIL_NORMAL_RESTITUTION),
            tangential_friction_coefficient: Scale::from_f64(1.0),
        }
    }
}

impl Default for RailCollisionConfig {
    fn default() -> Self {
        Self::human_tuned()
    }
}

/// Detailed output for a ball-ball collision response.
///
/// `throw_angle_degrees` is the signed deflection of the object-ball departure away from the ideal
/// line-of-centers direction, measured in degrees toward the positive collision-tangent basis used
/// internally by the solver.
///
/// `transferred_spin` is the angular-velocity increment transferred to the struck ball. In the
/// current first-pass non-ideal model this is limited to z-axis spin for the common cut-shot case
/// into an initially stationary object ball.
#[derive(Clone, Debug, PartialEq)]
pub struct CollisionOutcome {
    pub a_after: OnTableBallState,
    pub b_after: OnTableBallState,
    pub throw_angle_degrees: Option<f64>,
    pub transferred_spin: Option<AngularVelocity3>,
}

/// A collision outcome bundled with the current cue-ball bend estimate convenience data.
#[derive(Clone, Debug, PartialEq)]
pub struct CollisionAnalysis {
    pub outcome: CollisionOutcome,
    pub cue_ball_bend: Option<PostContactCueBallBend>,
}

/// A convenience handle for continuing analysis or scheduling from a cue ball's immediate
/// post-contact state.
///
/// This wraps a `CollisionOutcome` and roots follow-up queries at the cue-ball branch
/// `CollisionOutcome::a_after`, which is the immediate post-impact cue-ball state returned by the
/// current ball-ball collision helpers. It exists to make staged “resolve contact, then ask what
/// happens next” workflows feel like an explicit continuation rather than an ad hoc extraction of
/// `a_after`.
#[derive(Clone, Debug, PartialEq)]
pub struct PostContactContinuation {
    source_contact: CollisionOutcome,
}

/// A first-pass estimate of the cue ball's later cloth-driven bend after contact.
#[derive(Clone, Debug, PartialEq)]
pub struct PostContactCueBallBend {
    pub time_until_bend_completes: Seconds,
    pub bend_angle_degrees: f64,
    pub state_after_bend: OnTableBallState,
}

/// A first-pass estimate of the cue ball's later side-spin-driven curve after contact.
#[derive(Clone, Debug, PartialEq)]
pub struct PostContactCueBallCurve {
    pub time_until_curve_starts: Seconds,
    pub time_until_curve_completes: Seconds,
    pub curve_angle_degrees: f64,
    pub heading_after_curve: Angle,
}

/// A predicted future ball-ball impact between two on-table balls.
///
/// The stored states are the validated on-table states at the first predicted impact time, making
/// this result directly composable with `collide_ball_ball_on_table(...)`.
#[derive(Clone, Debug, PartialEq)]
pub struct PredictedBallBallCollision {
    pub time_until_impact: Seconds,
    pub a_at_impact: OnTableBallState,
    pub b_at_impact: OnTableBallState,
}

/// A predicted future impact between one on-table ball and a table rail.
#[derive(Clone, Debug, PartialEq)]
pub struct PredictedBallRailImpact {
    pub rail: Rail,
    pub time_until_impact: Seconds,
    pub state_at_impact: OnTableBallState,
}

/// A predicted future capture of one on-table ball by a pocket.
#[derive(Clone, Debug, PartialEq)]
pub struct PredictedBallPocketCapture {
    pub pocket: Pocket,
    pub time_until_capture: Seconds,
    pub state_at_capture: OnTableBallState,
}

/// Identifies one of the two balls in the current two-ball event scheduler helpers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TwoBallEventBall {
    A,
    B,
}

/// The next predicted event among two on-table balls under the currently implemented predictors.
///
/// This scheduler event is reused by both the rail-free and rail-aware two-ball helpers. When the
/// rail-aware scheduler is used, simultaneous events are currently broken deterministically in this
/// order: ball-ball collision first, then ball A rail impact, then ball B rail impact, then ball A
/// motion transition, then ball B motion transition.
#[derive(Clone, Debug, PartialEq)]
pub enum TwoBallOnTableEvent {
    BallBallCollision(PredictedBallBallCollision),
    BallRailImpact {
        ball: TwoBallEventBall,
        impact: PredictedBallRailImpact,
    },
    MotionTransition {
        ball: TwoBallEventBall,
        transition: NextTransition,
    },
}

impl TwoBallOnTableEvent {
    /// Return the elapsed time until this predicted event occurs.
    pub fn time(&self) -> Seconds {
        match self {
            TwoBallOnTableEvent::BallBallCollision(collision) => collision.time_until_impact,
            TwoBallOnTableEvent::BallRailImpact { impact, .. } => impact.time_until_impact,
            TwoBallOnTableEvent::MotionTransition { transition, .. } => {
                transition.time_until_transition
            }
        }
    }

    /// Return the single distinguished ball for this event when one exists.
    ///
    /// Ball-ball collisions involve both balls symmetrically, so they return `None`. Rail impacts
    /// and motion transitions identify the affected ball and return `Some(...)`.
    pub fn primary_ball(&self) -> Option<TwoBallEventBall> {
        match self {
            TwoBallOnTableEvent::BallBallCollision(_) => None,
            TwoBallOnTableEvent::BallRailImpact { ball, .. }
            | TwoBallOnTableEvent::MotionTransition { ball, .. } => Some(*ball),
        }
    }
}

/// The next predicted event among an indexed set of on-table balls under the currently
/// implemented predictors.
///
/// This is the indexed / N-ball analogue of `TwoBallOnTableEvent`. Simultaneous events are
/// currently broken deterministically in this order:
///
/// 1. ball-ball collision, ordered lexicographically by `(first_ball_index, second_ball_index)`
/// 2. ball-rail impact, ordered by `ball_index`
/// 3. motion transition, ordered by `ball_index`
#[derive(Clone, Debug, PartialEq)]
pub enum NBallOnTableEvent {
    BallBallCollision {
        first_ball_index: usize,
        second_ball_index: usize,
        collision: PredictedBallBallCollision,
    },
    BallRailImpact {
        ball_index: usize,
        impact: PredictedBallRailImpact,
    },
    MotionTransition {
        ball_index: usize,
        transition: NextTransition,
    },
}

impl NBallOnTableEvent {
    /// Return the elapsed time until this predicted event occurs.
    pub fn time(&self) -> Seconds {
        match self {
            NBallOnTableEvent::BallBallCollision { collision, .. } => collision.time_until_impact,
            NBallOnTableEvent::BallRailImpact { impact, .. } => impact.time_until_impact,
            NBallOnTableEvent::MotionTransition { transition, .. } => {
                transition.time_until_transition
            }
        }
    }

    /// Return the single distinguished ball for this event when one exists.
    ///
    /// Ball-ball collisions involve both indexed balls symmetrically, so they return `None`. Rail
    /// impacts and motion transitions identify the affected ball and return `Some(ball_index)`.
    pub fn primary_ball(&self) -> Option<usize> {
        match self {
            NBallOnTableEvent::BallBallCollision { .. } => None,
            NBallOnTableEvent::BallRailImpact { ball_index, .. }
            | NBallOnTableEvent::MotionTransition { ball_index, .. } => Some(*ball_index),
        }
    }
}

const TWO_BALL_A_INDEX: usize = 0;
const TWO_BALL_B_INDEX: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum NBallSystemEventSource {
    BallBallCollision {
        first_ball_index: usize,
        second_ball_index: usize,
    },
    BallRailImpact {
        ball_index: usize,
    },
    MotionTransition {
        ball_index: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct NBallSystemEventCandidate {
    source: NBallSystemEventSource,
    event: NBallOnTableEvent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum NBallPocketAwareSystemEventSource {
    BallBallCollision {
        first_ball_index: usize,
        second_ball_index: usize,
    },
    BallPocketCapture {
        ball_index: usize,
    },
    BallRailImpact {
        ball_index: usize,
    },
    MotionTransition {
        ball_index: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct NBallPocketAwareSystemEventCandidate {
    source: NBallPocketAwareSystemEventSource,
    event: NBallSystemEvent,
}

/// The result of advancing two on-table balls to the next currently supported event.
///
/// `event` records the chosen primary event, if any. Because the current scheduler intentionally
/// breaks ties deterministically instead of merging simultaneous events, the returned states can be
/// exactly on additional event boundaries that are not separately represented here.
#[derive(Clone, Debug, PartialEq)]
pub struct TwoBallOnTableAdvance {
    pub a: OnTableBallState,
    pub b: OnTableBallState,
    pub elapsed: Seconds,
    pub event: Option<TwoBallOnTableEvent>,
}

/// The result of advancing any number of on-table balls to the next currently supported event.
///
/// `event` records the chosen indexed primary event, if any. Because the current scheduler still
/// breaks ties deterministically instead of merging simultaneous contacts, the returned states can
/// land exactly on additional event boundaries that are not separately represented here.
#[derive(Clone, Debug, PartialEq)]
pub struct NBallOnTableAdvance {
    pub states: Vec<OnTableBallState>,
    pub elapsed: Seconds,
    pub event: Option<NBallOnTableEvent>,
}

/// The result of simulating two on-table balls forward over a requested duration.
///
/// `events` records the ordered sequence of primary events resolved while consuming `elapsed`.
#[derive(Clone, Debug, PartialEq)]
pub struct TwoBallOnTableSimulation {
    pub a: OnTableBallState,
    pub b: OnTableBallState,
    pub elapsed: Seconds,
    pub events: Vec<TwoBallOnTableEvent>,
}

/// The result of simulating any number of on-table balls until no further supported event remains.
///
/// `events` records the ordered sequence of indexed primary events resolved while consuming
/// `elapsed`.
#[derive(Clone, Debug, PartialEq)]
pub struct NBallOnTableSimulation {
    pub states: Vec<OnTableBallState>,
    pub elapsed: Seconds,
    pub events: Vec<NBallOnTableEvent>,
}

/// A ball state inside the richer indexed N-ball system simulation.
///
/// The current pocket-aware system simulation starts from on-table states and can remove balls into
/// a terminal pocketed state. Airborne / leaves-table handling is intentionally deferred.
#[derive(Clone, Debug, PartialEq)]
pub enum NBallSystemState {
    OnTable(OnTableBallState),
    Pocketed {
        pocket: Pocket,
        state_at_capture: OnTableBallState,
    },
}

impl From<OnTableBallState> for NBallSystemState {
    fn from(state: OnTableBallState) -> Self {
        Self::OnTable(state)
    }
}

impl NBallSystemState {
    pub fn as_on_table(&self) -> Option<&OnTableBallState> {
        match self {
            NBallSystemState::OnTable(state) => Some(state),
            NBallSystemState::Pocketed { .. } => None,
        }
    }
}

/// The next predicted event among an indexed set of balls in the richer system simulation.
#[derive(Clone, Debug, PartialEq)]
pub enum NBallSystemEvent {
    BallBallCollision {
        first_ball_index: usize,
        second_ball_index: usize,
        collision: PredictedBallBallCollision,
    },
    BallPocketCapture {
        ball_index: usize,
        capture: PredictedBallPocketCapture,
    },
    BallRailImpact {
        ball_index: usize,
        impact: PredictedBallRailImpact,
    },
    MotionTransition {
        ball_index: usize,
        transition: NextTransition,
    },
}

impl NBallSystemEvent {
    pub fn time(&self) -> Seconds {
        match self {
            NBallSystemEvent::BallBallCollision { collision, .. } => collision.time_until_impact,
            NBallSystemEvent::BallPocketCapture { capture, .. } => capture.time_until_capture,
            NBallSystemEvent::BallRailImpact { impact, .. } => impact.time_until_impact,
            NBallSystemEvent::MotionTransition { transition, .. } => {
                transition.time_until_transition
            }
        }
    }

    pub fn primary_ball(&self) -> Option<usize> {
        match self {
            NBallSystemEvent::BallBallCollision { .. } => None,
            NBallSystemEvent::BallPocketCapture { ball_index, .. }
            | NBallSystemEvent::BallRailImpact { ball_index, .. }
            | NBallSystemEvent::MotionTransition { ball_index, .. } => Some(*ball_index),
        }
    }
}

/// The result of advancing the richer indexed N-ball system to the next supported event.
#[derive(Clone, Debug, PartialEq)]
pub struct NBallSystemAdvance {
    pub states: Vec<NBallSystemState>,
    pub elapsed: Seconds,
    pub event: Option<NBallSystemEvent>,
}

/// The result of simulating the richer indexed N-ball system until no further supported event
/// remains.
#[derive(Clone, Debug, PartialEq)]
pub struct NBallSystemSimulation {
    pub states: Vec<NBallSystemState>,
    pub elapsed: Seconds,
    pub events: Vec<NBallSystemEvent>,
}

/// How long a traced single-ball rail path should continue.
#[derive(Clone, Debug, PartialEq)]
pub enum BallPathStop {
    Duration(Seconds),
    UntilRest,
    RailImpacts(usize),
}

/// One visible segment of a traced single-ball path.
#[derive(Clone, Debug, PartialEq)]
pub struct BallPathSegment {
    pub start: OnTableBallState,
    pub end: OnTableBallState,
    pub duration: Seconds,
}

/// A first-pass traced single-ball path across the table.
///
/// This is currently an event-vertex path: it records visible straight segments between the traced
/// state's start point, rail impacts, motion-transition vertices, and the final stop condition. It
/// does not yet sample within-phase side-spin curvature inside a segment.
#[derive(Clone, Debug, PartialEq)]
pub struct BallPath {
    pub initial_state: OnTableBallState,
    pub final_state: OnTableBallState,
    pub elapsed: Seconds,
    pub rail_impacts: usize,
    pub segments: Vec<BallPathSegment>,
}

impl BallPath {
    /// Project this traced path to drawable table-space points using only its event vertices.
    pub fn projected_points(&self, table_spec: &TableSpec) -> Vec<Position> {
        let mut points = vec![self
            .initial_state
            .as_ball_state()
            .projected_position(table_spec)];
        for segment in &self.segments {
            points.push(segment.end.as_ball_state().projected_position(table_spec));
        }
        points
    }

    /// Sample additional drawable points within each traced segment using the current motion
    /// solver.
    ///
    /// This densifies the coarse event-vertex path by advancing from each segment's start state in
    /// time steps no larger than `max_time_step`. Because each stored segment is currently bounded
    /// by the earliest upcoming rail impact, motion transition, or requested trace horizon, these
    /// sub-advances remain within the same traced segment contract.
    pub fn sampled_points(
        &self,
        max_time_step: Seconds,
        ball: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        table_spec: &TableSpec,
    ) -> Vec<Position> {
        let max_time_step = max_time_step.as_f64();
        assert!(
            max_time_step.is_finite() && max_time_step > 0.0,
            "sampled path max_time_step must be positive and finite"
        );

        let mut points = vec![self
            .initial_state
            .as_ball_state()
            .projected_position(table_spec)];
        for segment in &self.segments {
            let duration = segment.duration.as_f64();
            let sample_count = ((duration / max_time_step).ceil() as usize).max(1);

            for step in 1..=sample_count {
                if step == sample_count {
                    points.push(segment.end.as_ball_state().projected_position(table_spec));
                } else {
                    let t = duration * step as f64 / sample_count as f64;
                    let sampled = advance_on_table_ball_without_event(
                        &segment.start,
                        Seconds::new(t),
                        ball,
                        motion,
                    );
                    points.push(sampled.as_ball_state().projected_position(table_spec));
                }
            }
        }
        points
    }
}

/// Thresholds used when classifying the qualitative motion phase of a ball.
#[derive(Clone, Debug, PartialEq)]
pub struct MotionPhaseThresholds {
    pub airborne_height: Inches,
    pub airborne_vertical_speed: InchesPerSecond,
    pub rest_linear_speed: InchesPerSecond,
    pub rest_vertical_speed: InchesPerSecond,
    pub rest_angular_speed: RadiansPerSecond,
}

impl Default for MotionPhaseThresholds {
    fn default() -> Self {
        Self {
            airborne_height: Inches::from_f64(1e-9),
            airborne_vertical_speed: InchesPerSecond::new(Inches::from_f64(1e-9)),
            rest_linear_speed: InchesPerSecond::new(Inches::from_f64(1e-9)),
            rest_vertical_speed: InchesPerSecond::new(Inches::from_f64(1e-9)),
            rest_angular_speed: RadiansPerSecond::new(1e-9),
        }
    }
}

/// How rolling without slip is detected from the cloth-contact slip speed.
#[derive(Clone, Debug, PartialEq)]
pub enum SlidingToRollingModel {
    /// Treat only numerically exact zero slip as rolling.
    ExactNoSlip,

    /// Treat sufficiently small slip as rolling.
    Thresholded {
        contact_speed_epsilon: InchesPerSecond,
    },
}

/// Configuration used when classifying a ball's qualitative motion phase.
#[derive(Clone, Debug, PartialEq)]
pub struct MotionPhaseConfig {
    pub thresholds: MotionPhaseThresholds,
    pub sliding_to_rolling: SlidingToRollingModel,
}

impl Default for MotionPhaseConfig {
    fn default() -> Self {
        Self {
            thresholds: MotionPhaseThresholds::default(),
            sliding_to_rolling: SlidingToRollingModel::Thresholded {
                contact_speed_epsilon: InchesPerSecond::new(Inches::from_f64(1e-9)),
            },
        }
    }
}

/// The sliding-friction model used when computing the next motion transition for a sliding ball.
#[derive(Clone, Debug, PartialEq)]
pub enum SlidingFrictionModel {
    /// Approximate on-cloth sliding with a constant-magnitude translational acceleration `f g`
    /// opposite the cloth-contact slip vector, matching Eq. (M1') in
    /// `whitepapers/art_of_billiards_play_files/bil_praa.html`.
    ConstantAcceleration {
        acceleration_magnitude: InchesPerSecondSq,
    },
}

/// The vertical-axis spin-decay model used when computing on-table spin evolution.
#[derive(Clone, Debug, PartialEq)]
pub enum SpinDecayModel {
    /// Approximate z-axis spin decay as a constant-magnitude angular deceleration opposite the
    /// current z-spin direction, matching Eq. (M14') in
    /// `whitepapers/art_of_billiards_play_files/bil_praa.html`.
    ConstantAngularDeceleration {
        angular_deceleration: RadiansPerSecondSq,
    },
}

/// The rolling-resistance model used when computing the next motion transition for a rolling
/// ball.
#[derive(Clone, Debug, PartialEq)]
pub enum RollingResistanceModel {
    /// Approximate rolling as a constant-magnitude linear deceleration opposite the direction of
    /// travel.
    ConstantDeceleration {
        linear_deceleration: InchesPerSecondSq,
    },
}

/// Configuration used by the current on-table single-ball motion model.
#[derive(Clone, Debug, PartialEq)]
pub struct MotionTransitionConfig {
    pub phase: MotionPhaseConfig,
    pub sliding_friction: SlidingFrictionModel,
    pub spin_decay: SpinDecayModel,
    pub rolling_resistance: RollingResistanceModel,
}

/// Preferred public name for the current on-table single-ball motion config.
pub type OnTableMotionConfig = MotionTransitionConfig;

/// The next computed future phase transition for a single ball.
#[derive(Clone, Debug, PartialEq)]
pub struct NextTransition {
    pub phase_before: MotionPhase,
    pub phase_after: MotionPhase,
    pub time_until_transition: Seconds,
}

/// The result of advancing the on-table motion model through a requested duration.
///
/// If one or more phase boundaries were crossed while consuming `elapsed`, `transition` records
/// the first such boundary encountered from the initial state.
#[derive(Clone, Debug, PartialEq)]
pub struct MotionAdvance {
    pub state: BallState,
    pub elapsed: Seconds,
    pub transition: Option<NextTransition>,
}

/// Error returned when attempting to validate a `BallState` as an on-table solver input.
#[derive(Clone, Debug, PartialEq)]
pub enum OnTableStateError {
    HeightAboveTablePlane {
        height: Inches,
        allowed_height: Inches,
    },
    VerticalVelocityPresent {
        vertical_velocity: InchesPerSecond,
        allowed_vertical_velocity: InchesPerSecond,
    },
}

/// Error returned when attempting to validate a resting on-table state.
#[derive(Clone, Debug, PartialEq)]
pub enum RestingOnTableStateError {
    NotOnTable(OnTableStateError),
    NotResting,
}

/// The kinematic state of a billiard ball.
///
/// The local references in `whitepapers/` consistently model each ball using center-of-mass
/// translational velocity together with angular velocity:
///
/// - `whitepapers/Collision_of_Billiard_Balls_in_3D_with_Spin_and_Friction.pdf` uses a full
///   translational velocity `U = (U, V, W)` and angular velocity `Ω = (Ωx, Ωy, Ωz)`, and in its
///   on-table rolling special case has `(u, v, 0)` with `Ω = (-v, u, 0) / r`.
/// - `whitepapers/Alciatore_pool_physics_article.pdf` distinguishes sidespin and massé spin
///   components, so we keep the full 3-axis angular-velocity vector even though most early motion
///   simulation work is planar.
/// - `whitepapers/motions_of_ball_after_stroke.pdf` likewise summarizes the struck ball by its
///   translational velocity and rotational angular velocity after impact.
///
/// `height` is measured relative to the resting on-table center plane, so an ordinary cloth-bound
/// ball typically has `height == 0` and `vertical_velocity == 0`.
#[derive(Clone, Debug, PartialEq)]
pub struct BallState {
    pub position: Inches2,
    pub height: Inches,
    pub velocity: Velocity2,
    pub vertical_velocity: InchesPerSecond,
    pub angular_velocity: AngularVelocity3,
}

impl Default for BallState {
    fn default() -> Self {
        Self::resting_at(Inches2::zero())
    }
}

impl BallState {
    pub fn new<H: Into<Inches>, VV: Into<Inches>>(
        position: Inches2,
        height: H,
        velocity: Velocity2,
        vertical_velocity: VV,
        angular_velocity: AngularVelocity3,
    ) -> Self {
        let height = height.into();
        assert!(height.as_f64() >= 0.0, "ball height must be non-negative");

        Self {
            position,
            height,
            velocity,
            vertical_velocity: InchesPerSecond::new(vertical_velocity),
            angular_velocity,
        }
    }

    pub fn resting_at(position: Inches2) -> Self {
        Self::on_table(position, Velocity2::zero(), AngularVelocity3::zero())
    }

    pub fn on_table(
        position: Inches2,
        velocity: Velocity2,
        angular_velocity: AngularVelocity3,
    ) -> Self {
        Self::new(
            position,
            Inches::zero(),
            velocity,
            Inches::zero(),
            angular_velocity,
        )
    }

    pub fn airborne<H: Into<Inches>, VV: Into<Inches>>(
        position: Inches2,
        height: H,
        velocity: Velocity2,
        vertical_velocity: VV,
        angular_velocity: AngularVelocity3,
    ) -> Self {
        Self::new(
            position,
            height,
            velocity,
            vertical_velocity,
            angular_velocity,
        )
    }

    pub fn resting_at_position(position: &Position, table_spec: &TableSpec) -> Self {
        Self::resting_at(Inches2::new(
            table_spec.diamond_to_inches(position.x.clone()),
            table_spec.diamond_to_inches(position.y.clone()),
        ))
    }

    pub fn from_position(position: &Position, table_spec: &TableSpec) -> Self {
        Self::resting_at_position(position, table_spec)
    }

    pub fn projected_position(&self, table_spec: &TableSpec) -> Position {
        projected_position(self, table_spec)
    }

    pub fn speed(&self) -> InchesPerSecond {
        ball_speed(self)
    }

    pub fn cloth_contact_velocity(&self, radius: Inches) -> Velocity2 {
        cloth_contact_velocity_on_table(self, radius)
    }

    pub fn cloth_contact_speed(&self, radius: Inches) -> InchesPerSecond {
        cloth_contact_speed_on_table(self, radius)
    }

    pub fn motion_phase(&self, radius: Inches) -> MotionPhase {
        classify_motion_phase(
            self,
            &BallSetPhysicsSpec { radius },
            &MotionPhaseConfig::default(),
        )
    }
}

/// A `BallState` validated for the current cloth-contact motion solver domain.
///
/// This wrapper guarantees that the ball center is on the table plane and carries no vertical
/// center-of-mass motion, making it suitable for the current on-table motion APIs.
#[derive(Clone, Debug, PartialEq)]
pub struct OnTableBallState(BallState);

impl OnTableBallState {
    pub fn try_new(state: BallState) -> Result<Self, OnTableStateError> {
        if state.height.as_f64() > 0.0 {
            return Err(OnTableStateError::HeightAboveTablePlane {
                height: state.height.clone(),
                allowed_height: Inches::zero(),
            });
        }

        if state.vertical_velocity.as_f64().abs() > 0.0 {
            return Err(OnTableStateError::VerticalVelocityPresent {
                vertical_velocity: state.vertical_velocity.clone(),
                allowed_vertical_velocity: InchesPerSecond::zero(),
            });
        }

        Ok(Self(state))
    }

    pub fn try_new_with_thresholds(
        state: BallState,
        thresholds: &MotionPhaseThresholds,
    ) -> Result<Self, OnTableStateError> {
        if state.height.as_f64() > thresholds.airborne_height.as_f64() {
            return Err(OnTableStateError::HeightAboveTablePlane {
                height: state.height.clone(),
                allowed_height: thresholds.airborne_height.clone(),
            });
        }

        if state.vertical_velocity.as_f64().abs() > thresholds.airborne_vertical_speed.as_f64() {
            return Err(OnTableStateError::VerticalVelocityPresent {
                vertical_velocity: state.vertical_velocity.clone(),
                allowed_vertical_velocity: thresholds.airborne_vertical_speed.clone(),
            });
        }

        Ok(Self(BallState {
            position: state.position,
            height: Inches::zero(),
            velocity: state.velocity,
            vertical_velocity: InchesPerSecond::zero(),
            angular_velocity: state.angular_velocity,
        }))
    }

    pub fn as_ball_state(&self) -> &BallState {
        &self.0
    }

    pub fn into_ball_state(self) -> BallState {
        self.0
    }
}

impl TryFrom<BallState> for OnTableBallState {
    type Error = OnTableStateError;

    fn try_from(state: BallState) -> Result<Self, Self::Error> {
        Self::try_new(state)
    }
}

impl TryFrom<&BallState> for OnTableBallState {
    type Error = OnTableStateError;

    fn try_from(state: &BallState) -> Result<Self, Self::Error> {
        Self::try_new(state.clone())
    }
}

impl From<OnTableBallState> for BallState {
    fn from(state: OnTableBallState) -> Self {
        state.into_ball_state()
    }
}

fn exact_resting_thresholds() -> MotionPhaseThresholds {
    MotionPhaseThresholds {
        airborne_height: Inches::zero(),
        airborne_vertical_speed: InchesPerSecond::zero(),
        rest_linear_speed: InchesPerSecond::zero(),
        rest_vertical_speed: InchesPerSecond::zero(),
        rest_angular_speed: RadiansPerSecond::zero(),
    }
}

fn validate_resting_on_table_state(
    state: OnTableBallState,
    thresholds: &MotionPhaseThresholds,
) -> Result<RestingOnTableBallState, RestingOnTableStateError> {
    let rest_config = MotionPhaseConfig {
        thresholds: thresholds.clone(),
        sliding_to_rolling: MotionPhaseConfig::default().sliding_to_rolling,
    };
    if classify_motion_phase(
        state.as_ball_state(),
        &BallSetPhysicsSpec::default(),
        &rest_config,
    ) != MotionPhase::Rest
    {
        return Err(RestingOnTableStateError::NotResting);
    }

    Ok(RestingOnTableBallState(
        OnTableBallState::try_from(BallState::resting_at(
            state.as_ball_state().position.clone(),
        ))
        .expect("resting-state normalization should preserve on-table invariants"),
    ))
}

/// A `BallState` validated as exactly resting on the table plane.
///
/// This wrapper is the natural source state for operations like a cue strike that assume the ball
/// starts from rest. Threshold-aware construction can accept tiny residual motion and normalize it
/// to exact rest.
#[derive(Clone, Debug, PartialEq)]
pub struct RestingOnTableBallState(OnTableBallState);

impl RestingOnTableBallState {
    pub fn try_new(state: BallState) -> Result<Self, RestingOnTableStateError> {
        let thresholds = exact_resting_thresholds();
        let on_table = OnTableBallState::try_new_with_thresholds(state, &thresholds)
            .map_err(RestingOnTableStateError::NotOnTable)?;

        validate_resting_on_table_state(on_table, &thresholds)
    }

    pub fn try_new_with_thresholds(
        state: BallState,
        thresholds: &MotionPhaseThresholds,
    ) -> Result<Self, RestingOnTableStateError> {
        let on_table = OnTableBallState::try_new_with_thresholds(state, thresholds)
            .map_err(RestingOnTableStateError::NotOnTable)?;

        validate_resting_on_table_state(on_table, thresholds)
    }

    pub fn as_on_table_ball_state(&self) -> &OnTableBallState {
        &self.0
    }

    pub fn into_on_table_ball_state(self) -> OnTableBallState {
        self.0
    }

    pub fn as_ball_state(&self) -> &BallState {
        self.0.as_ball_state()
    }

    pub fn into_ball_state(self) -> BallState {
        self.0.into_ball_state()
    }
}

impl TryFrom<BallState> for RestingOnTableBallState {
    type Error = RestingOnTableStateError;

    fn try_from(state: BallState) -> Result<Self, Self::Error> {
        Self::try_new(state)
    }
}

impl TryFrom<&BallState> for RestingOnTableBallState {
    type Error = RestingOnTableStateError;

    fn try_from(state: &BallState) -> Result<Self, Self::Error> {
        Self::try_new(state.clone())
    }
}

impl TryFrom<OnTableBallState> for RestingOnTableBallState {
    type Error = RestingOnTableStateError;

    fn try_from(state: OnTableBallState) -> Result<Self, Self::Error> {
        validate_resting_on_table_state(state, &exact_resting_thresholds())
    }
}

impl TryFrom<&OnTableBallState> for RestingOnTableBallState {
    type Error = RestingOnTableStateError;

    fn try_from(state: &OnTableBallState) -> Result<Self, Self::Error> {
        validate_resting_on_table_state(state.clone(), &exact_resting_thresholds())
    }
}

impl From<RestingOnTableBallState> for OnTableBallState {
    fn from(state: RestingOnTableBallState) -> Self {
        state.into_on_table_ball_state()
    }
}

impl From<RestingOnTableBallState> for BallState {
    fn from(state: RestingOnTableBallState) -> Self {
        state.into_ball_state()
    }
}

/// Error returned when constructing a cue-shot input or strike configuration.
#[derive(Clone, Debug, PartialEq)]
pub enum ShotError {
    CueTipContactOutsideBall {
        side_offset: Scale,
        height_offset: Scale,
        offset_radius: Scale,
    },
    NegativeCueSpeed {
        cue_speed: InchesPerSecond,
    },
    NonPositiveCueMassRatio {
        cue_mass_ratio: Scale,
    },
    CollisionEnergyLossOutOfRange {
        collision_energy_loss: Scale,
    },
    MiscueOffsetLimitOutOfRange {
        miscue_offset_limit: Scale,
    },
    Miscue {
        tip_contact: CueTipContact,
        miscue_offset_limit: Scale,
    },
    CueBallDoesNotSeparateFromCue {
        tip_contact: CueTipContact,
        cue_mass_ratio: Scale,
        collision_energy_loss: Scale,
    },
}

/// A cue-tip contact point on the cue ball, expressed in cue-local ball-radius units.
///
/// `side_offset` is positive to the striker's right and negative to the striker's left, relative
/// to the shot heading. `height_offset` is positive above ball center (follow / topspin) and
/// negative below center (draw).
///
/// The constructor validates that the contact point lies on or within the cue-ball disc in the
/// cue-local side/height plane: `side_offset^2 + height_offset^2 <= 1`.
#[derive(Clone, Debug, PartialEq)]
pub struct CueTipContact {
    side_offset: Scale,
    height_offset: Scale,
}

impl CueTipContact {
    pub fn new(side_offset: Scale, height_offset: Scale) -> Result<Self, ShotError> {
        let radius = side_offset.as_f64().hypot(height_offset.as_f64());
        if radius > 1.0 + 1e-12 {
            return Err(ShotError::CueTipContactOutsideBall {
                side_offset,
                height_offset,
                offset_radius: Scale::from_f64(radius),
            });
        }

        Ok(Self {
            side_offset,
            height_offset,
        })
    }

    pub fn center() -> Self {
        Self {
            side_offset: Scale::zero(),
            height_offset: Scale::zero(),
        }
    }

    pub fn side_offset(&self) -> &Scale {
        &self.side_offset
    }

    pub fn height_offset(&self) -> &Scale {
        &self.height_offset
    }

    pub fn offset_radius(&self) -> Scale {
        Scale::from_f64(self.side_offset.as_f64().hypot(self.height_offset.as_f64()))
    }
}

/// A fully specified cue shot intent for striking a resting cue ball.
///
/// This is a pure input description: the absolute shot heading, the cue speed at impact, and the
/// validated cue-tip contact point on the ball. Later strike-model helpers will map this input to
/// an immediate post-strike ball state.
#[derive(Clone, Debug, PartialEq)]
pub struct Shot {
    heading: Angle,
    cue_speed: InchesPerSecond,
    tip_contact: CueTipContact,
}

impl Shot {
    pub fn new(
        heading: Angle,
        cue_speed: InchesPerSecond,
        tip_contact: CueTipContact,
    ) -> Result<Self, ShotError> {
        if cue_speed.as_f64() < 0.0 {
            return Err(ShotError::NegativeCueSpeed { cue_speed });
        }

        Ok(Self {
            heading,
            cue_speed,
            tip_contact,
        })
    }

    pub fn heading(&self) -> Angle {
        self.heading
    }

    pub fn cue_speed(&self) -> &InchesPerSecond {
        &self.cue_speed
    }

    pub fn tip_contact(&self) -> &CueTipContact {
        &self.tip_contact
    }

    pub fn human_speed_validation(
        &self,
        cue: &CueStrikeConfig,
    ) -> Result<HumanShotSpeedValidation, ShotError> {
        validate_shot_human_speed(self, cue)
    }
}

/// Human-facing speed bands for classifying shot pace.
///
/// The ordinary shot-speed ladder follows the local Dr. Dave speed-control references (`TP B.5`
/// / `TP B.6`):
///
/// - `Touch` ≈ 1.5 mph
/// - `Slow` ≈ 3 mph
/// - `MediumSoft` ≈ 5 mph
/// - `Medium` ≈ 7 mph
/// - `MediumFast` ≈ 8 mph
/// - `Fast` ≈ 12 mph
/// - `Power` ≈ 20 mph
///
/// The break-speed bands follow the local break-speed notes in the corpus:
///
/// - very good power breaks are typically about 25–30 mph, and
/// - the best power breakers can approach 35 mph, which is unusual and hard to control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HumanShotSpeedBand {
    Touch,
    Slow,
    MediumSoft,
    Medium,
    MediumFast,
    Fast,
    Power,
    TypicalPowerBreak,
    ExceptionalPowerBreak,
    BeyondExceptionalPowerBreak,
}

impl HumanShotSpeedBand {
    pub fn from_speed(speed: &InchesPerSecond) -> Self {
        let mph = speed.as_mph();

        if mph <= 1.5 {
            Self::Touch
        } else if mph <= 3.0 {
            Self::Slow
        } else if mph <= 5.0 {
            Self::MediumSoft
        } else if mph <= 7.0 {
            Self::Medium
        } else if mph <= 8.0 {
            Self::MediumFast
        } else if mph <= 12.0 {
            Self::Fast
        } else if mph <= 20.0 {
            Self::Power
        } else if mph <= 30.0 {
            Self::TypicalPowerBreak
        } else if mph <= 35.0 {
            Self::ExceptionalPowerBreak
        } else {
            Self::BeyondExceptionalPowerBreak
        }
    }
}

/// A human-facing speed report for a shot intent under the current cue-strike model.
///
/// `cue_speed_at_impact` is the raw `Shot` input. `estimated_cue_ball_speed_after_impact` is the
/// immediate post-strike cue-ball launch speed implied by the current cue/ball transfer model.
///
/// This lets callers compare both the abstract stroke input and the resulting cue-ball launch speed
/// against ordinary human-play speed bands.
#[derive(Clone, Debug, PartialEq)]
pub struct HumanShotSpeedValidation {
    pub cue_speed_at_impact: InchesPerSecond,
    pub cue_speed_band: HumanShotSpeedBand,
    pub estimated_cue_ball_speed_after_impact: InchesPerSecond,
    pub cue_ball_speed_band: HumanShotSpeedBand,
}

impl HumanShotSpeedValidation {
    pub fn is_typical_table_shot(&self) -> bool {
        matches!(
            self.cue_ball_speed_band,
            HumanShotSpeedBand::Touch
                | HumanShotSpeedBand::Slow
                | HumanShotSpeedBand::MediumSoft
                | HumanShotSpeedBand::Medium
                | HumanShotSpeedBand::MediumFast
                | HumanShotSpeedBand::Fast
        )
    }

    pub fn requires_power_shot(&self) -> bool {
        !self.is_typical_table_shot()
    }

    pub fn exceeds_typical_human_power_break(&self) -> bool {
        matches!(
            self.cue_ball_speed_band,
            HumanShotSpeedBand::ExceptionalPowerBreak
                | HumanShotSpeedBand::BeyondExceptionalPowerBreak
        )
    }

    pub fn exceeds_exceptional_human_shot_speed(&self) -> bool {
        matches!(
            self.cue_ball_speed_band,
            HumanShotSpeedBand::BeyondExceptionalPowerBreak
        )
    }
}

/// Dimensionless parameters for the current cue-strike transfer model.
///
/// `cue_mass_ratio` is the effective cue-mass to ball-mass ratio `M'/M` in the current local
/// whitepaper notation. `collision_energy_loss` is the energy-loss fraction `e` from the same
/// cue-ball collision model. `miscue_offset_limit` is a first-pass hard tip-offset bound, in
/// cue-ball-radius units, beyond which the strike is treated as a miscue instead of a clean grip.
///
/// The default `miscue_offset_limit` is `0.5R`, matching the common local literature guidance that
/// the maximum recommended cue-tip offset for strong english is about half a ball radius; see the
/// local `TP A.22` / `TP A.25` notes and the `TP A.30` corpus summary in
/// `agent_knowledge/whitepapers_formula_candidates.txt`.
#[derive(Clone, Debug, PartialEq)]
pub struct CueStrikeConfig {
    cue_mass_ratio: Scale,
    collision_energy_loss: Scale,
    miscue_offset_limit: Scale,
}

impl CueStrikeConfig {
    pub fn new(cue_mass_ratio: Scale, collision_energy_loss: Scale) -> Result<Self, ShotError> {
        Self::new_with_miscue_offset_limit(
            cue_mass_ratio,
            collision_energy_loss,
            Scale::from_f64(0.5),
        )
    }

    pub fn new_with_miscue_offset_limit(
        cue_mass_ratio: Scale,
        collision_energy_loss: Scale,
        miscue_offset_limit: Scale,
    ) -> Result<Self, ShotError> {
        if cue_mass_ratio.as_f64() <= 0.0 {
            return Err(ShotError::NonPositiveCueMassRatio { cue_mass_ratio });
        }
        if !(0.0..=1.0).contains(&collision_energy_loss.as_f64()) {
            return Err(ShotError::CollisionEnergyLossOutOfRange {
                collision_energy_loss,
            });
        }
        if miscue_offset_limit.as_f64() <= 0.0 || miscue_offset_limit.as_f64() > 1.0 {
            return Err(ShotError::MiscueOffsetLimitOutOfRange {
                miscue_offset_limit,
            });
        }

        Ok(Self {
            cue_mass_ratio,
            collision_energy_loss,
            miscue_offset_limit,
        })
    }

    pub fn cue_mass_ratio(&self) -> &Scale {
        &self.cue_mass_ratio
    }

    pub fn collision_energy_loss(&self) -> &Scale {
        &self.collision_energy_loss
    }

    pub fn miscue_offset_limit(&self) -> &Scale {
        &self.miscue_offset_limit
    }
}

fn validate_shot_and_cue_for_strike(shot: &Shot, cue: &CueStrikeConfig) -> Result<(), ShotError> {
    let cue_mass_ratio = cue.cue_mass_ratio.as_f64();
    let collision_energy_loss = cue.collision_energy_loss.as_f64();
    let offset_radius = shot.tip_contact.offset_radius().as_f64();
    let offset_radius_squared = offset_radius * offset_radius;
    if offset_radius > cue.miscue_offset_limit.as_f64() + 1e-12 {
        return Err(ShotError::Miscue {
            tip_contact: shot.tip_contact.clone(),
            miscue_offset_limit: cue.miscue_offset_limit.clone(),
        });
    }

    let automatic_split_limit_squared =
        0.4 * (1.0 + 1.0 / cue_mass_ratio) * (1.0 - collision_energy_loss * (1.0 + cue_mass_ratio));
    if automatic_split_limit_squared <= 0.0
        || offset_radius_squared >= automatic_split_limit_squared
    {
        return Err(ShotError::CueBallDoesNotSeparateFromCue {
            tip_contact: shot.tip_contact.clone(),
            cue_mass_ratio: cue.cue_mass_ratio.clone(),
            collision_energy_loss: cue.collision_energy_loss.clone(),
        });
    }

    Ok(())
}

fn compute_post_strike_speed(
    shot: &Shot,
    cue: &CueStrikeConfig,
) -> Result<InchesPerSecond, ShotError> {
    validate_shot_and_cue_for_strike(shot, cue)?;

    let cue_mass_ratio = cue.cue_mass_ratio.as_f64();
    let collision_energy_loss = cue.collision_energy_loss.as_f64();
    let offset_radius_squared = shot.tip_contact.offset_radius().as_f64().powi(2);
    let k2 = 1.0 + 2.5 * offset_radius_squared;
    let cue_speed = shot.cue_speed.as_f64();
    let post_strike_speed = cue_speed
        * (1.0
            + (1.0 - collision_energy_loss - collision_energy_loss * k2 * cue_mass_ratio).sqrt())
        / (k2 + 1.0 / cue_mass_ratio);

    Ok(InchesPerSecond::new(Inches::from_f64(post_strike_speed)))
}

/// Validate a shot's raw cue speed and estimated cue-ball launch speed against human-play ranges.
///
/// The returned report keeps both numbers because the current public `Shot` input stores cue speed
/// at impact, while most human-facing literature and practical intuition talk about cue-ball speed.
pub fn validate_shot_human_speed(
    shot: &Shot,
    cue: &CueStrikeConfig,
) -> Result<HumanShotSpeedValidation, ShotError> {
    let estimated_cue_ball_speed_after_impact = compute_post_strike_speed(shot, cue)?;
    let cue_speed_at_impact = shot.cue_speed.clone();

    Ok(HumanShotSpeedValidation {
        cue_speed_band: HumanShotSpeedBand::from_speed(&cue_speed_at_impact),
        cue_ball_speed_band: HumanShotSpeedBand::from_speed(&estimated_cue_ball_speed_after_impact),
        cue_speed_at_impact,
        estimated_cue_ball_speed_after_impact,
    })
}

/// Strike a resting on-table ball with a first-pass horizontal cue shot.
///
/// This models the instantaneous cue→ball transfer only; it returns the immediate post-strike
/// on-table state that should then be fed into the existing motion / event simulation. The current
/// implementation follows the horizontal cue-ball collision model in
/// `whitepapers/art_of_billiards_play_files/bil_praa.html`, §7.2:
///
/// - cue movement is a translation in the shot heading,
/// - the ball starts from rest,
/// - cue-tip offset is specified by `CueTipContact` in cue-local ball-radius units, and
/// - only the automatic cue-ball separation regime is supported.
///
/// This first pass intentionally excludes cue elevation, jump / masse launch, squirt / swerve, and
/// detailed tip-size / miscue probability refinements. It does, however, enforce a first-pass hard
/// miscue limit on cue-tip offset via `CueStrikeConfig`.
pub fn strike_resting_ball_on_table(
    ball: &RestingOnTableBallState,
    shot: &Shot,
    cue: &CueStrikeConfig,
    ball_set: &BallSetPhysicsSpec,
) -> Result<OnTableBallState, ShotError> {
    let post_strike_speed = compute_post_strike_speed(shot, cue)?.as_f64();
    let velocity = Velocity2::from_polar(
        InchesPerSecond::new(Inches::from_f64(post_strike_speed)),
        shot.heading,
    );

    let heading_radians = shot.heading.as_degrees().to_radians();
    let shot_x = heading_radians.sin();
    let shot_y = heading_radians.cos();
    let shot_right_x = shot_y;
    let shot_right_y = -shot_x;
    let spin_scale = 2.5 * post_strike_speed / ball_set.radius.as_f64();
    let local_angular_right = -spin_scale * shot.tip_contact.height_offset.as_f64();
    let angular_velocity = AngularVelocity3::new(
        shot_right_x * local_angular_right,
        shot_right_y * local_angular_right,
        spin_scale * shot.tip_contact.side_offset.as_f64(),
    );

    Ok(build_on_table_ball_state(
        ball.as_ball_state().position.clone(),
        velocity,
        angular_velocity,
    ))
}

/// Return the planar table projection of a `BallState` in table-space coordinates.
pub fn projected_position(state: &BallState, table_spec: &TableSpec) -> Position {
    Position::new(
        table_spec.inches_to_diamond(state.position.x().clone()),
        table_spec.inches_to_diamond(state.position.y().clone()),
    )
}

/// Return the planar center-of-mass speed of the ball.
pub fn ball_speed(state: &BallState) -> InchesPerSecond {
    state.velocity.speed()
}

fn is_airborne(state: &BallState, thresholds: &MotionPhaseThresholds) -> bool {
    state.height.as_f64() > thresholds.airborne_height.as_f64()
        || state.vertical_velocity.as_f64().abs() > thresholds.airborne_vertical_speed.as_f64()
}

/// Compute the cloth-contact slip velocity for an on-table ball.
///
/// `whitepapers/TP_A-4.pdf`, Eq. (3), gives the contact-point velocity at the cloth as
///
/// `v_C = (v_x - R ω_y, v_y + R ω_x)`
///
/// and explicitly notes that z-axis spin does not affect this contact-point velocity. This helper
/// implements that table-contact velocity model directly for the current coordinate conventions.
///
/// Calling this helper for an airborne ball state is intentionally left as `todo!()` for now.
pub fn cloth_contact_velocity_on_table(state: &BallState, radius: Inches) -> Velocity2 {
    if state.height.as_f64() > 0.0 || state.vertical_velocity.as_f64().abs() > 0.0 {
        todo!("cloth-contact velocity for airborne ball states is not implemented yet")
    }

    let vx = state.velocity.x().as_f64();
    let vy = state.velocity.y().as_f64();
    let wx = state.angular_velocity.x().as_f64();
    let wy = state.angular_velocity.y().as_f64();
    let radius = radius.as_f64();

    Velocity2::from_components(
        InchesPerSecond::new(Inches::from_f64(vx - radius * wy)),
        InchesPerSecond::new(Inches::from_f64(vy + radius * wx)),
    )
}

/// Compute the cloth-contact slip-speed magnitude for an on-table ball.
pub fn cloth_contact_speed_on_table(state: &BallState, radius: Inches) -> InchesPerSecond {
    cloth_contact_velocity_on_table(state, radius).speed()
}

/// Classify a ball's qualitative motion regime from its kinematic state and configurable
/// thresholds / assumptions.
///
/// The classification logic follows the local technical proofs and supporting article notes:
///
/// - `whitepapers/TP_A-4.pdf`, Eqs. (3), (10), and (12), defines the cloth contact-point slip
///   velocity and shows that this slip speed decays to zero, after which the cue ball rolls
///   without slipping.
/// - `whitepapers/TP_4-2.pdf`, Eq. (3), gives the straight-line special case `v = ωR` for
///   immediate rolling without slipping.
/// - `whitepapers/Alciatore_pool_physics_article.pdf` explains that both draw (bottom spin) and
///   over-spin are still sliding states until rolling develops, so the rolling/sliding distinction
///   is based on cloth-contact slip, not merely on the presence or direction of spin.
pub fn classify_motion_phase(
    state: &BallState,
    ball: &BallSetPhysicsSpec,
    config: &MotionPhaseConfig,
) -> MotionPhase {
    let near_zero = |value: f64, threshold: f64| value.abs() <= threshold;

    if is_airborne(state, &config.thresholds) {
        return MotionPhase::Airborne;
    }

    let linear_speed = ball_speed(state).as_f64();
    let vertical_speed = state.vertical_velocity.as_f64();
    let wx = state.angular_velocity.x().as_f64();
    let wy = state.angular_velocity.y().as_f64();
    let wz = state.angular_velocity.z().as_f64();
    let angular_threshold = config.thresholds.rest_angular_speed.as_f64();

    if near_zero(linear_speed, config.thresholds.rest_linear_speed.as_f64())
        && near_zero(
            vertical_speed,
            config.thresholds.rest_vertical_speed.as_f64(),
        )
        && near_zero(wx, angular_threshold)
        && near_zero(wy, angular_threshold)
        && near_zero(wz, angular_threshold)
    {
        return MotionPhase::Rest;
    }

    if near_zero(linear_speed, config.thresholds.rest_linear_speed.as_f64())
        && near_zero(wx, angular_threshold)
        && near_zero(wy, angular_threshold)
        && !near_zero(wz, angular_threshold)
    {
        return MotionPhase::Spinning;
    }

    let contact_speed = cloth_contact_speed_on_table(state, ball.radius.clone()).as_f64();
    let is_rolling = match &config.sliding_to_rolling {
        SlidingToRollingModel::ExactNoSlip => contact_speed <= f64::EPSILON,
        SlidingToRollingModel::Thresholded {
            contact_speed_epsilon,
        } => contact_speed <= contact_speed_epsilon.as_f64(),
    };

    if is_rolling {
        MotionPhase::Rolling
    } else {
        MotionPhase::Sliding
    }
}

fn sliding_friction_acceleration(config: &MotionTransitionConfig) -> f64 {
    let acceleration_magnitude = match &config.sliding_friction {
        SlidingFrictionModel::ConstantAcceleration {
            acceleration_magnitude,
        } => acceleration_magnitude.as_f64(),
    };

    assert!(
        acceleration_magnitude > 0.0,
        "sliding acceleration magnitude must be positive"
    );

    acceleration_magnitude
}

fn spin_angular_deceleration(config: &MotionTransitionConfig) -> f64 {
    let angular_deceleration = match &config.spin_decay {
        SpinDecayModel::ConstantAngularDeceleration {
            angular_deceleration,
        } => angular_deceleration.as_f64(),
    };

    assert!(
        angular_deceleration > 0.0,
        "spin angular deceleration must be positive"
    );

    angular_deceleration
}

fn advance_vertical_axis_spin(
    initial_spin: f64,
    dt: Seconds,
    config: &MotionTransitionConfig,
) -> f64 {
    let remaining = (initial_spin.abs() - spin_angular_deceleration(config) * dt.as_f64()).max(0.0);
    initial_spin.signum() * remaining
}

fn time_until_vertical_axis_spin_stops(
    initial_spin: f64,
    config: &MotionTransitionConfig,
) -> Option<Seconds> {
    if initial_spin.abs() <= f64::EPSILON {
        None
    } else {
        Some(Seconds::new(
            initial_spin.abs() / spin_angular_deceleration(config),
        ))
    }
}

fn rolling_linear_deceleration(config: &MotionTransitionConfig) -> f64 {
    let linear_deceleration = match &config.rolling_resistance {
        RollingResistanceModel::ConstantDeceleration {
            linear_deceleration,
        } => linear_deceleration.as_f64(),
    };

    assert!(
        linear_deceleration > 0.0,
        "rolling linear deceleration must be positive"
    );

    linear_deceleration
}

fn require_on_table_state(state: &BallState, config: &MotionPhaseConfig) -> OnTableBallState {
    OnTableBallState::try_new_with_thresholds(state.clone(), &config.thresholds).expect(
        "on-table motion requires height and vertical velocity to remain within on-table thresholds",
    )
}

/// Compute the next qualitative motion transition for a single ball under the current on-table
/// motion model.
///
/// The currently implemented cases are grounded in the local references:
///
/// - `whitepapers/art_of_billiards_play_files/bil_praa.html`, §7.3, Eqs. (M4), (M8), and
///   (M10'), gives the exact on-cloth sliding transition under Coulomb friction.
///   In that notation, `WE` is the cloth-contact slip velocity. Our
///   `cloth_contact_velocity_on_table(...)` helper implements the same quantity, so
///   `tc = ||Wi - Wc|| / (f g)` and `Wc = Wi - (2/7) WEi` become
///   `tc = (2/7) ||WEi|| / (f g)`.
/// - `whitepapers/art_of_billiards_play_files/bil_praa.html`, §7.5, Eqs. (M13) through (M14''),
///   shows that vertical-axis spin decays linearly with time during both sliding and rolling, so a
///   rolling ball with residual z-spin can transition to `Spinning` when translation stops.
/// - `whitepapers/55. RollingBall.pdf` reports experimental cases where both `v` and `ω`
///   decreased linearly with time while the ball rolled to a stop, which makes a constant linear
///   rolling deceleration a reasonable first configurable approximation.
///
/// Airborne transition prediction is intentionally left as `todo!()` for now so the transition API
/// can stabilize before those richer branches are implemented.
pub fn compute_next_transition_on_table(
    state: &OnTableBallState,
    ball: &BallSetPhysicsSpec,
    config: &OnTableMotionConfig,
) -> Option<NextTransition> {
    let state = state.as_ball_state();

    match classify_motion_phase(state, ball, &config.phase) {
        MotionPhase::Rest => None,
        MotionPhase::Sliding => Some(NextTransition {
            phase_before: MotionPhase::Sliding,
            phase_after: MotionPhase::Rolling,
            time_until_transition: Seconds::new(
                (2.0 / 7.0) * cloth_contact_speed_on_table(state, ball.radius.clone()).as_f64()
                    / sliding_friction_acceleration(config),
            ),
        }),
        MotionPhase::Rolling => {
            let time_until_transition =
                Seconds::new(ball_speed(state).as_f64() / rolling_linear_deceleration(config));
            let phase_after = if advance_vertical_axis_spin(
                state.angular_velocity.z().as_f64(),
                time_until_transition,
                config,
            )
            .abs()
                > config.phase.thresholds.rest_angular_speed.as_f64()
            {
                MotionPhase::Spinning
            } else {
                MotionPhase::Rest
            };

            Some(NextTransition {
                phase_before: MotionPhase::Rolling,
                phase_after,
                time_until_transition,
            })
        }
        MotionPhase::Spinning => Some(NextTransition {
            phase_before: MotionPhase::Spinning,
            phase_after: MotionPhase::Rest,
            time_until_transition: time_until_vertical_axis_spin_stops(
                state.angular_velocity.z().as_f64(),
                config,
            )
            .expect("spinning balls should have non-zero z-spin"),
        }),
        MotionPhase::Airborne => todo!("airborne transition prediction is not implemented yet"),
    }
}

/// Compatibility wrapper for the older motion API name.
pub fn compute_next_transition(
    state: &BallState,
    ball: &BallSetPhysicsSpec,
    config: &MotionTransitionConfig,
) -> Option<NextTransition> {
    match classify_motion_phase(state, ball, &config.phase) {
        MotionPhase::Airborne => todo!("airborne transition prediction is not implemented yet"),
        _ => compute_next_transition_on_table(
            &require_on_table_state(state, &config.phase),
            ball,
            config,
        ),
    }
}

/// Advance an on-table state within a known qualitative motion phase.
///
/// The returned state is clamped to the end of the current phase if `dt` would otherwise cross a
/// phase boundary. Use `advance_motion_on_table(...)` when you want a full requested duration to
/// continue across one or more phase transitions.
pub fn advance_within_phase_on_table(
    state: &OnTableBallState,
    phase: MotionPhase,
    dt: Seconds,
    ball: &BallSetPhysicsSpec,
    config: &OnTableMotionConfig,
) -> OnTableBallState {
    assert!(dt.as_f64() >= 0.0, "advance duration must be non-negative");

    if dt.as_f64() == 0.0 {
        return state.clone();
    }

    let state = state.as_ball_state();
    debug_assert_eq!(classify_motion_phase(state, ball, &config.phase), phase);

    match phase {
        MotionPhase::Rest => OnTableBallState::try_from(state.clone())
            .expect("rest states should remain valid on-table states"),
        MotionPhase::Sliding => {
            let transition = compute_next_transition_on_table(
                &OnTableBallState::try_from(state.clone())
                    .expect("sliding states should remain valid on-table states"),
                ball,
                config,
            )
            .expect("sliding balls should predict a rolling transition");
            let transition_time = transition.time_until_transition.as_f64();
            let advance_time = dt.as_f64().min(transition_time);
            let alpha = advance_time / transition_time;
            let slip_velocity = cloth_contact_velocity_on_table(state, ball.radius.clone());
            let vx_i = state.velocity.x().as_f64();
            let vy_i = state.velocity.y().as_f64();
            let we_x = slip_velocity.x().as_f64();
            let we_y = slip_velocity.y().as_f64();
            let vx_c = vx_i - (2.0 / 7.0) * we_x;
            let vy_c = vy_i - (2.0 / 7.0) * we_y;
            let vx = vx_i - alpha * (vx_i - vx_c);
            let vy = vy_i - alpha * (vy_i - vy_c);
            let radius = ball.radius.as_f64();
            let delta_vx = vx - vx_i;
            let delta_vy = vy - vy_i;
            let base_angular_x =
                state.angular_velocity.x().as_f64() + (5.0 / (2.0 * radius)) * delta_vy;
            let base_angular_y =
                state.angular_velocity.y().as_f64() - (5.0 / (2.0 * radius)) * delta_vx;
            let curve = state.velocity.angle_from_north().and_then(|start_heading| {
                sliding_side_spin_curve_during(
                    start_heading,
                    vx_i,
                    vy_i,
                    vx_c,
                    vy_c,
                    state.angular_velocity.z().as_f64(),
                    Seconds::new(advance_time),
                    transition.time_until_transition,
                    ball,
                    config,
                )
            });
            let (dx, dy, vx, vy, angular_x, angular_y) = if let Some(curve) = curve {
                let curve_alpha = curve.duration.as_f64() / transition_time;
                let vx_curve = vx_i - curve_alpha * (vx_i - vx_c);
                let vy_curve = vy_i - curve_alpha * (vy_i - vy_c);
                let (mut dx, mut dy) = rotate_xy_clockwise(
                    0.5 * (vx_i + vx_curve) * curve.duration.as_f64(),
                    0.5 * (vy_i + vy_curve) * curve.duration.as_f64(),
                    0.5 * curve.angle_degrees,
                );
                let remaining_time = advance_time - curve.duration.as_f64();
                if remaining_time > f64::EPSILON {
                    let (dx_rest, dy_rest) = rotate_xy_clockwise(
                        0.5 * (vx_curve + vx) * remaining_time,
                        0.5 * (vy_curve + vy) * remaining_time,
                        curve.angle_degrees,
                    );
                    dx += dx_rest;
                    dy += dy_rest;
                }
                let (vx, vy) = rotate_xy_clockwise(vx, vy, curve.angle_degrees);
                let (angular_x, angular_y) =
                    rotate_xy_clockwise(base_angular_x, base_angular_y, curve.angle_degrees);
                (dx, dy, vx, vy, angular_x, angular_y)
            } else {
                (
                    0.5 * (vx_i + vx) * advance_time,
                    0.5 * (vy_i + vy) * advance_time,
                    vx,
                    vy,
                    base_angular_x,
                    base_angular_y,
                )
            };

            OnTableBallState::try_from(BallState {
                position: Inches2::new(
                    Inches::from_f64(state.position.x().as_f64() + dx),
                    Inches::from_f64(state.position.y().as_f64() + dy),
                ),
                height: state.height.clone(),
                velocity: Velocity2::from_components(
                    InchesPerSecond::new(Inches::from_f64(vx)),
                    InchesPerSecond::new(Inches::from_f64(vy)),
                ),
                vertical_velocity: state.vertical_velocity.clone(),
                angular_velocity: AngularVelocity3::new(
                    angular_x,
                    angular_y,
                    advance_vertical_axis_spin(
                        state.angular_velocity.z().as_f64(),
                        Seconds::new(advance_time),
                        config,
                    ),
                ),
            })
            .expect("sliding phase advance should preserve on-table invariants")
        }
        MotionPhase::Rolling => {
            let initial_speed = ball_speed(state).as_f64();
            let linear_deceleration = rolling_linear_deceleration(config);
            let stop_time = initial_speed / linear_deceleration;
            let advance_time = dt.as_f64().min(stop_time);
            let final_speed = (initial_speed - linear_deceleration * advance_time).max(0.0);
            let start_heading = state
                .velocity
                .angle_from_north()
                .expect("rolling states should have a velocity heading");
            let curve = rolling_side_spin_curve_during(
                start_heading,
                initial_speed,
                state.angular_velocity.z().as_f64(),
                Seconds::new(advance_time),
                ball,
                config,
            );
            let curve_duration = curve.map_or(0.0, |curve| curve.duration.as_f64());
            let speed_after_curve = (initial_speed - linear_deceleration * curve_duration).max(0.0);
            let curve_distance = 0.5 * (initial_speed + speed_after_curve) * curve_duration;
            let (curve_angle_degrees, heading_after_curve) = curve
                .map(|curve| (curve.angle_degrees, curve.heading_after_curve))
                .unwrap_or((0.0, start_heading));
            let curve_mid_heading = rotate_angle_degrees(start_heading, 0.5 * curve_angle_degrees);
            let curve_mid_radians = curve_mid_heading.as_degrees().to_radians();
            let mut dx = curve_distance * curve_mid_radians.sin();
            let mut dy = curve_distance * curve_mid_radians.cos();

            let remaining_time = advance_time - curve_duration;
            if remaining_time > f64::EPSILON {
                let straight_distance = 0.5 * (speed_after_curve + final_speed) * remaining_time;
                let straight_heading_radians = heading_after_curve.as_degrees().to_radians();
                dx += straight_distance * straight_heading_radians.sin();
                dy += straight_distance * straight_heading_radians.cos();
            }

            let final_velocity = if final_speed <= f64::EPSILON {
                Velocity2::zero()
            } else {
                Velocity2::from_polar(
                    InchesPerSecond::new(Inches::from_f64(final_speed)),
                    heading_after_curve,
                )
            };
            let radius = ball.radius.as_f64();
            let rolling_wx = if final_speed <= f64::EPSILON {
                0.0
            } else {
                -final_velocity.y().as_f64() / radius
            };
            let rolling_wy = if final_speed <= f64::EPSILON {
                0.0
            } else {
                final_velocity.x().as_f64() / radius
            };

            OnTableBallState::try_from(BallState {
                position: Inches2::new(
                    Inches::from_f64(state.position.x().as_f64() + dx),
                    Inches::from_f64(state.position.y().as_f64() + dy),
                ),
                height: state.height.clone(),
                velocity: final_velocity,
                vertical_velocity: state.vertical_velocity.clone(),
                angular_velocity: AngularVelocity3::new(
                    rolling_wx,
                    rolling_wy,
                    advance_vertical_axis_spin(
                        state.angular_velocity.z().as_f64(),
                        Seconds::new(advance_time),
                        config,
                    ),
                ),
            })
            .expect("rolling phase advance should preserve on-table invariants")
        }
        MotionPhase::Spinning => OnTableBallState::try_from(BallState {
            position: state.position.clone(),
            height: state.height.clone(),
            velocity: state.velocity.clone(),
            vertical_velocity: state.vertical_velocity.clone(),
            angular_velocity: AngularVelocity3::new(
                state.angular_velocity.x().as_f64(),
                state.angular_velocity.y().as_f64(),
                advance_vertical_axis_spin(state.angular_velocity.z().as_f64(), dt, config),
            ),
        })
        .expect("spinning phase advance should preserve on-table invariants"),
        MotionPhase::Airborne => todo!("airborne state advance is not implemented yet"),
    }
}

/// Advance the current on-table motion model through a full requested duration.
///
/// If the duration crosses one or more phase boundaries, the returned `transition` records the
/// first such boundary and `state` contains the final state after the entire requested `elapsed`
/// time has been consumed.
pub fn advance_motion_on_table(
    state: &OnTableBallState,
    dt: Seconds,
    ball: &BallSetPhysicsSpec,
    config: &OnTableMotionConfig,
) -> MotionAdvance {
    assert!(dt.as_f64() >= 0.0, "advance duration must be non-negative");

    let state_ref = state.as_ball_state();

    if dt.as_f64() == 0.0 {
        return MotionAdvance {
            state: state_ref.clone(),
            elapsed: dt,
            transition: None,
        };
    }

    let phase = classify_motion_phase(state_ref, ball, &config.phase);
    let next_transition = compute_next_transition_on_table(state, ball, config);

    match next_transition {
        None => MotionAdvance {
            state: advance_within_phase_on_table(state, phase, dt, ball, config).into_ball_state(),
            elapsed: dt,
            transition: None,
        },
        Some(transition) if dt.as_f64() <= transition.time_until_transition.as_f64() => {
            MotionAdvance {
                state: advance_within_phase_on_table(state, phase, dt, ball, config)
                    .into_ball_state(),
                elapsed: dt,
                transition: None,
            }
        }
        Some(transition) => {
            let at_transition = advance_within_phase_on_table(
                state,
                phase,
                transition.time_until_transition,
                ball,
                config,
            );
            let remainder = Seconds::new(dt.as_f64() - transition.time_until_transition.as_f64());
            let advanced = advance_motion_on_table(&at_transition, remainder, ball, config);

            MotionAdvance {
                state: advanced.state,
                elapsed: dt,
                transition: Some(transition),
            }
        }
    }
}

/// Compatibility wrapper for the older motion API name.
pub fn advance_ball_state(
    state: &BallState,
    dt: Seconds,
    ball: &BallSetPhysicsSpec,
    config: &MotionTransitionConfig,
) -> BallState {
    match classify_motion_phase(state, ball, &config.phase) {
        MotionPhase::Airborne => todo!("airborne state advance is not implemented yet"),
        _ => {
            advance_motion_on_table(
                &require_on_table_state(state, &config.phase),
                dt,
                ball,
                config,
            )
            .state
        }
    }
}

/// Advance the total on-table angular velocity implied by the current motion model.
///
/// This is a convenience helper for callers that care only about spin evolution. It still uses
/// the full `BallState` because the horizontal components are coupled to translational motion in
/// the sliding and rolling references, while the vertical component decays through the z-spin
/// model.
pub fn advance_spin_on_table(
    state: &OnTableBallState,
    dt: Seconds,
    ball: &BallSetPhysicsSpec,
    config: &OnTableMotionConfig,
) -> AngularVelocity3 {
    advance_motion_on_table(state, dt, ball, config)
        .state
        .angular_velocity
}

/// Compatibility wrapper for the older angular-velocity helper name.
pub fn advance_angular_velocity_on_table(
    state: &BallState,
    dt: Seconds,
    ball: &BallSetPhysicsSpec,
    config: &MotionTransitionConfig,
) -> AngularVelocity3 {
    advance_spin_on_table(
        &require_on_table_state(state, &config.phase),
        dt,
        ball,
        config,
    )
}

fn advance_on_table_with_constant_velocity(
    state: &OnTableBallState,
    dt: Seconds,
) -> OnTableBallState {
    let state = state.as_ball_state();

    OnTableBallState::try_from(BallState::on_table(
        Inches2::new(
            Inches::from_f64(
                state.position.x().as_f64() + state.velocity.x().as_f64() * dt.as_f64(),
            ),
            Inches::from_f64(
                state.position.y().as_f64() + state.velocity.y().as_f64() * dt.as_f64(),
            ),
        ),
        state.velocity.clone(),
        state.angular_velocity.clone(),
    ))
    .expect("constant-velocity advance should preserve on-table invariants")
}

fn center_distance_squared(a: &OnTableBallState, b: &OnTableBallState) -> f64 {
    let a = a.as_ball_state();
    let b = b.as_ball_state();
    let dx = b.position.x().as_f64() - a.position.x().as_f64();
    let dy = b.position.y().as_f64() - a.position.y().as_f64();

    dx * dx + dy * dy
}

fn advance_within_current_phase(
    state: &OnTableBallState,
    phase: MotionPhase,
    dt: Seconds,
    ball: &BallSetPhysicsSpec,
    config: &OnTableMotionConfig,
) -> OnTableBallState {
    advance_within_phase_on_table(state, phase, dt, ball, config)
}

fn ball_ball_collision_search_horizon(
    state: &OnTableBallState,
    ball: &BallSetPhysicsSpec,
    config: &OnTableMotionConfig,
) -> f64 {
    compute_next_transition_on_table(state, ball, config)
        .map(|transition| transition.time_until_transition.as_f64())
        .unwrap_or(f64::INFINITY)
}

fn collision_gap_during_current_phases(
    a: &OnTableBallState,
    a_phase: MotionPhase,
    b: &OnTableBallState,
    b_phase: MotionPhase,
    t: Seconds,
    ball: &BallSetPhysicsSpec,
    config: &OnTableMotionConfig,
) -> f64 {
    let a_at_t = advance_within_current_phase(a, a_phase, t, ball, config);
    let b_at_t = advance_within_current_phase(b, b_phase, t, ball, config);
    let contact_distance = 2.0 * ball.radius.as_f64();

    center_distance_squared(&a_at_t, &b_at_t) - contact_distance * contact_distance
}

fn refine_ball_ball_collision_time_during_current_phases(
    a: &OnTableBallState,
    a_phase: MotionPhase,
    b: &OnTableBallState,
    b_phase: MotionPhase,
    mut left: f64,
    mut right: f64,
    ball: &BallSetPhysicsSpec,
    config: &OnTableMotionConfig,
) -> Seconds {
    for _ in 0..60 {
        let midpoint = 0.5 * (left + right);
        let gap = collision_gap_during_current_phases(
            a,
            a_phase.clone(),
            b,
            b_phase.clone(),
            Seconds::new(midpoint),
            ball,
            config,
        );

        if gap <= 0.0 {
            right = midpoint;
        } else {
            left = midpoint;
        }
    }

    Seconds::new(right)
}

const THROW_AWARE_MAX_ANGLE_DEGREES: f64 = 5.0;
const THROW_AWARE_NUMERICAL_ZERO_SLIP_RATIO: f64 = 1e-12;
const AVERAGE_BALL_BALL_FRICTION_COEFFICIENT: f64 = 0.06;

fn collision_contact_basis(a: &OnTableBallState, b: &OnTableBallState) -> (f64, f64, f64, f64) {
    let a_state = a.as_ball_state();
    let b_state = b.as_ball_state();
    let dx = b_state.position.x().as_f64() - a_state.position.x().as_f64();
    let dy = b_state.position.y().as_f64() - a_state.position.y().as_f64();
    let center_distance = dx.hypot(dy);

    assert!(
        center_distance > f64::EPSILON,
        "ball-ball collision requires distinct ball centers"
    );

    let normal_x = dx / center_distance;
    let normal_y = dy / center_distance;
    let tangent_x = normal_y;
    let tangent_y = -normal_x;

    (normal_x, normal_y, tangent_x, tangent_y)
}

fn project_velocity_on_basis(velocity: &Velocity2, basis_x: f64, basis_y: f64) -> f64 {
    velocity.x().as_f64() * basis_x + velocity.y().as_f64() * basis_y
}

fn signed_angle_difference_degrees(from: Angle, to: Angle) -> f64 {
    let delta = (to.as_degrees() - from.as_degrees()).rem_euclid(360.0);
    if delta > 180.0 {
        delta - 360.0
    } else {
        delta
    }
}

fn rotate_angle_degrees(angle: Angle, delta: f64) -> Angle {
    Angle((angle.as_degrees() + delta).rem_euclid(360.0))
}

#[derive(Clone, Copy, Debug)]
struct SideSpinCurve {
    duration: Seconds,
    angle_degrees: f64,
    heading_after_curve: Angle,
}

fn rotate_xy_clockwise(x: f64, y: f64, delta_degrees: f64) -> (f64, f64) {
    let radians = delta_degrees.to_radians();
    let sin = radians.sin();
    let cos = radians.cos();

    (x * cos + y * sin, y * cos - x * sin)
}

fn side_spin_curve_from_speed_window(
    start_heading: Angle,
    initial_speed: f64,
    speed_at_curve_end: f64,
    initial_z_spin: f64,
    curve_duration: Seconds,
    phase_horizon: Seconds,
    ball: &BallSetPhysicsSpec,
    config: &MotionTransitionConfig,
) -> Option<SideSpinCurve> {
    if curve_duration.as_f64() <= f64::EPSILON
        || phase_horizon.as_f64() <= f64::EPSILON
        || initial_speed <= f64::EPSILON
        || initial_z_spin.abs() <= f64::EPSILON
    {
        return None;
    }

    let end_z_spin = advance_vertical_axis_spin(initial_z_spin, curve_duration, config);
    let average_speed = 0.5 * (initial_speed + speed_at_curve_end);
    if average_speed <= f64::EPSILON {
        return None;
    }

    let average_z_spin = 0.5 * (initial_z_spin + end_z_spin);
    let slip_ratio = (ball.radius.as_f64() * average_z_spin / average_speed).clamp(-1.0, 1.0);
    if slip_ratio.abs() <= f64::EPSILON {
        return None;
    }

    let duration_fraction = (curve_duration.as_f64() / phase_horizon.as_f64()).clamp(0.0, 1.0);
    let angle_degrees = (slip_ratio * duration_fraction).to_degrees();
    if angle_degrees.abs() <= f64::EPSILON {
        return None;
    }

    Some(SideSpinCurve {
        duration: curve_duration,
        angle_degrees,
        heading_after_curve: rotate_angle_degrees(start_heading, angle_degrees),
    })
}

fn sliding_side_spin_curve_during(
    start_heading: Angle,
    vx_i: f64,
    vy_i: f64,
    vx_c: f64,
    vy_c: f64,
    initial_z_spin: f64,
    max_duration: Seconds,
    transition_time: Seconds,
    ball: &BallSetPhysicsSpec,
    config: &MotionTransitionConfig,
) -> Option<SideSpinCurve> {
    let initial_speed = vx_i.hypot(vy_i);
    if max_duration.as_f64() <= f64::EPSILON
        || transition_time.as_f64() <= f64::EPSILON
        || initial_speed <= f64::EPSILON
        || initial_z_spin.abs() <= f64::EPSILON
    {
        return None;
    }

    let curve_duration = Seconds::new(
        time_until_vertical_axis_spin_stops(initial_z_spin, config)
            .expect("non-zero z-spin should have a stop time")
            .as_f64()
            .min(max_duration.as_f64())
            .min(transition_time.as_f64()),
    );
    if curve_duration.as_f64() <= f64::EPSILON {
        return None;
    }

    let alpha = curve_duration.as_f64() / transition_time.as_f64();
    let vx_curve = vx_i - alpha * (vx_i - vx_c);
    let vy_curve = vy_i - alpha * (vy_i - vy_c);
    let speed_at_curve_end = vx_curve.hypot(vy_curve);

    side_spin_curve_from_speed_window(
        start_heading,
        initial_speed,
        speed_at_curve_end,
        initial_z_spin,
        curve_duration,
        transition_time,
        ball,
        config,
    )
}

fn rolling_side_spin_curve_during(
    start_heading: Angle,
    initial_speed: f64,
    initial_z_spin: f64,
    max_duration: Seconds,
    ball: &BallSetPhysicsSpec,
    config: &MotionTransitionConfig,
) -> Option<SideSpinCurve> {
    if max_duration.as_f64() <= f64::EPSILON
        || initial_speed <= f64::EPSILON
        || initial_z_spin.abs() <= f64::EPSILON
    {
        return None;
    }

    let stop_time = initial_speed / rolling_linear_deceleration(config);
    if stop_time <= f64::EPSILON {
        return None;
    }

    let curve_duration = Seconds::new(
        time_until_vertical_axis_spin_stops(initial_z_spin, config)
            .expect("non-zero z-spin should have a stop time")
            .as_f64()
            .min(max_duration.as_f64())
            .min(stop_time),
    );
    if curve_duration.as_f64() <= f64::EPSILON {
        return None;
    }

    let linear_deceleration = rolling_linear_deceleration(config);
    let speed_at_curve_end =
        (initial_speed - linear_deceleration * curve_duration.as_f64()).max(0.0);

    side_spin_curve_from_speed_window(
        start_heading,
        initial_speed,
        speed_at_curve_end,
        initial_z_spin,
        curve_duration,
        Seconds::new(stop_time),
        ball,
        config,
    )
}

fn build_on_table_ball_state(
    position: Inches2,
    velocity: Velocity2,
    angular_velocity: AngularVelocity3,
) -> OnTableBallState {
    OnTableBallState::try_from(BallState::on_table(position, velocity, angular_velocity))
        .expect("ball-ball collision should preserve on-table invariants")
}

fn ideal_ball_ball_collision_velocities(
    a: &Velocity2,
    b: &Velocity2,
    normal_x: f64,
    normal_y: f64,
) -> (Velocity2, Velocity2) {
    let tangent_x = normal_y;
    let tangent_y = -normal_x;
    let project = |velocity: &Velocity2, basis_x: f64, basis_y: f64| {
        velocity.x().as_f64() * basis_x + velocity.y().as_f64() * basis_y
    };

    let a_normal = project(a, normal_x, normal_y);
    let a_tangent = project(a, tangent_x, tangent_y);
    let b_normal = project(b, normal_x, normal_y);
    let b_tangent = project(b, tangent_x, tangent_y);

    let rebuild = |normal_component: f64, tangent_component: f64| {
        Velocity2::new(
            Inches::from_f64(normal_component * normal_x + tangent_component * tangent_x),
            Inches::from_f64(normal_component * normal_y + tangent_component * tangent_y),
        )
    };

    (rebuild(b_normal, a_tangent), rebuild(a_normal, b_tangent))
}

/// Predict the next future ball-ball impact for two on-table balls under a constant-velocity
/// pre-impact approximation.
///
/// This helper is retained as a simple compatibility approximation. For scheduler work that should
/// stay consistent with the current on-table motion model, prefer
/// `compute_next_ball_ball_collision_during_current_phases_on_table(...)`.
///
/// The local references ground the collision geometry itself:
///
/// - `whitepapers/Physics Of Billiards.html` describes the struck ball moving along the line
///   joining the ball centers at contact, which implies impact occurs when the center distance first
///   reaches `2R`.
/// - `whitepapers/art_of_billiards_play_files/bil_praa.html` treats the ball-ball event as an
///   instantaneous collision at a single contact configuration, which is the event this helper
///   predicts.
///
/// Within that contact geometry, this helper uses the current translational velocities as a local
/// constant-velocity approximation and solves the standard relative-motion quadratic for the first
/// future time `t > 0` such that `|r + v t| = 2R`. Angular velocity does not affect the timing in
/// this first-pass ideal scheduler.
pub fn compute_next_ball_ball_collision_on_table(
    a: &OnTableBallState,
    b: &OnTableBallState,
    ball: &BallSetPhysicsSpec,
) -> Option<PredictedBallBallCollision> {
    let a_state = a.as_ball_state();
    let b_state = b.as_ball_state();
    let rx = b_state.position.x().as_f64() - a_state.position.x().as_f64();
    let ry = b_state.position.y().as_f64() - a_state.position.y().as_f64();
    let vx = b_state.velocity.x().as_f64() - a_state.velocity.x().as_f64();
    let vy = b_state.velocity.y().as_f64() - a_state.velocity.y().as_f64();
    let contact_distance = 2.0 * ball.radius.as_f64();
    let quadratic_a = vx * vx + vy * vy;
    let quadratic_b = 2.0 * (rx * vx + ry * vy);
    let quadratic_c = rx * rx + ry * ry - contact_distance * contact_distance;

    if quadratic_c <= 0.0 || quadratic_a <= f64::EPSILON || quadratic_b >= 0.0 {
        return None;
    }

    let discriminant = quadratic_b * quadratic_b - 4.0 * quadratic_a * quadratic_c;
    if discriminant < -f64::EPSILON {
        return None;
    }

    let impact_time = (-quadratic_b - discriminant.max(0.0).sqrt()) / (2.0 * quadratic_a);
    if impact_time < 0.0 {
        return None;
    }

    let time_until_impact = Seconds::new(impact_time);

    Some(PredictedBallBallCollision {
        time_until_impact,
        a_at_impact: advance_on_table_with_constant_velocity(a, time_until_impact),
        b_at_impact: advance_on_table_with_constant_velocity(b, time_until_impact),
    })
}

/// Predict the next future ball-ball impact that occurs before either ball leaves its current
/// qualitative motion phase.
///
/// This helper combines the current whitepaper-backed on-table motion model with the standard
/// ball-ball contact geometry:
///
/// - `whitepapers/art_of_billiards_play_files/bil_praa.html`, §7.3 and §7.5, provide the current
///   within-phase sliding, rolling, and z-spin evolution used by `advance_within_phase_on_table(...)`.
/// - `whitepapers/Physics Of Billiards.html` describes the ball-ball impact geometry through the
///   line of centers, so first contact still occurs when the center distance reaches `2R`.
///
/// Rather than extrapolating with constant translational velocity, this predictor advances each ball
/// with the current within-phase motion model and numerically locates the first contact time, if
/// any, over the interval from `t = 0` up to the earliest upcoming single-ball motion transition.
/// If no contact occurs before that phase boundary, the caller should let the earlier transition
/// happen first and then recompute collision timing from the new states.
pub fn compute_next_ball_ball_collision_during_current_phases_on_table(
    a: &OnTableBallState,
    b: &OnTableBallState,
    ball: &BallSetPhysicsSpec,
    config: &OnTableMotionConfig,
) -> Option<PredictedBallBallCollision> {
    let initial_gap = center_distance_squared(a, b) - (2.0 * ball.radius.as_f64()).powi(2);
    if initial_gap <= 0.0 {
        return None;
    }

    let a_phase = classify_motion_phase(a.as_ball_state(), ball, &config.phase);
    let b_phase = classify_motion_phase(b.as_ball_state(), ball, &config.phase);
    let horizon = ball_ball_collision_search_horizon(a, ball, config)
        .min(ball_ball_collision_search_horizon(b, ball, config));
    if !horizon.is_finite() || horizon <= f64::EPSILON {
        return None;
    }

    const COLLISION_SCAN_STEPS: usize = 512;
    let mut previous_t = 0.0;
    let mut previous_gap = initial_gap;

    for step in 1..=COLLISION_SCAN_STEPS {
        let t = horizon * step as f64 / COLLISION_SCAN_STEPS as f64;
        let gap = collision_gap_during_current_phases(
            a,
            a_phase.clone(),
            b,
            b_phase.clone(),
            Seconds::new(t),
            ball,
            config,
        );

        if previous_gap > 0.0 && gap <= 0.0 {
            let time_until_impact = refine_ball_ball_collision_time_during_current_phases(
                a,
                a_phase.clone(),
                b,
                b_phase.clone(),
                previous_t,
                t,
                ball,
                config,
            );
            let a_at_impact =
                advance_within_current_phase(a, a_phase.clone(), time_until_impact, ball, config);
            let b_at_impact =
                advance_within_current_phase(b, b_phase.clone(), time_until_impact, ball, config);

            return Some(PredictedBallBallCollision {
                time_until_impact,
                a_at_impact,
                b_at_impact,
            });
        }

        previous_t = t;
        previous_gap = gap;
    }

    None
}

fn rail_collision_plane_coordinate(
    rail: Rail,
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
) -> f64 {
    match rail {
        Rail::Top => table.diamond_to_inches(Diamond::eight()).as_f64() - ball.radius.as_f64(),
        Rail::Bottom => ball.radius.as_f64(),
        Rail::Left => ball.radius.as_f64(),
        Rail::Right => table.diamond_to_inches(Diamond::four()).as_f64() - ball.radius.as_f64(),
    }
}

fn rail_collision_gap_during_current_phase(
    state: &OnTableBallState,
    phase: MotionPhase,
    rail: Rail,
    t: Seconds,
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    config: &OnTableMotionConfig,
) -> f64 {
    let at_t = advance_within_current_phase(state, phase, t, ball, config);
    let state_at_t = at_t.as_ball_state();
    let plane = rail_collision_plane_coordinate(rail, ball, table);

    match rail {
        Rail::Top => plane - state_at_t.position.y().as_f64(),
        Rail::Bottom => state_at_t.position.y().as_f64() - plane,
        Rail::Left => state_at_t.position.x().as_f64() - plane,
        Rail::Right => plane - state_at_t.position.x().as_f64(),
    }
}

fn refine_ball_rail_collision_time_during_current_phase(
    state: &OnTableBallState,
    phase: MotionPhase,
    rail: Rail,
    mut left: f64,
    mut right: f64,
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    config: &OnTableMotionConfig,
) -> Seconds {
    for _ in 0..60 {
        let midpoint = 0.5 * (left + right);
        let gap = rail_collision_gap_during_current_phase(
            state,
            phase.clone(),
            rail,
            Seconds::new(midpoint),
            ball,
            table,
            config,
        );

        if gap <= 0.0 {
            right = midpoint;
        } else {
            left = midpoint;
        }
    }

    Seconds::new(right)
}

/// Predict the next future rail impact for one on-table ball.
///
/// This helper uses the same within-phase on-table motion model as the current single-ball solver,
/// but checks for first contact against the four ideal rail planes implied by the table geometry.
/// The current implementation only searches until the ball's next motion transition, making this
/// the rail analogue of `compute_next_ball_ball_collision_during_current_phases_on_table(...)`.
pub fn compute_next_ball_rail_impact_on_table(
    state: &OnTableBallState,
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    config: &OnTableMotionConfig,
) -> Option<PredictedBallRailImpact> {
    let phase = classify_motion_phase(state.as_ball_state(), ball, &config.phase);
    let horizon = compute_next_transition_on_table(state, ball, config)
        .map(|transition| transition.time_until_transition.as_f64())
        .unwrap_or(f64::INFINITY);
    if !horizon.is_finite() || horizon <= f64::EPSILON {
        return None;
    }

    const RAIL_COLLISION_SCAN_STEPS: usize = 512;
    let mut best: Option<PredictedBallRailImpact> = None;

    for rail in [Rail::Top, Rail::Right, Rail::Bottom, Rail::Left] {
        let initial_gap = rail_collision_gap_during_current_phase(
            state,
            phase.clone(),
            rail,
            Seconds::zero(),
            ball,
            table,
            config,
        );
        if initial_gap <= 0.0 {
            continue;
        }

        let mut previous_t = 0.0;
        let mut previous_gap = initial_gap;

        for step in 1..=RAIL_COLLISION_SCAN_STEPS {
            let t = horizon * step as f64 / RAIL_COLLISION_SCAN_STEPS as f64;
            let gap = rail_collision_gap_during_current_phase(
                state,
                phase.clone(),
                rail,
                Seconds::new(t),
                ball,
                table,
                config,
            );

            if previous_gap > 0.0 && gap <= 0.0 {
                let time_until_impact = refine_ball_rail_collision_time_during_current_phase(
                    state,
                    phase.clone(),
                    rail,
                    previous_t,
                    t,
                    ball,
                    table,
                    config,
                );
                let state_at_impact = advance_within_current_phase(
                    state,
                    phase.clone(),
                    time_until_impact,
                    ball,
                    config,
                );
                let impact = PredictedBallRailImpact {
                    rail,
                    time_until_impact,
                    state_at_impact,
                };

                if best.as_ref().is_none_or(|current| {
                    impact.time_until_impact.as_f64() < current.time_until_impact.as_f64()
                }) {
                    best = Some(impact);
                }
                break;
            }

            previous_t = t;
            previous_gap = gap;
        }
    }

    best
}

fn pocket_center_in_inches(pocket: Pocket, table: &TableSpec) -> (f64, f64) {
    let center = pocket.aiming_center();
    (
        table.diamond_to_inches(center.x).as_f64(),
        table.diamond_to_inches(center.y).as_f64(),
    )
}

fn pocket_capture_radius_in_inches(pocket: Pocket, table: &TableSpec) -> f64 {
    0.5 * table
        .diamond_to_inches(table.pockets[pocket.index()].width.clone())
        .as_f64()
}

fn pocket_capture_gap_during_current_phase(
    state: &OnTableBallState,
    phase: MotionPhase,
    pocket: Pocket,
    t: Seconds,
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    config: &OnTableMotionConfig,
) -> f64 {
    let at_t = advance_within_current_phase(state, phase, t, ball, config);
    let state_at_t = at_t.as_ball_state();
    let (pocket_x, pocket_y) = pocket_center_in_inches(pocket, table);
    let dx = state_at_t.position.x().as_f64() - pocket_x;
    let dy = state_at_t.position.y().as_f64() - pocket_y;

    dx.hypot(dy) - pocket_capture_radius_in_inches(pocket, table)
}

fn refine_ball_pocket_capture_time_during_current_phase(
    state: &OnTableBallState,
    phase: MotionPhase,
    pocket: Pocket,
    mut left: f64,
    mut right: f64,
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    config: &OnTableMotionConfig,
) -> Seconds {
    for _ in 0..60 {
        let midpoint = 0.5 * (left + right);
        let gap = pocket_capture_gap_during_current_phase(
            state,
            phase.clone(),
            pocket,
            Seconds::new(midpoint),
            ball,
            table,
            config,
        );

        if gap <= 0.0 {
            right = midpoint;
        } else {
            left = midpoint;
        }
    }

    Seconds::new(right)
}

/// Predict the next future pocket capture for one on-table ball.
///
/// This is the pocket analogue of `compute_next_ball_rail_impact_on_table(...)`: it advances the
/// current within-phase motion model up to the ball's next motion transition and checks for first
/// entry into a conservative first-pass pocket capture region around each pocket's current aiming
/// center.
pub fn compute_next_ball_pocket_capture_on_table(
    state: &OnTableBallState,
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    config: &OnTableMotionConfig,
) -> Option<PredictedBallPocketCapture> {
    let phase = classify_motion_phase(state.as_ball_state(), ball, &config.phase);
    let horizon = compute_next_transition_on_table(state, ball, config)
        .map(|transition| transition.time_until_transition.as_f64())
        .unwrap_or(f64::INFINITY);
    if !horizon.is_finite() || horizon <= f64::EPSILON {
        return None;
    }

    const POCKET_CAPTURE_SCAN_STEPS: usize = 512;
    let mut best: Option<PredictedBallPocketCapture> = None;

    for pocket in Pocket::ALL {
        let initial_gap = pocket_capture_gap_during_current_phase(
            state,
            phase.clone(),
            pocket,
            Seconds::zero(),
            ball,
            table,
            config,
        );
        if initial_gap <= 0.0 {
            continue;
        }

        let mut previous_t = 0.0;
        let mut previous_gap = initial_gap;

        for step in 1..=POCKET_CAPTURE_SCAN_STEPS {
            let t = horizon * step as f64 / POCKET_CAPTURE_SCAN_STEPS as f64;
            let gap = pocket_capture_gap_during_current_phase(
                state,
                phase.clone(),
                pocket,
                Seconds::new(t),
                ball,
                table,
                config,
            );

            if previous_gap > 0.0 && gap <= 0.0 {
                let time_until_capture = refine_ball_pocket_capture_time_during_current_phase(
                    state,
                    phase.clone(),
                    pocket,
                    previous_t,
                    t,
                    ball,
                    table,
                    config,
                );
                let state_at_capture = advance_within_current_phase(
                    state,
                    phase.clone(),
                    time_until_capture,
                    ball,
                    config,
                );
                let capture = PredictedBallPocketCapture {
                    pocket,
                    time_until_capture,
                    state_at_capture,
                };

                if best.as_ref().is_none_or(|current| {
                    capture.time_until_capture.as_f64() < current.time_until_capture.as_f64()
                }) {
                    best = Some(capture);
                }
                break;
            }

            previous_t = t;
            previous_gap = gap;
        }
    }

    best
}

#[derive(Clone, Debug, PartialEq)]
enum SingleBallOnTableEvent {
    RailImpact(PredictedBallRailImpact),
    MotionTransition(NextTransition),
}

impl SingleBallOnTableEvent {
    fn time(&self) -> Seconds {
        match self {
            SingleBallOnTableEvent::RailImpact(impact) => impact.time_until_impact,
            SingleBallOnTableEvent::MotionTransition(transition) => {
                transition.time_until_transition
            }
        }
    }
}

fn single_ball_event_priority(event: &SingleBallOnTableEvent) -> u8 {
    match event {
        SingleBallOnTableEvent::RailImpact(_) => 0,
        SingleBallOnTableEvent::MotionTransition(_) => 1,
    }
}

fn earlier_single_ball_event(
    candidate: &SingleBallOnTableEvent,
    current: &SingleBallOnTableEvent,
) -> bool {
    let candidate_time = candidate.time().as_f64();
    let current_time = current.time().as_f64();

    candidate_time < current_time
        || ((candidate_time - current_time).abs() <= 1e-12
            && single_ball_event_priority(candidate) < single_ball_event_priority(current))
}

fn compute_next_single_ball_event_with_rails_on_table(
    state: &OnTableBallState,
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    config: &OnTableMotionConfig,
) -> Option<SingleBallOnTableEvent> {
    let mut next = compute_next_ball_rail_impact_on_table(state, ball, table, config)
        .map(SingleBallOnTableEvent::RailImpact);

    if let Some(transition) = compute_next_transition_on_table(state, ball, config) {
        let candidate = SingleBallOnTableEvent::MotionTransition(transition);
        if next
            .as_ref()
            .is_none_or(|current| earlier_single_ball_event(&candidate, current))
        {
            next = Some(candidate);
        }
    }

    next
}

fn two_ball_event_ball_from_index(ball_index: usize) -> TwoBallEventBall {
    match ball_index {
        TWO_BALL_A_INDEX => TwoBallEventBall::A,
        TWO_BALL_B_INDEX => TwoBallEventBall::B,
        _ => panic!("two-ball scheduler ball index must be 0 or 1, got {ball_index}"),
    }
}

fn two_ball_event_from_n_ball_event(event: NBallOnTableEvent) -> TwoBallOnTableEvent {
    match event {
        NBallOnTableEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } => match (first_ball_index, second_ball_index) {
            (TWO_BALL_A_INDEX, TWO_BALL_B_INDEX) => TwoBallOnTableEvent::BallBallCollision(collision),
            _ => panic!(
                "two-ball collision event indices must be 0/1, got {first_ball_index}/{second_ball_index}"
            ),
        },
        NBallOnTableEvent::BallRailImpact { ball_index, impact } => TwoBallOnTableEvent::BallRailImpact {
            ball: two_ball_event_ball_from_index(ball_index),
            impact,
        },
        NBallOnTableEvent::MotionTransition {
            ball_index,
            transition,
        } => TwoBallOnTableEvent::MotionTransition {
            ball: two_ball_event_ball_from_index(ball_index),
            transition,
        },
    }
}

fn earlier_n_ball_event_candidate(
    candidate: &NBallSystemEventCandidate,
    current: &NBallSystemEventCandidate,
) -> bool {
    let candidate_time = candidate.event.time().as_f64();
    let current_time = current.event.time().as_f64();

    candidate_time < current_time
        || ((candidate_time - current_time).abs() <= 1e-12 && candidate.source < current.source)
}

fn select_earliest_n_ball_event_from_states(
    states: &[&OnTableBallState],
    ball: &BallSetPhysicsSpec,
    table: Option<&TableSpec>,
    config: &OnTableMotionConfig,
) -> Option<NBallOnTableEvent> {
    let mut next: Option<NBallSystemEventCandidate> = None;

    for first_ball_index in 0..states.len() {
        for second_ball_index in first_ball_index + 1..states.len() {
            let Some(collision) = compute_next_ball_ball_collision_during_current_phases_on_table(
                states[first_ball_index],
                states[second_ball_index],
                ball,
                config,
            ) else {
                continue;
            };
            let candidate = NBallSystemEventCandidate {
                source: NBallSystemEventSource::BallBallCollision {
                    first_ball_index,
                    second_ball_index,
                },
                event: NBallOnTableEvent::BallBallCollision {
                    first_ball_index,
                    second_ball_index,
                    collision,
                },
            };

            if next
                .as_ref()
                .is_none_or(|current| earlier_n_ball_event_candidate(&candidate, current))
            {
                next = Some(candidate);
            }
        }
    }

    if let Some(table) = table {
        for (ball_index, state) in states.iter().enumerate() {
            let Some(impact) = compute_next_ball_rail_impact_on_table(state, ball, table, config)
            else {
                continue;
            };
            let candidate = NBallSystemEventCandidate {
                source: NBallSystemEventSource::BallRailImpact { ball_index },
                event: NBallOnTableEvent::BallRailImpact { ball_index, impact },
            };

            if next
                .as_ref()
                .is_none_or(|current| earlier_n_ball_event_candidate(&candidate, current))
            {
                next = Some(candidate);
            }
        }
    }

    for (ball_index, state) in states.iter().enumerate() {
        let Some(transition) = compute_next_transition_on_table(state, ball, config) else {
            continue;
        };
        let candidate = NBallSystemEventCandidate {
            source: NBallSystemEventSource::MotionTransition { ball_index },
            event: NBallOnTableEvent::MotionTransition {
                ball_index,
                transition,
            },
        };

        if next
            .as_ref()
            .is_none_or(|current| earlier_n_ball_event_candidate(&candidate, current))
        {
            next = Some(candidate);
        }
    }

    next.map(|candidate| candidate.event)
}

/// Compute the earliest currently supported future event among any number of on-table balls.
///
/// This is the indexed / N-ball analogue of `compute_next_two_ball_event_on_table(...)`. It
/// evaluates every pair's next predicted ball-ball collision plus every ball's next on-table motion
/// transition, then returns the earliest candidate event, if any.
pub fn compute_next_n_ball_event_on_table(
    states: &[&OnTableBallState],
    ball: &BallSetPhysicsSpec,
    config: &OnTableMotionConfig,
) -> Option<NBallOnTableEvent> {
    select_earliest_n_ball_event_from_states(states, ball, None, config)
}

/// Compute the earliest supported future event among any number of on-table balls while also
/// considering rail impacts against the current table geometry.
///
/// This extends `compute_next_n_ball_event_on_table(...)` by adding each ball's next predicted rail
/// impact as an additional candidate source.
pub fn compute_next_n_ball_event_with_rails_on_table(
    states: &[&OnTableBallState],
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    config: &OnTableMotionConfig,
) -> Option<NBallOnTableEvent> {
    select_earliest_n_ball_event_from_states(states, ball, Some(table), config)
}

/// Compute the earliest currently supported future event for two on-table balls.
///
/// This compatibility helper preserves the older two-ball public event shape while delegating the
/// actual candidate selection to `compute_next_n_ball_event_on_table(...)`.
pub fn compute_next_two_ball_event_on_table(
    a: &OnTableBallState,
    b: &OnTableBallState,
    ball: &BallSetPhysicsSpec,
    config: &OnTableMotionConfig,
) -> Option<TwoBallOnTableEvent> {
    compute_next_n_ball_event_on_table(&[a, b], ball, config).map(two_ball_event_from_n_ball_event)
}

/// Compute the earliest supported future event for two on-table balls while also considering ideal
/// rail impacts against the current table geometry.
///
/// This compatibility helper preserves the older two-ball public event shape while delegating the
/// actual candidate selection to `compute_next_n_ball_event_with_rails_on_table(...)`.
pub fn compute_next_two_ball_event_with_rails_on_table(
    a: &OnTableBallState,
    b: &OnTableBallState,
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    config: &OnTableMotionConfig,
) -> Option<TwoBallOnTableEvent> {
    compute_next_n_ball_event_with_rails_on_table(&[a, b], ball, table, config)
        .map(two_ball_event_from_n_ball_event)
}

/// Compatibility wrapper for the original two-ball event helper name.
pub fn compute_next_event_for_two_on_table_balls(
    a: &OnTableBallState,
    b: &OnTableBallState,
    ball: &BallSetPhysicsSpec,
    config: &OnTableMotionConfig,
) -> Option<TwoBallOnTableEvent> {
    compute_next_two_ball_event_on_table(a, b, ball, config)
}

/// Compatibility wrapper for the original rail-aware two-ball event helper name.
pub fn compute_next_event_for_two_on_table_balls_with_rails(
    a: &OnTableBallState,
    b: &OnTableBallState,
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    config: &OnTableMotionConfig,
) -> Option<TwoBallOnTableEvent> {
    compute_next_two_ball_event_with_rails_on_table(a, b, ball, table, config)
}

fn advance_on_table_ball_without_event(
    state: &OnTableBallState,
    dt: Seconds,
    ball: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
) -> OnTableBallState {
    OnTableBallState::try_from(advance_motion_on_table(state, dt, ball, motion).state)
        .expect("two-ball motion advance should preserve on-table invariants")
}

fn advance_two_on_table_balls_without_event(
    a: &OnTableBallState,
    b: &OnTableBallState,
    dt: Seconds,
    ball: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
) -> (OnTableBallState, OnTableBallState) {
    (
        advance_on_table_ball_without_event(a, dt, ball, motion),
        advance_on_table_ball_without_event(b, dt, ball, motion),
    )
}

fn advance_n_on_table_balls_without_event(
    states: &[OnTableBallState],
    dt: Seconds,
    ball: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
) -> Vec<OnTableBallState> {
    states
        .iter()
        .map(|state| advance_on_table_ball_without_event(state, dt, ball, motion))
        .collect()
}

fn advance_to_next_n_ball_event_with_scheduler<FindNextEvent>(
    states: &[OnTableBallState],
    ball: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
    rail_response: Option<(RailModel, RailCollisionConfig)>,
    find_next_event: FindNextEvent,
) -> NBallOnTableAdvance
where
    FindNextEvent: Fn(&[&OnTableBallState]) -> Option<NBallOnTableEvent>,
{
    let state_refs = states.iter().collect::<Vec<_>>();
    let Some(event) = find_next_event(&state_refs) else {
        return NBallOnTableAdvance {
            states: states.to_vec(),
            elapsed: Seconds::zero(),
            event: None,
        };
    };

    let elapsed = event.time();
    let mut states_after = advance_n_on_table_balls_without_event(states, elapsed, ball, motion);
    match &event {
        NBallOnTableEvent::MotionTransition { .. } => {}
        NBallOnTableEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } => {
            let (first_after, second_after) = collide_ball_ball_on_table(
                &collision.a_at_impact,
                &collision.b_at_impact,
                collision_model,
            );
            states_after[*first_ball_index] = first_after;
            states_after[*second_ball_index] = second_after;
        }
        NBallOnTableEvent::BallRailImpact { ball_index, impact } => {
            let (rail_model, rail_config) = rail_response
                .as_ref()
                .expect("rail impacts require a rail collision model");
            states_after[*ball_index] = collide_ball_rail_on_table_with_radius_and_config(
                &impact.state_at_impact,
                impact.rail,
                ball.radius.clone(),
                *rail_model,
                rail_config,
            );
        }
    }

    NBallOnTableAdvance {
        states: states_after,
        elapsed,
        event: Some(event),
    }
}

/// Advance any number of on-table balls to the next supported event and resolve it.
///
/// This is the indexed / N-ball execution step built on top of `compute_next_n_ball_event_on_table(...)`.
pub fn advance_to_next_n_ball_event_on_table(
    states: &[OnTableBallState],
    ball: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
) -> NBallOnTableAdvance {
    advance_to_next_n_ball_event_with_scheduler(
        states,
        ball,
        motion,
        collision_model,
        None,
        |state_refs| compute_next_n_ball_event_on_table(state_refs, ball, motion),
    )
}

/// Advance any number of on-table balls to the next supported event while also resolving rail
/// impacts using explicit rail-response coefficients.
pub fn advance_to_next_n_ball_event_with_rail_config_on_table(
    states: &[OnTableBallState],
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
    rail_model: RailModel,
    rail_config: &RailCollisionConfig,
) -> NBallOnTableAdvance {
    advance_to_next_n_ball_event_with_scheduler(
        states,
        ball,
        motion,
        collision_model,
        Some((rail_model, rail_config.clone())),
        |state_refs| compute_next_n_ball_event_with_rails_on_table(state_refs, ball, table, motion),
    )
}

/// Advance any number of on-table balls to the next supported event while also resolving rail
/// impacts against the current table geometry.
///
/// This compatibility wrapper uses the default rail-response coefficients. Prefer
/// `advance_to_next_n_ball_event_with_rail_config_on_table(...)` when restitution should be
/// explicit.
pub fn advance_to_next_n_ball_event_with_rails_on_table(
    states: &[OnTableBallState],
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
    rail_model: RailModel,
) -> NBallOnTableAdvance {
    advance_to_next_n_ball_event_with_rail_config_on_table(
        states,
        ball,
        table,
        motion,
        collision_model,
        rail_model,
        &RailCollisionConfig::default(),
    )
}

/// Advance two on-table balls to the next supported event and resolve it.
///
/// This compatibility helper preserves the older two-ball public shape while delegating the actual
/// execution to `advance_to_next_n_ball_event_on_table(...)`.
pub fn advance_to_next_two_ball_event_on_table(
    a: &OnTableBallState,
    b: &OnTableBallState,
    ball: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
) -> TwoBallOnTableAdvance {
    let advanced = advance_to_next_n_ball_event_on_table(
        &[a.clone(), b.clone()],
        ball,
        motion,
        collision_model,
    );
    let [a_after, b_after]: [OnTableBallState; 2] = advanced
        .states
        .try_into()
        .expect("two-ball advance should return exactly two states");

    TwoBallOnTableAdvance {
        a: a_after,
        b: b_after,
        elapsed: advanced.elapsed,
        event: advanced.event.map(two_ball_event_from_n_ball_event),
    }
}

/// Advance two on-table balls to the next supported event while also resolving rail impacts using
/// explicit rail-response coefficients.
pub fn advance_to_next_two_ball_event_with_rail_config_on_table(
    a: &OnTableBallState,
    b: &OnTableBallState,
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
    rail_model: RailModel,
    rail_config: &RailCollisionConfig,
) -> TwoBallOnTableAdvance {
    let advanced = advance_to_next_n_ball_event_with_rail_config_on_table(
        &[a.clone(), b.clone()],
        ball,
        table,
        motion,
        collision_model,
        rail_model,
        rail_config,
    );
    let [a_after, b_after]: [OnTableBallState; 2] = advanced
        .states
        .try_into()
        .expect("two-ball advance should return exactly two states");

    TwoBallOnTableAdvance {
        a: a_after,
        b: b_after,
        elapsed: advanced.elapsed,
        event: advanced.event.map(two_ball_event_from_n_ball_event),
    }
}

/// Advance two on-table balls to the next supported event while also resolving rail impacts against
/// the current table geometry.
///
/// This compatibility wrapper uses the default rail-response coefficients. Prefer
/// `advance_to_next_two_ball_event_with_rail_config_on_table(...)` when restitution should be
/// explicit.
pub fn advance_to_next_two_ball_event_with_rails_on_table(
    a: &OnTableBallState,
    b: &OnTableBallState,
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
    rail_model: RailModel,
) -> TwoBallOnTableAdvance {
    advance_to_next_two_ball_event_with_rail_config_on_table(
        a,
        b,
        ball,
        table,
        motion,
        collision_model,
        rail_model,
        &RailCollisionConfig::default(),
    )
}

/// Compatibility wrapper for the original two-ball advance helper name.
pub fn advance_to_next_event_for_two_on_table_balls(
    a: &OnTableBallState,
    b: &OnTableBallState,
    ball: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
) -> TwoBallOnTableAdvance {
    advance_to_next_two_ball_event_on_table(a, b, ball, motion, collision_model)
}

fn simulate_two_ball_system_on_table<FindNextEvent, AdvanceNextEvent>(
    a: &OnTableBallState,
    b: &OnTableBallState,
    dt: Seconds,
    ball: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
    find_next_event: FindNextEvent,
    advance_next_event: AdvanceNextEvent,
) -> TwoBallOnTableSimulation
where
    FindNextEvent: Fn(&OnTableBallState, &OnTableBallState) -> Option<TwoBallOnTableEvent>,
    AdvanceNextEvent: Fn(&OnTableBallState, &OnTableBallState) -> TwoBallOnTableAdvance,
{
    assert!(
        dt.as_f64() >= 0.0,
        "simulation duration must be non-negative"
    );

    let mut a_state = a.clone();
    let mut b_state = b.clone();
    let mut elapsed = Seconds::zero();
    let mut remaining = dt.as_f64();
    let mut events = Vec::new();

    while remaining > f64::EPSILON {
        let Some(next_event) = find_next_event(&a_state, &b_state) else {
            let (a_after, b_after) = advance_two_on_table_balls_without_event(
                &a_state,
                &b_state,
                Seconds::new(remaining),
                ball,
                motion,
            );
            a_state = a_after;
            b_state = b_after;
            elapsed = Seconds::new(elapsed.as_f64() + remaining);
            break;
        };

        let event_time = next_event.time().as_f64();
        if event_time > remaining {
            let (a_after, b_after) = advance_two_on_table_balls_without_event(
                &a_state,
                &b_state,
                Seconds::new(remaining),
                ball,
                motion,
            );
            a_state = a_after;
            b_state = b_after;
            elapsed = Seconds::new(elapsed.as_f64() + remaining);
            break;
        }

        let advanced = advance_next_event(&a_state, &b_state);
        let step_elapsed = advanced.elapsed.as_f64();
        assert!(
            step_elapsed > f64::EPSILON,
            "next two-ball event must advance simulation time"
        );

        a_state = advanced.a;
        b_state = advanced.b;
        elapsed = Seconds::new(elapsed.as_f64() + step_elapsed);
        remaining -= step_elapsed;

        if let Some(event) = advanced.event {
            events.push(event);
        }
    }

    TwoBallOnTableSimulation {
        a: a_state,
        b: b_state,
        elapsed,
        events,
    }
}

/// Simulate two on-table balls forward over a requested duration.
///
/// This repeatedly chooses the currently earliest supported event, advances / resolves it, and
/// continues until either the full requested duration has been consumed or no further event occurs
/// within the remaining time budget. Any leftover time after the last in-window event is consumed
/// by advancing both balls through ordinary on-table motion without recording an additional event.
pub fn simulate_two_balls_on_table(
    a: &OnTableBallState,
    b: &OnTableBallState,
    dt: Seconds,
    ball: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
) -> TwoBallOnTableSimulation {
    simulate_two_ball_system_on_table(
        a,
        b,
        dt,
        ball,
        motion,
        |a_state, b_state| compute_next_two_ball_event_on_table(a_state, b_state, ball, motion),
        |a_state, b_state| {
            advance_to_next_two_ball_event_on_table(a_state, b_state, ball, motion, collision_model)
        },
    )
}

/// Simulate two on-table balls forward over a requested duration while also resolving rail impacts
/// using explicit rail-response coefficients.
pub fn simulate_two_balls_with_rail_config_on_table(
    a: &OnTableBallState,
    b: &OnTableBallState,
    dt: Seconds,
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
    rail_model: RailModel,
    rail_config: &RailCollisionConfig,
) -> TwoBallOnTableSimulation {
    simulate_two_ball_system_on_table(
        a,
        b,
        dt,
        ball,
        motion,
        |a_state, b_state| {
            compute_next_two_ball_event_with_rails_on_table(a_state, b_state, ball, table, motion)
        },
        |a_state, b_state| {
            advance_to_next_two_ball_event_with_rail_config_on_table(
                a_state,
                b_state,
                ball,
                table,
                motion,
                collision_model,
                rail_model,
                rail_config,
            )
        },
    )
}

/// Simulate two on-table balls forward over a requested duration while also resolving rail impacts
/// against the current table geometry.
///
/// This compatibility wrapper uses the default rail-response coefficients. Prefer
/// `simulate_two_balls_with_rail_config_on_table(...)` when restitution should be explicit.
pub fn simulate_two_balls_with_rails_on_table(
    a: &OnTableBallState,
    b: &OnTableBallState,
    dt: Seconds,
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
    rail_model: RailModel,
) -> TwoBallOnTableSimulation {
    simulate_two_balls_with_rail_config_on_table(
        a,
        b,
        dt,
        ball,
        table,
        motion,
        collision_model,
        rail_model,
        &RailCollisionConfig::default(),
    )
}

/// Compatibility wrapper for the original two-ball simulation helper name.
pub fn simulate_two_on_table_balls(
    a: &OnTableBallState,
    b: &OnTableBallState,
    dt: Seconds,
    ball: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
) -> TwoBallOnTableSimulation {
    simulate_two_balls_on_table(a, b, dt, ball, motion, collision_model)
}

fn simulate_n_ball_system_on_table_until_rest<AdvanceNextEvent>(
    states: &[OnTableBallState],
    advance_next_event: AdvanceNextEvent,
) -> NBallOnTableSimulation
where
    AdvanceNextEvent: Fn(&[OnTableBallState]) -> NBallOnTableAdvance,
{
    let mut states = states.to_vec();
    let mut elapsed = Seconds::zero();
    let mut events = Vec::new();

    loop {
        let advanced = advance_next_event(&states);
        let step_elapsed = advanced.elapsed.as_f64();

        let Some(event) = advanced.event else {
            assert!(
                step_elapsed.abs() <= f64::EPSILON,
                "n-ball until-rest simulation should not consume time without an event"
            );
            states = advanced.states;
            break;
        };

        assert!(
            step_elapsed > f64::EPSILON,
            "next n-ball event must advance simulation time"
        );

        states = advanced.states;
        elapsed = Seconds::new(elapsed.as_f64() + step_elapsed);
        events.push(event);
    }

    NBallOnTableSimulation {
        states,
        elapsed,
        events,
    }
}

/// Simulate any number of on-table balls until the current scheduler finds no further supported
/// event.
///
/// Under the current on-table, no-pocket, no-airborne slice, this is the natural "until rest"
/// simulation entry point: it repeatedly chooses the earliest indexed event, advances / resolves
/// it, and stops when all remaining balls are resting and separated.
pub fn simulate_n_balls_on_table_until_rest(
    states: &[OnTableBallState],
    ball: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
) -> NBallOnTableSimulation {
    simulate_n_ball_system_on_table_until_rest(states, |current| {
        advance_to_next_n_ball_event_on_table(current, ball, motion, collision_model)
    })
}

/// Simulate any number of on-table balls until rest while also resolving rail impacts using
/// explicit rail-response coefficients.
pub fn simulate_n_balls_with_rail_config_on_table_until_rest(
    states: &[OnTableBallState],
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
    rail_model: RailModel,
    rail_config: &RailCollisionConfig,
) -> NBallOnTableSimulation {
    simulate_n_ball_system_on_table_until_rest(states, |current| {
        advance_to_next_n_ball_event_with_rail_config_on_table(
            current,
            ball,
            table,
            motion,
            collision_model,
            rail_model,
            rail_config,
        )
    })
}

/// Simulate any number of on-table balls until rest while also resolving rail impacts against the
/// current table geometry.
///
/// This compatibility wrapper uses the default rail-response coefficients. Prefer
/// `simulate_n_balls_with_rail_config_on_table_until_rest(...)` when restitution should be
/// explicit.
pub fn simulate_n_balls_with_rails_on_table_until_rest(
    states: &[OnTableBallState],
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
    rail_model: RailModel,
) -> NBallOnTableSimulation {
    simulate_n_balls_with_rail_config_on_table_until_rest(
        states,
        ball,
        table,
        motion,
        collision_model,
        rail_model,
        &RailCollisionConfig::default(),
    )
}

fn advance_n_ball_system_without_event(
    states: &[NBallSystemState],
    dt: Seconds,
    ball: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
) -> Vec<NBallSystemState> {
    states
        .iter()
        .map(|state| match state {
            NBallSystemState::OnTable(on_table) => NBallSystemState::OnTable(
                advance_on_table_ball_without_event(on_table, dt, ball, motion),
            ),
            NBallSystemState::Pocketed {
                pocket,
                state_at_capture,
            } => NBallSystemState::Pocketed {
                pocket: *pocket,
                state_at_capture: state_at_capture.clone(),
            },
        })
        .collect()
}

fn earlier_n_ball_pocket_aware_event_candidate(
    candidate: &NBallPocketAwareSystemEventCandidate,
    current: &NBallPocketAwareSystemEventCandidate,
) -> bool {
    let candidate_time = candidate.event.time().as_f64();
    let current_time = current.event.time().as_f64();

    candidate_time < current_time
        || ((candidate_time - current_time).abs() <= 1e-12 && candidate.source < current.source)
}

/// Compute the earliest supported future event for the richer indexed N-ball system while also
/// considering rail impacts and pocket captures against the current table geometry.
///
/// Pocketed balls are inert and excluded from future prediction. Airborne / leaves-table handling
/// remains intentionally unsupported in this first-pass system layer.
pub fn compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
    states: &[NBallSystemState],
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    config: &OnTableMotionConfig,
) -> Option<NBallSystemEvent> {
    let mut next: Option<NBallPocketAwareSystemEventCandidate> = None;

    for first_ball_index in 0..states.len() {
        let Some(first_state) = states[first_ball_index].as_on_table() else {
            continue;
        };

        for second_ball_index in first_ball_index + 1..states.len() {
            let Some(second_state) = states[second_ball_index].as_on_table() else {
                continue;
            };
            let Some(collision) = compute_next_ball_ball_collision_during_current_phases_on_table(
                first_state,
                second_state,
                ball,
                config,
            ) else {
                continue;
            };
            let candidate = NBallPocketAwareSystemEventCandidate {
                source: NBallPocketAwareSystemEventSource::BallBallCollision {
                    first_ball_index,
                    second_ball_index,
                },
                event: NBallSystemEvent::BallBallCollision {
                    first_ball_index,
                    second_ball_index,
                    collision,
                },
            };

            if next.as_ref().is_none_or(|current| {
                earlier_n_ball_pocket_aware_event_candidate(&candidate, current)
            }) {
                next = Some(candidate);
            }
        }

        if let Some(capture) =
            compute_next_ball_pocket_capture_on_table(first_state, ball, table, config)
        {
            let candidate = NBallPocketAwareSystemEventCandidate {
                source: NBallPocketAwareSystemEventSource::BallPocketCapture {
                    ball_index: first_ball_index,
                },
                event: NBallSystemEvent::BallPocketCapture {
                    ball_index: first_ball_index,
                    capture,
                },
            };

            if next.as_ref().is_none_or(|current| {
                earlier_n_ball_pocket_aware_event_candidate(&candidate, current)
            }) {
                next = Some(candidate);
            }
        }

        if let Some(impact) =
            compute_next_ball_rail_impact_on_table(first_state, ball, table, config)
        {
            let candidate = NBallPocketAwareSystemEventCandidate {
                source: NBallPocketAwareSystemEventSource::BallRailImpact {
                    ball_index: first_ball_index,
                },
                event: NBallSystemEvent::BallRailImpact {
                    ball_index: first_ball_index,
                    impact,
                },
            };

            if next.as_ref().is_none_or(|current| {
                earlier_n_ball_pocket_aware_event_candidate(&candidate, current)
            }) {
                next = Some(candidate);
            }
        }

        if let Some(transition) = compute_next_transition_on_table(first_state, ball, config) {
            let candidate = NBallPocketAwareSystemEventCandidate {
                source: NBallPocketAwareSystemEventSource::MotionTransition {
                    ball_index: first_ball_index,
                },
                event: NBallSystemEvent::MotionTransition {
                    ball_index: first_ball_index,
                    transition,
                },
            };

            if next.as_ref().is_none_or(|current| {
                earlier_n_ball_pocket_aware_event_candidate(&candidate, current)
            }) {
                next = Some(candidate);
            }
        }
    }

    next.map(|candidate| candidate.event)
}

/// Advance the richer indexed N-ball system to the next supported event while also resolving rail
/// impacts using explicit rail-response coefficients and pocket captures against the current table
/// geometry.
pub fn advance_to_next_n_ball_system_event_with_rail_config_and_pockets_on_table(
    states: &[NBallSystemState],
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
    rail_model: RailModel,
    rail_config: &RailCollisionConfig,
) -> NBallSystemAdvance {
    let Some(event) = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
        states, ball, table, motion,
    ) else {
        return NBallSystemAdvance {
            states: states.to_vec(),
            elapsed: Seconds::zero(),
            event: None,
        };
    };

    let elapsed = event.time();
    let mut states_after = advance_n_ball_system_without_event(states, elapsed, ball, motion);
    match &event {
        NBallSystemEvent::MotionTransition { .. } => {}
        NBallSystemEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } => {
            let (first_after, second_after) = collide_ball_ball_on_table(
                &collision.a_at_impact,
                &collision.b_at_impact,
                collision_model,
            );
            states_after[*first_ball_index] = NBallSystemState::OnTable(first_after);
            states_after[*second_ball_index] = NBallSystemState::OnTable(second_after);
        }
        NBallSystemEvent::BallPocketCapture {
            ball_index,
            capture,
        } => {
            states_after[*ball_index] = NBallSystemState::Pocketed {
                pocket: capture.pocket,
                state_at_capture: capture.state_at_capture.clone(),
            };
        }
        NBallSystemEvent::BallRailImpact { ball_index, impact } => {
            states_after[*ball_index] =
                NBallSystemState::OnTable(collide_ball_rail_on_table_with_radius_and_config(
                    &impact.state_at_impact,
                    impact.rail,
                    ball.radius.clone(),
                    rail_model,
                    rail_config,
                ));
        }
    }

    NBallSystemAdvance {
        states: states_after,
        elapsed,
        event: Some(event),
    }
}

/// Advance the richer indexed N-ball system to the next supported event while also resolving rail
/// impacts against the current table geometry and using default rail-response coefficients.
pub fn advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
    states: &[NBallSystemState],
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
    rail_model: RailModel,
) -> NBallSystemAdvance {
    advance_to_next_n_ball_system_event_with_rail_config_and_pockets_on_table(
        states,
        ball,
        table,
        motion,
        collision_model,
        rail_model,
        &RailCollisionConfig::default(),
    )
}

fn simulate_n_ball_system_with_pockets_until_rest<AdvanceNextEvent>(
    states: &[NBallSystemState],
    advance_next_event: AdvanceNextEvent,
) -> NBallSystemSimulation
where
    AdvanceNextEvent: Fn(&[NBallSystemState]) -> NBallSystemAdvance,
{
    let mut states = states.to_vec();
    let mut elapsed = Seconds::zero();
    let mut events = Vec::new();

    loop {
        let advanced = advance_next_event(&states);
        let step_elapsed = advanced.elapsed.as_f64();

        let Some(event) = advanced.event else {
            assert!(
                step_elapsed.abs() <= f64::EPSILON,
                "pocket-aware n-ball until-rest simulation should not consume time without an event"
            );
            states = advanced.states;
            break;
        };

        assert!(
            step_elapsed > f64::EPSILON,
            "next pocket-aware n-ball event must advance simulation time"
        );

        states = advanced.states;
        elapsed = Seconds::new(elapsed.as_f64() + step_elapsed);
        events.push(event);
    }

    NBallSystemSimulation {
        states,
        elapsed,
        events,
    }
}

/// Simulate the richer indexed N-ball system until rest while also resolving rail impacts using
/// explicit rail-response coefficients and pocket captures.
pub fn simulate_n_ball_system_with_rail_config_and_pockets_on_table_until_rest(
    states: &[NBallSystemState],
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
    rail_model: RailModel,
    rail_config: &RailCollisionConfig,
) -> NBallSystemSimulation {
    simulate_n_ball_system_with_pockets_until_rest(states, |current| {
        advance_to_next_n_ball_system_event_with_rail_config_and_pockets_on_table(
            current,
            ball,
            table,
            motion,
            collision_model,
            rail_model,
            rail_config,
        )
    })
}

/// Simulate the richer indexed N-ball system until rest while also resolving rail impacts and
/// pocket captures against the current table geometry.
pub fn simulate_n_ball_system_with_rails_and_pockets_on_table_until_rest(
    states: &[NBallSystemState],
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
    rail_model: RailModel,
) -> NBallSystemSimulation {
    simulate_n_ball_system_with_rail_config_and_pockets_on_table_until_rest(
        states,
        ball,
        table,
        motion,
        collision_model,
        rail_model,
        &RailCollisionConfig::default(),
    )
}

/// Simulate any number of initially on-table balls until rest while also resolving rail impacts and
/// pocket captures against the current table geometry.
pub fn simulate_n_balls_with_rails_and_pockets_on_table_until_rest(
    states: &[OnTableBallState],
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    motion: &OnTableMotionConfig,
    collision_model: CollisionModel,
    rail_model: RailModel,
) -> NBallSystemSimulation {
    let system_states = states
        .iter()
        .cloned()
        .map(NBallSystemState::from)
        .collect::<Vec<_>>();
    simulate_n_ball_system_with_rails_and_pockets_on_table_until_rest(
        &system_states,
        ball,
        table,
        motion,
        collision_model,
        rail_model,
    )
}

fn ball_path_segment_has_visible_displacement(
    start: &OnTableBallState,
    end: &OnTableBallState,
) -> bool {
    let dx =
        end.as_ball_state().position.x().as_f64() - start.as_ball_state().position.x().as_f64();
    let dy =
        end.as_ball_state().position.y().as_f64() - start.as_ball_state().position.y().as_f64();

    dx.abs() > 1e-12 || dy.abs() > 1e-12
}

fn push_visible_ball_path_segment(
    segments: &mut Vec<BallPathSegment>,
    start: &OnTableBallState,
    end: &OnTableBallState,
    duration: Seconds,
) {
    if ball_path_segment_has_visible_displacement(start, end) {
        segments.push(BallPathSegment {
            start: start.clone(),
            end: end.clone(),
            duration,
        });
    }
}

/// Trace a single ball forward over the table while resolving rail impacts using explicit rail
/// response coefficients.
///
/// This is currently a first-pass event-vertex trace: it records visible straight segments between
/// the current state, any in-window motion-transition vertices, any in-window rail impacts, and the
/// final requested stop condition. It does not yet sample within-phase side-spin curvature inside a
/// segment.
pub fn trace_ball_path_with_rail_config_on_table(
    state: &OnTableBallState,
    stop: BallPathStop,
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    motion: &OnTableMotionConfig,
    rail_model: RailModel,
    rail_config: &RailCollisionConfig,
) -> BallPath {
    let mut current = state.clone();
    let mut elapsed = Seconds::zero();
    let mut rail_impacts = 0usize;
    let mut segments = Vec::new();
    let mut remaining_time = match stop {
        BallPathStop::Duration(dt) => {
            assert!(dt.as_f64() >= 0.0, "trace duration must be non-negative");
            Some(dt.as_f64())
        }
        _ => None,
    };
    let mut remaining_rail_impacts = match stop {
        BallPathStop::RailImpacts(count) => Some(count),
        _ => None,
    };

    loop {
        if remaining_time.is_some_and(|time| time <= f64::EPSILON)
            || remaining_rail_impacts == Some(0)
            || classify_motion_phase(current.as_ball_state(), ball, &motion.phase)
                == MotionPhase::Rest
        {
            break;
        }

        let Some(next_event) =
            compute_next_single_ball_event_with_rails_on_table(&current, ball, table, motion)
        else {
            if let Some(remaining) = remaining_time {
                let end = advance_on_table_ball_without_event(
                    &current,
                    Seconds::new(remaining),
                    ball,
                    motion,
                );
                push_visible_ball_path_segment(
                    &mut segments,
                    &current,
                    &end,
                    Seconds::new(remaining),
                );
                current = end;
                elapsed = Seconds::new(elapsed.as_f64() + remaining);
            }
            break;
        };

        let step_time = next_event.time().as_f64();
        if step_time <= f64::EPSILON {
            break;
        }

        if let Some(remaining) = remaining_time {
            if step_time > remaining {
                let end = advance_on_table_ball_without_event(
                    &current,
                    Seconds::new(remaining),
                    ball,
                    motion,
                );
                push_visible_ball_path_segment(
                    &mut segments,
                    &current,
                    &end,
                    Seconds::new(remaining),
                );
                current = end;
                elapsed = Seconds::new(elapsed.as_f64() + remaining);
                break;
            }
        }

        match next_event {
            SingleBallOnTableEvent::MotionTransition(transition) => {
                let end = advance_on_table_ball_without_event(
                    &current,
                    transition.time_until_transition,
                    ball,
                    motion,
                );
                push_visible_ball_path_segment(
                    &mut segments,
                    &current,
                    &end,
                    transition.time_until_transition,
                );
                current = end;
                elapsed = Seconds::new(elapsed.as_f64() + step_time);
            }
            SingleBallOnTableEvent::RailImpact(impact) => {
                let rail = impact.rail;
                let impact_duration = impact.time_until_impact;
                let impact_state = impact.state_at_impact;
                push_visible_ball_path_segment(
                    &mut segments,
                    &current,
                    &impact_state,
                    impact_duration,
                );
                current = impact_state;
                elapsed = Seconds::new(elapsed.as_f64() + step_time);
                rail_impacts += 1;

                if let Some(remaining) = remaining_rail_impacts.as_mut() {
                    *remaining = remaining.saturating_sub(1);
                    if *remaining == 0 {
                        break;
                    }
                }

                current = collide_ball_rail_on_table_with_radius_and_config(
                    &current,
                    rail,
                    ball.radius.clone(),
                    rail_model,
                    rail_config,
                );
            }
        }

        if let Some(remaining) = remaining_time.as_mut() {
            *remaining = (*remaining - step_time).max(0.0);
        }
    }

    BallPath {
        initial_state: state.clone(),
        final_state: current,
        elapsed,
        rail_impacts,
        segments,
    }
}

/// Trace a single ball forward over the table while resolving rail impacts against the current
/// table geometry.
///
/// This compatibility wrapper uses the default rail-response coefficients. Prefer
/// `trace_ball_path_with_rail_config_on_table(...)` when restitution should be explicit.
pub fn trace_ball_path_with_rails_on_table(
    state: &OnTableBallState,
    stop: BallPathStop,
    ball: &BallSetPhysicsSpec,
    table: &TableSpec,
    motion: &OnTableMotionConfig,
    rail_model: RailModel,
) -> BallPath {
    trace_ball_path_with_rail_config_on_table(
        state,
        stop,
        ball,
        table,
        motion,
        rail_model,
        &RailCollisionConfig::default(),
    )
}

fn ideal_collision_outcome_on_table(
    a: &OnTableBallState,
    b: &OnTableBallState,
) -> CollisionOutcome {
    let a_state = a.as_ball_state();
    let b_state = b.as_ball_state();
    let (normal_x, normal_y, _, _) = collision_contact_basis(a, b);
    let (a_velocity, b_velocity) = ideal_ball_ball_collision_velocities(
        &a_state.velocity,
        &b_state.velocity,
        normal_x,
        normal_y,
    );

    CollisionOutcome {
        a_after: build_on_table_ball_state(
            a_state.position.clone(),
            a_velocity,
            a_state.angular_velocity.clone(),
        ),
        b_after: build_on_table_ball_state(
            b_state.position.clone(),
            b_velocity,
            b_state.angular_velocity.clone(),
        ),
        throw_angle_degrees: None,
        transferred_spin: None,
    }
}

fn transferred_spin_from_contact_slip(
    tangent_x: f64,
    tangent_y: f64,
    tangential_contact_slip: f64,
    vertical_contact_slip: f64,
    normal_relative_speed: f64,
    ball_radius: f64,
) -> Option<AngularVelocity3> {
    // `whitepapers/art_of_billiards_play_files/bil_praa.html`, Eqs. (C10), (C11), and (C13),
    // support the same qualitative structure we already use for spin-aware rails: the no-slip
    // spin-change limit is proportional to the contact-slip vector, but the realized change is
    // capped by a friction-limited impulse scale. The earlier helper applied only the no-slip
    // limit, which substantially over-seeded cue-ball `ωz` for ordinary no-English cut shots.
    // Here we therefore scale that same limit by a first-pass friction cap based on the current
    // ball-ball friction coefficient and the closing normal speed.
    let contact_slip_norm = tangential_contact_slip.hypot(vertical_contact_slip);
    if contact_slip_norm <= f64::EPSILON || normal_relative_speed <= f64::EPSILON {
        return None;
    }

    let no_slip_delta_scale = 5.0 / (14.0 * ball_radius);
    let friction_limited_scale = (AVERAGE_BALL_BALL_FRICTION_COEFFICIENT * normal_relative_speed
        / contact_slip_norm)
        .clamp(0.0, 1.0);
    let spin_gain_scale = no_slip_delta_scale * friction_limited_scale;
    let delta_x = -spin_gain_scale * vertical_contact_slip * tangent_x;
    let delta_y = -spin_gain_scale * vertical_contact_slip * tangent_y;
    let delta_z = spin_gain_scale * tangential_contact_slip;

    if delta_x.abs() <= 1e-9 && delta_y.abs() <= 1e-9 && delta_z.abs() <= 1e-9 {
        None
    } else {
        Some(AngularVelocity3::new(delta_x, delta_y, delta_z))
    }
}

fn spin_post_impact_cue_state_from_tp_a8_a24(
    a: &OnTableBallState,
    b: &OnTableBallState,
    ball_radius: f64,
) -> Option<(Velocity2, f64, f64)> {
    let a_state = a.as_ball_state();
    let b_state = b.as_ball_state();
    if ball_speed(b_state).as_f64() > 1e-9 {
        return None;
    }

    let shot_speed = ball_speed(a_state).as_f64();
    if shot_speed <= f64::EPSILON {
        return None;
    }
    let (normal_x, normal_y, _, _) = collision_contact_basis(a, b);
    let shot_heading = a_state.velocity.angle_from_north()?;
    let object_heading = Angle::from_north(normal_x, normal_y);
    let cut_angle = CutAngle::from_headings(shot_heading, object_heading);
    if cut_angle.as_degrees() <= 1e-9 {
        return None;
    }

    let shot_x = a_state.velocity.x().as_f64() / shot_speed;
    let shot_y = a_state.velocity.y().as_f64() / shot_speed;
    let shot_right_x = shot_y;
    let shot_right_y = -shot_x;
    let signed_cut =
        cut_angle.as_degrees().to_radians() * (shot_x * normal_y - shot_y * normal_x).signum();
    if signed_cut.abs() <= 1e-9 {
        return None;
    }

    let local_pre_angular_x = shot_right_x * a_state.angular_velocity.x().as_f64()
        + shot_right_y * a_state.angular_velocity.y().as_f64();
    let local_pre_angular_y = shot_x * a_state.angular_velocity.x().as_f64()
        + shot_y * a_state.angular_velocity.y().as_f64();
    let english = a_state.angular_velocity.z().as_f64();
    let mu_balls = AVERAGE_BALL_BALL_FRICTION_COEFFICIENT;
    let cos_phi = signed_cut.cos();
    let sin_phi = signed_cut.sin();

    // `whitepapers/tp_a_8_the_effects_of_english_on_the_30_degree_rule.pdf` derives the shot-
    // aligned contact-point slip geometry for cut shots with English, while
    // `whitepapers/tp_a_24_the_effects_of_follow_and_draw_on_throw_and_ob_swerve.pdf` makes the
    // same tangential / vertical slip decomposition explicit for combined follow/draw (`ωx`) and
    // side spin (`ωz`). We therefore seed the common cut-shot cue-ball branch from those two slip
    // components in the shot basis, using the current local horizontal spin components instead of
    // assuming only natural roll.
    let tangential_contact_slip = shot_speed * sin_phi - ball_radius * english;
    let vertical_contact_slip =
        ball_radius * (local_pre_angular_x * cos_phi + local_pre_angular_y * sin_phi);
    if tangential_contact_slip.abs() <= f64::EPSILON && vertical_contact_slip.abs() <= f64::EPSILON
    {
        return None;
    }
    let contact_slip_denominator =
        (tangential_contact_slip.powi(2) + vertical_contact_slip.powi(2)).sqrt();
    if contact_slip_denominator <= f64::EPSILON {
        return None;
    }

    let tangential_impulse_per_mass =
        -mu_balls * shot_speed * cos_phi * tangential_contact_slip / contact_slip_denominator;
    let vertical_impulse_per_mass =
        -mu_balls * shot_speed * cos_phi * vertical_contact_slip / contact_slip_denominator;
    let local_tangential_velocity = shot_speed * sin_phi + tangential_impulse_per_mass;
    let local_velocity_x = local_tangential_velocity * cos_phi;
    let local_velocity_y = local_tangential_velocity * sin_phi;
    let local_angular_x =
        local_pre_angular_x + (5.0 / (2.0 * ball_radius)) * cos_phi * vertical_impulse_per_mass;
    let local_angular_y =
        local_pre_angular_y + (5.0 / (2.0 * ball_radius)) * sin_phi * vertical_impulse_per_mass;

    let velocity = Velocity2::new(
        Inches::from_f64(shot_right_x * local_velocity_x + shot_x * local_velocity_y),
        Inches::from_f64(shot_right_y * local_velocity_x + shot_y * local_velocity_y),
    );
    let angular_x = shot_right_x * local_angular_x + shot_x * local_angular_y;
    let angular_y = shot_right_y * local_angular_x + shot_y * local_angular_y;

    Some((velocity, angular_x, angular_y))
}

fn throw_aware_collision_outcome_on_table(
    a: &OnTableBallState,
    b: &OnTableBallState,
) -> CollisionOutcome {
    let ideal = ideal_collision_outcome_on_table(a, b);
    let a_state = a.as_ball_state();
    let b_state = b.as_ball_state();

    // TODO(physics): model the later post-contact cue-ball bend caused by residual follow / draw /
    // side spin interacting with the cloth after impact.
    if ball_speed(b_state).as_f64() > 1e-9 {
        return ideal;
    }

    let (normal_x, normal_y, tangent_x, tangent_y) = collision_contact_basis(a, b);
    let ball_radius = 0.5 * center_distance_squared(a, b).sqrt();
    let tangential_relative_speed =
        project_velocity_on_basis(&a_state.velocity, tangent_x, tangent_y)
            - project_velocity_on_basis(&b_state.velocity, tangent_x, tangent_y);
    let normal_relative_speed = (project_velocity_on_basis(&a_state.velocity, normal_x, normal_y)
        - project_velocity_on_basis(&b_state.velocity, normal_x, normal_y))
    .max(0.0);
    let tangential_contact_slip = tangential_relative_speed
        - ball_radius
            * (a_state.angular_velocity.z().as_f64() + b_state.angular_velocity.z().as_f64());
    let vertical_contact_slip = ball_radius
        * (normal_y
            * (a_state.angular_velocity.x().as_f64() + b_state.angular_velocity.x().as_f64())
            - normal_x
                * (a_state.angular_velocity.y().as_f64() + b_state.angular_velocity.y().as_f64()));
    let slip_scale = tangential_relative_speed.abs()
        + ball_radius
            * (a_state.angular_velocity.z().as_f64().abs()
                + b_state.angular_velocity.z().as_f64().abs());
    let numerical_zero_slip =
        THROW_AWARE_NUMERICAL_ZERO_SLIP_RATIO * normal_relative_speed.max(1.0);
    let throw_angle_degrees = if slip_scale <= numerical_zero_slip {
        0.0
    } else {
        THROW_AWARE_MAX_ANGLE_DEGREES * (tangential_contact_slip / slip_scale).clamp(-1.0, 1.0)
    };

    let object_speed = ball_speed(ideal.b_after.as_ball_state()).as_f64();
    let throw_radians = throw_angle_degrees.to_radians();
    let b_velocity = Velocity2::new(
        Inches::from_f64(
            object_speed * (throw_radians.cos() * normal_x + throw_radians.sin() * tangent_x),
        ),
        Inches::from_f64(
            object_speed * (throw_radians.cos() * normal_y + throw_radians.sin() * tangent_y),
        ),
    );
    let total_momentum_x = a_state.velocity.x().as_f64() + b_state.velocity.x().as_f64();
    let total_momentum_y = a_state.velocity.y().as_f64() + b_state.velocity.y().as_f64();
    let a_velocity_from_momentum = Velocity2::new(
        Inches::from_f64(total_momentum_x - b_velocity.x().as_f64()),
        Inches::from_f64(total_momentum_y - b_velocity.y().as_f64()),
    );

    // `whitepapers/art_of_billiards_play_files/bil_praa.html`, Eqs. (C11) and (C13), imply that in
    // the equal-ball adherence / no-slip limit the collision-induced spin increment is proportional
    // to `x* × WCa`. Using our on-table basis, `WCa` has an in-plane tangential component driven by
    // cut / side-spin slip and a vertical component driven by top / bottom spin, which lets this
    // first-pass model transfer both z-spin and the horizontal spin component aligned with the shot.
    let transferred_spin = transferred_spin_from_contact_slip(
        tangent_x,
        tangent_y,
        tangential_contact_slip,
        vertical_contact_slip,
        normal_relative_speed,
        ball_radius,
    );
    let spin_delta_x = transferred_spin
        .as_ref()
        .map(|spin| spin.x().as_f64())
        .unwrap_or(0.0);
    let spin_delta_y = transferred_spin
        .as_ref()
        .map(|spin| spin.y().as_f64())
        .unwrap_or(0.0);
    let spin_delta_z = transferred_spin
        .as_ref()
        .map(|spin| spin.z().as_f64())
        .unwrap_or(0.0);
    let tp_a8_cue_state = spin_post_impact_cue_state_from_tp_a8_a24(a, b, ball_radius);
    let (a_velocity, a_angular_velocity) = match tp_a8_cue_state {
        Some((velocity, angular_x, angular_y)) => (
            velocity,
            AngularVelocity3::new(
                angular_x,
                angular_y,
                a_state.angular_velocity.z().as_f64() + spin_delta_z,
            ),
        ),
        None => (
            a_velocity_from_momentum,
            AngularVelocity3::new(
                a_state.angular_velocity.x().as_f64() + spin_delta_x,
                a_state.angular_velocity.y().as_f64() + spin_delta_y,
                a_state.angular_velocity.z().as_f64() + spin_delta_z,
            ),
        ),
    };
    let b_angular_velocity = AngularVelocity3::new(
        b_state.angular_velocity.x().as_f64() + spin_delta_x,
        b_state.angular_velocity.y().as_f64() + spin_delta_y,
        b_state.angular_velocity.z().as_f64() + spin_delta_z,
    );

    CollisionOutcome {
        a_after: build_on_table_ball_state(
            a_state.position.clone(),
            a_velocity,
            a_angular_velocity,
        ),
        b_after: build_on_table_ball_state(
            b_state.position.clone(),
            b_velocity,
            b_angular_velocity,
        ),
        throw_angle_degrees: Some(throw_angle_degrees),
        transferred_spin,
    }
}

/// Resolve an instantaneous ball-ball collision for two validated on-table states and return the
/// detailed post-impact outcome.
///
/// `CollisionModel::ThrowAware` currently implements a first-pass tangential-slip throw model for
/// the common case of a cut shot into an initially stationary object ball. The sign and zero-slip
/// condition are grounded in the local references:
///
/// - `whitepapers/Alciatore_pool_physics_article.pdf`, Section VI "Throw", describes the throw
///   term as proportional to `(v sin(φ) - R ω_z)`.
/// - `whitepapers/art_of_billiards_play_files/bil_praa.html`, Eqs. (C6'), (C11), and (C13),
///   express the same contact-patch tangential slip and the no-slip / gearing condition.
///
/// This first slice keeps the ideal equal-mass line-of-centers speed transfer, maps the signed
/// tangential contact slip to a bounded signed deflection angle, and adds a first-pass transferred
/// z-spin increment for the stationary-object equal-ball case. For the common case of a cut shot
/// into a stationary object ball with residual cue-ball spin, the cue-ball branch also seeds its
/// immediate post-impact velocity / horizontal spin state from a broader local TP A.8 / A.24-style
/// shot-basis model before the on-cloth sliding solver takes over. Exact throw magnitudes and
/// richer transferred-spin components remain future work.
pub fn collide_ball_ball_detailed_on_table(
    a: &OnTableBallState,
    b: &OnTableBallState,
    model: CollisionModel,
) -> CollisionOutcome {
    match model {
        CollisionModel::Ideal => ideal_collision_outcome_on_table(a, b),
        CollisionModel::ThrowAware => throw_aware_collision_outcome_on_table(a, b),
        CollisionModel::SpinFriction => {
            todo!("spin-friction ball-ball collisions are not implemented yet")
        }
    }
}

/// Resolve an instantaneous ball-ball collision for two validated on-table states.
///
/// This convenience helper returns only the post-impact states. Use
/// `collide_ball_ball_detailed_on_table(...)` if you also want throw metadata.
pub fn collide_ball_ball_on_table(
    a: &OnTableBallState,
    b: &OnTableBallState,
    model: CollisionModel,
) -> (OnTableBallState, OnTableBallState) {
    let outcome = collide_ball_ball_detailed_on_table(a, b, model);

    (outcome.a_after, outcome.b_after)
}

/// Resolve an instantaneous ball-ball collision and also bundle the current cue-ball bend estimate
/// convenience data.
pub fn collide_ball_ball_analyzed_on_table(
    a: &OnTableBallState,
    b: &OnTableBallState,
    model: CollisionModel,
    ball: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
) -> CollisionAnalysis {
    collide_ball_ball_detailed_on_table(a, b, model).with_post_contact_cue_ball_bend(ball, motion)
}

/// Estimate the later side-spin-driven cue-ball curve generated by cloth interaction after an
/// impact.
///
/// This helper follows the current motion phase using the same bounded side-spin coupling used by
/// `advance_within_phase_on_table(...)`:
///
/// - if the cue ball is currently sliding, the estimate reports the immediate side-spin-driven
///   curve interval during that sliding phase; and
/// - if the cue ball is already rolling, the estimate reports the rolling curve interval.
///
/// The local references motivating this are:
///
/// - `whitepapers/publications_presentations_and_software_index.html`, which indexes the local Dr.
///   Dave proofs for post-impact cue-ball trajectory (`TP_A-4`) and sidespin effects on the 90° /
///   30° rules (`TP_A-7`, `TP_A-8`).
/// - `whitepapers/art_of_billiards_play_files/bil_praa.html`, §7.3 and §7.5, which provide the
///   sliding / rolling deceleration and z-spin decay primitives used to bound the estimate in time.
///
/// This remains a first-pass bounded model rather than a full separate derivation of the cue ball's
/// side-spin cloth force, but it now matches the current solver in both the sliding and rolling
/// intervals where side-spin curve is modeled.
pub fn estimate_post_contact_cue_ball_curve_on_table(
    state: &OnTableBallState,
    ball: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
) -> Option<PostContactCueBallCurve> {
    let state_ref = state.as_ball_state();
    match classify_motion_phase(state_ref, ball, &motion.phase) {
        MotionPhase::Sliding => {
            let transition = compute_next_transition_on_table(state, ball, motion)?;
            if transition.phase_before != MotionPhase::Sliding {
                return None;
            }

            let slip_velocity = cloth_contact_velocity_on_table(state_ref, ball.radius.clone());
            let vx_i = state_ref.velocity.x().as_f64();
            let vy_i = state_ref.velocity.y().as_f64();
            let we_x = slip_velocity.x().as_f64();
            let we_y = slip_velocity.y().as_f64();
            let vx_c = vx_i - (2.0 / 7.0) * we_x;
            let vy_c = vy_i - (2.0 / 7.0) * we_y;
            let curve = state_ref
                .velocity
                .angle_from_north()
                .and_then(|start_heading| {
                    sliding_side_spin_curve_during(
                        start_heading,
                        vx_i,
                        vy_i,
                        vx_c,
                        vy_c,
                        state_ref.angular_velocity.z().as_f64(),
                        transition.time_until_transition,
                        transition.time_until_transition,
                        ball,
                        motion,
                    )
                })?;

            Some(PostContactCueBallCurve {
                time_until_curve_starts: Seconds::zero(),
                time_until_curve_completes: curve.duration,
                curve_angle_degrees: curve.angle_degrees,
                heading_after_curve: curve.heading_after_curve,
            })
        }
        MotionPhase::Rolling => {
            let transition = compute_next_transition_on_table(state, ball, motion)?;
            if transition.phase_before != MotionPhase::Rolling {
                return None;
            }

            let curve = rolling_side_spin_curve_during(
                state_ref.velocity.angle_from_north()?,
                ball_speed(state_ref).as_f64(),
                state_ref.angular_velocity.z().as_f64(),
                transition.time_until_transition,
                ball,
                motion,
            )?;

            Some(PostContactCueBallCurve {
                time_until_curve_starts: Seconds::zero(),
                time_until_curve_completes: curve.duration,
                curve_angle_degrees: curve.angle_degrees,
                heading_after_curve: curve.heading_after_curve,
            })
        }
        _ => None,
    }
}

/// Convenience helpers for working with an immediate ball-ball collision outcome.
///
/// The current ball-ball collision APIs return the cue ball on branch `a_after`; methods below
/// therefore expose both post-contact analysis helpers and a `PostContactContinuation` rooted at
/// that cue-ball state.
impl CollisionOutcome {
    /// Package this immediate post-collision result as a continuation rooted at the cue-ball
    /// branch.
    pub fn cue_ball_continuation(&self) -> PostContactContinuation {
        PostContactContinuation {
            source_contact: self.clone(),
        }
    }

    /// Consume this immediate post-collision result into a continuation rooted at the cue-ball
    /// branch.
    pub fn into_cue_ball_continuation(self) -> PostContactContinuation {
        PostContactContinuation {
            source_contact: self,
        }
    }

    /// Estimate the later cloth-driven cue-ball bend from this immediate post-collision outcome.
    ///
    /// This is a convenience wrapper around `estimate_post_contact_cue_ball_bend_on_table(...)`
    /// applied to `self.a_after`, which is the cue-ball branch of the current ball-ball collision
    /// helpers.
    pub fn estimate_post_contact_cue_ball_bend(
        &self,
        ball: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
    ) -> Option<PostContactCueBallBend> {
        estimate_post_contact_cue_ball_bend_on_table(&self.a_after, ball, motion)
    }

    /// Estimate the later side-spin-driven cue-ball curve from this immediate post-collision
    /// outcome.
    pub fn estimate_post_contact_cue_ball_curve(
        &self,
        ball: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
    ) -> Option<PostContactCueBallCurve> {
        estimate_post_contact_cue_ball_curve_on_table(&self.a_after, ball, motion)
    }

    /// Bundle this outcome with the current cue-ball bend estimate convenience data.
    pub fn with_post_contact_cue_ball_bend(
        &self,
        ball: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
    ) -> CollisionAnalysis {
        CollisionAnalysis {
            outcome: self.clone(),
            cue_ball_bend: self.estimate_post_contact_cue_ball_bend(ball, motion),
        }
    }
}

impl PostContactContinuation {
    /// Return the underlying immediate collision outcome that produced this continuation.
    pub fn source_contact(&self) -> &CollisionOutcome {
        &self.source_contact
    }

    /// Consume this continuation and return the underlying collision outcome.
    pub fn into_source_contact(self) -> CollisionOutcome {
        self.source_contact
    }

    /// Return the cue ball's immediate post-contact on-table state.
    pub fn cue_ball(&self) -> &OnTableBallState {
        &self.source_contact.a_after
    }

    /// Return the struck ball's immediate post-contact on-table state.
    pub fn struck_ball(&self) -> &OnTableBallState {
        &self.source_contact.b_after
    }

    /// Predict the next phase-aware ball-ball collision between this post-contact cue ball and a
    /// second on-table ball.
    pub fn next_collision_against_ball(
        &self,
        other: &OnTableBallState,
        ball: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
    ) -> Option<PredictedBallBallCollision> {
        compute_next_ball_ball_collision_during_current_phases_on_table(
            self.cue_ball(),
            other,
            ball,
            motion,
        )
    }

    /// Predict the earliest supported next event between this post-contact cue ball and a second
    /// on-table ball.
    pub fn next_event_against_ball(
        &self,
        other: &OnTableBallState,
        ball: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
    ) -> Option<TwoBallOnTableEvent> {
        compute_next_two_ball_event_on_table(self.cue_ball(), other, ball, motion)
    }

    /// Predict the cue ball's next rail impact under the current table geometry.
    pub fn next_rail_impact(
        &self,
        ball: &BallSetPhysicsSpec,
        table: &TableSpec,
        motion: &OnTableMotionConfig,
    ) -> Option<PredictedBallRailImpact> {
        compute_next_ball_rail_impact_on_table(self.cue_ball(), ball, table, motion)
    }

    /// Predict the earliest supported next event between this post-contact cue ball and a second
    /// on-table ball while also considering the table rails.
    pub fn next_event_against_ball_with_rails(
        &self,
        other: &OnTableBallState,
        ball: &BallSetPhysicsSpec,
        table: &TableSpec,
        motion: &OnTableMotionConfig,
    ) -> Option<TwoBallOnTableEvent> {
        compute_next_two_ball_event_with_rails_on_table(self.cue_ball(), other, ball, table, motion)
    }
}

pub fn estimate_post_contact_cue_ball_bend_on_table(
    state: &OnTableBallState,
    ball: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
) -> Option<PostContactCueBallBend> {
    if classify_motion_phase(state.as_ball_state(), ball, &motion.phase) != MotionPhase::Sliding {
        return None;
    }

    let start_heading = state.as_ball_state().velocity.angle_from_north()?;
    let transition = compute_next_transition_on_table(state, ball, motion)?;
    if transition.phase_before != MotionPhase::Sliding
        || transition.phase_after != MotionPhase::Rolling
    {
        return None;
    }

    // `PostContactCueBallBend` intentionally isolates the sliding-to-rolling follow / draw bend
    // component. Side-spin-driven curve is reported separately by
    // `estimate_post_contact_cue_ball_curve_on_table(...)`, so this analysis zeros `ωz` before
    // advancing through the bend interval.
    let bend_only_state = OnTableBallState::try_from(BallState {
        angular_velocity: AngularVelocity3::new(
            state.as_ball_state().angular_velocity.x().as_f64(),
            state.as_ball_state().angular_velocity.y().as_f64(),
            0.0,
        ),
        ..state.as_ball_state().clone()
    })
    .expect("post-contact bend estimate input should preserve on-table invariants");
    let state_after_bend = OnTableBallState::try_from(
        advance_motion_on_table(
            &bend_only_state,
            transition.time_until_transition,
            ball,
            motion,
        )
        .state,
    )
    .expect("post-contact bend estimate should preserve on-table invariants");
    let end_heading = state_after_bend
        .as_ball_state()
        .velocity
        .angle_from_north()?;

    Some(PostContactCueBallBend {
        time_until_bend_completes: transition.time_until_transition,
        bend_angle_degrees: signed_angle_difference_degrees(start_heading, end_heading),
        state_after_bend,
    })
}

fn rail_collision_basis(rail: Rail) -> (f64, f64, f64, f64) {
    match rail {
        Rail::Top => (0.0, -1.0, 1.0, 0.0),
        Rail::Bottom => (0.0, 1.0, 1.0, 0.0),
        Rail::Left => (1.0, 0.0, 0.0, 1.0),
        Rail::Right => (-1.0, 0.0, 0.0, 1.0),
    }
}

fn rebuild_velocity_from_basis(
    normal_component: f64,
    tangent_component: f64,
    normal_x: f64,
    normal_y: f64,
    tangent_x: f64,
    tangent_y: f64,
) -> Velocity2 {
    Velocity2::new(
        Inches::from_f64(normal_component * normal_x + tangent_component * tangent_x),
        Inches::from_f64(normal_component * normal_y + tangent_component * tangent_y),
    )
}

fn validated_rail_normal_restitution(config: &RailCollisionConfig) -> f64 {
    let restitution = config.normal_restitution.as_f64();
    assert!(
        (0.0..=1.0).contains(&restitution),
        "rail normal restitution must lie in [0, 1]"
    );
    restitution
}

fn validated_rail_tangential_friction_coefficient(config: &RailCollisionConfig) -> f64 {
    let friction = config.tangential_friction_coefficient.as_f64();
    assert!(
        friction >= 0.0,
        "rail tangential friction coefficient must be non-negative"
    );
    friction
}

fn restitution_aware_ball_rail_collision_velocity(
    velocity: &Velocity2,
    rail: Rail,
    normal_restitution: f64,
) -> Velocity2 {
    let (normal_x, normal_y, tangent_x, tangent_y) = rail_collision_basis(rail);
    let normal_component = project_velocity_on_basis(velocity, normal_x, normal_y);
    let tangent_component = project_velocity_on_basis(velocity, tangent_x, tangent_y);

    rebuild_velocity_from_basis(
        -normal_restitution * normal_component,
        tangent_component,
        normal_x,
        normal_y,
        tangent_x,
        tangent_y,
    )
}

fn ideal_ball_rail_collision_velocity(velocity: &Velocity2, rail: Rail) -> Velocity2 {
    restitution_aware_ball_rail_collision_velocity(velocity, rail, 1.0)
}

fn rail_vertical_contact_slip(
    angular_velocity: &AngularVelocity3,
    normal_x: f64,
    normal_y: f64,
    ball_radius: f64,
) -> f64 {
    ball_radius
        * (normal_x * angular_velocity.y().as_f64() - normal_y * angular_velocity.x().as_f64())
}

fn spin_aware_ball_rail_collision_on_table(
    state: &OnTableBallState,
    rail: Rail,
    ball_radius: f64,
    normal_restitution: f64,
    tangential_friction_coefficient: f64,
) -> OnTableBallState {
    assert!(
        ball_radius > f64::EPSILON,
        "spin-aware rail collisions require a positive ball radius"
    );

    let state_ref = state.as_ball_state();
    let (normal_x, normal_y, tangent_x, tangent_y) = rail_collision_basis(rail);
    let normal_component = project_velocity_on_basis(&state_ref.velocity, normal_x, normal_y);
    let tangent_component = project_velocity_on_basis(&state_ref.velocity, tangent_x, tangent_y);
    let contact_x = -ball_radius * normal_x;
    let contact_y = -ball_radius * normal_y;
    let tangential_spin_scale = contact_x * tangent_y - contact_y * tangent_x;
    let tangential_contact_slip =
        tangent_component + state_ref.angular_velocity.z().as_f64() * tangential_spin_scale;
    let vertical_contact_slip =
        rail_vertical_contact_slip(&state_ref.angular_velocity, normal_x, normal_y, ball_radius);
    let contact_slip_norm = tangential_contact_slip.hypot(vertical_contact_slip);
    let coupling_scale = if contact_slip_norm <= f64::EPSILON {
        0.0
    } else {
        let no_slip_scale: f64 = 2.0 / 7.0;
        let friction_limited_scale = tangential_friction_coefficient
            * (1.0 + normal_restitution)
            * (-normal_component).max(0.0)
            / contact_slip_norm;
        no_slip_scale.min(friction_limited_scale)
    };
    let tangential_delta = -coupling_scale * tangential_contact_slip;
    let vertical_contact_delta = -coupling_scale * vertical_contact_slip;
    let tangential_after = tangent_component + tangential_delta;
    let velocity = rebuild_velocity_from_basis(
        -normal_restitution * normal_component,
        tangential_after,
        normal_x,
        normal_y,
        tangent_x,
        tangent_y,
    );

    // `whitepapers/art_of_billiards_play_files/bil_praa.html`, Figure 6 and §7.1, describe the
    // rail rebound in terms of normal elasticity `N`, tangential friction `fi`, and the full
    // contact-slip vector `WCa` inside the rail tangent plane. Eq. (C10) gives a friction-limited
    // change along `-WCa`, while Eqs. (C11) and (C13) provide the no-slip / adherence limit.
    // This first-pass rail model therefore uses the smaller of those two magnitudes on the full
    // 2D slip vector: along-rail slip from running / reverse english and vertical slip from top /
    // draw both contribute. Because the current on-table state does not represent post-impact
    // vertical center-of-mass velocity, the vertical translational component of that frictional rail
    // impulse is projected out after using it to compute the coupled spin change.
    let normal_cross_tangent_z = normal_x * tangent_y - normal_y * tangent_x;
    let delta_wz = -(5.0 / (2.0 * ball_radius)) * normal_cross_tangent_z * tangential_delta;
    let delta_wx = -(5.0 / (2.0 * ball_radius)) * vertical_contact_delta * normal_y;
    let delta_wy = (5.0 / (2.0 * ball_radius)) * vertical_contact_delta * normal_x;
    let angular_velocity = AngularVelocity3::new(
        state_ref.angular_velocity.x().as_f64() + delta_wx,
        state_ref.angular_velocity.y().as_f64() + delta_wy,
        state_ref.angular_velocity.z().as_f64() + delta_wz,
    );

    build_on_table_ball_state(state_ref.position.clone(), velocity, angular_velocity)
}

/// Resolve an instantaneous ball-rail collision for a validated on-table state using an explicit
/// ball radius and explicit rail-response coefficients.
///
/// The local rail reference material decomposes the incoming velocity into components tangential and
/// perpendicular to the cushion and discusses how elasticity and cushion friction modify those
/// components after impact. See `whitepapers/art_of_billiards_play_files/bil_praa.html`, Figure 6
/// and §7.1. `RailModel::Mirror` is the simplest limiting case of that picture: perfectly elastic
/// in the normal direction, no tangential loss, and no spin change.
///
/// `RailModel::RestitutionOnly` uses `RailCollisionConfig::normal_restitution` as the whitepaper's
/// coefficient of elasticity `N` in the cushion-normal direction while leaving the tangential speed
/// and angular velocity unchanged.
///
/// `RailModel::SpinAware` currently implements the smallest useful richer slice: the same
/// configurable normal restitution plus a tunable cushion-friction response for the full rail-face
/// contact-slip vector. Running / reverse english (`ωz`) changes the returned tangential speed and
/// z-spin, while top / draw (`ωx`, `ωy`) changes the vertical slip component and therefore the
/// horizontal spin carried away from the cushion.
///
/// A rolling ball that rebounds from a rail will generally leave the cushion in a `Sliding` phase,
/// not `Rolling`: the pre-impact rolling spin is still present after the bounce, but the reversed
/// normal component of translational velocity changes the no-slip condition. In that sense, carried
/// rolling spin can act like draw relative to the post-rail travel direction until cloth friction
/// re-establishes rolling.
pub fn collide_ball_rail_on_table_with_radius_and_config(
    state: &OnTableBallState,
    rail: Rail,
    ball_radius: Inches,
    model: RailModel,
    config: &RailCollisionConfig,
) -> OnTableBallState {
    let state_ref = state.as_ball_state();
    match model {
        RailModel::Mirror => build_on_table_ball_state(
            state_ref.position.clone(),
            ideal_ball_rail_collision_velocity(&state_ref.velocity, rail),
            state_ref.angular_velocity.clone(),
        ),
        RailModel::RestitutionOnly => build_on_table_ball_state(
            state_ref.position.clone(),
            restitution_aware_ball_rail_collision_velocity(
                &state_ref.velocity,
                rail,
                validated_rail_normal_restitution(config),
            ),
            state_ref.angular_velocity.clone(),
        ),
        RailModel::SpinAware => spin_aware_ball_rail_collision_on_table(
            state,
            rail,
            ball_radius.as_f64(),
            validated_rail_normal_restitution(config),
            validated_rail_tangential_friction_coefficient(config),
        ),
    }
}

/// Resolve an instantaneous ball-rail collision for a validated on-table state using an explicit
/// ball radius.
///
/// This compatibility wrapper uses the default rail-response coefficients. Prefer
/// `collide_ball_rail_on_table_with_radius_and_config(...)` when restitution should be explicit.
pub fn collide_ball_rail_on_table_with_radius(
    state: &OnTableBallState,
    rail: Rail,
    ball_radius: Inches,
    model: RailModel,
) -> OnTableBallState {
    collide_ball_rail_on_table_with_radius_and_config(
        state,
        rail,
        ball_radius,
        model,
        &RailCollisionConfig::default(),
    )
}

/// Resolve an instantaneous ball-rail collision for a validated on-table state.
///
/// This compatibility wrapper uses the default ball radius and default rail-response coefficients.
/// Prefer `collide_ball_rail_on_table_with_radius_and_config(...)` when either one should be
/// explicit.
pub fn collide_ball_rail_on_table(
    state: &OnTableBallState,
    rail: Rail,
    model: RailModel,
) -> OnTableBallState {
    collide_ball_rail_on_table_with_radius(state, rail, BallSetPhysicsSpec::default().radius, model)
}

/// Gives the polar direction (e.g. positive or negative).
/// For example, if a ball is in the top-right quadrant of the pool table, it's
/// PolarDirection from the center is (Positive, Positive). Conversely, a ball
/// in the bottom-left is (Negative, Negative).
pub enum PolarDirection {
    Positive,
    Negative,
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
/// A point on the table, interpreted as follows:
///   - Top-down view of the table, headstring at the top and rack spot at the bottom.
///   - The diamond that would exist at the bottom-left pocket is x=0, y=0.
///   - The diamond that would exist at the top-right pocket is x=4, y=8.
///   - The headstring is the imaginary line from (0, 6) <-> (4, 6).
///   - The rack spot is the point (2, 2).
///   - The center of the table is the point (2, 4).
///   - The kitchen is the rectangle from (0, 8) <-> (4, 6).
#[derive(Default)]
pub struct Position {
    pub x: Diamond,
    pub y: Diamond,
    unresolved_x_shift: Option<Inches>,
    unresolved_y_shift: Option<Inches>,
}

impl Position {
    pub fn new<X: Into<Diamond>, Y: Into<Diamond>>(x: X, y: Y) -> Self {
        Self {
            x: x.into(),
            y: y.into(),
            ..Default::default()
        }
    }

    /// Calculates the displacement from this position to another.
    pub fn displacement(&self, to: &Self) -> Displacement {
        Displacement {
            dx: to.x.clone() - self.x.clone(),
            dy: to.y.clone() - self.y.clone(),
        }
    }

    /// Gives the relative direction from center for this position.
    /// The tuple is always (X direction, Y direction).
    pub fn direction_from_center(&self) -> (PolarDirection, PolarDirection) {
        match (self.x > CENTER_SPOT.x, self.y > CENTER_SPOT.y) {
            (true, true) => (PolarDirection::Positive, PolarDirection::Positive),
            (true, false) => (PolarDirection::Positive, PolarDirection::Negative),
            (false, false) => (PolarDirection::Negative, PolarDirection::Negative),
            (false, true) => (PolarDirection::Negative, PolarDirection::Positive),
        }
    }

    /// If this position is left of the center line, return true.
    pub fn is_left_of_center(&self) -> bool {
        self.x < CENTER_SPOT.x
    }

    /// If this position is right of the center line, return true.
    pub fn is_right_of_center(&self) -> bool {
        self.x > CENTER_SPOT.x
    }

    /// If this position is above the center line, return true.
    pub fn is_above_center(&self) -> bool {
        self.y > CENTER_SPOT.y
    }

    /// If this position is below the center line, return true.
    pub fn is_below_center(&self) -> bool {
        self.y < CENTER_SPOT.y
    }

    pub fn merge_unset_component(mut self, diamond: Diamond) -> Self {
        if self.x == Diamond::zero() {
            self.x = diamond;
            self
        } else if self.y == Diamond::zero() {
            self.y = diamond;
            self
        } else {
            unreachable!();
        }
    }

    /// Gives the angle from this position to `target`.
    pub fn angle_to(&self, target: &Self) -> Angle {
        let dx = (target.x.magnitude.clone() - self.x.magnitude.clone())
            .to_f64()
            .unwrap();
        let dy = (target.y.magnitude.clone() - self.y.magnitude.clone())
            .to_f64()
            .unwrap();

        Angle::from_north(dx, dy)
    }

    /// Gives the angle to the aiming center of the given Pocket.
    pub fn angle_to_pocket(&self, pocket: Pocket) -> Angle {
        self.angle_to(&pocket.aiming_center())
    }

    /// Calculates the Angle of the line going from the aiming center of the
    /// given Pocket towards this position.
    pub fn angle_from_pocket(&self, pocket: Pocket) -> Angle {
        self.angle_to_pocket(pocket).flipped()
    }

    pub fn zeroed() -> Self {
        Self::new(Diamond::zero(), Diamond::zero())
    }

    pub fn shift_horizontally(&mut self, distance: Diamond) -> &mut Self {
        self.x = self.x.clone() + distance.clone();
        self
    }

    pub fn shift_vertically(&mut self, distance: Diamond) -> &mut Self {
        self.y = self.y.clone() + distance.clone();
        self
    }

    pub fn shift_horizontally_inches(&mut self, distance: Inches) -> &mut Self {
        self.unresolved_x_shift =
            Some(self.unresolved_x_shift.clone().unwrap_or_default() + distance);
        self
    }

    pub fn shift_vertically_inches(&mut self, distance: Inches) -> &mut Self {
        self.unresolved_y_shift =
            Some(self.unresolved_y_shift.clone().unwrap_or_default() + distance);
        self
    }

    pub fn resolve_shifts(&mut self, table_spec: &TableSpec) {
        if let Some(shift) = &self.unresolved_x_shift {
            self.shift_horizontally(table_spec.inches_to_diamond(shift.clone()));
            self.unresolved_x_shift = None;
        }

        if let Some(shift) = &self.unresolved_y_shift {
            self.shift_vertically(table_spec.inches_to_diamond(shift.clone()));
            self.unresolved_y_shift = None;
        }
    }

    /// Move `dd` diamonds away from `self` along `angle`, returning the new `Position`.
    ///
    /// Internally we:
    /// 1. turn the `angle` into radians;
    /// 2. compute the unit-vector components in X (sin) and Y (cos)
    /// 3. scale those components by `dd`
    /// 4. build new `Diamond`s and add them to the current coordinates.
    pub fn translate(&self, dd: Diamond, angle: Angle) -> Self {
        let rad = angle.0.to_radians();

        let ux = rad.sin();
        let uy = rad.cos();

        let dx = Diamond {
            magnitude: dd.magnitude.clone() * BigDecimal::from_f64(ux).unwrap(),
        };
        let dy = Diamond {
            magnitude: dd.magnitude.clone() * BigDecimal::from_f64(uy).unwrap(),
        };

        Position {
            x: self.x.clone() + dx,
            y: self.y.clone() + dy,
            unresolved_x_shift: self.unresolved_x_shift.clone(),
            unresolved_y_shift: self.unresolved_y_shift.clone(),
        }
    }

    /// Translate along `angle` by an `Inches` magnitude.
    ///
    /// The shift is stored in `unresolved_{x,y}_shift` so the caller does not
    /// need to pass a `TableSpec` just to convert inches to diamonds.
    pub fn translate_inches(&self, inches: Inches, angle: Angle) -> Self {
        let rad = angle.0.to_radians();
        let ux = rad.sin();
        let uy = rad.cos();

        let dx = Inches {
            magnitude: inches.magnitude.clone() * BigDecimal::from_f64(ux).unwrap(),
        };
        let dy = Inches {
            magnitude: inches.magnitude.clone() * BigDecimal::from_f64(uy).unwrap(),
        };

        Self {
            unresolved_x_shift: Some(self.unresolved_x_shift.clone().unwrap_or_default() + dx),
            unresolved_y_shift: Some(self.unresolved_y_shift.clone().unwrap_or_default() + dy),
            ..self.clone()
        }
    }

    /// Return the "ghost–ball" position one ball-diameter (2*R) away from `self`
    /// in the direction `angle`.
    pub fn translate_ghost_ball(&self, angle: Angle) -> Self {
        let shift_inches = TYPICAL_BALL_RADIUS.clone().double();
        self.translate_inches(shift_inches, angle)
    }
}

/// Compute the idealized gearing-english spin magnitude for a cut shot.
///
/// The local references in `whitepapers/` all encode gearing as the no-slip / no-throw
/// condition at the cue-ball/object-ball contact patch:
///
/// - `whitepapers/Alciatore_pool_physics_article.pdf`, Section VI "Throw", gives a throw term
///   proportional to `(v sin(φ) - R ω_z)`, so zero throw occurs when that factor is zero.
/// - `whitepapers/art_of_billiards_play_files/bil_praa.html`, Eqs. (C6'), (C11), and (C13),
///   defines the relative tangential contact velocity `WCa` and the adherence condition
///   `WCi = 0`, which is the same zero-relative-motion condition at impact.
/// - `whitepapers/billiards_ball_collisions.pdf`, Eq. (26), gives the no-slip condition at the
///   impact point in terms of tangential center-of-mass velocity and spin.
///
/// Under the common pool-shot simplifications used here — object ball initially at rest,
/// negligible pre-impact object-ball spin, and interest only in the side-spin component that
/// cancels tangential slip — these relations reduce to `R * |ω| = v * sin(φ)`, so this helper
/// returns `|ω| = v * sin(φ) / R`.
///
/// Assumptions and caveats:
/// - `shot_speed` is the cue-ball speed at *ball-ball impact*, not the launch speed off the cue.
/// - `cut_angle` is the unsigned cut-angle magnitude `φ` at impact, typically in `[0°, 90°]`.
///   If you have absolute table headings instead, derive this with
///   `CutAngle::from_headings(cue_ball_heading, object_ball_heading)`.
/// - The return value is a spin magnitude; callers must choose the left/right sign for the
///   appropriate outside-english convention.
///
/// Returns the required outside angular velocity magnitude as `RadiansPerSecond`.
pub fn gearing_english(cut_angle: CutAngle, shot_speed: InchesPerSecond) -> RadiansPerSecond {
    let omega = shot_speed.inches.magnitude.to_f64().unwrap()
        * cut_angle.as_degrees().to_radians().sin()
        / TYPICAL_BALL_RADIUS.magnitude.to_f64().unwrap();
    RadiansPerSecond::new(omega)
}

/// A displacement indicating a direction and distance.
#[derive(Clone, Debug)]
pub struct Displacement {
    /// The delta x component of the displacement.
    pub dx: Diamond,

    /// The delta y component of the displacement.
    pub dy: Diamond,
}

impl Displacement {
    pub fn new(dx: &str, dy: &str) -> Self {
        Displacement {
            dx: Diamond::from(dx),
            dy: Diamond::from(dy),
        }
    }

    pub fn absolute_distance(&self) -> Diamond {
        let dx = self.dx.magnitude.to_f64().unwrap();
        let dy = self.dy.magnitude.to_f64().unwrap();

        // dx² + dy² = dist²
        let dist = (dx * dx + dy * dy).sqrt();

        Diamond {
            magnitude: bigdecimal::BigDecimal::from_f64(dist).unwrap(),
        }
    }

    pub fn angle_from_north(&self) -> Angle {
        Angle::from_north(
            self.dx.magnitude.to_f64().unwrap(),
            self.dy.magnitude.to_f64().unwrap(),
        )
    }
}

impl Sub for Diamond {
    type Output = Diamond;

    fn sub(self, rhs: Diamond) -> Self::Output {
        Self {
            magnitude: self.magnitude - rhs.magnitude,
        }
    }
}

impl Neg for Diamond {
    type Output = Diamond;

    fn neg(self) -> Self {
        Self {
            magnitude: self.magnitude.neg(),
        }
    }
}

impl Add for Diamond {
    type Output = Diamond;

    fn add(self, rhs: Diamond) -> Self::Output {
        Self {
            magnitude: self.magnitude + rhs.magnitude,
        }
    }
}

impl Add for Inches {
    type Output = Inches;

    fn add(self, rhs: Inches) -> Self::Output {
        Self {
            magnitude: self.magnitude + rhs.magnitude,
        }
    }
}

impl Mul<Scale> for Inches {
    type Output = Inches;

    fn mul(self, rhs: Scale) -> Self::Output {
        Self {
            magnitude: self.magnitude * rhs.magnitude,
        }
    }
}

impl Mul<Inches> for Scale {
    type Output = Inches;

    fn mul(self, rhs: Inches) -> Self::Output {
        rhs * self
    }
}

impl Mul<Seconds> for InchesPerSecond {
    type Output = Inches;

    fn mul(self, rhs: Seconds) -> Self::Output {
        Inches::from_f64(self.as_f64() * rhs.as_f64())
    }
}

impl Mul<InchesPerSecond> for Seconds {
    type Output = Inches;

    fn mul(self, rhs: InchesPerSecond) -> Self::Output {
        rhs * self
    }
}

impl Mul<Seconds> for InchesPerSecondSq {
    type Output = InchesPerSecond;

    fn mul(self, rhs: Seconds) -> Self::Output {
        InchesPerSecond::new(Inches::from_f64(self.as_f64() * rhs.as_f64()))
    }
}

impl Mul<InchesPerSecondSq> for Seconds {
    type Output = InchesPerSecond;

    fn mul(self, rhs: InchesPerSecondSq) -> Self::Output {
        rhs * self
    }
}

impl Mul<Inches> for RadiansPerSecond {
    type Output = InchesPerSecond;

    fn mul(self, rhs: Inches) -> Self::Output {
        InchesPerSecond::new(Inches::from_f64(self.as_f64() * rhs.as_f64()))
    }
}

impl Mul<RadiansPerSecond> for Inches {
    type Output = InchesPerSecond;

    fn mul(self, rhs: RadiansPerSecond) -> Self::Output {
        rhs * self
    }
}

impl Mul<Seconds> for RadiansPerSecondSq {
    type Output = RadiansPerSecond;

    fn mul(self, rhs: Seconds) -> Self::Output {
        RadiansPerSecond::new(self.as_f64() * rhs.as_f64())
    }
}

impl Mul<RadiansPerSecondSq> for Seconds {
    type Output = RadiansPerSecond;

    fn mul(self, rhs: RadiansPerSecondSq) -> Self::Output {
        rhs * self
    }
}

impl Mul<Seconds> for Velocity2 {
    type Output = Inches2;

    fn mul(self, rhs: Seconds) -> Self::Output {
        Inches2::new(self.x * rhs, self.y * rhs)
    }
}

impl Mul<Velocity2> for Seconds {
    type Output = Inches2;

    fn mul(self, rhs: Velocity2) -> Self::Output {
        rhs * self
    }
}

impl Sub for Inches {
    type Output = Inches;

    fn sub(self, rhs: Inches) -> Self::Output {
        Self {
            magnitude: self.magnitude - rhs.magnitude,
        }
    }
}

impl Div<BigDecimal> for Inches {
    type Output = Inches;

    fn div(self, rhs: BigDecimal) -> Self::Output {
        Inches {
            magnitude: self.magnitude / rhs,
        }
    }
}

impl Mul<BigDecimal> for Diamond {
    type Output = Diamond;

    fn mul(self, rhs: BigDecimal) -> Self::Output {
        Diamond {
            magnitude: self.magnitude * rhs,
        }
    }
}

impl From<u8> for Scale {
    fn from(value: u8) -> Self {
        Self {
            magnitude: BigDecimal::from_u8(value).unwrap(),
        }
    }
}

impl From<i64> for Scale {
    fn from(value: i64) -> Self {
        Self {
            magnitude: BigDecimal::from_i64(value).unwrap(),
        }
    }
}

impl From<&str> for Scale {
    fn from(value: &str) -> Self {
        Self {
            magnitude: BigDecimal::from_str(value).unwrap(),
        }
    }
}

impl From<u8> for Inches {
    fn from(value: u8) -> Self {
        Self {
            magnitude: BigDecimal::from_u8(value).unwrap(),
        }
    }
}

impl From<i64> for Inches {
    fn from(value: i64) -> Self {
        Self {
            magnitude: BigDecimal::from_i64(value).unwrap(),
        }
    }
}

impl From<&str> for Inches {
    fn from(value: &str) -> Self {
        Self {
            magnitude: BigDecimal::from_str(value).unwrap(),
        }
    }
}

impl From<u8> for Diamond {
    fn from(value: u8) -> Self {
        Self {
            magnitude: BigDecimal::from_u8(value).unwrap(),
        }
    }
}

impl From<&str> for Diamond {
    fn from(value: &str) -> Self {
        Self {
            magnitude: BigDecimal::from_str(value).unwrap(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pocket {
    TopRight,
    CenterRight,
    BottomRight,
    BottomLeft,
    CenterLeft,
    TopLeft,
}

impl Pocket {
    const ALL: [Pocket; 6] = [
        Pocket::TopRight,
        Pocket::CenterRight,
        Pocket::BottomRight,
        Pocket::BottomLeft,
        Pocket::CenterLeft,
        Pocket::TopLeft,
    ];

    fn index(self) -> usize {
        match self {
            Pocket::TopRight => 0,
            Pocket::CenterRight => 1,
            Pocket::BottomRight => 2,
            Pocket::BottomLeft => 3,
            Pocket::CenterLeft => 4,
            Pocket::TopLeft => 5,
        }
    }

    /// Gives the natural "aiming center" of the pocket. This is currently
    /// relatively unsophisticated; for example, the aiming center may in fact
    /// have to be a function of the position of the cue ball in the future.
    pub fn aiming_center(&self) -> Position {
        match *self {
            Pocket::TopRight => AIM_TOP_RIGHT_POCKET.clone(),
            Pocket::CenterRight => CENTER_RIGHT_DIAMOND.clone(),
            Pocket::BottomRight => AIM_BOTTOM_RIGHT_POCKET.clone(),
            Pocket::BottomLeft => AIM_BOTTOM_LEFT_POCKET.clone(),
            Pocket::CenterLeft => CENTER_LEFT_DIAMOND.clone(),
            Pocket::TopLeft => AIM_TOP_LEFT_POCKET.clone(),
        }
    }
}

#[derive(Clone, Debug)]
/// A type of pocket.
pub enum PocketType {
    /// One of the four corner pockets.
    Corner,

    /// One of the two side pockets.
    Side,
}

#[derive(Clone, Debug)]
/// Physical specifications of a pocket.
pub struct PocketSpec {
    pub ty: PocketType,
    pub depth: Diamond,
    pub width: Diamond,
}

#[derive(Clone, Debug)]
/// Physical specifications of a pool table.
pub struct TableSpec {
    pub pockets: [PocketSpec; 6],
    pub cushion_diamond_buffer: Diamond,
    pub diamond_length: Inches,
}

impl Default for TableSpec {
    fn default() -> Self {
        Self::brunswick_gc4_9ft()
    }
}

#[derive(Clone, Debug)]
/// Physical specifications of a pool ball.
pub struct BallSpec {
    pub radius: Inches,
}

impl Default for BallSpec {
    fn default() -> Self {
        Self {
            radius: Inches {
                magnitude: BigDecimal::from_str("1.125").unwrap(),
            },
        }
    }
}

impl TableSpec {
    /// A typical 9ft Brunswick Gold Crown IV specification.
    pub fn brunswick_gc4_9ft() -> Self {
        let diamond_length = Inches {
            magnitude: BigDecimal::from_str("12.5").unwrap(),
        };
        Self {
            diamond_length: diamond_length.clone(),
            cushion_diamond_buffer: Diamond {
                magnitude: DIAMOND_SIGHT_NOSE_OFFSET.magnitude.clone()
                    / diamond_length.magnitude.clone(),
            },
            pockets: [
                Self::brunswick_gc4_corner_pocket(diamond_length.clone()),
                Self::brunswick_gc4_side_pocket(diamond_length.clone()),
                Self::brunswick_gc4_corner_pocket(diamond_length.clone()),
                Self::brunswick_gc4_corner_pocket(diamond_length.clone()),
                Self::brunswick_gc4_side_pocket(diamond_length.clone()),
                Self::brunswick_gc4_corner_pocket(diamond_length.clone()),
            ],
        }
    }

    /// A typical Brunswick GC IV corner pocket specification.
    pub fn brunswick_gc4_corner_pocket(diamond_length: Inches) -> PocketSpec {
        PocketSpec {
            ty: PocketType::Corner,
            depth: Diamond {
                magnitude: GC4_POCKET_DEPTH.magnitude.clone() / diamond_length.magnitude.clone(),
            },
            width: Diamond {
                magnitude: GC4_CORNER_POCKET_WIDTH.magnitude.clone()
                    / diamond_length.magnitude.clone(),
            },
        }
    }

    /// A typical Brunswick GC IV side pocket specification.
    pub fn brunswick_gc4_side_pocket(diamond_length: Inches) -> PocketSpec {
        PocketSpec {
            ty: PocketType::Side,
            depth: Diamond {
                magnitude: GC4_POCKET_DEPTH.magnitude.clone() / diamond_length.magnitude.clone(),
            },
            width: Diamond {
                magnitude: GC4_SIDE_POCKET_WIDTH.magnitude.clone()
                    / diamond_length.magnitude.clone(),
            },
        }
    }

    /// For a given table, convert Diamond Units into Inches.
    /// On a typical 9ft table, 1 Diamond is equal to 12.5 inches.
    pub fn diamond_to_inches(&self, val: Diamond) -> Inches {
        Inches {
            magnitude: val.magnitude * self.diamond_length.magnitude.clone(),
        }
    }

    /// For a given table, convert inches into Diamond Units.
    pub fn inches_to_diamond(&self, val: Inches) -> Diamond {
        Diamond {
            magnitude: val.magnitude / self.diamond_length.magnitude.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
/// A type of ball, for example, Cue ball, the eight ball, etc.
#[derive(Default)]
pub enum BallType {
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    #[default]
    Cue,
}

#[derive(Clone, Debug)]
/// Represents a ball on the table, incl. its position, physical spec, type.
#[derive(Default)]
pub struct Ball {
    pub ty: BallType,
    pub position: Position,
    pub spec: BallSpec,
}

impl Ball {
    /// Calculates the displacement between two balls. (Distance w/ direction.)
    pub fn displacement(&self, to: &Self) -> Displacement {
        self.position.displacement(&to.position)
    }

    /// Calculates the absolute distance between two balls.
    pub fn distance(&self, to: &Self) -> Diamond {
        self.displacement(to).absolute_distance()
    }

    /// Compute the idealized ghost-ball center for potting this object ball to `destination`.
    ///
    /// Local references in `whitepapers/` describe the same center-of-centers construction:
    ///
    /// - `whitepapers/Alciatore_pool_physics_article.pdf`, Section II "Terminology", states that
    ///   the object ball heads along the impact line / line of centers, and that the ghost ball is
    ///   where the cue ball must be to send the object ball in the desired direction.
    /// - `whitepapers/Physics Of Billiards.html` states that after impact, the struck ball moves in
    ///   the direction of the line joining the centers of the two balls.
    ///
    /// Under that ideal equal-ball-size, no-throw model, the cue-ball center must therefore sit one
    /// ball diameter behind the object ball on the reverse of the target line.
    pub fn ghost_ball(&self, destination: &Position, table_spec: &TableSpec) -> Position {
        let reverse_target_line = self.position.angle_to(destination).flipped();
        let ball_diameter = table_spec.inches_to_diamond(self.spec.radius.clone().double());

        self.position.translate(ball_diameter, reverse_target_line)
    }

    /// Compute the idealized ghost-ball center for potting this object ball to a pocket.
    pub fn ghost_ball_to_pocket(&self, pocket: Pocket, table_spec: &TableSpec) -> Position {
        self.ghost_ball(&pocket.aiming_center(), table_spec)
    }

    /// Compute the idealized aim angle from `shooting_position` to the ghost-ball target that
    /// would pot this object ball to `destination`.
    pub fn aim_angle(
        &self,
        destination: &Position,
        shooting_position: &Position,
        table_spec: &TableSpec,
    ) -> Angle {
        let ghost_ball = self.ghost_ball(destination, table_spec);
        shooting_position.angle_to(&ghost_ball)
    }

    /// Compute the idealized aim angle from `shooting_position` for potting this object ball to
    /// the aiming center of `pocket`.
    pub fn aim_angle_to_pocket(
        &self,
        pocket: Pocket,
        shooting_position: &Position,
        table_spec: &TableSpec,
    ) -> Angle {
        self.aim_angle(&pocket.aiming_center(), shooting_position, table_spec)
    }
}

/// The kinematics of a ball; all of the characteristics of its motion.
pub struct Kinematics {
    /// Velocity of a ball: vx, vy, vz.
    pub velocity: [InchesPerSecond; 3],

    /// Acceleration of a ball: ax, ay, az.
    pub acceleration: [InchesPerSecondSq; 3],

    /// The angular velocity (spin) on the ball.
    pub angular_velocity: [RadiansPerSecond; 3],
}

#[derive(Clone, Debug)]
/// The type of game, e.g. Nineball, EightBall, OnePocket, etc.
pub enum GameType {
    NineBall,
    EightBall,
    TenBall,
    OnePocket,
    Banks,
}

impl Default for GameType {
    fn default() -> Self {
        Self::NineBall
    }
}

#[derive(Clone, Debug)]
/// A modifier being applied to the Cueball, for example ball in hand.
pub enum CueballModifier {
    AsItLays,
    BreakPlacement,
    BallInHand,
    KitchenPlacement,
}

impl Default for CueballModifier {
    fn default() -> Self {
        Self::AsItLays
    }
}

/// How a bank / rail-approach angle is measured relative to a rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailAngleReference {
    /// Measure the angle away from the straight-into-the-rail heading.
    FromNormal,
    /// Measure the angle away from the rail face itself.
    FromRailFace,
}

/// Which direction along the rail the bank heading should favor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailTangentDirection {
    /// Use the positive along-rail tangent direction.
    ///
    /// This means:
    /// - for `Rail::Top` and `Rail::Bottom`, toward increasing X / the right side of the table;
    /// - for `Rail::Left` and `Rail::Right`, toward increasing Y / the top of the table.
    Positive,
    /// Use the negative along-rail tangent direction.
    Negative,
}

impl RailTangentDirection {
    fn sign(self) -> f64 {
        match self {
            RailTangentDirection::Positive => 1.0,
            RailTangentDirection::Negative => -1.0,
        }
    }
}

/// The rails on a pool table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rail {
    Top,
    Bottom,
    Left,
    Right,
}

impl Rail {
    pub fn rail_origin(&self) -> Position {
        match *self {
            Rail::Top => Position {
                x: Diamond::zero(),
                y: Diamond::eight(),
                ..Default::default()
            },
            Rail::Right => Position {
                x: Diamond::four(),
                y: Diamond::zero(),
                ..Default::default()
            },
            Rail::Bottom => Position {
                x: Diamond::zero(),
                y: Diamond::zero(),
                ..Default::default()
            },
            Rail::Left => Position {
                x: Diamond::zero(),
                y: Diamond::zero(),
                ..Default::default()
            },
        }
    }

    pub fn is_vertical(&self) -> bool {
        matches!(*self, Rail::Left | Rail::Right)
    }

    pub fn is_horizontal(&self) -> bool {
        matches!(*self, Rail::Top | Rail::Bottom)
    }

    fn approach_heading_components(&self) -> (f64, f64) {
        match *self {
            Rail::Top => (0.0, 1.0),
            Rail::Bottom => (0.0, -1.0),
            Rail::Left => (-1.0, 0.0),
            Rail::Right => (1.0, 0.0),
        }
    }

    fn positive_tangent_components(&self) -> (f64, f64) {
        match *self {
            Rail::Top | Rail::Bottom => (1.0, 0.0),
            Rail::Left | Rail::Right => (0.0, 1.0),
        }
    }

    /// Return an absolute table heading that would send a ball toward this rail at the requested
    /// bank angle.
    ///
    /// `angle_degrees` may be specified either from the rail normal (`FromNormal`) or from the
    /// rail face (`FromRailFace`). `tangent_direction` chooses which along-rail branch to use.
    ///
    /// Examples for `angle_degrees = 30` measured `FromNormal`:
    /// - `Rail::Top` + `Positive` => `30°`
    /// - `Rail::Top` + `Negative` => `330°`
    /// - `Rail::Right` + `Positive` => `60°`
    /// - `Rail::Right` + `Negative` => `120°`
    ///
    /// Angles must lie in `[0°, 90°]`. A `FromRailFace` angle of `0°` is parallel to the rail and
    /// therefore does not itself guarantee eventual contact, but it is still accepted as a valid
    /// absolute heading construction.
    pub fn bank_heading_toward(
        self,
        angle_degrees: f64,
        reference: RailAngleReference,
        tangent_direction: RailTangentDirection,
    ) -> Angle {
        assert!(
            (0.0..=90.0).contains(&angle_degrees),
            "bank angle must be in [0°, 90°], got {angle_degrees}"
        );

        let angle_from_normal = match reference {
            RailAngleReference::FromNormal => angle_degrees,
            RailAngleReference::FromRailFace => 90.0 - angle_degrees,
        }
        .to_radians();
        let (approach_x, approach_y) = self.approach_heading_components();
        let (tangent_x, tangent_y) = self.positive_tangent_components();
        let tangent_sign = tangent_direction.sign();

        Angle::from_north(
            approach_x * angle_from_normal.cos()
                + tangent_sign * tangent_x * angle_from_normal.sin(),
            approach_y * angle_from_normal.cos()
                + tangent_sign * tangent_y * angle_from_normal.sin(),
        )
    }
}

#[derive(Clone, Debug)]
enum Overlay {
    DashedLine {
        start: Position,
        end: Position,
        color: Rgba<u8>,
    },
    SmoothPolyline {
        points: Vec<Position>,
        width_px: f32,
        color: Rgba<u8>,
    },
    GhostBall {
        center: Position,
        fill_color: Rgba<u8>,
        outline_color: Rgba<u8>,
    },
}

#[derive(Clone, Debug)]
/// The full and complete data structure describing the state of a game.
#[derive(Default)]
pub struct GameState {
    pub table_spec: TableSpec,
    ball_positions: Vec<Ball>,
    pub ty: GameType,
    pub cueball_modifier: CueballModifier,

    lines_to_draw: Vec<Overlay>,
}

impl GameState {
    pub fn new(table_spec: TableSpec) -> Self {
        Self {
            table_spec,
            ..Default::default()
        }
    }

    pub fn with_balls<I>(table_spec: TableSpec, balls: I) -> Self
    where
        I: IntoIterator<Item = Ball>,
    {
        let mut state = Self::new(table_spec);
        state.add_balls(balls);
        state
    }

    pub fn balls(&self) -> &[Ball] {
        &self.ball_positions
    }

    pub fn add_ball(&mut self, ball: Ball) {
        self.ball_positions.push(ball);
    }

    pub fn add_balls<I>(&mut self, balls: I)
    where
        I: IntoIterator<Item = Ball>,
    {
        for ball in balls {
            self.add_ball(ball);
        }
    }

    pub fn select_ball(&self, ball_type: BallType) -> Option<&Ball> {
        self.ball_positions.iter().find(|b| b.ty == ball_type)
    }

    /// This is mildly hacky, but works for now to resolve all the unresolved
    /// inches adjustments.
    pub fn resolve_positions(&mut self) {
        for ball in self.ball_positions.iter_mut() {
            ball.position.resolve_shifts(&self.table_spec);
        }
    }

    pub fn freeze_to_rail(&mut self, rail: Rail, diamond: Diamond, mut ball: Ball) {
        match rail {
            Rail::Top => {
                ball.position.y =
                    Diamond::eight() - self.table_spec.inches_to_diamond(ball.spec.radius.clone());
                ball.position.x = diamond;
            }
            Rail::Right => {
                ball.position.x =
                    Diamond::four() - self.table_spec.inches_to_diamond(ball.spec.radius.clone());
                ball.position.y = diamond;
            }
            Rail::Bottom => {
                ball.position.y =
                    Diamond::zero() + self.table_spec.inches_to_diamond(ball.spec.radius.clone());
                ball.position.x = diamond;
            }
            Rail::Left => {
                ball.position.x =
                    Diamond::zero() + self.table_spec.inches_to_diamond(ball.spec.radius.clone());
                ball.position.y = diamond;
            }
        };

        self.add_ball(ball);
    }

    pub fn add_dotted_line(&mut self, from: &Position, to: &Position, color: Rgba<u8>) {
        let mut from = from.clone();
        from.resolve_shifts(&self.table_spec);

        let mut to = to.clone();
        to.resolve_shifts(&self.table_spec);

        self.lines_to_draw.push(Overlay::DashedLine {
            start: from,
            end: to,
            color,
        })
    }

    /// Add a dotted polyline by drawing dashed segments between each consecutive pair of points.
    pub fn add_dotted_polyline(&mut self, points: &[Position], color: Rgba<u8>) {
        for window in points.windows(2) {
            self.add_dotted_line(&window[0], &window[1], color);
        }
    }

    /// Add a smooth anti-aliased polyline overlay.
    pub fn add_smooth_polyline(&mut self, points: &[Position], color: Rgba<u8>) {
        self.add_smooth_polyline_with_width(points, 4.0, color);
    }

    /// Add a smooth anti-aliased polyline overlay with an explicit width.
    pub fn add_smooth_polyline_with_width(
        &mut self,
        points: &[Position],
        width_px: f32,
        color: Rgba<u8>,
    ) {
        let mut resolved = Vec::with_capacity(points.len());
        for point in points {
            let mut point = point.clone();
            point.resolve_shifts(&self.table_spec);
            resolved.push(point);
        }

        self.lines_to_draw.push(Overlay::SmoothPolyline {
            points: resolved,
            width_px,
            color,
        });
    }

    /// Add a translucent ghost-ball marker at `position`.
    pub fn add_ghost_ball(
        &mut self,
        position: &Position,
        fill_color: Rgba<u8>,
        outline_color: Rgba<u8>,
    ) {
        let mut position = position.clone();
        position.resolve_shifts(&self.table_spec);

        self.lines_to_draw.push(Overlay::GhostBall {
            center: position,
            fill_color,
            outline_color,
        });
    }

    /// Add a dotted overlay for a traced ball path.
    pub fn add_dotted_ball_path(&mut self, path: &BallPath, color: Rgba<u8>) {
        self.add_dotted_polyline(&path.projected_points(&self.table_spec), color);
    }

    /// Add a dotted overlay for a traced ball path and include a ghost ball at the start.
    pub fn add_dotted_ball_path_with_start_ghost(
        &mut self,
        path: &BallPath,
        path_color: Rgba<u8>,
        ghost_fill_color: Rgba<u8>,
        ghost_outline_color: Rgba<u8>,
    ) {
        let start = path
            .initial_state
            .as_ball_state()
            .projected_position(&self.table_spec);
        self.add_ghost_ball(&start, ghost_fill_color, ghost_outline_color);
        self.add_dotted_ball_path(path, path_color);
    }

    /// Add a dotted idealized aim line from `shooting_position` to the ghost-ball target that
    /// would pot `object_ball` to `destination`.
    pub fn add_dotted_aim_line(
        &mut self,
        object_ball: &Ball,
        destination: &Position,
        shooting_position: &Position,
        color: Rgba<u8>,
    ) -> Position {
        let ghost_ball = object_ball.ghost_ball(destination, &self.table_spec);
        self.add_ghost_ball(
            &ghost_ball,
            Rgba([255, 255, 255, 64]),
            Rgba([0, 0, 0, 96]),
        );
        self.add_dotted_line(shooting_position, &ghost_ball, color);
        ghost_ball
    }

    /// Add a dotted idealized aim line from `shooting_position` to the ghost-ball target that
    /// would pot `object_ball` to the aiming center of `pocket`.
    pub fn add_dotted_aim_line_to_pocket(
        &mut self,
        object_ball: &Ball,
        pocket: Pocket,
        shooting_position: &Position,
        color: Rgba<u8>,
    ) -> Position {
        self.add_dotted_aim_line(
            object_ball,
            &pocket.aiming_center(),
            shooting_position,
            color,
        )
    }

    /// Draws a 2D diagram of the current `GameState` and returns encoded PNG bytes.
    pub fn draw_2d_diagram(&self) -> Vec<u8> {
        use image::codecs::png::PngEncoder;
        use image::imageops::overlay;
        use image::{ImageEncoder, ImageFormat, RgbaImage};

        let ball_diameter_px = ideal_ball_size_px();
        let mut resolved = self.clone();
        resolved.resolve_positions();

        let mut table: RgbaImage =
            image::load_from_memory_with_format(assets::TABLE_DIAGRAM, ImageFormat::Png)
                .expect("broken table asset")
                .into_rgba8();

        let (tw, th) = table.dimensions();

        for overlay in resolved.lines_to_draw.iter() {
            match overlay {
                Overlay::DashedLine { start, end, color } => {
                    drawing::draw_dashed_line_thick_mut(
                        &mut table, start, end, 3., 12., 2., *color,
                    );
                }
                Overlay::SmoothPolyline {
                    points,
                    width_px,
                    color,
                } => {
                    drawing::draw_smooth_polyline_mut(&mut table, points, *width_px, *color);
                }
                Overlay::GhostBall {
                    center,
                    fill_color,
                    outline_color,
                } => {
                    drawing::draw_ghost_ball_mut(
                        &mut table,
                        center,
                        ball_diameter_px,
                        *fill_color,
                        *outline_color,
                    );
                }
            }
        }

        for ball in &resolved.ball_positions {
            let ball_png = assets::ball_img(ball.ty.clone());
            let mut ball_img: RgbaImage =
                image::load_from_memory_with_format(&ball_png, ImageFormat::Png)
                    .expect("bad ball image")
                    .into_rgba8();
            ball_img = resize(
                &ball_img,
                ball_diameter_px,
                ball_diameter_px,
                FilterType::CatmullRom,
            );
            let (bw, bh) = ball_img.dimensions();

            // Compute where the ball's *centre* should go
            let (px, py) = diamond_to_pixel(&ball.position);

            // Compute where to begin drawing the ball.
            // We have to account for the width and height of the ball.
            // Overlaying a png starts drawing at the top-left corner of the
            // ball, so we need to start drawing at px - bw/2, py - bh/2
            let mut px_shifted = px - (bw as i32 / 2);
            let mut py_shifted = py - (bh as i32 / 2);

            // Prevent any out of bounds weirdness (shouldn't happen).
            px_shifted = px_shifted.clamp(0, (tw - bw) as i32);
            py_shifted = py_shifted.clamp(0, (th - bh) as i32);

            overlay(&mut table, &ball_img, px_shifted.into(), py_shifted.into());
        }

        let mut buf = Vec::new();
        PngEncoder::new(&mut buf)
            .write_image(&table, tw, th, image::ColorType::Rgba8.into())
            .expect("PNG encode failed");
        buf
    }
}

// TODO: Return result, swap unwraps to ?.
pub fn write_png_to_file(png_bytes: &[u8], path: Option<&Path>) {
    let out_path = path.unwrap_or_else(|| Path::new("output.png"));
    let mut file = File::create(out_path).unwrap();
    file.write_all(png_bytes).unwrap();
    file.flush().unwrap();
}

pub fn rack_9_ball() -> Vec<Ball> {
    let ball_types = [
        BallType::One,
        BallType::Two,
        BallType::Three,
        BallType::Four,
        BallType::Nine,
        BallType::Five,
        BallType::Six,
        BallType::Seven,
        BallType::Eight,
    ];

    racked_ball_positions()
        .into_iter()
        .enumerate()
        .map(|(idx, pos)| Ball {
            ty: ball_types[idx].clone(),
            position: pos,
            spec: Default::default(),
        })
        .collect()
}

pub fn racked_ball_positions() -> Vec<Position> {
    let head_ball_position = RACK_SPOT.clone();
    let mut second_row_left = head_ball_position.clone();

    second_row_left
        .shift_vertically_inches(
            TYPICAL_BALL_RADIUS.clone().neg() * OPTIMAL_PACKING_RADIUS_SHIFT.clone(),
        )
        .shift_horizontally_inches(TYPICAL_BALL_RADIUS.clone().neg());

    let mut second_row_right = second_row_left.clone();
    second_row_right.shift_horizontally_inches(TYPICAL_BALL_RADIUS.clone().double());

    let mut third_row_left = second_row_left.clone();
    third_row_left
        .shift_vertically_inches(
            TYPICAL_BALL_RADIUS.clone().neg() * OPTIMAL_PACKING_RADIUS_SHIFT.clone(),
        )
        .shift_horizontally_inches(TYPICAL_BALL_RADIUS.clone().neg());

    let mut third_row_center = third_row_left.clone();
    third_row_center.shift_horizontally_inches(TYPICAL_BALL_RADIUS.clone().double());

    let mut third_row_right = third_row_center.clone();
    third_row_right.shift_horizontally_inches(TYPICAL_BALL_RADIUS.clone().double());

    let mut fourth_row_left = third_row_center.clone();
    fourth_row_left
        .shift_vertically_inches(
            TYPICAL_BALL_RADIUS.clone().neg() * OPTIMAL_PACKING_RADIUS_SHIFT.clone(),
        )
        .shift_horizontally_inches(TYPICAL_BALL_RADIUS.clone().neg());

    let mut fourth_row_right = fourth_row_left.clone();
    fourth_row_right.shift_horizontally_inches(TYPICAL_BALL_RADIUS.clone().double());

    let mut final_ball = third_row_center.clone();
    final_ball.shift_vertically_inches(
        TYPICAL_BALL_RADIUS.clone().double().neg() * OPTIMAL_PACKING_RADIUS_SHIFT.clone(),
    );

    let mut positions = vec![
        head_ball_position,
        second_row_left,
        second_row_right,
        third_row_left,
        third_row_center,
        third_row_right,
        fourth_row_left,
        fourth_row_right,
        final_ball,
    ];
    let table_spec = TableSpec::default();
    for position in &mut positions {
        position.resolve_shifts(&table_spec);
    }
    positions
}
