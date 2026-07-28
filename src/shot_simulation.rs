mod robust;

pub use robust::*;

#[cfg(test)]
use crate::resolve_n_ball_system_event_detailed_with_physics_and_pockets_on_table;
use crate::{
    advance_airborne_ball, advance_motion_on_table, classify_motion_phase,
    human_tuned_preview_motion_config, n_ball_system_collision_delta,
    resolve_validated_n_ball_system_event_detailed_with_physics_and_pockets_on_table,
    strike_resting_ball, validate_and_recover_n_ball_system_states, Angle, BallBallCollisionConfig,
    BallSetPhysicsSpec, BallState, CollisionModel, CueStrikeConfig, CueTipContact, Inches, Inches2,
    InchesPerSecond, MotionPhase, NBallGeometryError, NBallSystemAppliedEffect, NBallSystemState,
    OnTableMotionConfig, PlayingConditionsPreset, Pocket, PocketAwareEventCache, PocketJaw, Rail,
    RailCollisionProfile, RailModel, RestingOnTableBallState, Scale, Seconds, Shot, ShotError,
    TableSpec, MAX_CONSECUTIVE_ZERO_TIME_N_BALL_EVENTS, SHARED_BALL_BALL_CONTACT_STATE_EPSILON,
    SIMULTANEOUS_EVENT_TOLERANCE_SECONDS, STANDARD_GRAVITY_INCHES_PER_SECOND_SQUARED,
};
use bigdecimal::ToPrimitive;
use std::error::Error;
use std::fmt;

/// Stable identity for one physical ball in a shot scene.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BallId(u16);

impl BallId {
    pub const WHITE: Self = Self(0);
    pub const YELLOW: Self = Self(1);
    pub const RED: Self = Self(2);

    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u16 {
        self.0
    }

    pub const fn value(self) -> u16 {
        self.0
    }
}

/// Rule role for one ball in a three-cushion scene.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CaromBallRole {
    Cue,
    YellowCue,
    Red,
}

/// A resting scene ball with stable identity and explicit rule role.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneBall {
    pub id: BallId,
    pub role: CaromBallRole,
    pub state: RestingOnTableBallState,
}

impl SceneBall {
    pub fn resting(id: BallId, role: CaromBallRole, position: Inches2) -> Self {
        Self {
            id,
            role,
            state: RestingOnTableBallState::try_from(BallState::resting_at(position))
                .expect("an exactly resting state must satisfy the resting-state invariant"),
        }
    }
}

/// Canonically ID-ordered and geometry-validated three-ball carom layout.
#[derive(Clone, Debug, PartialEq)]
pub struct ShotLayout {
    balls: Box<[SceneBall]>,
}

impl ShotLayout {
    pub fn new(
        physics: &PhysicsProfile,
        balls: impl IntoIterator<Item = SceneBall>,
    ) -> Result<Self, ShotSimulationError> {
        let mut balls = balls.into_iter().collect::<Vec<_>>();
        balls.sort_by_key(|ball| ball.id);
        if balls.len() != 3 {
            return Err(ShotSimulationError::InvalidBallCount {
                expected: 3,
                actual: balls.len(),
            });
        }
        for pair in balls.windows(2) {
            if pair[0].id == pair[1].id {
                return Err(ShotSimulationError::DuplicateBallId(pair[0].id));
            }
        }
        for role in [
            CaromBallRole::Cue,
            CaromBallRole::YellowCue,
            CaromBallRole::Red,
        ] {
            let count = balls.iter().filter(|ball| ball.role == role).count();
            if count != 1 {
                return Err(ShotSimulationError::InvalidRoleCount { role, count });
            }
        }

        let radius = physics.ball.radius.as_f64();
        let width = 4.0 * physics.table.diamond_length.as_f64();
        let length = 8.0 * physics.table.diamond_length.as_f64();
        for ball in &balls {
            let position = &ball.state.as_ball_state().position;
            let x = position.x().as_f64();
            let y = position.y().as_f64();
            if !x.is_finite() || !y.is_finite() {
                return Err(ShotSimulationError::NonFiniteInput("ball position"));
            }
            if x < radius || x > width - radius || y < radius || y > length - radius {
                return Err(ShotSimulationError::BallOutsidePlayingSurface {
                    ball: ball.id,
                    x_inches: x,
                    y_inches: y,
                });
            }
        }
        for first in 0..balls.len() {
            for second in first + 1..balls.len() {
                let first_position = &balls[first].state.as_ball_state().position;
                let second_position = &balls[second].state.as_ball_state().position;
                let dx = second_position.x().as_f64() - first_position.x().as_f64();
                let dy = second_position.y().as_f64() - first_position.y().as_f64();
                let center_distance = dx.hypot(dy);
                let required = 2.0 * radius;
                if center_distance < required - 1e-6 {
                    return Err(ShotSimulationError::OverlappingBalls {
                        first: balls[first].id,
                        second: balls[second].id,
                        center_distance_inches: center_distance,
                        required_center_distance_inches: required,
                    });
                }
            }
        }

        Ok(Self {
            balls: balls.into_boxed_slice(),
        })
    }

    pub fn three_cushion(
        white: Inches2,
        yellow: Inches2,
        red: Inches2,
    ) -> Result<Self, ShotSimulationError> {
        let physics = PhysicsProfile::three_cushion_default();
        Self::new(
            &physics,
            [
                SceneBall::resting(BallId::WHITE, CaromBallRole::Cue, white),
                SceneBall::resting(BallId::YELLOW, CaromBallRole::YellowCue, yellow),
                SceneBall::resting(BallId::RED, CaromBallRole::Red, red),
            ],
        )
    }

    pub fn three_cushion_from_diamonds(
        white: (f64, f64),
        yellow: (f64, f64),
        red: (f64, f64),
    ) -> Result<Self, ShotSimulationError> {
        Self::three_cushion(
            carom_position_from_diamonds(white.0, white.1)?,
            carom_position_from_diamonds(yellow.0, yellow.1)?,
            carom_position_from_diamonds(red.0, red.1)?,
        )
    }

    pub fn balls(&self) -> &[SceneBall] {
        &self.balls
    }

    pub fn ball(&self, id: BallId) -> Option<&SceneBall> {
        self.balls
            .binary_search_by_key(&id, |ball| ball.id)
            .ok()
            .map(|index| &self.balls[index])
    }

    fn id_for_role(&self, role: CaromBallRole) -> BallId {
        self.balls
            .iter()
            .find(|ball| ball.role == role)
            .expect("validated layout must contain every carom role")
            .id
    }

    fn index_for_id(&self, id: BallId) -> Option<usize> {
        self.balls.binary_search_by_key(&id, |ball| ball.id).ok()
    }
}

/// Convert canonical carom table diamond coordinates to inches.
pub fn carom_position_from_diamonds(
    x_diamonds: f64,
    y_diamonds: f64,
) -> Result<Inches2, ShotSimulationError> {
    if !x_diamonds.is_finite() || !y_diamonds.is_finite() {
        return Err(ShotSimulationError::NonFiniteInput("diamond position"));
    }
    let diamond = TableSpec::three_cushion_carom_10ft()
        .diamond_length
        .as_f64();
    Ok(Inches2::new(
        Inches::from_f64(x_diamonds * diamond),
        Inches::from_f64(y_diamonds * diamond),
    ))
}

/// Immutable validated physical configuration for direct shot execution.
#[derive(Clone, Debug, PartialEq)]
pub struct PhysicsProfile {
    table: TableSpec,
    ball: BallSetPhysicsSpec,
    motion: OnTableMotionConfig,
    collision_model: CollisionModel,
    collision: BallBallCollisionConfig,
    rail_model: RailModel,
    rails: RailCollisionProfile,
}

impl PhysicsProfile {
    pub fn new(
        table: TableSpec,
        ball: BallSetPhysicsSpec,
        motion: OnTableMotionConfig,
        collision_model: CollisionModel,
        collision: BallBallCollisionConfig,
        rail_model: RailModel,
        rails: RailCollisionProfile,
    ) -> Result<Self, ShotSimulationError> {
        let finite_inches = |value: &Inches| {
            value
                .magnitude
                .to_f64()
                .is_some_and(|value| value.is_finite())
        };
        let finite_scale = |value: &Scale| {
            value
                .magnitude
                .to_f64()
                .is_some_and(|value| value.is_finite())
        };
        let valid_nonnegative_scale = |value: &Scale| {
            finite_scale(value)
                && value
                    .magnitude
                    .to_f64()
                    .is_some_and(|magnitude| magnitude >= 0.0)
        };
        let valid_restitution =
            |value: &Scale| finite_scale(value) && (0.0..=1.0).contains(&value.as_f64());
        let valid_lossy_restitution =
            |value: &Scale| finite_scale(value) && (0.0..1.0).contains(&value.as_f64());
        let valid_unit_interval_scale =
            |value: &Scale| finite_scale(value) && (0.0..=1.0).contains(&value.as_f64());
        if !finite_inches(&table.diamond_length) || table.diamond_length.as_f64() <= 0.0 {
            return Err(ShotSimulationError::InvalidPhysicsProfile(
                "table diamond length must be finite and positive",
            ));
        }
        if !table
            .cushion_diamond_buffer
            .magnitude
            .to_f64()
            .is_some_and(|value| value.is_finite() && value >= 0.0)
        {
            return Err(ShotSimulationError::InvalidPhysicsProfile(
                "table cushion buffer must be finite and non-negative",
            ));
        }
        for pocket in &table.pockets {
            for value in [&pocket.depth.magnitude, &pocket.width.magnitude] {
                if !value
                    .to_f64()
                    .is_some_and(|value| value.is_finite() && value >= 0.0)
                {
                    return Err(ShotSimulationError::InvalidPhysicsProfile(
                        "pocket measurements must be finite and non-negative",
                    ));
                }
            }
        }
        if !finite_inches(&ball.radius) || ball.radius.as_f64() <= 0.0 {
            return Err(ShotSimulationError::InvalidPhysicsProfile(
                "ball radius must be finite and positive",
            ));
        }
        if !valid_lossy_restitution(&ball.airborne_table_contact.normal_restitution)
            || !valid_nonnegative_scale(&ball.airborne_table_contact.sliding_friction_coefficient)
            || !ball
                .airborne_table_contact
                .minimum_rebound_vertical_speed
                .as_f64()
                .is_finite()
            || ball
                .airborne_table_contact
                .minimum_rebound_vertical_speed
                .as_f64()
                < 0.0
        {
            return Err(ShotSimulationError::InvalidPhysicsProfile(
                "airborne table-contact coefficients are outside their finite physical ranges",
            ));
        }
        let phase_threshold_values = [
            motion.phase.thresholds.airborne_height.as_f64(),
            motion.phase.thresholds.airborne_vertical_speed.as_f64(),
            motion.phase.thresholds.rest_linear_speed.as_f64(),
            motion.phase.thresholds.rest_angular_speed.as_f64(),
        ];
        if !phase_threshold_values
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0)
        {
            return Err(ShotSimulationError::InvalidPhysicsProfile(
                "motion phase thresholds must be finite and non-negative",
            ));
        }
        if let crate::SlidingToRollingModel::Thresholded {
            contact_speed_epsilon,
        } = &motion.phase.sliding_to_rolling
        {
            let contact_speed_epsilon = contact_speed_epsilon.as_f64();
            if !contact_speed_epsilon.is_finite() || contact_speed_epsilon < 0.0 {
                return Err(ShotSimulationError::InvalidPhysicsProfile(
                    "sliding-to-rolling contact-speed epsilon must be finite and non-negative",
                ));
            }
        }
        let motion_values = [
            match &motion.sliding_friction {
                crate::SlidingFrictionModel::ConstantAcceleration {
                    acceleration_magnitude,
                } => acceleration_magnitude.as_f64(),
            },
            match &motion.spin_decay {
                crate::SpinDecayModel::ConstantAngularDeceleration {
                    angular_deceleration,
                } => angular_deceleration.as_f64(),
            },
            match &motion.rolling_resistance {
                crate::RollingResistanceModel::ConstantDeceleration {
                    linear_deceleration,
                } => linear_deceleration.as_f64(),
            },
        ];
        if !motion_values
            .iter()
            .all(|value| value.is_finite() && *value > 0.0)
        {
            return Err(ShotSimulationError::InvalidPhysicsProfile(
                "motion coefficients must be finite and positive",
            ));
        }
        if !valid_restitution(&collision.normal_restitution)
            || !valid_nonnegative_scale(&collision.tangential_friction_coefficient)
            || !valid_nonnegative_scale(&collision.object_table_static_friction_coefficient)
            || !valid_nonnegative_scale(&collision.friction_model.reference_coefficient())
        {
            return Err(ShotSimulationError::InvalidPhysicsProfile(
                "ball-collision coefficients are outside their finite physical ranges",
            ));
        }
        for rail in [&rails.top, &rails.right, &rails.bottom, &rails.left] {
            if !valid_restitution(&rail.normal_restitution)
                || !valid_nonnegative_scale(&rail.tangential_friction_coefficient)
                || !valid_nonnegative_scale(&rail.impact_cloth_friction_coefficient)
                || !valid_unit_interval_scale(&rail.effective_contact_height_ratio)
            {
                return Err(ShotSimulationError::InvalidPhysicsProfile(
                    "rail coefficients are outside their finite physical ranges",
                ));
            }
        }
        Ok(Self {
            table,
            ball,
            motion,
            collision_model,
            collision,
            rail_model,
            rails,
        })
    }

    pub fn three_cushion_with_conditions(conditions: PlayingConditionsPreset) -> Self {
        let table = TableSpec::three_cushion_carom_10ft();
        let conditions = conditions.conditions();
        Self::new(
            table.clone(),
            table.default_ball_set_physics_spec(),
            human_tuned_preview_motion_config().applying_conditions(&conditions),
            CollisionModel::ThrowAware,
            BallBallCollisionConfig::human_tuned().applying_conditions(&conditions),
            RailModel::SpinAware,
            RailCollisionProfile::human_tuned().applying_conditions(&conditions),
        )
        .expect("canonical three-cushion physics profile must be valid")
    }

    pub fn three_cushion_default() -> Self {
        Self::three_cushion_with_conditions(PlayingConditionsPreset::HeatedCarom)
    }

    pub fn table(&self) -> &TableSpec {
        &self.table
    }

    pub fn ball_set(&self) -> &BallSetPhysicsSpec {
        &self.ball
    }

    pub fn motion(&self) -> &OnTableMotionConfig {
        &self.motion
    }

    pub fn collision_model(&self) -> CollisionModel {
        self.collision_model
    }

    pub fn collision(&self) -> &BallBallCollisionConfig {
        &self.collision
    }

    pub fn rail_model(&self) -> RailModel {
        self.rail_model
    }

    pub fn rails(&self) -> &RailCollisionProfile {
        &self.rails
    }
}

/// Finite human-facing controls for a one-shot cue strike.
#[derive(Clone, Debug, PartialEq)]
pub struct ShotControls {
    heading_degrees: f64,
    cue_ball_speed_inches_per_second: f64,
    tip_contact: CueTipContact,
    cue_elevation_degrees: f64,
}

impl ShotControls {
    pub fn new(
        heading_degrees: f64,
        cue_ball_speed_inches_per_second: f64,
        side_tip_offset: f64,
        height_tip_offset: f64,
        cue_elevation_degrees: f64,
    ) -> Result<Self, ShotSimulationError> {
        for (name, value) in [
            ("heading", heading_degrees),
            ("cue-ball speed", cue_ball_speed_inches_per_second),
            ("side tip offset", side_tip_offset),
            ("height tip offset", height_tip_offset),
            ("cue elevation", cue_elevation_degrees),
        ] {
            if !value.is_finite() {
                return Err(ShotSimulationError::NonFiniteInput(name));
            }
        }
        if cue_ball_speed_inches_per_second < 0.0 {
            return Err(ShotSimulationError::NegativeLaunchSpeed(
                cue_ball_speed_inches_per_second,
            ));
        }
        if !(0.0..=90.0).contains(&cue_elevation_degrees) {
            return Err(ShotSimulationError::CueElevationOutOfRange(
                cue_elevation_degrees,
            ));
        }
        let tip_contact = CueTipContact::new(
            Scale::from_f64(side_tip_offset),
            Scale::from_f64(height_tip_offset),
        )
        .map_err(ShotSimulationError::Shot)?;

        Ok(Self {
            heading_degrees: heading_degrees.rem_euclid(360.0),
            cue_ball_speed_inches_per_second,
            tip_contact,
            cue_elevation_degrees,
        })
    }

    pub fn heading_degrees(&self) -> f64 {
        self.heading_degrees
    }

    pub fn cue_ball_speed_inches_per_second(&self) -> f64 {
        self.cue_ball_speed_inches_per_second
    }

    pub fn side_tip_offset(&self) -> f64 {
        self.tip_contact.side_offset().as_f64()
    }

    pub fn height_tip_offset(&self) -> f64 {
        self.tip_contact.height_offset().as_f64()
    }

    pub fn cue_elevation_degrees(&self) -> f64 {
        self.cue_elevation_degrees
    }

    fn to_shot(&self, cue: &CueStrikeConfig) -> Result<Shot, ShotSimulationError> {
        let heading_radians = self.heading_degrees.to_radians();
        let elevation_radians = self.cue_elevation_degrees.to_radians();
        let heading = Angle::from_north(heading_radians.sin(), heading_radians.cos());
        let elevation = Angle::from_north(elevation_radians.sin(), elevation_radians.cos());
        let tip = self.tip_contact.clone();
        Shot::new_for_cue_ball_launch_speed(
            heading,
            InchesPerSecond::new(Inches::from_f64(self.cue_ball_speed_inches_per_second)),
            tip,
            cue,
        )
        .and_then(|shot| shot.with_cue_elevation(elevation))
        .map_err(ShotSimulationError::Shot)
    }
}

/// Which of the two cue-colored balls the player strikes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ThreeCushionShooter {
    Cue,
    YellowCue,
}

/// Typed three-cushion shot request using a canonical cue configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct ThreeCushionShot {
    shooter: ThreeCushionShooter,
    controls: ShotControls,
    cue: CueStrikeConfig,
}

pub fn canonical_three_cushion_cue_config() -> CueStrikeConfig {
    CueStrikeConfig::new(Scale::from_f64(1.0), Scale::from_f64(0.1))
        .expect("canonical cue coefficients are valid")
}

impl ThreeCushionShot {
    pub fn new(shooter: ThreeCushionShooter, controls: ShotControls) -> Self {
        Self {
            shooter,
            controls,
            cue: canonical_three_cushion_cue_config(),
        }
    }

    pub fn with_cue_config(mut self, cue: CueStrikeConfig) -> Self {
        self.cue = cue;
        self
    }

    pub fn shooter(&self) -> ThreeCushionShooter {
        self.shooter
    }

    pub fn controls(&self) -> &ShotControls {
        &self.controls
    }

    pub fn command(&self, layout: &ShotLayout) -> Result<ShotCommand, ShotSimulationError> {
        let role = match self.shooter {
            ThreeCushionShooter::Cue => CaromBallRole::Cue,
            ThreeCushionShooter::YellowCue => CaromBallRole::YellowCue,
        };
        ShotCommand::new(
            layout.id_for_role(role),
            self.controls.to_shot(&self.cue)?,
            self.cue.clone(),
        )
    }
}

/// Validated low-level command for striking one stable-ID resting ball.
#[derive(Clone, Debug, PartialEq)]
pub struct ShotCommand {
    cue_ball: BallId,
    shot: Shot,
    cue: CueStrikeConfig,
}

impl ShotCommand {
    pub fn new(
        cue_ball: BallId,
        shot: Shot,
        cue: CueStrikeConfig,
    ) -> Result<Self, ShotSimulationError> {
        for (name, value) in [
            ("shot heading", shot.heading().as_degrees()),
            ("cue speed", shot.cue_speed().as_f64()),
            ("side tip offset", shot.tip_contact().side_offset().as_f64()),
            (
                "height tip offset",
                shot.tip_contact().height_offset().as_f64(),
            ),
            ("cue elevation", shot.cue_elevation().as_degrees()),
            ("cue mass ratio", cue.cue_mass_ratio().as_f64()),
            ("cue collision loss", cue.collision_energy_loss().as_f64()),
            (
                "cue endmass ratio",
                cue.cue_ball_to_endmass_ratio().as_f64(),
            ),
            ("cue miscue limit", cue.miscue_offset_limit().as_f64()),
        ] {
            if !value.is_finite() {
                return Err(ShotSimulationError::NonFiniteInput(name));
            }
        }
        Ok(Self {
            cue_ball,
            shot,
            cue,
        })
    }

    pub fn cue_ball(&self) -> BallId {
        self.cue_ball
    }
}

/// Execution bound for one physical shot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShotLimit {
    UntilSettled,
    EventCount(usize),
}

/// Why direct physical execution stopped.
#[derive(Clone, Debug, PartialEq)]
pub enum ShotTermination {
    Settled,
    EventLimitReached { limit: usize },
    UnsupportedPhysics { reason: UnsupportedPhysicsReason },
    ZeroTimeCycle { consecutive_events: usize },
    NoProgress,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UnsupportedPhysicsReason {
    NonIdealSharedBallBallContact { balls: Box<[BallId]> },
}

/// Nature of an applied ball-ball response when causal ordering is or is not modeled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BallBallContactResolution {
    Pairwise,
    SharedCoupledCausalityUnresolved,
}

/// One applied, rule-facing physical effect using stable ball identity.
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedEffect {
    BallBallContact {
        first: BallId,
        second: BallId,
        resolution: BallBallContactResolution,
    },
    BallRailContact {
        ball: BallId,
        rail: Rail,
    },
    BallJawContact {
        ball: BallId,
        pocket: Pocket,
        jaw: PocketJaw,
    },
    BallPocketed {
        ball: BallId,
        pocket: Pocket,
    },
    BallTableContact {
        ball: BallId,
    },
    MotionTransition {
        ball: BallId,
        before: MotionPhase,
        after: MotionPhase,
    },
    UnsupportedContact {
        reason: UnsupportedPhysicsReason,
    },
}

/// All applied rule-relevant effects at one absolute shot instant.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedEvent {
    pub at: Seconds,
    pub effects: Box<[ResolvedEffect]>,
}

/// Stable-ID final physical state.
#[derive(Clone, Debug, PartialEq)]
pub struct FinalBallState {
    pub id: BallId,
    pub role: CaromBallRole,
    pub state: NBallSystemState,
}

/// Complete owned physical facts from one direct shot.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnedShotResult {
    pub elapsed: Seconds,
    pub termination: ShotTermination,
    pub roles: ThreeCushionRoles,
    pub events: Box<[ResolvedEvent]>,
    pub final_states: Box<[FinalBallState]>,
    pub maximum_cue_ball_height: Inches,
    /// Estimated minimum 3D surface clearance to the untouched object ball after three cushions.
    ///
    /// `None` means the shot never entered that progress state. This is search guidance, not
    /// collision evidence; rule adjudication continues to use resolved contacts.
    pub estimated_closest_second_object_clearance: Option<Inches>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThreeCushionRoles {
    pub cue: BallId,
    pub object_a: BallId,
    pub object_b: BallId,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactInstant {
    pub event_index: usize,
    pub at: Seconds,
}

/// Maximum legal cue-ball height above its resting center plane during a three-cushion point.
pub const THREE_CUSHION_MAX_CUE_BALL_HEIGHT_INCHES: f64 = 1.0;

#[derive(Clone, Debug, PartialEq)]
pub struct ThreeCushionFacts {
    pub object_a_first_contact: Option<ContactInstant>,
    pub object_b_first_contact: Option<ContactInstant>,
    pub completion: Option<ContactInstant>,
    pub cushion_contacts_before_completion: u16,
    pub first_three_qualifying_cushions: [Option<Rail>; 3],
    pub maximum_cue_ball_height: Inches,
    /// Estimated minimum 3D surface clearance to the remaining object after three cushions and
    /// exactly one object-ball contact.
    pub estimated_closest_second_object_clearance: Option<Inches>,
}

impl ThreeCushionFacts {
    /// Whether object A was contacted by the cue ball.
    pub const fn object_a_touched(&self) -> bool {
        self.object_a_first_contact.is_some()
    }

    /// Whether object B was contacted by the cue ball.
    pub const fn object_b_touched(&self) -> bool {
        self.object_b_first_contact.is_some()
    }

    /// Whether at least three qualifying cue-ball cushion contacts occurred before completion.
    pub const fn three_cushions_touched(&self) -> bool {
        self.cushion_contacts_before_completion >= 3
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ThreeCushionMiss {
    MissingObjectContact,
    InsufficientCushions { required: u16, observed: u16 },
    CueBallHeightExceeded { limit: Inches, observed: Inches },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ThreeCushionIndeterminate {
    EventLimitReached { limit: usize },
    UnsupportedPhysics { reason: UnsupportedPhysicsReason },
    ZeroTimeCycle { consecutive_events: usize },
    NoProgress,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ThreeCushionAdjudication {
    Scored(ThreeCushionFacts),
    Miss {
        facts: ThreeCushionFacts,
        reason: ThreeCushionMiss,
    },
    Indeterminate {
        facts: ThreeCushionFacts,
        reason: ThreeCushionIndeterminate,
    },
}

impl ThreeCushionAdjudication {
    pub fn facts(&self) -> &ThreeCushionFacts {
        match self {
            Self::Scored(facts) | Self::Miss { facts, .. } | Self::Indeterminate { facts, .. } => {
                facts
            }
        }
    }

    pub fn is_scored(&self) -> bool {
        matches!(self, Self::Scored(_))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShotCompletion<S> {
    pub elapsed: Seconds,
    pub termination: ShotTermination,
    pub summary: S,
}

/// Full physical result plus its UMB Article 83 contact-condition projection.
#[derive(Clone, Debug, PartialEq)]
pub struct ThreeCushionResult {
    pub completion: ShotCompletion<ThreeCushionAdjudication>,
    pub final_states: Box<[FinalBallState]>,
    pub events: Box<[ResolvedEvent]>,
}

/// Allocation-compact result retaining adjudication and final states but not the effect ledger.
#[derive(Clone, Debug, PartialEq)]
pub struct CompactThreeCushionResult {
    pub completion: ShotCompletion<ThreeCushionAdjudication>,
    pub final_states: Box<[FinalBallState]>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ShotSimulationError {
    NonFiniteInput(&'static str),
    InvalidPhysicsProfile(&'static str),
    NegativeLaunchSpeed(f64),
    CueElevationOutOfRange(f64),
    InvalidBallCount {
        expected: usize,
        actual: usize,
    },
    DuplicateBallId(BallId),
    InvalidRoleCount {
        role: CaromBallRole,
        count: usize,
    },
    BallOutsidePlayingSurface {
        ball: BallId,
        x_inches: f64,
        y_inches: f64,
    },
    OverlappingBalls {
        first: BallId,
        second: BallId,
        center_distance_inches: f64,
        required_center_distance_inches: f64,
    },
    CueBallNotFound(BallId),
    InvalidCueBallRole {
        ball: BallId,
        role: CaromBallRole,
    },
    Shot(ShotError),
    Geometry(NBallGeometryError),
}

impl fmt::Display for ShotSimulationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteInput(name) => write!(formatter, "{name} must be finite"),
            Self::InvalidPhysicsProfile(reason) => {
                write!(formatter, "invalid physics profile: {reason}")
            }
            Self::NegativeLaunchSpeed(speed) => {
                write!(formatter, "cue-ball launch speed must be non-negative, got {speed}")
            }
            Self::CueElevationOutOfRange(elevation) => {
                write!(formatter, "cue elevation must be in [0, 90] degrees, got {elevation}")
            }
            Self::InvalidBallCount { expected, actual } => {
                write!(formatter, "expected {expected} carom balls, got {actual}")
            }
            Self::DuplicateBallId(id) => write!(formatter, "duplicate ball ID {}", id.get()),
            Self::InvalidRoleCount { role, count } => {
                write!(formatter, "expected one {role:?} ball, got {count}")
            }
            Self::BallOutsidePlayingSurface {
                ball,
                x_inches,
                y_inches,
            } => write!(
                formatter,
                "ball {} center ({x_inches}, {y_inches}) is outside the playing surface",
                ball.get()
            ),
            Self::OverlappingBalls {
                first,
                second,
                center_distance_inches,
                required_center_distance_inches,
            } => write!(
                formatter,
                "balls {} and {} are {center_distance_inches} in apart; require {required_center_distance_inches} in",
                first.get(),
                second.get()
            ),
            Self::CueBallNotFound(id) => write!(formatter, "cue ball {} is not in the layout", id.get()),
            Self::InvalidCueBallRole { ball, role } => write!(
                formatter,
                "ball {} has non-shootable role {role:?}",
                ball.get()
            ),
            Self::Shot(error) => write!(formatter, "invalid shot: {error:?}"),
            Self::Geometry(error) => write!(formatter, "invalid shot geometry: {error}"),
        }
    }
}

impl Error for ShotSimulationError {}

impl From<NBallGeometryError> for ShotSimulationError {
    fn from(value: NBallGeometryError) -> Self {
        Self::Geometry(value)
    }
}

#[derive(Default)]
struct ThreeCushionAccumulator {
    object_a_first_contact: Option<ContactInstant>,
    object_b_first_contact: Option<ContactInstant>,
    completion: Option<ContactInstant>,
    first_unsupported_contact: Option<ContactInstant>,
    cushion_contacts_before_completion: u16,
    first_three_qualifying_cushions: [Option<Rail>; 3],
    maximum_cue_ball_height: f64,
    estimated_closest_second_object_clearance: Option<f64>,
}

impl ThreeCushionAccumulator {
    fn observe(&mut self, event_index: usize, event: &ResolvedEvent, roles: ThreeCushionRoles) {
        let instant = ContactInstant {
            event_index,
            at: event.at,
        };
        if event
            .effects
            .iter()
            .any(|effect| matches!(effect, ResolvedEffect::UnsupportedContact { .. }))
        {
            self.first_unsupported_contact.get_or_insert(instant);
        }
        for effect in &event.effects {
            let ResolvedEffect::BallBallContact {
                first,
                second,
                resolution,
            } = effect
            else {
                continue;
            };
            if *resolution == BallBallContactResolution::SharedCoupledCausalityUnresolved {
                self.first_unsupported_contact.get_or_insert(instant);
                continue;
            }
            if (*first == roles.cue && *second == roles.object_a)
                || (*second == roles.cue && *first == roles.object_a)
            {
                self.object_a_first_contact.get_or_insert(instant);
            }
            if (*first == roles.cue && *second == roles.object_b)
                || (*second == roles.cue && *first == roles.object_b)
            {
                self.object_b_first_contact.get_or_insert(instant);
            }
        }
        if self.completion.is_none()
            && self.object_a_first_contact.is_some()
            && self.object_b_first_contact.is_some()
        {
            self.completion = Some(instant);
        }
        if self.completion.is_none() {
            for effect in &event.effects {
                if let ResolvedEffect::BallRailContact { ball, rail } = effect {
                    if *ball != roles.cue {
                        continue;
                    }
                    let count = self.cushion_contacts_before_completion;
                    if count < 3 {
                        self.first_three_qualifying_cushions[count as usize] = Some(*rail);
                    }
                    self.cushion_contacts_before_completion = count.saturating_add(1);
                }
            }
        }
    }

    fn observe_cue_ball_height_evidence(&mut self, maximum_height: f64) {
        self.maximum_cue_ball_height = self.maximum_cue_ball_height.max(maximum_height);
    }

    fn observe_second_object_clearance(&mut self, clearance: f64) {
        self.estimated_closest_second_object_clearance = Some(
            self.estimated_closest_second_object_clearance
                .map_or(clearance, |minimum| minimum.min(clearance)),
        );
    }

    fn remaining_object_after_three_cushions(&self, roles: ThreeCushionRoles) -> Option<BallId> {
        if self.cushion_contacts_before_completion < 3 || self.completion.is_some() {
            return None;
        }
        match (
            self.object_a_first_contact.is_some(),
            self.object_b_first_contact.is_some(),
        ) {
            (true, false) => Some(roles.object_b),
            (false, true) => Some(roles.object_a),
            (false, false) | (true, true) => None,
        }
    }

    fn facts(&self) -> ThreeCushionFacts {
        ThreeCushionFacts {
            object_a_first_contact: self.object_a_first_contact,
            object_b_first_contact: self.object_b_first_contact,
            completion: self.completion,
            cushion_contacts_before_completion: self.cushion_contacts_before_completion,
            first_three_qualifying_cushions: self.first_three_qualifying_cushions,
            maximum_cue_ball_height: Inches::from_f64(self.maximum_cue_ball_height),
            estimated_closest_second_object_clearance: self
                .estimated_closest_second_object_clearance
                .map(Inches::from_f64),
        }
    }

    fn adjudicate(&self, termination: &ShotTermination) -> ThreeCushionAdjudication {
        let facts = self.facts();
        let completion_is_final = facts.completion.is_some_and(|completion| {
            self.first_unsupported_contact
                .is_none_or(|unsupported| completion.event_index < unsupported.event_index)
        });
        if completion_is_final {
            if facts.maximum_cue_ball_height.as_f64() > THREE_CUSHION_MAX_CUE_BALL_HEIGHT_INCHES {
                return ThreeCushionAdjudication::Miss {
                    reason: ThreeCushionMiss::CueBallHeightExceeded {
                        limit: Inches::from_f64(THREE_CUSHION_MAX_CUE_BALL_HEIGHT_INCHES),
                        observed: facts.maximum_cue_ball_height.clone(),
                    },
                    facts,
                };
            }
            if facts.cushion_contacts_before_completion >= 3 {
                return ThreeCushionAdjudication::Scored(facts);
            }
            return ThreeCushionAdjudication::Miss {
                reason: ThreeCushionMiss::InsufficientCushions {
                    required: 3,
                    observed: facts.cushion_contacts_before_completion,
                },
                facts,
            };
        }
        match termination {
            ShotTermination::Settled => ThreeCushionAdjudication::Miss {
                facts,
                reason: ThreeCushionMiss::MissingObjectContact,
            },
            ShotTermination::EventLimitReached { limit } => {
                ThreeCushionAdjudication::Indeterminate {
                    facts,
                    reason: ThreeCushionIndeterminate::EventLimitReached { limit: *limit },
                }
            }
            ShotTermination::UnsupportedPhysics { reason } => {
                ThreeCushionAdjudication::Indeterminate {
                    facts,
                    reason: ThreeCushionIndeterminate::UnsupportedPhysics {
                        reason: reason.clone(),
                    },
                }
            }
            ShotTermination::ZeroTimeCycle { consecutive_events } => {
                ThreeCushionAdjudication::Indeterminate {
                    facts,
                    reason: ThreeCushionIndeterminate::ZeroTimeCycle {
                        consecutive_events: *consecutive_events,
                    },
                }
            }
            ShotTermination::NoProgress => ThreeCushionAdjudication::Indeterminate {
                facts,
                reason: ThreeCushionIndeterminate::NoProgress,
            },
        }
    }
}

fn three_cushion_roles(
    layout: &ShotLayout,
    cue: BallId,
) -> Result<ThreeCushionRoles, ShotSimulationError> {
    let cue_ball = layout
        .ball(cue)
        .ok_or(ShotSimulationError::CueBallNotFound(cue))?;
    let (object_a, object_b) = match cue_ball.role {
        CaromBallRole::Cue => (
            layout.id_for_role(CaromBallRole::YellowCue),
            layout.id_for_role(CaromBallRole::Red),
        ),
        CaromBallRole::YellowCue => (
            layout.id_for_role(CaromBallRole::Cue),
            layout.id_for_role(CaromBallRole::Red),
        ),
        CaromBallRole::Red => {
            return Err(ShotSimulationError::InvalidCueBallRole {
                ball: cue,
                role: cue_ball.role,
            })
        }
    };
    Ok(ThreeCushionRoles {
        cue,
        object_a,
        object_b,
    })
}

fn map_pair(
    first: usize,
    second: usize,
    layout: &ShotLayout,
    resolution: BallBallContactResolution,
) -> ResolvedEffect {
    let first = layout.balls[first].id;
    let second = layout.balls[second].id;
    ResolvedEffect::BallBallContact {
        first: first.min(second),
        second: first.max(second),
        resolution,
    }
}

fn map_applied_effects(
    applied: Vec<NBallSystemAppliedEffect>,
    layout: &ShotLayout,
) -> Vec<ResolvedEffect> {
    let mut resolved = Vec::new();
    for effect in applied {
        match effect {
            NBallSystemAppliedEffect::BallBallPair {
                first_ball_index,
                second_ball_index,
            }
            | NBallSystemAppliedEffect::AirborneBallBallPair {
                first_ball_index,
                second_ball_index,
            } => resolved.push(map_pair(
                first_ball_index,
                second_ball_index,
                layout,
                BallBallContactResolution::Pairwise,
            )),
            NBallSystemAppliedEffect::SharedBallBallContact { ball_ball_pairs } => {
                for (first, second) in ball_ball_pairs {
                    resolved.push(map_pair(
                        first,
                        second,
                        layout,
                        BallBallContactResolution::SharedCoupledCausalityUnresolved,
                    ));
                }
            }
            NBallSystemAppliedEffect::BallJawContact {
                ball_index,
                pocket,
                jaw,
                captured,
            } => {
                let ball = layout.balls[ball_index].id;
                resolved.push(ResolvedEffect::BallJawContact { ball, pocket, jaw });
                if captured {
                    resolved.push(ResolvedEffect::BallPocketed { ball, pocket });
                }
            }
            NBallSystemAppliedEffect::BallPocketed { ball_index, pocket } => {
                resolved.push(ResolvedEffect::BallPocketed {
                    ball: layout.balls[ball_index].id,
                    pocket,
                });
            }
            NBallSystemAppliedEffect::BallRailContact { ball_index, rail } => {
                resolved.push(ResolvedEffect::BallRailContact {
                    ball: layout.balls[ball_index].id,
                    rail,
                });
            }
            NBallSystemAppliedEffect::BallTableContact { ball_index } => {
                resolved.push(ResolvedEffect::BallTableContact {
                    ball: layout.balls[ball_index].id,
                });
            }
            NBallSystemAppliedEffect::MotionTransition {
                ball_index,
                phase_before,
                phase_after,
            } => resolved.push(ResolvedEffect::MotionTransition {
                ball: layout.balls[ball_index].id,
                before: phase_before,
                after: phase_after,
            }),
        }
    }
    resolved
}

fn unsupported_shared_reason(
    ball_ball_pairs: &[(usize, usize)],
    layout: &ShotLayout,
) -> UnsupportedPhysicsReason {
    let mut ids = ball_ball_pairs
        .iter()
        .flat_map(|(first, second)| [layout.balls[*first].id, layout.balls[*second].id])
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    UnsupportedPhysicsReason::NonIdealSharedBallBallContact {
        balls: ids.into_boxed_slice(),
    }
}

#[derive(Clone, Debug, Default)]
struct CueBallHeightEvidence {
    maximum_height: f64,
}

impl CueBallHeightEvidence {
    fn observe_segment(&mut self, state: &NBallSystemState, duration_seconds: f64) {
        let NBallSystemState::Airborne(state) = state else {
            return;
        };
        let maximum_height = airborne_segment_maximum_height(state, duration_seconds);
        self.maximum_height = self.maximum_height.max(maximum_height);
    }
}

fn airborne_segment_maximum_height(state: &BallState, duration_seconds: f64) -> f64 {
    let vertical_velocity = state.vertical_velocity.as_f64();
    let apex_time = (vertical_velocity / STANDARD_GRAVITY_INCHES_PER_SECOND_SQUARED)
        .clamp(0.0, duration_seconds);
    state.height.as_f64() + vertical_velocity * apex_time
        - 0.5 * STANDARD_GRAVITY_INCHES_PER_SECOND_SQUARED * apex_time * apex_time
}

const SECOND_OBJECT_CLEARANCE_SUBDIVISIONS: u8 = 8;

fn advance_state_for_clearance(
    state: &NBallSystemState,
    elapsed: Seconds,
    ball: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
) -> Option<BallState> {
    match state {
        NBallSystemState::OnTable(state) => {
            Some(advance_motion_on_table(state, elapsed, ball, motion).state)
        }
        NBallSystemState::Airborne(state) => Some(advance_airborne_ball(state, elapsed)),
        NBallSystemState::Pocketed { .. } => None,
    }
}

fn estimated_closest_surface_clearance(
    first: &NBallSystemState,
    second: &NBallSystemState,
    duration_seconds: f64,
    ball: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
) -> Option<f64> {
    let mut minimum = f64::INFINITY;
    for subdivision in 0..=SECOND_OBJECT_CLEARANCE_SUBDIVISIONS {
        let elapsed = Seconds::new(
            duration_seconds * f64::from(subdivision)
                / f64::from(SECOND_OBJECT_CLEARANCE_SUBDIVISIONS),
        );
        let first = advance_state_for_clearance(first, elapsed, ball, motion)?;
        let second = advance_state_for_clearance(second, elapsed, ball, motion)?;
        let delta_x = first.position.x().as_f64() - second.position.x().as_f64();
        let delta_y = first.position.y().as_f64() - second.position.y().as_f64();
        let delta_z = first.height.as_f64() - second.height.as_f64();
        let center_distance = delta_x
            .mul_add(delta_x, delta_y.mul_add(delta_y, delta_z * delta_z))
            .sqrt();
        let contact_distance = 2.0 * ball.radius.as_f64();
        minimum = minimum.min((center_distance - contact_distance).max(0.0));
    }
    minimum.is_finite().then_some(minimum)
}

fn observe_second_object_clearance_segment(
    accumulator: &mut ThreeCushionAccumulator,
    states: &[NBallSystemState],
    layout: &ShotLayout,
    roles: ThreeCushionRoles,
    cue_index: usize,
    duration_seconds: f64,
    ball: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
) {
    let Some(target_id) = accumulator.remaining_object_after_three_cushions(roles) else {
        return;
    };
    let Some(target_index) = layout.index_for_id(target_id) else {
        return;
    };
    let Some(clearance) = estimated_closest_surface_clearance(
        &states[cue_index],
        &states[target_index],
        duration_seconds,
        ball,
        motion,
    ) else {
        return;
    };
    accumulator.observe_second_object_clearance(clearance);
}

struct CoreShotResult {
    elapsed: Seconds,
    termination: ShotTermination,
    roles: ThreeCushionRoles,
    events: Vec<ResolvedEvent>,
    final_states: Box<[FinalBallState]>,
    maximum_cue_ball_height: Inches,
    estimated_closest_second_object_clearance: Option<Inches>,
    adjudication: ThreeCushionAdjudication,
}

fn flush_pending_event(
    pending: &mut Option<(Seconds, Vec<ResolvedEffect>)>,
    accumulator: &mut ThreeCushionAccumulator,
    observed_event_count: &mut usize,
    roles: ThreeCushionRoles,
    retain_event: bool,
    retained: &mut Vec<ResolvedEvent>,
) {
    if let Some((at, effects)) = pending.take() {
        let event = ResolvedEvent {
            at,
            effects: effects.into_boxed_slice(),
        };
        accumulator.observe(*observed_event_count, &event, roles);
        *observed_event_count += 1;
        if retain_event {
            retained.push(event);
        }
    }
}

fn execute_core(
    physics: &PhysicsProfile,
    layout: &ShotLayout,
    command: &ShotCommand,
    limit: ShotLimit,
    retain_events: bool,
) -> Result<CoreShotResult, ShotSimulationError> {
    let cue_index = layout
        .index_for_id(command.cue_ball)
        .ok_or(ShotSimulationError::CueBallNotFound(command.cue_ball))?;
    let roles = three_cushion_roles(layout, command.cue_ball)?;
    let struck = strike_resting_ball(
        &layout.balls[cue_index].state,
        &command.shot,
        &command.cue,
        &physics.ball,
    )
    .map_err(ShotSimulationError::Shot)?;
    let mut states = layout
        .balls
        .iter()
        .map(|ball| NBallSystemState::from(ball.state.clone().into_on_table_ball_state()))
        .collect::<Vec<_>>();
    states[cue_index] = NBallSystemState::from(struck);
    states = validate_and_recover_n_ball_system_states(&states, &physics.ball)?;

    let mut elapsed = Seconds::zero();
    let mut primary_event_count = 0usize;
    let mut consecutive_zero_time_events = 0usize;
    let mut cache =
        PocketAwareEventCache::build(&states, &physics.ball, &physics.table, &physics.motion);
    let mut pending: Option<(Seconds, Vec<ResolvedEffect>)> = None;
    let mut retained = Vec::new();
    let mut accumulator = ThreeCushionAccumulator::default();
    let mut observed_event_count = 0usize;
    let mut cue_ball_height_evidence = CueBallHeightEvidence::default();

    let termination = loop {
        if let ShotLimit::EventCount(limit) = limit {
            if primary_event_count >= limit {
                break ShotTermination::EventLimitReached { limit };
            }
        }
        let Some(event) = cache.next_event() else {
            let settled = states.iter().all(|state| match state {
                NBallSystemState::OnTable(state) => {
                    classify_motion_phase(
                        state.as_ball_state(),
                        &physics.ball,
                        &physics.motion.phase,
                    ) == MotionPhase::Rest
                }
                NBallSystemState::Pocketed { .. } => true,
                NBallSystemState::Airborne(_) => false,
            });
            break if settled {
                ShotTermination::Settled
            } else {
                ShotTermination::NoProgress
            };
        };
        let step_elapsed = event.time().as_f64();
        assert!(
            step_elapsed >= 0.0,
            "next shot event must not go backwards in time"
        );
        cue_ball_height_evidence.observe_segment(&states[cue_index], step_elapsed);
        let absolute = Seconds::new(elapsed.as_f64() + step_elapsed);
        if pending.as_ref().is_some_and(|(at, _)| {
            (at.as_f64() - absolute.as_f64()).abs() > SIMULTANEOUS_EVENT_TOLERANCE_SECONDS
        }) {
            flush_pending_event(
                &mut pending,
                &mut accumulator,
                &mut observed_event_count,
                roles,
                retain_events,
                &mut retained,
            );
        }
        observe_second_object_clearance_segment(
            &mut accumulator,
            &states,
            layout,
            roles,
            cue_index,
            step_elapsed,
            &physics.ball,
            &physics.motion,
        );
        let states_before =
            (step_elapsed <= SIMULTANEOUS_EVENT_TOLERANCE_SECONDS).then(|| states.clone());
        let detailed =
            resolve_validated_n_ball_system_event_detailed_with_physics_and_pockets_on_table(
                &states,
                &event,
                &physics.ball,
                &physics.table,
                &physics.motion,
                physics.collision_model,
                &physics.collision,
                physics.rail_model,
                &physics.rails,
            );
        let detailed = detailed.map_err(ShotSimulationError::Geometry)?;
        states = detailed.states;
        let mut effects = map_applied_effects(detailed.effects, layout);
        let unsupported_reason = detailed
            .unsupported
            .map(|unsupported| unsupported_shared_reason(&unsupported.ball_ball_pairs, layout));
        if let Some(reason) = &unsupported_reason {
            effects.push(ResolvedEffect::UnsupportedContact {
                reason: reason.clone(),
            });
        }

        if let Some((_, combined)) = pending.as_mut() {
            combined.extend(effects);
        } else {
            pending = Some((absolute, effects));
        }

        elapsed = absolute;
        primary_event_count += 1;
        if let Some(reason) = unsupported_reason {
            break ShotTermination::UnsupportedPhysics { reason };
        }

        cache =
            PocketAwareEventCache::build(&states, &physics.ball, &physics.table, &physics.motion);
        if let Some(states_before) = states_before {
            consecutive_zero_time_events += 1;
            if consecutive_zero_time_events >= MAX_CONSECUTIVE_ZERO_TIME_N_BALL_EVENTS {
                break ShotTermination::ZeroTimeCycle {
                    consecutive_events: consecutive_zero_time_events,
                };
            }
            if n_ball_system_collision_delta(&states_before, &states)
                <= SHARED_BALL_BALL_CONTACT_STATE_EPSILON
            {
                break ShotTermination::NoProgress;
            }
        } else {
            consecutive_zero_time_events = 0;
        }
    };
    flush_pending_event(
        &mut pending,
        &mut accumulator,
        &mut observed_event_count,
        roles,
        retain_events,
        &mut retained,
    );
    observe_second_object_clearance_segment(
        &mut accumulator,
        &states,
        layout,
        roles,
        cue_index,
        0.0,
        &physics.ball,
        &physics.motion,
    );
    accumulator.observe_cue_ball_height_evidence(cue_ball_height_evidence.maximum_height);
    let adjudication = accumulator.adjudicate(&termination);
    let final_states = layout
        .balls
        .iter()
        .zip(states)
        .map(|(ball, state)| FinalBallState {
            id: ball.id,
            role: ball.role,
            state,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    Ok(CoreShotResult {
        elapsed,
        termination,
        roles,
        events: retained,
        final_states,
        maximum_cue_ball_height: Inches::from_f64(cue_ball_height_evidence.maximum_height),
        estimated_closest_second_object_clearance: accumulator
            .estimated_closest_second_object_clearance
            .map(Inches::from_f64),
        adjudication,
    })
}

/// Execute a deterministic typed shot and retain complete owned rule-facing physical facts.
pub fn execute_shot(
    physics: &PhysicsProfile,
    layout: &ShotLayout,
    command: &ShotCommand,
    limit: ShotLimit,
) -> Result<OwnedShotResult, ShotSimulationError> {
    let result = execute_core(physics, layout, command, limit, true)?;
    Ok(OwnedShotResult {
        elapsed: result.elapsed,
        termination: result.termination,
        roles: result.roles,
        events: result.events.into_boxed_slice(),
        final_states: result.final_states,
        maximum_cue_ball_height: result.maximum_cue_ball_height,
        estimated_closest_second_object_clearance: result.estimated_closest_second_object_clearance,
    })
}

/// Project complete owned shot facts onto the UMB Article 83 three-cushion contact condition.
pub fn project_three_cushion(result: &OwnedShotResult) -> ThreeCushionAdjudication {
    let mut accumulator = ThreeCushionAccumulator::default();
    accumulator.observe_cue_ball_height_evidence(result.maximum_cue_ball_height.as_f64());
    if let Some(clearance) = &result.estimated_closest_second_object_clearance {
        accumulator.observe_second_object_clearance(clearance.as_f64());
    }
    for (event_index, event) in result.events.iter().enumerate() {
        accumulator.observe(event_index, event, result.roles);
    }
    accumulator.adjudicate(&result.termination)
}

/// Execute a typed three-cushion shot and retain both physical evidence and adjudication.
pub fn execute_three_cushion(
    physics: &PhysicsProfile,
    layout: &ShotLayout,
    shot: &ThreeCushionShot,
    limit: ShotLimit,
) -> Result<ThreeCushionResult, ShotSimulationError> {
    let command = shot.command(layout)?;
    let result = execute_core(physics, layout, &command, limit, true)?;
    Ok(ThreeCushionResult {
        completion: ShotCompletion {
            elapsed: result.elapsed,
            termination: result.termination,
            summary: result.adjudication,
        },
        final_states: result.final_states,
        events: result.events.into_boxed_slice(),
    })
}

/// Execute the same shot without retaining the owned effect ledger.
pub fn execute_three_cushion_compact(
    physics: &PhysicsProfile,
    layout: &ShotLayout,
    shot: &ThreeCushionShot,
    limit: ShotLimit,
) -> Result<CompactThreeCushionResult, ShotSimulationError> {
    let command = shot.command(layout)?;
    let result = execute_core(physics, layout, &command, limit, false)?;
    Ok(CompactThreeCushionResult {
        completion: ShotCompletion {
            elapsed: result.elapsed,
            termination: result.termination,
            summary: result.adjudication,
        },
        final_states: result.final_states,
    })
}

#[cfg(test)]
mod applied_effect_tests {
    use super::*;
    use crate::{
        compute_next_n_ball_system_event_with_rails_and_pockets_on_table, AngularVelocity3,
        InchesPerSecondSq, MotionPhaseConfig, MotionTransitionConfig, NBallSystemEvent,
        OnTableBallState, RadiansPerSecondSq, RollingResistanceModel, SlidingFrictionModel,
        SpinDecayModel, Velocity2, TYPICAL_BALL_RADIUS,
    };

    fn motion() -> OnTableMotionConfig {
        MotionTransitionConfig {
            phase: MotionPhaseConfig::default(),
            sliding_friction: SlidingFrictionModel::ConstantAcceleration {
                acceleration_magnitude: InchesPerSecondSq::new("5"),
            },
            spin_decay: SpinDecayModel::ConstantAngularDeceleration {
                angular_deceleration: RadiansPerSecondSq::new(2.0),
            },
            rolling_resistance: RollingResistanceModel::ConstantDeceleration {
                linear_deceleration: InchesPerSecondSq::new("5"),
            },
        }
    }

    fn position(x: f64, y: f64) -> Inches2 {
        Inches2::new(Inches::from_f64(x), Inches::from_f64(y))
    }

    #[test]
    fn airborne_height_evidence_captures_apexes_and_uses_a_strict_boundary() {
        let vertical_velocity = 40.0;
        let state = BallState::airborne(
            position(10.0, 20.0),
            Inches::zero(),
            Velocity2::zero(),
            Inches::from_f64(vertical_velocity),
            AngularVelocity3::zero(),
        );
        let maximum = airborne_segment_maximum_height(&state, 0.2);
        let expected = vertical_velocity * vertical_velocity
            / (2.0 * STANDARD_GRAVITY_INCHES_PER_SECOND_SQUARED);
        assert!((maximum - expected).abs() <= 1e-12);

        let boundary = BallState::airborne(
            position(10.0, 20.0),
            Inches::from_f64(THREE_CUSHION_MAX_CUE_BALL_HEIGHT_INCHES),
            Velocity2::zero(),
            Inches::zero(),
            AngularVelocity3::zero(),
        );
        assert_eq!(
            airborne_segment_maximum_height(&boundary, 0.1),
            THREE_CUSHION_MAX_CUE_BALL_HEIGHT_INCHES
        );
    }

    #[test]
    fn estimated_clearance_uses_physical_interior_states_and_three_dimensions() {
        let ball = BallSetPhysicsSpec::default();
        let motion = motion();
        let cue = NBallSystemState::from(on_table(BallState::on_table(
            position(-5.0, 0.0),
            Velocity2::new("20", "0"),
            AngularVelocity3::zero(),
        )));
        let target = NBallSystemState::from(on_table(BallState::resting_at(position(0.0, 0.0))));
        let start =
            estimated_closest_surface_clearance(&cue, &target, 0.0, &ball, &motion).unwrap();
        let interior =
            estimated_closest_surface_clearance(&cue, &target, 0.5, &ball, &motion).unwrap();
        let final_cue =
            advance_state_for_clearance(&cue, Seconds::new(0.5), &ball, &motion).unwrap();
        let final_cue = NBallSystemState::from(on_table(final_cue));
        let end =
            estimated_closest_surface_clearance(&final_cue, &target, 0.0, &ball, &motion).unwrap();
        assert_eq!(interior, 0.0);
        assert!(interior < start);
        assert!(interior < end);

        let airborne = NBallSystemState::Airborne(BallState::airborne(
            position(0.0, 0.0),
            Inches::from_f64(3.0),
            Velocity2::zero(),
            Inches::zero(),
            AngularVelocity3::zero(),
        ));
        let vertical_clearance =
            estimated_closest_surface_clearance(&airborne, &target, 0.0, &ball, &motion).unwrap();
        let expected = 3.0 - 2.0 * ball.radius.as_f64();
        assert!((vertical_clearance - expected).abs() <= 1e-12);
    }

    fn on_table(state: BallState) -> OnTableBallState {
        OnTableBallState::try_from(state).unwrap()
    }

    fn resolve(
        states: &[NBallSystemState],
        ball: &BallSetPhysicsSpec,
        table: &TableSpec,
        motion: &OnTableMotionConfig,
        collision_model: CollisionModel,
        collision: &BallBallCollisionConfig,
        rail_model: RailModel,
    ) -> crate::NBallSystemResolvedStep {
        let event = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
            states, ball, table, motion,
        )
        .unwrap()
        .unwrap();
        resolve_n_ball_system_event_detailed_with_physics_and_pockets_on_table(
            states,
            &event,
            ball,
            table,
            motion,
            collision_model,
            collision,
            rail_model,
            &RailCollisionProfile::default(),
        )
        .unwrap()
    }

    #[test]
    fn detailed_resolution_records_every_disjoint_pair_applied_on_one_step() {
        let radius = TYPICAL_BALL_RADIUS.as_f64();
        let states = [
            on_table(BallState::on_table(
                position(10.0, 20.0 - (2.0 * radius + 7.5)),
                Velocity2::new("0", "10"),
                AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
            )),
            on_table(BallState::resting_at(position(10.0, 20.0))),
            on_table(BallState::on_table(
                position(30.0, 20.0 - (2.0 * radius + 7.5)),
                Velocity2::new("0", "10"),
                AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
            )),
            on_table(BallState::resting_at(position(30.0, 20.0))),
        ]
        .map(NBallSystemState::from);
        let resolved = resolve(
            &states,
            &BallSetPhysicsSpec::default(),
            &TableSpec::default(),
            &motion(),
            CollisionModel::Ideal,
            &BallBallCollisionConfig::ideal(),
            RailModel::Mirror,
        );
        assert_eq!(
            resolved.effects,
            vec![
                NBallSystemAppliedEffect::BallBallPair {
                    first_ball_index: 0,
                    second_ball_index: 1,
                },
                NBallSystemAppliedEffect::BallBallPair {
                    first_ball_index: 2,
                    second_ball_index: 3,
                },
            ]
        );
    }

    #[test]
    fn detailed_resolution_marks_compliant_shared_coupling_and_unresolved_causality() {
        let radius = TYPICAL_BALL_RADIUS.as_f64();
        let states = [
            on_table(BallState::on_table(
                position(10.0 - (2.0 * radius + 7.5), 20.0),
                Velocity2::new("10", "0"),
                AngularVelocity3::new(0.0, 10.0 / radius, 0.0),
            )),
            on_table(BallState::resting_at(position(10.0, 20.0))),
            on_table(BallState::resting_at(position(10.0 + 2.0 * radius, 20.0))),
        ]
        .map(NBallSystemState::from);
        let resolved = resolve(
            &states,
            &BallSetPhysicsSpec::default(),
            &TableSpec::default(),
            &motion(),
            CollisionModel::Ideal,
            &BallBallCollisionConfig::ideal(),
            RailModel::Mirror,
        );
        assert_eq!(
            resolved.effects.first(),
            Some(&NBallSystemAppliedEffect::SharedBallBallContact {
                ball_ball_pairs: vec![(0, 1), (1, 2)],
            })
        );
    }

    #[test]
    fn jaw_induced_capture_records_both_jaw_and_pocket_effects() {
        let states = [NBallSystemState::from(on_table(BallState::on_table(
            position(43.0, 57.0),
            Velocity2::new("12", "-10"),
            AngularVelocity3::zero(),
        )))];
        let resolved = resolve(
            &states,
            &BallSetPhysicsSpec::default(),
            &TableSpec::default(),
            &motion(),
            CollisionModel::Ideal,
            &BallBallCollisionConfig::ideal(),
            RailModel::Mirror,
        );
        assert!(matches!(
            resolved.effects.as_slice(),
            [NBallSystemAppliedEffect::BallJawContact {
                ball_index: 0,
                pocket: Pocket::CenterRight,
                captured: true,
                ..
            }]
        ));
        assert!(matches!(
            resolved.states[0],
            NBallSystemState::Pocketed {
                pocket: Pocket::CenterRight,
                ..
            }
        ));
    }

    #[test]
    fn coupled_shared_contact_retains_the_complete_contact_graph() {
        let radius = TYPICAL_BALL_RADIUS.as_f64();
        let contact_y = 30.0 - 3.0_f64.sqrt() * radius;
        let states = [
            on_table(BallState::on_table(
                position(20.0, contact_y - 7.5),
                Velocity2::new("0", "10"),
                AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
            )),
            on_table(BallState::resting_at(position(20.0 - radius, 30.0))),
            on_table(BallState::resting_at(position(20.0 + radius, 30.0))),
        ]
        .map(NBallSystemState::from);
        let ball = BallSetPhysicsSpec::default();
        let table = TableSpec::default();
        let motion = motion();
        let event = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
            &states, &ball, &table, &motion,
        )
        .unwrap()
        .unwrap();
        let resolved = resolve_n_ball_system_event_detailed_with_physics_and_pockets_on_table(
            &states,
            &event,
            &ball,
            &table,
            &motion,
            CollisionModel::ThrowAware,
            &BallBallCollisionConfig::human_tuned(),
            RailModel::Mirror,
            &RailCollisionProfile::default(),
        )
        .unwrap();

        assert!(
            resolved.unsupported.is_none(),
            "non-ideal shared contact should resolve through the coupled solver"
        );
        assert_eq!(
            resolved.effects,
            vec![NBallSystemAppliedEffect::SharedBallBallContact {
                ball_ball_pairs: vec![(0, 1), (0, 2), (1, 2)],
            }]
        );
        for object_state in &resolved.states[1..] {
            assert!(
                object_state.as_ball_state().velocity.y().as_f64() > 0.0,
                "each contacted object ball should receive forward momentum"
            );
        }
    }
    #[test]
    fn airborne_seed_materializes_the_complete_mixed_table_height_contact_component() {
        let ball = BallSetPhysicsSpec::default();
        let radius = ball.radius.as_f64();
        let sqrt_three = 3.0_f64.sqrt();
        let one = BallState::airborne(
            position(30.0, 30.0 - 2.0 * radius),
            Inches::zero(),
            Velocity2::new("0", "20"),
            Inches::from_f64(4.2536),
            AngularVelocity3::zero(),
        );
        let nine = BallState::on_table(
            position(30.0, 30.0),
            Velocity2::new("0", "10"),
            AngularVelocity3::zero(),
        );
        let six = BallState::resting_at(position(30.0 - radius, 30.0 + sqrt_three * radius));
        let seven = BallState::resting_at(position(30.0 + radius, 30.0 + sqrt_three * radius));
        let states = vec![
            NBallSystemState::Airborne(one.clone()),
            NBallSystemState::OnTable(on_table(nine.clone())),
            NBallSystemState::OnTable(on_table(six)),
            NBallSystemState::OnTable(on_table(seven)),
        ];
        let event = NBallSystemEvent::AirborneBallBallCollision {
            first_ball_index: 0,
            second_ball_index: 1,
            contact: crate::PredictedAirborneBallBallCollision {
                time_until_contact: Seconds::zero(),
                first_at_contact: one,
                second_at_contact: nine,
            },
        };
        let motion = motion();
        let resolved = resolve_n_ball_system_event_detailed_with_physics_and_pockets_on_table(
            &states,
            &event,
            &ball,
            &TableSpec::default(),
            &motion,
            CollisionModel::Ideal,
            &BallBallCollisionConfig::ideal(),
            RailModel::Mirror,
            &RailCollisionProfile::default(),
        )
        .unwrap();

        assert_eq!(
            resolved.effects,
            vec![NBallSystemAppliedEffect::SharedBallBallContact {
                ball_ball_pairs: vec![(0, 1), (1, 2), (1, 3), (2, 3)],
            }]
        );
        assert!(matches!(resolved.states[0], NBallSystemState::Airborne(_)));
        assert!(
            (resolved.states[0]
                .as_ball_state()
                .vertical_velocity
                .as_f64()
                - 4.2536)
                .abs()
                <= 1e-12,
            "the transactional island update must preserve the airborne endpoint's base vz"
        );

        let scheduled_event = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
            &states,
            &ball,
            &TableSpec::default(),
            &motion,
        )
        .unwrap()
        .expect("the mixed closing component should schedule");
        let scheduled = resolve_n_ball_system_event_detailed_with_physics_and_pockets_on_table(
            &states,
            &scheduled_event,
            &ball,
            &TableSpec::default(),
            &motion,
            CollisionModel::Ideal,
            &BallBallCollisionConfig::ideal(),
            RailModel::Mirror,
            &RailCollisionProfile::default(),
        )
        .unwrap();
        assert_eq!(
            scheduled.effects,
            vec![NBallSystemAppliedEffect::SharedBallBallContact {
                ball_ball_pairs: vec![(0, 1), (1, 2), (1, 3), (2, 3)],
            }]
        );
        let replayed = resolve_n_ball_system_event_detailed_with_physics_and_pockets_on_table(
            &states,
            &scheduled_event,
            &ball,
            &TableSpec::default(),
            &motion,
            CollisionModel::Ideal,
            &BallBallCollisionConfig::ideal(),
            RailModel::Mirror,
            &RailCollisionProfile::default(),
        )
        .unwrap();
        assert_eq!(
            scheduled.states, replayed.states,
            "recorded preliminary event replay must remain exact while topology is materialized internally"
        );
        assert_eq!(scheduled.effects, replayed.effects);

        for base_vertical_velocity in [-1.0, 1e-6] {
            let mut supported_states = states.clone();
            let mut endpoint = supported_states[0].as_ball_state().clone();
            endpoint.vertical_velocity =
                crate::InchesPerSecond::new(Inches::from_f64(base_vertical_velocity));
            supported_states[0] = NBallSystemState::Airborne(endpoint.clone());
            let support_event = NBallSystemEvent::AirborneBallBallCollision {
                first_ball_index: 0,
                second_ball_index: 1,
                contact: crate::PredictedAirborneBallBallCollision {
                    time_until_contact: Seconds::zero(),
                    first_at_contact: endpoint,
                    second_at_contact: supported_states[1].as_ball_state().clone(),
                },
            };
            let supported = resolve_n_ball_system_event_detailed_with_physics_and_pockets_on_table(
                &supported_states,
                &support_event,
                &ball,
                &TableSpec::default(),
                &motion,
                CollisionModel::Ideal,
                &BallBallCollisionConfig::ideal(),
                RailModel::Mirror,
                &RailCollisionProfile::default(),
            )
            .unwrap();
            assert!(
                matches!(supported.states[0], NBallSystemState::OnTable(_)),
                "mixed table support must clamp base vz={base_vertical_velocity}"
            );
            assert_eq!(
                supported.states[0]
                    .as_ball_state()
                    .vertical_velocity
                    .as_f64(),
                0.0
            );
        }

        let permuted_states = vec![
            states[3].clone(),
            states[0].clone(),
            states[2].clone(),
            states[1].clone(),
        ];
        let permuted_event = NBallSystemEvent::AirborneBallBallCollision {
            first_ball_index: 1,
            second_ball_index: 3,
            contact: crate::PredictedAirborneBallBallCollision {
                time_until_contact: Seconds::zero(),
                first_at_contact: permuted_states[1].as_ball_state().clone(),
                second_at_contact: permuted_states[3].as_ball_state().clone(),
            },
        };
        let permuted = resolve_n_ball_system_event_detailed_with_physics_and_pockets_on_table(
            &permuted_states,
            &permuted_event,
            &ball,
            &TableSpec::default(),
            &motion,
            CollisionModel::Ideal,
            &BallBallCollisionConfig::ideal(),
            RailModel::Mirror,
            &RailCollisionProfile::default(),
        )
        .unwrap();
        assert_eq!(
            permuted.effects,
            vec![NBallSystemAppliedEffect::SharedBallBallContact {
                ball_ball_pairs: vec![(0, 2), (0, 3), (1, 3), (2, 3)],
            }]
        );
        for (original_index, permuted_index) in [(0, 1), (1, 3), (2, 2), (3, 0)] {
            let original = resolved.states[original_index].as_ball_state();
            let reordered = permuted.states[permuted_index].as_ball_state();
            for (expected, actual) in [
                (
                    original.velocity.x().as_f64(),
                    reordered.velocity.x().as_f64(),
                ),
                (
                    original.velocity.y().as_f64(),
                    reordered.velocity.y().as_f64(),
                ),
                (
                    original.vertical_velocity.as_f64(),
                    reordered.vertical_velocity.as_f64(),
                ),
                (
                    original.angular_velocity.x().as_f64(),
                    reordered.angular_velocity.x().as_f64(),
                ),
                (
                    original.angular_velocity.y().as_f64(),
                    reordered.angular_velocity.y().as_f64(),
                ),
                (
                    original.angular_velocity.z().as_f64(),
                    reordered.angular_velocity.z().as_f64(),
                ),
            ] {
                assert!(
                    (actual - expected).abs() <= 1e-7,
                    "physical output changed under index permutation: expected={expected}, actual={actual}"
                );
            }
        }

        let frictional = resolve_n_ball_system_event_detailed_with_physics_and_pockets_on_table(
            &states,
            &event,
            &ball,
            &TableSpec::default(),
            &motion,
            CollisionModel::ThrowAware,
            &BallBallCollisionConfig::human_tuned(),
            RailModel::Mirror,
            &RailCollisionProfile::default(),
        )
        .unwrap();
        assert_eq!(
            frictional.effects,
            vec![NBallSystemAppliedEffect::SharedBallBallContact {
                ball_ball_pairs: vec![(0, 1), (1, 2), (1, 3), (2, 3)],
            }]
        );
        let specific_energy = |states: &[NBallSystemState]| {
            states
                .iter()
                .map(|state| {
                    let state = state.as_ball_state();
                    let linear = state.velocity.x().as_f64().powi(2)
                        + state.velocity.y().as_f64().powi(2)
                        + state.vertical_velocity.as_f64().powi(2);
                    let angular = state.angular_velocity.x().as_f64().powi(2)
                        + state.angular_velocity.y().as_f64().powi(2)
                        + state.angular_velocity.z().as_f64().powi(2);
                    0.5 * linear + radius * radius * angular / 5.0
                })
                .sum::<f64>()
        };
        let energy_before = specific_energy(&states);
        let energy_after = specific_energy(&frictional.states);
        assert!(
            energy_after.is_finite() && energy_after <= energy_before + 1e-8 * energy_before,
            "mixed coupled friction must be finite and passive: before={energy_before}, after={energy_after}"
        );
        for (first, second) in [(0, 1), (1, 2), (1, 3), (2, 3)] {
            let first_state = frictional.states[first].as_ball_state();
            let second_state = frictional.states[second].as_ball_state();
            let dx = second_state.position.x().as_f64() - first_state.position.x().as_f64();
            let dy = second_state.position.y().as_f64() - first_state.position.y().as_f64();
            let distance = dx.hypot(dy);
            let relative_normal =
                (second_state.velocity.x().as_f64() - first_state.velocity.x().as_f64()) * dx
                    / distance
                    + (second_state.velocity.y().as_f64() - first_state.velocity.y().as_f64()) * dy
                        / distance;
            assert!(
                relative_normal >= -1e-7,
                "mixed onset edge {first}-{second} remains closing at {relative_normal}"
            );
        }

        let next = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
            &resolved.states,
            &ball,
            &TableSpec::default(),
            &motion,
        )
        .unwrap();
        assert!(
            !matches!(
                next,
                Some(NBallSystemEvent::AirborneBallBallCollision {
                    first_ball_index: 0,
                    second_ball_index: 1,
                    ref contact,
                }) if contact.time_until_contact.as_f64() <= 1e-12
            ),
            "the resolved One-Nine edge must not remain as an immediate collision"
        );
    }
}
