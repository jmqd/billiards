#![allow(
    clippy::let_unit_value,
    clippy::needless_lifetimes,
    clippy::result_large_err
)]

use std::collections::HashMap;

use crate::{
    advance_airborne_ball, advance_motion_on_table,
    resolve_n_ball_system_event_with_physics_and_pockets_on_table,
    simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit,
    simulate_n_ball_system_with_physics_and_pockets_on_table_until_rest,
    trace_ball_path_with_rail_profile_on_table,
    visualization::{
        BallPathRenderOptions, BallPathWidthMode, EventMarkerStyle, GhostBallStyle,
        LabelOverlayStyle, PathColorMode, SmoothPolylineStyle,
    },
    Angle, Ball, BallBallCollisionConfig, BallPath, BallPathError, BallPathSegment, BallPathStop,
    BallSetPhysicsSpec, BallState, BallType, CollisionModel, CueStrikeConfig, CueTipContact,
    Diamond, GameState, GameType, HumanShotSpeedValidation, Inches, InchesPerSecond, MotionPhase,
    MotionPhaseThresholds, NBallGeometryError, NBallSystemEvent, NBallSystemSimulation,
    NBallSystemState, OnTableBallState, OnTableMotionConfig, OnTableStateError, PlayingConditions,
    PlayingConditionsPreset, Pocket, PocketJaw, Position, Rail, RailCollisionConfig,
    RailCollisionProfile, RailModel, RestingOnTableBallState, Scale, Seconds,
    SharedBallBallContactResolution, Shot, ShotError, ShotSpeedPreset, TableSpec,
    BOTTOM_LEFT_DIAMOND, BOTTOM_RIGHT_DIAMOND, CENTER_LEFT_DIAMOND, CENTER_RIGHT_DIAMOND,
    CENTER_SPOT, RACK_SPOT, TOP_LEFT_DIAMOND, TOP_RIGHT_DIAMOND,
};

use image::Rgba;

use winnow::ascii::{float, line_ending, till_line_ending};
use winnow::combinator::{alt, cut_err, delimited, eof, opt, peek, preceded, repeat, terminated};
use winnow::error::{ErrMode, InputError};
use winnow::prelude::*;
use winnow::stream::{LocatingSlice, Location};
use winnow::token::take_while;

const DEFAULT_JUMP_CUE_ELEVATION_DEGREES: f64 = 45.0;

/// Conservative scenario-DSL default for ordinary side-English shots that omit `.elevation(...)`.
///
/// TP A.3's rail-clearance geometry gives 1.384° for a head-spot to foot-spot center-ball hit.
/// The low-level `Shot` API remains idealized at 0° by default; DSL authors can explicitly opt
/// back into that level-cue idealization with `.elevation(0deg)`.
const DEFAULT_SIDE_ENGLISH_CUE_ELEVATION_DEGREES: f64 = 1.384;

#[derive(Debug, Clone, PartialEq)]
pub struct DslDoc {
    pub table: Option<TableRef>,
    pub game: Option<GameRef>,
    pub trace_max_events: Option<usize>,
    pub entries: Vec<DslEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DslEntry {
    Alias(AliasDef),
    Ball(BallPlacement),
    CueStrike(CueStrikeDef),
    BallBall(BallBallDef),
    RailResponse(RailResponseDef),
    Rails(RailsDef),
    Simulation(SimulationDef),
    Shot(ShotDef),
}

#[derive(Debug, Clone)]
pub struct DslScenario {
    pub game_state: GameState,
    pub shot: Option<ScenarioShot>,
    pub trace_max_events: Option<usize>,
    pub ball_ball_configs: HashMap<String, BallBallCollisionConfig>,
    pub rail_responses: HashMap<String, RailCollisionConfig>,
    pub rail_profiles: HashMap<String, RailCollisionProfile>,
    pub simulations: HashMap<String, SimulationPreset>,
}

struct EffectiveSimulationPhysics {
    motion: OnTableMotionConfig,
    collision_model: CollisionModel,
    collision_config: BallBallCollisionConfig,
    rail_model: RailModel,
    rail_profile: RailCollisionProfile,
    max_events: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
enum ScenarioTraceStop {
    UntilRest,
    EventLimit(usize),
}

impl ScenarioTraceStop {
    fn constrained_by(self, max_events: Option<usize>) -> Self {
        let Some(max_events) = max_events else {
            return self;
        };

        match self {
            Self::UntilRest => Self::EventLimit(max_events),
            Self::EventLimit(requested_max_events) => {
                Self::EventLimit(requested_max_events.min(max_events))
            }
        }
    }
}

impl DslScenario {
    pub fn ball_ball_config_named(
        &self,
        name: &str,
    ) -> Result<&BallBallCollisionConfig, DslBuildError> {
        self.ball_ball_configs
            .get(name)
            .ok_or_else(|| DslBuildError::UnknownBallBallConfig(name.to_string()))
    }

    pub fn rail_response_named(&self, name: &str) -> Result<&RailCollisionConfig, DslBuildError> {
        self.rail_responses
            .get(name)
            .ok_or_else(|| DslBuildError::UnknownRailResponse(name.to_string()))
    }

    pub fn rail_profile_named(&self, name: &str) -> Result<&RailCollisionProfile, DslBuildError> {
        self.rail_profiles
            .get(name)
            .ok_or_else(|| DslBuildError::UnknownRailProfile(name.to_string()))
    }

    pub fn simulation_named(&self, name: &str) -> Result<&SimulationPreset, DslBuildError> {
        self.simulations
            .get(name)
            .ok_or_else(|| DslBuildError::UnknownSimulation(name.to_string()))
    }

    pub fn ball_set_physics_spec(&self) -> BallSetPhysicsSpec {
        self.game_state.table_spec.default_ball_set_physics_spec()
    }

    pub fn preferred_simulation_name(&self) -> Option<&str> {
        if self.simulations.contains_key("default") {
            Some("default")
        } else if self.simulations.len() == 1 {
            self.simulations.keys().next().map(String::as_str)
        } else {
            None
        }
    }

    fn effective_simulation_physics(
        &self,
        motion: &OnTableMotionConfig,
        simulation_name: &str,
    ) -> Result<EffectiveSimulationPhysics, DslBuildError> {
        let simulation = self.simulation_named(simulation_name)?;
        let conditions = &simulation.conditions;

        Ok(EffectiveSimulationPhysics {
            motion: motion.applying_conditions(conditions),
            collision_model: simulation.collision_model,
            collision_config: self
                .ball_ball_config_named(&simulation.ball_ball_name)?
                .applying_conditions(conditions),
            rail_model: simulation.rail_model,
            rail_profile: self
                .rail_profile_named(&simulation.rails_name)?
                .applying_conditions(conditions),
            max_events: simulation.max_events,
        })
    }

    pub fn simulate_shot_system_with_simulation_on_table_until_rest(
        &self,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        simulation_name: &str,
    ) -> Result<Option<NBallSystemSimulation>, DslBuildError> {
        let simulation = self.effective_simulation_physics(motion, simulation_name)?;
        self.simulate_shot_system_with_physics_on_table_until_rest(
            ball_set,
            &simulation.motion,
            simulation.collision_model,
            &simulation.collision_config,
            simulation.rail_model,
            &simulation.rail_profile,
        )
    }

    pub fn simulate_shot_trace_with_simulation_on_table_until_rest(
        &self,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        simulation_name: &str,
    ) -> Result<Option<ScenarioShotTrace>, DslBuildError> {
        let simulation = self.effective_simulation_physics(motion, simulation_name)?;
        let stop = ScenarioTraceStop::UntilRest.constrained_by(simulation.max_events);
        self.execute_shot_trace_with_physics_on_table(
            ball_set,
            &simulation.motion,
            simulation.collision_model,
            &simulation.collision_config,
            simulation.rail_model,
            &simulation.rail_profile,
            stop,
        )
    }

    pub fn trace_shot_path_with_simulation_on_table(
        &self,
        stop: BallPathStop,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        simulation_name: &str,
    ) -> Result<Option<BallPath>, DslBuildError> {
        let simulation = self.effective_simulation_physics(motion, simulation_name)?;
        self.trace_shot_path_with_rail_profile_on_table(
            stop,
            ball_set,
            &simulation.motion,
            simulation.rail_model,
            &simulation.rail_profile,
        )
    }

    pub fn trace_shot_path_until_rest_with_simulation_on_table(
        &self,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        simulation_name: &str,
    ) -> Result<Option<BallPath>, DslBuildError> {
        self.trace_shot_path_with_simulation_on_table(
            BallPathStop::UntilRest,
            ball_set,
            motion,
            simulation_name,
        )
    }

    pub fn validate_shot_human_speed(
        &self,
    ) -> Result<Option<HumanShotSpeedValidation>, DslBuildError> {
        let Some(shot) = &self.shot else {
            return Ok(None);
        };

        shot.shot
            .human_speed_validation(&shot.cue_strike)
            .map(Some)
            .map_err(DslBuildError::InvalidShot)
    }

    pub fn strike_shot(
        &self,
        ball_set: &BallSetPhysicsSpec,
    ) -> Result<Option<BallState>, DslBuildError> {
        let Some(shot) = &self.shot else {
            return Ok(None);
        };
        let ball = self
            .game_state
            .select_ball(shot.ball.clone())
            .ok_or(DslBuildError::ShotTargetBallNotPlaced(shot.ball_ref))?;
        let resting = RestingOnTableBallState::try_from(BallState::from_position(
            &ball.position,
            &self.game_state.table_spec,
        ))
        .expect("game-state ball placements should always correspond to resting on-table states");

        crate::strike_resting_ball(&resting, &shot.shot, &shot.cue_strike, ball_set)
            .map(Some)
            .map_err(DslBuildError::InvalidShot)
    }

    pub fn strike_shot_on_table(
        &self,
        ball_set: &BallSetPhysicsSpec,
    ) -> Result<Option<OnTableBallState>, DslBuildError> {
        let Some(state) = self.strike_shot(ball_set)? else {
            return Ok(None);
        };

        OnTableBallState::try_new_with_thresholds(state, &crate::MotionPhaseThresholds::default())
            .map(Some)
            .map_err(|error| {
                DslBuildError::InvalidShot(match error {
                    crate::OnTableStateError::VerticalVelocityPresent {
                        vertical_velocity, ..
                    } => ShotError::ElevatedShotLeavesTable {
                        cue_elevation: self
                            .shot
                            .as_ref()
                            .expect("shot exists")
                            .shot
                            .cue_elevation(),
                        vertical_velocity,
                    },
                    crate::OnTableStateError::HeightAboveTablePlane { .. } => {
                        ShotError::ElevatedShotLeavesTable {
                            cue_elevation: self
                                .shot
                                .as_ref()
                                .expect("shot exists")
                                .shot
                                .cue_elevation(),
                            vertical_velocity: InchesPerSecond::zero(),
                        }
                    }
                })
            })
    }

    fn invalid_n_ball_geometry_error(&self, error: NBallGeometryError) -> DslBuildError {
        match error {
            error @ NBallGeometryError::OverlappingOnTableBalls {
                first_ball_index,
                second_ball_index,
                ..
            } => DslBuildError::InvalidNBallGeometry {
                first_ball: self.game_state.balls()[first_ball_index].ty.clone(),
                second_ball: self.game_state.balls()[second_ball_index].ty.clone(),
                error,
            },
            error @ (NBallGeometryError::UnsupportedNonIdealSharedBallBallContact { .. }
            | NBallGeometryError::ZeroTimeNoProgress
            | NBallGeometryError::ZeroTimeEventLimitExceeded { .. }) => {
                DslBuildError::UnsupportedNBallPhysics { error }
            }
        }
    }

    pub fn initial_shot_system_states_on_table(
        &self,
        ball_set: &BallSetPhysicsSpec,
    ) -> Result<Option<Vec<NBallSystemState>>, DslBuildError> {
        let Some(shot) = &self.shot else {
            return Ok(None);
        };
        let shot_target_index = self
            .game_state
            .balls()
            .iter()
            .position(|ball| ball.ty == shot.ball)
            .ok_or(DslBuildError::ShotTargetBallNotPlaced(shot.ball_ref))?;
        let mut states = self
            .game_state
            .balls()
            .iter()
            .map(|game_ball| {
                let resting = RestingOnTableBallState::try_from(BallState::from_position(
                    &game_ball.position,
                    &self.game_state.table_spec,
                ))
                .expect("game-state ball placements should always correspond to resting on-table states");
                NBallSystemState::from(resting.into_on_table_ball_state())
            })
            .collect::<Vec<_>>();
        states = crate::validate_and_recover_n_ball_system_states(&states, ball_set)
            .map_err(|error| self.invalid_n_ball_geometry_error(error))?;

        let NBallSystemState::OnTable(target) = &states[shot_target_index] else {
            unreachable!("validated initial layouts contain only on-table resting balls");
        };
        let resting = RestingOnTableBallState::try_from(target.as_ball_state().clone())
            .expect("validated initial layouts remain resting before the cue strike");
        let struck = crate::strike_resting_ball(&resting, &shot.shot, &shot.cue_strike, ball_set)
            .map_err(DslBuildError::InvalidShot)?;
        states[shot_target_index] = NBallSystemState::from(struck);

        Ok(Some(states))
    }

    pub fn simulate_shot_system_with_physics_on_table_until_rest(
        &self,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        collision_model: CollisionModel,
        collision_config: &BallBallCollisionConfig,
        rail_model: RailModel,
        rail_profile: &RailCollisionProfile,
    ) -> Result<Option<NBallSystemSimulation>, DslBuildError> {
        let Some(states) = self.initial_shot_system_states_on_table(ball_set)? else {
            return Ok(None);
        };

        let simulation = simulate_n_ball_system_with_physics_and_pockets_on_table_until_rest(
            &states,
            ball_set,
            &self.game_state.table_spec,
            motion,
            collision_model,
            collision_config,
            rail_model,
            rail_profile,
        )
        .map_err(|error| self.invalid_n_ball_geometry_error(error))?;
        Ok(Some(simulation))
    }

    pub fn simulate_shot_system_with_rails_and_pockets_on_table_until_rest(
        &self,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        collision_model: CollisionModel,
        rail_model: RailModel,
    ) -> Result<Option<NBallSystemSimulation>, DslBuildError> {
        self.simulate_shot_system_with_physics_on_table_until_rest(
            ball_set,
            motion,
            collision_model,
            &BallBallCollisionConfig::human_tuned(),
            rail_model,
            &RailCollisionProfile::default(),
        )
    }

    pub fn simulate_shot_trace_with_physics_on_table_until_rest(
        &self,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        collision_model: CollisionModel,
        collision_config: &BallBallCollisionConfig,
        rail_model: RailModel,
        rail_profile: &RailCollisionProfile,
    ) -> Result<Option<ScenarioShotTrace>, DslBuildError> {
        self.execute_shot_trace_with_physics_on_table(
            ball_set,
            motion,
            collision_model,
            collision_config,
            rail_model,
            rail_profile,
            ScenarioTraceStop::UntilRest,
        )
    }

    fn execute_shot_trace_with_physics_on_table(
        &self,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        collision_model: CollisionModel,
        collision_config: &BallBallCollisionConfig,
        rail_model: RailModel,
        rail_profile: &RailCollisionProfile,
        stop: ScenarioTraceStop,
    ) -> Result<Option<ScenarioShotTrace>, DslBuildError> {
        let Some(initial_states) = self.initial_shot_system_states_on_table(ball_set)? else {
            return Ok(None);
        };
        let simulation = match stop {
            ScenarioTraceStop::UntilRest => {
                simulate_n_ball_system_with_physics_and_pockets_on_table_until_rest(
                    &initial_states,
                    ball_set,
                    &self.game_state.table_spec,
                    motion,
                    collision_model,
                    collision_config,
                    rail_model,
                    rail_profile,
                )
            }
            ScenarioTraceStop::EventLimit(max_events) => {
                simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit(
                    &initial_states,
                    ball_set,
                    &self.game_state.table_spec,
                    motion,
                    collision_model,
                    collision_config,
                    rail_model,
                    rail_profile,
                    Some(max_events),
                )
            }
        }
        .map_err(|error| self.invalid_n_ball_geometry_error(error))?;
        let event_log = scenario_event_log_from_simulation(&simulation, self.game_state.balls());
        let ball_traces = self.ball_traces_from_simulation(
            &initial_states,
            &simulation,
            ball_set,
            motion,
            collision_model,
            collision_config,
            rail_model,
            rail_profile,
        )?;

        Ok(Some(ScenarioShotTrace {
            simulation,
            event_log,
            ball_traces,
            ball_set: ball_set.clone(),
            motion: motion.clone(),
        }))
    }

    pub fn simulate_shot_trace_with_physics_on_table_until_event_limit(
        &self,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        collision_model: CollisionModel,
        collision_config: &BallBallCollisionConfig,
        rail_model: RailModel,
        rail_profile: &RailCollisionProfile,
        max_events: usize,
    ) -> Result<Option<ScenarioShotTrace>, DslBuildError> {
        self.execute_shot_trace_with_physics_on_table(
            ball_set,
            motion,
            collision_model,
            collision_config,
            rail_model,
            rail_profile,
            ScenarioTraceStop::EventLimit(max_events),
        )
    }

    pub fn simulate_shot_trace_with_rails_and_pockets_on_table_until_rest(
        &self,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        collision_model: CollisionModel,
        rail_model: RailModel,
    ) -> Result<Option<ScenarioShotTrace>, DslBuildError> {
        self.execute_shot_trace_with_physics_on_table(
            ball_set,
            motion,
            collision_model,
            &BallBallCollisionConfig::human_tuned(),
            rail_model,
            &RailCollisionProfile::default(),
            ScenarioTraceStop::UntilRest,
        )
    }

    pub fn simulate_shot_trace_with_preferred_physics_on_table_until_rest(
        &self,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        collision_model: CollisionModel,
        rail_model: RailModel,
    ) -> Result<Option<ScenarioShotTrace>, DslBuildError> {
        let stop = ScenarioTraceStop::UntilRest;
        if let Some(simulation_name) = self.preferred_simulation_name() {
            let simulation = self.effective_simulation_physics(motion, simulation_name)?;
            self.execute_shot_trace_with_physics_on_table(
                ball_set,
                &simulation.motion,
                simulation.collision_model,
                &simulation.collision_config,
                simulation.rail_model,
                &simulation.rail_profile,
                stop.constrained_by(simulation.max_events),
            )
        } else {
            self.execute_shot_trace_with_physics_on_table(
                ball_set,
                motion,
                collision_model,
                &BallBallCollisionConfig::human_tuned(),
                rail_model,
                &RailCollisionProfile::default(),
                stop,
            )
        }
    }

    pub fn simulate_shot_trace_with_preferred_physics_on_table_until_event_limit(
        &self,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        collision_model: CollisionModel,
        rail_model: RailModel,
        max_events: usize,
    ) -> Result<Option<ScenarioShotTrace>, DslBuildError> {
        let stop = ScenarioTraceStop::EventLimit(max_events);
        if let Some(simulation_name) = self.preferred_simulation_name() {
            let simulation = self.effective_simulation_physics(motion, simulation_name)?;
            self.execute_shot_trace_with_physics_on_table(
                ball_set,
                &simulation.motion,
                simulation.collision_model,
                &simulation.collision_config,
                simulation.rail_model,
                &simulation.rail_profile,
                stop.constrained_by(simulation.max_events),
            )
        } else {
            self.execute_shot_trace_with_physics_on_table(
                ball_set,
                motion,
                collision_model,
                &BallBallCollisionConfig::human_tuned(),
                rail_model,
                &RailCollisionProfile::default(),
                stop,
            )
        }
    }

    pub fn game_state_for_system_states(&self, states: &[NBallSystemState]) -> GameState {
        assert_eq!(
            states.len(),
            self.game_state.balls().len(),
            "rendering system states requires one state per original ball"
        );

        let balls = self
            .game_state
            .balls()
            .iter()
            .zip(states)
            .filter_map(|(ball, state)| match state {
                NBallSystemState::OnTable(on_table) => Some(Ball {
                    ty: ball.ty.clone(),
                    position: on_table
                        .as_ball_state()
                        .projected_position(&self.game_state.table_spec),
                    spec: ball.spec.clone(),
                }),
                NBallSystemState::Airborne(airborne) => Some(Ball {
                    ty: ball.ty.clone(),
                    position: airborne.projected_position(&self.game_state.table_spec),
                    spec: ball.spec.clone(),
                }),
                NBallSystemState::Pocketed { .. } => None,
            })
            .collect::<Vec<_>>();
        let mut game_state = GameState::with_balls(self.game_state.table_spec.clone(), balls);
        game_state.ty = self.game_state.ty.clone();
        game_state.cueball_modifier = self.game_state.cueball_modifier.clone();
        game_state
    }

    fn ball_traces_from_simulation(
        &self,
        initial_states: &[NBallSystemState],
        simulation: &NBallSystemSimulation,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        collision_model: CollisionModel,
        collision_config: &BallBallCollisionConfig,
        rail_model: RailModel,
        rail_profile: &RailCollisionProfile,
    ) -> Result<Vec<ScenarioBallTrace>, DslBuildError> {
        let mut current_states = initial_states.to_vec();
        let mut elapsed = Seconds::zero();
        let mut traces = self
            .game_state
            .balls()
            .iter()
            .zip(initial_states)
            .map(|(ball, state)| ScenarioBallTrace {
                ball: ball.ty.clone(),
                initial_state: state.as_ball_state().clone(),
                final_state: state.clone(),
                segments: Vec::new(),
                timeline_segments: Vec::new(),
            })
            .collect::<Vec<_>>();

        for (event_index, event) in simulation.events.iter().enumerate() {
            let step_time = event.time();
            let event_time = Seconds::new(elapsed.as_f64() + step_time.as_f64());
            let event_human = format!(
                "t={}  {}",
                format_scenario_trace_time(event_time),
                scenario_event_kind_from_system_event(event, self.game_state.balls())
                    .format_human()
            );
            for (ball_index, (trace, state)) in traces.iter_mut().zip(&current_states).enumerate() {
                let start_state = state.as_ball_state().clone();
                let end_state = match state {
                    NBallSystemState::OnTable(start) => {
                        advance_motion_on_table(start, step_time, ball_set, motion).state
                    }
                    NBallSystemState::Airborne(airborne) => {
                        advance_airborne_ball(airborne, step_time)
                    }
                    NBallSystemState::Pocketed { .. } => continue,
                };
                trace.timeline_segments.push(ScenarioBallTimelineSegment {
                    start_time: elapsed,
                    start: start_state,
                    end: end_state.clone(),
                    duration: step_time,
                });

                let (NBallSystemState::OnTable(start), Ok(end)) =
                    (state, OnTableBallState::try_from(end_state))
                else {
                    continue;
                };
                let event_marker_label = scenario_event_involves_ball(event, ball_index)
                    .then(|| format!("({})", event_index + 1));
                let event_marker_title = event_marker_label
                    .as_ref()
                    .map(|label| format!("{label} {event_human}"));
                push_visible_trace_segment(
                    &mut trace.segments,
                    start,
                    &end,
                    step_time,
                    event_marker_label.is_some(),
                    event_marker_label,
                    event_marker_title,
                );
            }

            current_states = resolve_n_ball_system_event_with_physics_and_pockets_on_table(
                &current_states,
                event,
                ball_set,
                &self.game_state.table_spec,
                motion,
                collision_model,
                collision_config,
                rail_model,
                rail_profile,
            )
            .map_err(|error| self.invalid_n_ball_geometry_error(error))?;
            elapsed = Seconds::new(elapsed.as_f64() + step_time.as_f64());
        }

        for (trace, final_state) in traces.iter_mut().zip(&simulation.states) {
            trace.final_state = final_state.clone();
        }

        Ok(traces)
    }

    pub fn trace_shot_path_with_rail_profile_on_table(
        &self,
        stop: BallPathStop,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        rail_model: RailModel,
        rail_profile: &RailCollisionProfile,
    ) -> Result<Option<BallPath>, DslBuildError> {
        let Some(initial_state) = self.strike_shot_on_table(ball_set)? else {
            return Ok(None);
        };

        let path = trace_ball_path_with_rail_profile_on_table(
            &initial_state,
            stop,
            ball_set,
            &self.game_state.table_spec,
            motion,
            rail_model,
            rail_profile,
        )
        .map_err(DslBuildError::BallPath)?;
        Ok(Some(path))
    }

    pub fn trace_shot_path_with_rails_on_table(
        &self,
        stop: BallPathStop,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        rail_model: RailModel,
    ) -> Result<Option<BallPath>, DslBuildError> {
        self.trace_shot_path_with_rail_profile_on_table(
            stop,
            ball_set,
            motion,
            rail_model,
            &RailCollisionProfile::default(),
        )
    }

    pub fn trace_shot_path_until_rest_with_rails_on_table(
        &self,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        rail_model: RailModel,
    ) -> Result<Option<BallPath>, DslBuildError> {
        self.trace_shot_path_with_rails_on_table(
            BallPathStop::UntilRest,
            ball_set,
            motion,
            rail_model,
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioShot {
    pub ball_ref: BallRef,
    pub ball: BallType,
    pub shot: Shot,
    pub cue_strike: CueStrikeConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioShotTrace {
    pub simulation: NBallSystemSimulation,
    pub event_log: Vec<ScenarioShotTraceEvent>,
    pub ball_traces: Vec<ScenarioBallTrace>,
    pub ball_set: BallSetPhysicsSpec,
    pub motion: OnTableMotionConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioPlaybackFrame {
    pub time: Seconds,
    pub balls: Vec<ScenarioPlaybackBall>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioPlaybackBall {
    pub ball: BallType,
    pub state: BallState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioTraceRenderOptions {
    pub path_render: BallPathRenderOptions,
    pub start_ghost_balls: bool,
    pub event_markers: bool,
    pub labels: bool,
    pub spin_glyphs: bool,
    pub path_color_mode: PathColorMode,
}

impl Default for ScenarioTraceRenderOptions {
    fn default() -> Self {
        Self {
            path_render: BallPathRenderOptions::default()
                .with_width_mode(BallPathWidthMode::ScaleBySpeed),
            start_ghost_balls: false,
            event_markers: false,
            labels: false,
            spin_glyphs: false,
            path_color_mode: PathColorMode::Solid,
        }
    }
}

impl ScenarioTraceRenderOptions {
    pub fn rich_defaults() -> Self {
        Self {
            start_ghost_balls: true,
            event_markers: true,
            spin_glyphs: true,
            ..Self::default()
        }
    }
}

const SCENARIO_TRACE_TIME_DISPLAY_DECIMALS: usize = 6;
// The current event scheduler intentionally breaks ties deterministically instead of producing a
// composite simultaneous event, so break-style cluster contacts can arrive as back-to-back entries
// separated only by floating-point residue. Group those for human-facing trace lines.
const SCENARIO_TRACE_SIMULTANEOUS_EVENT_EPSILON_SECONDS: f64 = 1e-9;
const SCENARIO_PLAYBACK_TIME_EPSILON_SECONDS: f64 = 1e-9;

impl ScenarioShotTrace {
    pub fn event_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        let mut index = 0;

        while index < self.event_log.len() {
            let group_time = self.event_log[index].time;
            let mut group_kinds = vec![self.event_log[index].kind.format_human()];
            index += 1;

            while index < self.event_log.len()
                && scenario_trace_times_are_effectively_simultaneous(
                    group_time,
                    self.event_log[index].time,
                )
            {
                group_kinds.push(self.event_log[index].kind.format_human());
                index += 1;
            }

            lines.push(format!(
                "t={}  {}",
                format_scenario_trace_time(group_time),
                group_kinds.join(" | ")
            ));
        }

        lines
    }

    pub fn playback_frames(&self, max_time_step: Seconds) -> Vec<ScenarioPlaybackFrame> {
        let max_time_step = max_time_step.as_f64();
        assert!(
            max_time_step.is_finite() && max_time_step > 0.0,
            "playback max_time_step must be positive and finite"
        );

        let mut times = vec![0.0, self.simulation.elapsed.as_f64()];
        times.extend(self.event_log.iter().map(|event| event.time.as_f64()));
        for ball_trace in &self.ball_traces {
            for segment in &ball_trace.timeline_segments {
                let start = segment.start_time.as_f64();
                let duration = segment.duration.as_f64();
                times.push(start);
                for sample in TimelineSubdivision::new(duration, Some(max_time_step)) {
                    times.push(start + sample.elapsed.as_f64());
                }
            }
        }

        times.retain(|time| time.is_finite() && *time >= 0.0);
        times.sort_by(f64::total_cmp);
        times.dedup_by(|a, b| (*a - *b).abs() <= SCENARIO_PLAYBACK_TIME_EPSILON_SECONDS);

        times
            .into_iter()
            .map(|time| ScenarioPlaybackFrame {
                time: Seconds::new(time),
                balls: self
                    .ball_traces
                    .iter()
                    .filter_map(|ball_trace| {
                        ball_trace
                            .state_at_elapsed(Seconds::new(time), &self.ball_set, &self.motion)
                            .map(|state| ScenarioPlaybackBall {
                                ball: ball_trace.ball.clone(),
                                state,
                            })
                    })
                    .collect(),
            })
            .collect()
    }

    pub fn rendered_final_layout_with_traces(
        &self,
        scenario: &DslScenario,
        max_time_step: Seconds,
    ) -> GameState {
        self.rendered_final_layout_with_trace_options(
            scenario,
            &ScenarioTraceRenderOptions {
                path_render: BallPathRenderOptions {
                    max_time_step,
                    ..ScenarioTraceRenderOptions::default().path_render
                },
                ..ScenarioTraceRenderOptions::default()
            },
        )
    }

    pub fn rendered_final_layout_with_trace_options(
        &self,
        scenario: &DslScenario,
        options: &ScenarioTraceRenderOptions,
    ) -> GameState {
        let mut game_state = scenario.game_state_for_system_states(&self.simulation.states);
        if options.spin_glyphs {
            for (ball, state) in scenario
                .game_state
                .balls()
                .iter()
                .zip(self.simulation.states.iter())
            {
                if let NBallSystemState::OnTable(on_table) = state {
                    game_state.add_spin_glyph_for_on_table_state(on_table, &ball.spec);
                }
            }
        }

        for ball_trace in &self.ball_traces {
            let trace_color = ball_trace_color(&ball_trace.ball);
            let mut path_style = crate::visualization::BallPathStyle::new(trace_color)
                .with_color_mode(options.path_color_mode);
            if options.start_ghost_balls {
                path_style = path_style.with_start_ghost(ball_trace_ghost_style(trace_color));
            }
            if options.event_markers {
                path_style = path_style.with_event_markers(EventMarkerStyle::enabled(trace_color));
            }
            if options.labels {
                path_style =
                    path_style.with_labels(LabelOverlayStyle::enabled(Rgba([0, 0, 0, 255])));
            }

            let path_render = options.path_render.clone();

            if let Some(path) = ball_trace.as_ball_path() {
                game_state.add_rendered_ball_path_styled(
                    &path,
                    &self.ball_set,
                    &self.motion,
                    &path_render,
                    &path_style,
                );
            } else {
                let sampled_points = ball_trace.sampled_points(
                    path_render.max_time_step,
                    &self.ball_set,
                    &self.motion,
                    &scenario.game_state.table_spec,
                );
                if sampled_points.len() >= 2 {
                    game_state.add_smooth_polyline_styled(
                        &sampled_points,
                        SmoothPolylineStyle {
                            color: trace_color,
                            width_px: path_render.width_px_for_speed(
                                ball_trace.reference_speed_ips(),
                                ball_trace.reference_speed_ips(),
                            ),
                            layer: path_style.line.layer,
                        },
                    );
                }
            }

            if options.start_ghost_balls && ball_trace.ball == BallType::Cue {
                let start = ball_trace
                    .initial_state
                    .projected_position(&scenario.game_state.table_spec);
                game_state.add_origin_marker_styled(&start, cue_origin_marker_style());
            }

            if let Some(pocket_terminal) = ball_trace.pocket_terminal_point() {
                let (capture_point, capture_width_px) = match &ball_trace.final_state {
                    NBallSystemState::Pocketed {
                        state_at_capture, ..
                    } => {
                        let capture_point = state_at_capture
                            .as_ball_state()
                            .projected_position(&scenario.game_state.table_spec);
                        let capture_width_px = path_render.width_px_for_speed(
                            ball_trace.reference_speed_ips(),
                            state_at_capture.as_ball_state().speed().as_f64(),
                        );
                        (capture_point, capture_width_px)
                    }
                    NBallSystemState::OnTable(_) | NBallSystemState::Airborne(_) => continue,
                };
                if capture_point != pocket_terminal {
                    game_state.add_smooth_polyline_with_width(
                        &[capture_point, pocket_terminal],
                        capture_width_px,
                        trace_color,
                    );
                }
            }
        }
        game_state
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioShotTraceEvent {
    pub time: Seconds,
    pub kind: ScenarioShotTraceEventKind,
}

impl ScenarioShotTraceEvent {
    pub fn format_human(&self) -> String {
        format!(
            "t={}  {}",
            format_scenario_trace_time(self.time),
            self.kind.format_human()
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScenarioShotTraceEventKind {
    BallBallCollision {
        first_ball: BallType,
        second_ball: BallType,
    },
    AirborneBallBallCollision {
        first_ball: BallType,
        second_ball: BallType,
    },
    SharedBallBallContact {
        balls: Vec<BallType>,
        ball_ball_pairs: Vec<(BallType, BallType)>,
        resolution: SharedBallBallContactResolution,
    },
    BallPocketCapture {
        ball: BallType,
        pocket: Pocket,
    },
    BallRailImpact {
        ball: BallType,
        rail: Rail,
    },
    BallJawImpact {
        ball: BallType,
        pocket: Pocket,
        jaw: PocketJaw,
    },
    BallTableBounce {
        ball: BallType,
    },
    MotionTransition {
        ball: BallType,
        phase_before: MotionPhase,
        phase_after: MotionPhase,
    },
}

impl ScenarioShotTraceEventKind {
    pub fn format_human(&self) -> String {
        match self {
            ScenarioShotTraceEventKind::BallBallCollision {
                first_ball,
                second_ball,
            } => format!(
                "{} -> {} collision",
                ball_type_name(first_ball),
                ball_type_name(second_ball)
            ),
            ScenarioShotTraceEventKind::AirborneBallBallCollision {
                first_ball,
                second_ball,
            } => format!(
                "{} -> {} airborne collision",
                ball_type_name(first_ball),
                ball_type_name(second_ball)
            ),
            ScenarioShotTraceEventKind::SharedBallBallContact {
                balls,
                ball_ball_pairs,
                resolution,
            } => format!(
                "shared contact among [{}] via {}: {}",
                balls
                    .iter()
                    .map(ball_type_name)
                    .collect::<Vec<_>>()
                    .join(", "),
                resolution.as_str(),
                ball_ball_pairs
                    .iter()
                    .map(|(first, second)| format!(
                        "{}-{}",
                        ball_type_name(first),
                        ball_type_name(second)
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ScenarioShotTraceEventKind::BallPocketCapture { ball, pocket } => format!(
                "{} pocketed in {}",
                ball_type_name(ball),
                pocket_name(*pocket)
            ),
            ScenarioShotTraceEventKind::BallRailImpact { ball, rail } => {
                format!("{} rail impact: {}", ball_type_name(ball), rail_name(*rail))
            }
            ScenarioShotTraceEventKind::BallJawImpact { ball, pocket, jaw } => format!(
                "{} jaw impact: {} {}",
                ball_type_name(ball),
                pocket_name(*pocket),
                jaw_name(*jaw)
            ),
            ScenarioShotTraceEventKind::BallTableBounce { ball } => {
                format!("{} table bounce", ball_type_name(ball))
            }
            ScenarioShotTraceEventKind::MotionTransition {
                ball,
                phase_before,
                phase_after,
            } => format!(
                "{} {:?} -> {:?}",
                ball_type_name(ball),
                phase_before,
                phase_after
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioBallTrace {
    pub ball: BallType,
    pub initial_state: BallState,
    pub final_state: NBallSystemState,
    pub segments: Vec<BallPathSegment>,
    pub timeline_segments: Vec<ScenarioBallTimelineSegment>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioBallTimelineSegment {
    pub start_time: Seconds,
    pub start: BallState,
    pub end: BallState,
    pub duration: Seconds,
}

struct TimelineSubdivisionSample {
    elapsed: Seconds,
    is_endpoint: bool,
}

struct TimelineSubdivision {
    duration: f64,
    sample_count: usize,
    next_sample: usize,
}

impl TimelineSubdivision {
    fn new(duration: f64, max_time_step: Option<f64>) -> Self {
        let sample_count = max_time_step.map_or(1, |max_time_step| {
            ((duration / max_time_step).ceil() as usize).max(1)
        });
        Self {
            duration,
            sample_count,
            next_sample: 1,
        }
    }
}

impl Iterator for TimelineSubdivision {
    type Item = TimelineSubdivisionSample;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_sample == 0 {
            return None;
        }

        let sample_index = self.next_sample;
        let is_endpoint = sample_index == self.sample_count;
        self.next_sample = if is_endpoint { 0 } else { sample_index + 1 };
        Some(TimelineSubdivisionSample {
            elapsed: Seconds::new(self.duration * sample_index as f64 / self.sample_count as f64),
            is_endpoint,
        })
    }
}

impl ScenarioBallTrace {
    fn pocket_terminal_point(&self) -> Option<Position> {
        match &self.final_state {
            NBallSystemState::Pocketed { pocket, .. } => Some(pocket.aiming_center()),
            NBallSystemState::OnTable(_) | NBallSystemState::Airborne(_) => None,
        }
    }

    pub fn state_at_elapsed(
        &self,
        elapsed: Seconds,
        ball: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
    ) -> Option<BallState> {
        let target_time = elapsed.as_f64().max(0.0);
        for (segment_index, segment) in self.timeline_segments.iter().enumerate() {
            let start_time = segment.start_time.as_f64();
            let duration = segment.duration.as_f64();
            let end_time = start_time + duration;
            if target_time + SCENARIO_PLAYBACK_TIME_EPSILON_SECONDS < start_time {
                return Some(segment.start.clone());
            }
            if target_time < end_time {
                let segment_elapsed = (target_time - start_time).clamp(0.0, duration);
                if segment_elapsed <= SCENARIO_PLAYBACK_TIME_EPSILON_SECONDS {
                    return Some(segment.start.clone());
                }
                return Some(advance_timeline_ball_state(
                    &segment.start,
                    Seconds::new(segment_elapsed),
                    ball,
                    motion,
                ));
            }
            if target_time <= end_time + SCENARIO_PLAYBACK_TIME_EPSILON_SECONDS {
                if self
                    .timeline_segments
                    .get(segment_index + 1)
                    .is_some_and(|next| {
                        next.start_time.as_f64()
                            <= end_time + SCENARIO_PLAYBACK_TIME_EPSILON_SECONDS
                    })
                {
                    continue;
                }
                if segment_index + 1 == self.timeline_segments.len() {
                    return self.resolved_final_playback_state();
                }
                return Some(segment.end.clone());
            }
        }

        self.resolved_final_playback_state()
    }

    fn resolved_final_playback_state(&self) -> Option<BallState> {
        match &self.final_state {
            NBallSystemState::OnTable(state) => Some(state.as_ball_state().clone()),
            NBallSystemState::Airborne(state) => Some(state.clone()),
            NBallSystemState::Pocketed { .. } => None,
        }
    }

    fn reference_speed_ips(&self) -> f64 {
        let mut reference_speed_ips = self.initial_state.speed().as_f64();
        for segment in &self.timeline_segments {
            reference_speed_ips = reference_speed_ips.max(segment.start.speed().as_f64());
            reference_speed_ips = reference_speed_ips.max(segment.end.speed().as_f64());
        }
        if let NBallSystemState::Pocketed {
            state_at_capture, ..
        } = &self.final_state
        {
            reference_speed_ips =
                reference_speed_ips.max(state_at_capture.as_ball_state().speed().as_f64());
        }
        reference_speed_ips
    }

    fn as_ball_path(&self) -> Option<BallPath> {
        if self.timeline_segments.iter().any(|segment| {
            OnTableBallState::try_from(segment.start.clone()).is_err()
                || OnTableBallState::try_from(segment.end.clone()).is_err()
        }) {
            return None;
        }

        let final_state = match &self.final_state {
            NBallSystemState::OnTable(state) => state.clone(),
            NBallSystemState::Airborne(_) => return None,
            NBallSystemState::Pocketed {
                state_at_capture, ..
            } => state_at_capture.clone(),
        };

        let initial_state = self.segments.first().map(|segment| segment.start.clone())?;
        let elapsed = Seconds::new(
            self.segments
                .iter()
                .map(|segment| segment.duration.as_f64())
                .sum(),
        );

        Some(BallPath {
            initial_state,
            final_state,
            elapsed,
            rail_impacts: 0,
            segments: self.segments.clone(),
        })
    }

    pub fn projected_points(&self, table_spec: &TableSpec) -> Vec<Position> {
        let mut points = self.as_ball_path().map_or_else(
            || {
                let mut points = vec![self.initial_state.projected_position(table_spec)];
                for segment in &self.timeline_segments {
                    let projected_end = segment.end.projected_position(table_spec);
                    if points.last() != Some(&projected_end) {
                        points.push(projected_end);
                    }
                }
                points
            },
            |path| path.projected_points(table_spec),
        );
        if let Some(pocket_terminal) = self.pocket_terminal_point() {
            if points.last() != Some(&pocket_terminal) {
                points.push(pocket_terminal);
            }
        }
        points
    }

    pub fn sampled_points(
        &self,
        max_time_step: Seconds,
        ball: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        table_spec: &TableSpec,
    ) -> Vec<Position> {
        let mut points = self.as_ball_path().map_or_else(
            || self.sampled_timeline_points(max_time_step, ball, motion, table_spec),
            |path| path.sampled_points(max_time_step, ball, motion, table_spec),
        );
        if let Some(pocket_terminal) = self.pocket_terminal_point() {
            if points.last() != Some(&pocket_terminal) {
                points.push(pocket_terminal);
            }
        }
        points
    }

    fn sampled_timeline_points(
        &self,
        max_time_step: Seconds,
        ball: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        table_spec: &TableSpec,
    ) -> Vec<Position> {
        let mut points = vec![self.initial_state.projected_position(table_spec)];
        let max_time_step = max_time_step.as_f64();
        let subdivision_step =
            (max_time_step.is_finite() && max_time_step > 0.0).then_some(max_time_step);
        for segment in &self.timeline_segments {
            for sample in
                TimelineSubdivision::new(segment.duration.as_f64().max(0.0), subdivision_step)
            {
                let state = if sample.is_endpoint {
                    segment.end.clone()
                } else {
                    advance_timeline_ball_state(&segment.start, sample.elapsed, ball, motion)
                };
                let projected = state.projected_position(table_spec);
                if points.last() != Some(&projected) {
                    points.push(projected);
                }
            }
        }
        points
    }
}

fn advance_timeline_ball_state(
    state: &BallState,
    elapsed: Seconds,
    ball: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
) -> BallState {
    match OnTableBallState::try_new_with_thresholds(
        state.clone(),
        &MotionPhaseThresholds::default(),
    ) {
        Ok(on_table) => advance_motion_on_table(&on_table, elapsed, ball, motion).state,
        Err(OnTableStateError::HeightAboveTablePlane { .. })
        | Err(OnTableStateError::VerticalVelocityPresent { .. }) => {
            advance_airborne_ball(state, elapsed)
        }
    }
}

fn trace_segment_has_visible_displacement(
    start: &OnTableBallState,
    end: &OnTableBallState,
) -> bool {
    let dx =
        end.as_ball_state().position.x().as_f64() - start.as_ball_state().position.x().as_f64();
    let dy =
        end.as_ball_state().position.y().as_f64() - start.as_ball_state().position.y().as_f64();

    dx.abs() > 1e-12 || dy.abs() > 1e-12
}

fn push_visible_trace_segment(
    segments: &mut Vec<BallPathSegment>,
    start: &OnTableBallState,
    end: &OnTableBallState,
    duration: Seconds,
    event_marker_at_end: bool,
    event_marker_label: Option<String>,
    event_marker_title: Option<String>,
) {
    if trace_segment_has_visible_displacement(start, end) {
        segments.push(BallPathSegment {
            start: start.clone(),
            end: end.clone(),
            duration,
            event_marker_at_end,
            event_marker_label,
            event_marker_title,
        });
    }
}

fn format_scenario_trace_time(time: Seconds) -> String {
    format!("{:.*}", SCENARIO_TRACE_TIME_DISPLAY_DECIMALS, time.as_f64())
}

fn scenario_trace_times_are_effectively_simultaneous(a: Seconds, b: Seconds) -> bool {
    (a.as_f64() - b.as_f64()).abs() <= SCENARIO_TRACE_SIMULTANEOUS_EVENT_EPSILON_SECONDS
}

fn scenario_event_log_from_simulation(
    simulation: &NBallSystemSimulation,
    balls: &[Ball],
) -> Vec<ScenarioShotTraceEvent> {
    let mut elapsed = Seconds::zero();

    simulation
        .events
        .iter()
        .map(|event| {
            elapsed = Seconds::new(elapsed.as_f64() + event.time().as_f64());
            ScenarioShotTraceEvent {
                time: elapsed,
                kind: scenario_event_kind_from_system_event(event, balls),
            }
        })
        .collect()
}

fn scenario_event_involves_ball(event: &NBallSystemEvent, ball_index: usize) -> bool {
    match event {
        NBallSystemEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            ..
        }
        | NBallSystemEvent::AirborneBallBallCollision {
            first_ball_index,
            second_ball_index,
            ..
        } => *first_ball_index == ball_index || *second_ball_index == ball_index,
        NBallSystemEvent::SharedBallBallContact { ball_indices, .. } => {
            ball_indices.contains(&ball_index)
        }
        NBallSystemEvent::BallPocketCapture {
            ball_index: event_ball,
            ..
        }
        | NBallSystemEvent::BallRailImpact {
            ball_index: event_ball,
            ..
        }
        | NBallSystemEvent::BallJawImpact {
            ball_index: event_ball,
            ..
        }
        | NBallSystemEvent::BallTableBounce {
            ball_index: event_ball,
            ..
        }
        | NBallSystemEvent::MotionTransition {
            ball_index: event_ball,
            ..
        } => *event_ball == ball_index,
    }
}

fn scenario_event_kind_from_system_event(
    event: &NBallSystemEvent,
    balls: &[Ball],
) -> ScenarioShotTraceEventKind {
    match event {
        NBallSystemEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            ..
        } => ScenarioShotTraceEventKind::BallBallCollision {
            first_ball: balls[*first_ball_index].ty.clone(),
            second_ball: balls[*second_ball_index].ty.clone(),
        },
        NBallSystemEvent::AirborneBallBallCollision {
            first_ball_index,
            second_ball_index,
            ..
        } => ScenarioShotTraceEventKind::AirborneBallBallCollision {
            first_ball: balls[*first_ball_index].ty.clone(),
            second_ball: balls[*second_ball_index].ty.clone(),
        },
        NBallSystemEvent::SharedBallBallContact {
            ball_indices,
            ball_ball_pairs,
            resolution,
            ..
        } => ScenarioShotTraceEventKind::SharedBallBallContact {
            balls: ball_indices
                .iter()
                .map(|ball_index| balls[*ball_index].ty.clone())
                .collect(),
            ball_ball_pairs: ball_ball_pairs
                .iter()
                .map(|(first, second)| (balls[*first].ty.clone(), balls[*second].ty.clone()))
                .collect(),
            resolution: *resolution,
        },
        NBallSystemEvent::BallPocketCapture {
            ball_index,
            capture,
        } => ScenarioShotTraceEventKind::BallPocketCapture {
            ball: balls[*ball_index].ty.clone(),
            pocket: capture.pocket,
        },
        NBallSystemEvent::BallRailImpact { ball_index, impact } => {
            ScenarioShotTraceEventKind::BallRailImpact {
                ball: balls[*ball_index].ty.clone(),
                rail: impact.rail,
            }
        }
        NBallSystemEvent::BallJawImpact { ball_index, impact } => {
            ScenarioShotTraceEventKind::BallJawImpact {
                ball: balls[*ball_index].ty.clone(),
                pocket: impact.pocket,
                jaw: impact.jaw,
            }
        }
        NBallSystemEvent::BallTableBounce { ball_index, .. } => {
            ScenarioShotTraceEventKind::BallTableBounce {
                ball: balls[*ball_index].ty.clone(),
            }
        }
        NBallSystemEvent::MotionTransition {
            ball_index,
            transition,
        } => ScenarioShotTraceEventKind::MotionTransition {
            ball: balls[*ball_index].ty.clone(),
            phase_before: transition.phase_before.clone(),
            phase_after: transition.phase_after.clone(),
        },
    }
}

fn ball_type_name(ball: &BallType) -> &'static str {
    match ball {
        BallType::Cue => "cue",
        BallType::One => "one",
        BallType::Two => "two",
        BallType::Three => "three",
        BallType::Four => "four",
        BallType::Five => "five",
        BallType::Six => "six",
        BallType::Seven => "seven",
        BallType::Eight => "eight",
        BallType::Nine => "nine",
        BallType::YellowCue => "yellow",
        BallType::Red => "red",
    }
}

fn pocket_name(pocket: Pocket) -> &'static str {
    match pocket {
        Pocket::TopRight => "top-right",
        Pocket::CenterRight => "center-right",
        Pocket::BottomRight => "bottom-right",
        Pocket::BottomLeft => "bottom-left",
        Pocket::CenterLeft => "center-left",
        Pocket::TopLeft => "top-left",
    }
}

fn rail_name(rail: Rail) -> &'static str {
    match rail {
        Rail::Top => "top",
        Rail::Right => "right",
        Rail::Bottom => "bottom",
        Rail::Left => "left",
    }
}

fn jaw_name(jaw: PocketJaw) -> &'static str {
    match jaw {
        PocketJaw::First => "jaw-1",
        PocketJaw::Second => "jaw-2",
    }
}

fn ball_trace_color(ball: &BallType) -> Rgba<u8> {
    match ball {
        BallType::Cue => Rgba([225, 225, 225, 255]),
        BallType::One | BallType::Nine => Rgba([255, 215, 0, 255]),
        BallType::Two => Rgba([65, 105, 225, 255]),
        BallType::Three => Rgba([220, 20, 60, 255]),
        BallType::Four => Rgba([138, 43, 226, 255]),
        BallType::Five => Rgba([255, 140, 0, 255]),
        BallType::Six => Rgba([34, 139, 34, 255]),
        BallType::Seven => Rgba([128, 0, 0, 255]),
        BallType::Eight => Rgba([32, 32, 32, 255]),
        BallType::YellowCue => Rgba([246, 213, 79, 255]),
        BallType::Red => Rgba([190, 24, 24, 255]),
    }
}

fn ball_trace_ghost_style(color: Rgba<u8>) -> GhostBallStyle {
    GhostBallStyle {
        fill_color: Rgba([color[0], color[1], color[2], 64]),
        outline_color: Rgba([color[0], color[1], color[2], 160]),
        ..GhostBallStyle::default()
    }
}

fn cue_origin_marker_style() -> LabelOverlayStyle {
    LabelOverlayStyle {
        enabled: true,
        color: Rgba([12, 20, 24, 176]),
        offset_x_px: 0,
        offset_y_px: 0,
        scale_px: 2,
        ..LabelOverlayStyle::default()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AliasDef {
    pub name: String,
    pub position: PositionExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BallPlacement {
    At {
        ball: BallRef,
        position: PositionExpr,
    },
    Frozen {
        ball: BallRef,
        rail: RailSide,
        coord: f64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CueStrikeDef {
    pub name: String,
    pub methods: Vec<CueStrikeMethodExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CueStrikeMethodExpr {
    MassRatio(f64),
    EnergyLoss(f64),
    EndmassRatio(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct BallBallDef {
    pub name: String,
    pub methods: Vec<BallBallMethodExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BallBallMethodExpr {
    NormalRestitution(f64),
    TangentialFriction(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RailResponseDef {
    pub name: String,
    pub methods: Vec<RailResponseMethodExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RailResponseMethodExpr {
    NormalRestitution(f64),
    TangentialFriction(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RailsDef {
    pub name: String,
    pub methods: Vec<RailsMethodExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RailsMethodExpr {
    Default(String),
    Top(String),
    Right(String),
    Bottom(String),
    Left(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimulationDef {
    pub name: String,
    pub methods: Vec<SimulationMethodExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SimulationMethodExpr {
    CollisionModel(CollisionModel),
    BallBall(String),
    RailModel(RailModel),
    Rails(String),
    Conditions(String),
    MaxEvents(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimulationPreset {
    pub collision_model: CollisionModel,
    pub ball_ball_name: String,
    pub rail_model: RailModel,
    pub rails_name: String,
    pub conditions: PlayingConditions,
    pub max_events: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShotDef {
    pub ball: BallRef,
    pub methods: Vec<ShotMethodExpr>,
}

#[derive(Debug, Clone, Copy, Default)]
struct ShotSourceMetadata {
    aim: Option<ShotAimSource>,
    speed_literal: Option<ByteSpan>,
    tip: Option<ShotTipSource>,
    elevation: Option<ShotElevationSource>,
}

#[derive(Debug)]
struct ParsedShotDef {
    def: ShotDef,
    source: ShotSourceMetadata,
}

#[derive(Debug)]
struct ParsedDslDoc {
    doc: DslDoc,
    shot_source: Option<ShotSourceMetadata>,
}

#[derive(Debug, Clone, Copy)]
struct ShotAimSource {
    method: ByteSpan,
    heading_literal: Option<ByteSpan>,
}

#[derive(Debug, Clone, Copy)]
struct ShotTipSource {
    method: ByteSpan,
    side_literal: ByteSpan,
    height_literal: ByteSpan,
}

#[derive(Debug, Clone, Copy)]
struct ShotElevationSource {
    method: ByteSpan,
    literal: Option<ByteSpan>,
    is_jump: bool,
}

#[derive(Debug, Clone, Copy)]
struct ByteSpan {
    start: usize,
    end: usize,
}

impl ByteSpan {
    fn between(start: usize, input: &Stream<'_>) -> Self {
        Self {
            start,
            end: input.current_token_start(),
        }
    }
}

#[derive(Debug)]
struct ParsedShotMethod {
    expr: ShotMethodExpr,
    source: ParsedShotMethodSource,
}

#[derive(Debug)]
enum ParsedShotMethodSource {
    Aim(ShotAimSource),
    Speed { literal: ByteSpan },
    Tip(ShotTipSource),
    Elevation(ShotElevationSource),
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotCutDirection {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShotMethodExpr {
    HeadingDegrees(f64),
    ToPocket {
        object_ball: BallRef,
        pocket: Pocket,
    },
    Cut {
        object_ball: BallRef,
        direction: ShotCutDirection,
        degrees: f64,
    },
    ElevationDegrees(f64),
    SpeedIps(f64),
    Tip {
        side: f64,
        height: f64,
    },
    Using(String),
}

#[derive(Debug, Clone, PartialEq)]
enum BallPlacementKind {
    At(PositionExpr),
    Frozen { rail: RailSide, coord: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableRef {
    BrunswickGc4_9ft,
    ThreeCushionCarom10ft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameRef {
    NineBall,
    EightBall,
    TenBall,
    OnePocket,
    Banks,
    ThreeCushion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BallRef {
    Cue,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Yellow,
    Red,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PositionExpr {
    Diamond { x: f64, y: f64 },
    Named(NamedPosition),
    Alias(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedPosition {
    Center,
    Rack,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    CenterLeft,
    CenterRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailSide {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateAxis {
    X,
    Y,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DslParseError {
    pub message: String,
    pub offset: usize,
}

impl std::fmt::Display for DslParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at byte {}", self.message, self.offset)
    }
}

impl std::error::Error for DslParseError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicsConfigKind {
    BallBall,
    RailResponse,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DslBuildError {
    UnknownAlias(String),
    DuplicateAlias(String),
    DuplicateBallPlacement(BallRef),
    CoordinateOutOfRange {
        axis: CoordinateAxis,
        value: f64,
        min: f64,
        max: f64,
    },
    FrozenCoordinateOutOfRange {
        rail: RailSide,
        value: f64,
        min: f64,
        max: f64,
    },
    DuplicateCueStrike(String),
    DuplicateCueStrikeMethod {
        name: String,
        method: String,
    },
    MissingCueStrikeMethod {
        name: String,
        method: String,
    },
    DuplicateBallBall(String),
    DuplicateBallBallMethod {
        name: String,
        method: String,
    },
    MissingBallBallMethod {
        name: String,
        method: String,
    },
    DuplicateRailResponse(String),
    DuplicateRailResponseMethod {
        name: String,
        method: String,
    },
    MissingRailResponseMethod {
        name: String,
        method: String,
    },
    DuplicateRails(String),
    DuplicateRailsMethod {
        name: String,
        method: String,
    },
    MissingRailsMethod {
        name: String,
        method: String,
    },
    DuplicateSimulation(String),
    DuplicateSimulationMethod {
        name: String,
        method: String,
    },
    MissingSimulationMethod {
        name: String,
        method: String,
    },
    InvalidPhysicsConfigValue {
        kind: PhysicsConfigKind,
        name: String,
        method: String,
        value: f64,
        expected: String,
    },
    DuplicateShotMethod {
        method: String,
    },
    ConflictingShotAimMethods {
        first: String,
        second: String,
    },
    MissingShotAimMethod,
    MissingShotMethod {
        method: String,
    },
    UnknownCueStrike(String),
    UnknownBallBallConfig(String),
    UnknownRailResponse(String),
    UnknownRailProfile(String),
    UnknownSimulation(String),
    UnknownPlayingConditionsPreset(String),
    MultipleShotsNotSupported {
        count: usize,
    },
    ShotTargetMustBeCueBall(BallRef),
    ShotTargetBallNotPlaced(BallRef),
    ShotAimingBallMustNotBeCueBall(BallRef),
    ShotAimingBallNotPlaced(BallRef),
    CutAngleOutOfRange {
        degrees: f64,
    },
    CueElevationOutOfRange {
        degrees: f64,
    },
    InvalidCueStrikeConfig {
        name: String,
        error: ShotError,
    },
    InvalidShot(ShotError),
    InvalidNBallGeometry {
        first_ball: BallType,
        second_ball: BallType,
        error: NBallGeometryError,
    },
    UnsupportedNBallPhysics {
        error: NBallGeometryError,
    },
    BallPath(BallPathError),
}

impl std::fmt::Display for DslBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownAlias(name) => write!(f, "unknown alias '{name}'"),
            Self::DuplicateAlias(name) => {
                write!(f, "alias '{name}' was defined more than once")
            }
            Self::DuplicateBallPlacement(ball) => {
                write!(f, "ball '{ball}' was placed more than once")
            }
            Self::CoordinateOutOfRange {
                axis,
                value,
                min,
                max,
            } => write!(
                f,
                "{axis}-coordinate {value} is out of bounds; expected {min}..={max}"
            ),
            Self::FrozenCoordinateOutOfRange {
                rail,
                value,
                min,
                max,
            } => write!(
                f,
                "frozen {rail} coordinate {value} is out of bounds; expected {min}..={max}"
            ),
            Self::DuplicateCueStrike(name) => {
                write!(f, "cue_strike '{name}' was defined more than once")
            }
            Self::DuplicateCueStrikeMethod { name, method } => write!(
                f,
                "cue_strike '{name}' specified .{method}(...) more than once"
            ),
            Self::MissingCueStrikeMethod { name, method } => {
                write!(f, "cue_strike '{name}' is missing .{method}(...)")
            }
            Self::DuplicateBallBall(name) => {
                write!(f, "ball_ball '{name}' was defined more than once")
            }
            Self::DuplicateBallBallMethod { name, method } => write!(
                f,
                "ball_ball '{name}' specified .{method}(...) more than once"
            ),
            Self::MissingBallBallMethod { name, method } => {
                write!(f, "ball_ball '{name}' is missing .{method}(...)")
            }
            Self::DuplicateRailResponse(name) => {
                write!(f, "rail_response '{name}' was defined more than once")
            }
            Self::DuplicateRailResponseMethod { name, method } => write!(
                f,
                "rail_response '{name}' specified .{method}(...) more than once"
            ),
            Self::MissingRailResponseMethod { name, method } => {
                write!(f, "rail_response '{name}' is missing .{method}(...)")
            }
            Self::DuplicateRails(name) => write!(f, "rails '{name}' was defined more than once"),
            Self::DuplicateRailsMethod { name, method } => {
                write!(f, "rails '{name}' specified .{method}(...) more than once")
            }
            Self::MissingRailsMethod { name, method } => {
                write!(f, "rails '{name}' is missing .{method}(...)")
            }
            Self::DuplicateSimulation(name) => {
                write!(f, "simulation '{name}' was defined more than once")
            }
            Self::DuplicateSimulationMethod { name, method } => {
                write!(
                    f,
                    "simulation '{name}' specified .{method}(...) more than once"
                )
            }
            Self::MissingSimulationMethod { name, method } => {
                write!(f, "simulation '{name}' is missing .{method}(...)")
            }
            Self::InvalidPhysicsConfigValue {
                kind,
                name,
                method,
                value,
                expected,
            } => write!(
                f,
                "{kind:?} '{name}' has invalid .{method}({value}); expected {expected}"
            ),
            Self::DuplicateShotMethod { method } => {
                write!(f, "shot specified .{method}(...) more than once")
            }
            Self::ConflictingShotAimMethods { first, second } => write!(
                f,
                "shot specified conflicting aim helpers .{first}(...) and .{second}(...)"
            ),
            Self::MissingShotAimMethod => {
                write!(
                    f,
                    "shot is missing one aim helper: .heading(...), .to_pocket(...), or .cut(...)"
                )
            }
            Self::MissingShotMethod { method } => {
                write!(f, "shot is missing .{method}(...)")
            }
            Self::UnknownCueStrike(name) => write!(f, "unknown cue_strike '{name}'"),
            Self::UnknownBallBallConfig(name) => write!(f, "unknown ball_ball '{name}'"),
            Self::UnknownRailResponse(name) => write!(f, "unknown rail_response '{name}'"),
            Self::UnknownRailProfile(name) => write!(f, "unknown rails '{name}'"),
            Self::UnknownSimulation(name) => write!(f, "unknown simulation '{name}'"),
            Self::UnknownPlayingConditionsPreset(name) => {
                write!(f, "unknown playing conditions preset '{name}'")
            }
            Self::MultipleShotsNotSupported { count } => write!(
                f,
                "the current DSL supports at most one shot statement, but found {count}"
            ),
            Self::ShotTargetMustBeCueBall(ball) => write!(
                f,
                "the current DSL only supports shot(cue), but found shot({ball})"
            ),
            Self::ShotTargetBallNotPlaced(ball) => {
                write!(f, "shot target ball '{ball}' is not present in the layout")
            }
            Self::ShotAimingBallMustNotBeCueBall(ball) => write!(
                f,
                "shot aiming helpers require an object ball, not '{ball}'"
            ),
            Self::ShotAimingBallNotPlaced(ball) => {
                write!(f, "shot aiming ball '{ball}' is not present in the layout")
            }
            Self::CutAngleOutOfRange { degrees } => write!(
                f,
                "cut angle {degrees}deg is out of bounds; expected 0..=90"
            ),
            Self::CueElevationOutOfRange { degrees } => write!(
                f,
                "cue elevation {degrees}deg is out of bounds; expected 0..={}",
                crate::MAX_CUE_ELEVATION_DEGREES
            ),
            Self::InvalidCueStrikeConfig { name, error } => {
                write!(f, "cue_strike '{name}' is invalid: {error:?}")
            }
            Self::InvalidShot(error) => write!(f, "invalid shot: {error:?}"),
            Self::InvalidNBallGeometry {
                first_ball,
                second_ball,
                error,
            } => write!(
                f,
                "invalid layout: balls '{first_ball:?}' and '{second_ball:?}' violate rigid geometry: {error}"
            ),
            Self::UnsupportedNBallPhysics { error } => write!(f, "unsupported N-ball physics: {error}"),
            Self::BallPath(error) => write!(f, "ball path trace failed: {error}"),
        }
    }
}

impl std::error::Error for DslBuildError {}

#[derive(Debug, Clone, PartialEq)]
pub enum DslError {
    Parse(DslParseError),
    Build(DslBuildError),
}

impl std::fmt::Display for DslError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(err) => write!(f, "{err}"),
            Self::Build(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for DslError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShotControls {
    pub heading_degrees: f64,
    pub speed_ips: f64,
    pub tip_side: f64,
    pub tip_height: f64,
    pub tip_max_radius: f64,
    pub cue_elevation_degrees: f64,
    pub cue_elevation_explicit: bool,
    pub speed_max_ips: f64,
    pub cue_elevation_max_degrees: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotControl {
    Heading,
    Speed,
    Elevation,
    TipSide,
    TipHeight,
}

impl std::str::FromStr for ShotControl {
    type Err = ShotControlError;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "heading" => Ok(Self::Heading),
            "speed" => Ok(Self::Speed),
            "elevation" => Ok(Self::Elevation),
            "tip-side" => Ok(Self::TipSide),
            "tip-height" => Ok(Self::TipHeight),
            _ => Err(ShotControlError::UnknownControl(name.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShotControlError {
    Dsl(DslError),
    NoShot,
    UnknownControl(String),
    NonFiniteValue { control: &'static str },
    MissingSourceMetadata { control: &'static str },
}

impl std::fmt::Display for ShotControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dsl(error) => write!(f, "{error}"),
            Self::NoShot => write!(f, "the DSL does not contain a shot"),
            Self::UnknownControl(control) => write!(f, "unknown shot control '{control}'"),
            Self::NonFiniteValue { control } => {
                write!(f, "shot control '{control}' requires a finite value")
            }
            Self::MissingSourceMetadata { control } => {
                write!(f, "shot control '{control}' has no editable source range")
            }
        }
    }
}

impl std::error::Error for ShotControlError {}

impl From<DslError> for ShotControlError {
    fn from(error: DslError) -> Self {
        Self::Dsl(error)
    }
}

type Stream<'i> = LocatingSlice<&'i str>;

type ParseError<'i> = ErrMode<InputError<Stream<'i>>>;

type ParseResult<'i, T> = Result<T, ParseError<'i>>;

fn parse_dsl_inner(input: &str) -> ParseResult<'_, ParsedDslDoc> {
    let mut stream = LocatingSlice::new(input);
    dsl_doc.parse_next(&mut stream)
}

fn parse_dsl_with_metadata(input: &str) -> Result<ParsedDslDoc, DslParseError> {
    parse_dsl_inner(input).map_err(parse_error)
}

pub fn parse_dsl(input: &str) -> Result<DslDoc, DslParseError> {
    parse_dsl_with_metadata(input).map(|parsed| parsed.doc)
}

pub fn parse_dsl_to_game_state(input: &str) -> Result<GameState, DslError> {
    let doc = parse_dsl(input).map_err(DslError::Parse)?;
    build_game_state(&doc).map_err(DslError::Build)
}

pub fn parse_dsl_to_scenario(input: &str) -> Result<DslScenario, DslError> {
    let doc = parse_dsl(input).map_err(DslError::Parse)?;
    build_scenario(&doc).map_err(DslError::Build)
}

pub fn shot_controls_from_dsl(source: &str) -> Result<Option<ShotControls>, ShotControlError> {
    let parsed = parse_dsl_with_metadata(source).map_err(DslError::Parse)?;
    let scenario = build_scenario(&parsed.doc).map_err(DslError::Build)?;
    let Some(built_shot) = scenario.shot.as_ref() else {
        return Ok(None);
    };
    let shot_def =
        shot_def(&parsed.doc).ok_or(ShotControlError::MissingSourceMetadata { control: "shot" })?;
    let shot_source = parsed
        .shot_source
        .ok_or(ShotControlError::MissingSourceMetadata { control: "shot" })?;
    let speed_ips = shot_def
        .methods
        .iter()
        .find_map(|method| match method {
            ShotMethodExpr::SpeedIps(speed_ips) => Some(*speed_ips),
            _ => None,
        })
        .ok_or(ShotControlError::MissingSourceMetadata { control: "speed" })?;
    let preset_max = ShotSpeedPreset::ExceptionalPowerBreak
        .inches_per_second()
        .as_f64();
    let tip_contact = built_shot.shot.tip_contact();

    Ok(Some(ShotControls {
        heading_degrees: built_shot.shot.heading().as_degrees(),
        speed_ips,
        tip_side: tip_contact.side_offset().as_f64(),
        tip_height: tip_contact.height_offset().as_f64(),
        tip_max_radius: built_shot.cue_strike.miscue_offset_limit().as_f64(),
        cue_elevation_degrees: built_shot.shot.cue_elevation().as_degrees(),
        cue_elevation_explicit: shot_source.elevation.is_some(),
        speed_max_ips: preset_max.max(speed_ips),
        cue_elevation_max_degrees: crate::MAX_CUE_ELEVATION_DEGREES,
    }))
}

pub fn update_shot_control_in_dsl(
    source: &str,
    control: ShotControl,
    value: f64,
) -> Result<String, ShotControlError> {
    let control_name = match control {
        ShotControl::Heading => "heading",
        ShotControl::Speed => "speed",
        ShotControl::Elevation => "elevation",
        ShotControl::TipSide => "tip-side",
        ShotControl::TipHeight => "tip-height",
    };
    if !value.is_finite() {
        return Err(ShotControlError::NonFiniteValue {
            control: control_name,
        });
    }

    let source_metadata = editable_shot_source(source)?;
    let number = format_shot_control_number(value);
    let mut candidate = source.to_string();
    match control {
        ShotControl::Heading => {
            let aim = source_metadata
                .aim
                .ok_or(ShotControlError::MissingSourceMetadata { control: "heading" })?;
            if let Some(literal) = aim.heading_literal {
                replace_source_span(&mut candidate, literal, &format!("{number}deg"));
            } else {
                replace_source_span(&mut candidate, aim.method, &format!("heading({number}deg)"));
            }
        }
        ShotControl::Speed => {
            let literal = source_metadata
                .speed_literal
                .ok_or(ShotControlError::MissingSourceMetadata { control: "speed" })?;
            replace_source_span(&mut candidate, literal, &format!("{number}ips"));
        }
        ShotControl::TipSide => {
            let literal = source_metadata
                .tip
                .ok_or(ShotControlError::MissingSourceMetadata { control: "tip" })?
                .side_literal;
            replace_source_span(&mut candidate, literal, &format!("{number}R"));
        }
        ShotControl::TipHeight => {
            let literal = source_metadata
                .tip
                .ok_or(ShotControlError::MissingSourceMetadata { control: "tip" })?
                .height_literal;
            replace_source_span(&mut candidate, literal, &format!("{number}R"));
        }
        ShotControl::Elevation => match source_metadata.elevation {
            Some(elevation) if elevation.is_jump => replace_source_span(
                &mut candidate,
                elevation.method,
                &format!("elevation({number}deg)"),
            ),
            Some(elevation) => {
                let literal = elevation
                    .literal
                    .ok_or(ShotControlError::MissingSourceMetadata {
                        control: "elevation",
                    })?;
                replace_source_span(&mut candidate, literal, &format!("{number}deg"));
            }
            None => {
                let tip = source_metadata
                    .tip
                    .ok_or(ShotControlError::MissingSourceMetadata { control: "tip" })?;
                candidate.insert_str(tip.method.end, &format!(".elevation({number}deg)"));
            }
        },
    }

    validate_edited_shot_source(candidate)
}

pub fn update_shot_tip_in_dsl(
    source: &str,
    side: f64,
    height: f64,
) -> Result<String, ShotControlError> {
    if !side.is_finite() {
        return Err(ShotControlError::NonFiniteValue {
            control: "tip side",
        });
    }
    if !height.is_finite() {
        return Err(ShotControlError::NonFiniteValue {
            control: "tip height",
        });
    }

    let source_metadata = editable_shot_source(source)?;
    let tip = source_metadata
        .tip
        .ok_or(ShotControlError::MissingSourceMetadata { control: "tip" })?;
    let side_literal = format!("{}R", format_shot_control_number(side));
    let height_literal = format!("{}R", format_shot_control_number(height));
    let mut replacements = [
        (tip.side_literal, side_literal),
        (tip.height_literal, height_literal),
    ];
    replacements.sort_unstable_by(|(left, _), (right, _)| right.start.cmp(&left.start));

    let mut candidate = source.to_string();
    for (span, replacement) in replacements {
        replace_source_span(&mut candidate, span, &replacement);
    }
    validate_edited_shot_source(candidate)
}

fn shot_def(doc: &DslDoc) -> Option<&ShotDef> {
    doc.entries.iter().find_map(|entry| match entry {
        DslEntry::Shot(shot) => Some(shot),
        _ => None,
    })
}

fn editable_shot_source(source: &str) -> Result<ShotSourceMetadata, ShotControlError> {
    let parsed = parse_dsl_with_metadata(source).map_err(DslError::Parse)?;
    let scenario = build_scenario(&parsed.doc).map_err(DslError::Build)?;
    if scenario.shot.is_none() {
        return Err(ShotControlError::NoShot);
    }
    parsed
        .shot_source
        .ok_or(ShotControlError::MissingSourceMetadata { control: "shot" })
}

fn validate_edited_shot_source(candidate: String) -> Result<String, ShotControlError> {
    let scenario = parse_dsl_to_scenario(&candidate)?;
    if scenario.shot.is_none() {
        return Err(ShotControlError::NoShot);
    }
    Ok(candidate)
}

fn replace_source_span(source: &mut String, span: ByteSpan, replacement: &str) {
    source.replace_range(span.start..span.end, replacement);
}

fn format_shot_control_number(value: f64) -> String {
    if value == 0.0 {
        "0".to_string()
    } else {
        value.to_string()
    }
}

pub fn build_game_state(doc: &DslDoc) -> Result<GameState, DslBuildError> {
    build_scenario(doc).map(|scenario| scenario.game_state)
}

pub fn build_scenario(doc: &DslDoc) -> Result<DslScenario, DslBuildError> {
    let table_spec = doc
        .table
        .unwrap_or(TableRef::BrunswickGc4_9ft)
        .to_table_spec();
    let default_ball_spec = table_spec.default_ball_spec();
    let mut game_state = GameState::new(table_spec);
    if let Some(game) = doc.game {
        game_state.ty = game.to_game_type();
    }
    let mut aliases = HashMap::new();
    let mut cue_strikes = HashMap::new();
    let mut ball_ball_defs = Vec::new();
    let mut rail_response_defs = Vec::new();
    let mut rails_defs = Vec::new();
    let mut simulation_defs = Vec::new();
    let mut shots = Vec::new();

    for entry in &doc.entries {
        match entry {
            DslEntry::Alias(alias) => {
                if aliases.contains_key(&alias.name) {
                    return Err(DslBuildError::DuplicateAlias(alias.name.clone()));
                }
                let resolved = resolve_position_expr(&aliases, &alias.position)?;
                aliases.insert(alias.name.clone(), resolved);
            }
            DslEntry::Ball(placement) => {
                let ball = match placement {
                    BallPlacement::At { ball, .. } | BallPlacement::Frozen { ball, .. } => ball,
                };
                if game_state
                    .balls()
                    .iter()
                    .any(|placed| placed.ty == ball.to_ball_type())
                {
                    return Err(DslBuildError::DuplicateBallPlacement(*ball));
                }

                match placement {
                    BallPlacement::At { ball, position } => {
                        let pos = resolve_position_expr(&aliases, position)?;
                        game_state.add_ball(Ball {
                            ty: ball.to_ball_type(),
                            position: pos,
                            spec: default_ball_spec.clone(),
                        });
                    }
                    BallPlacement::Frozen { ball, rail, coord } => {
                        validate_frozen_coordinate(*rail, *coord)?;
                        let rail = rail.to_rail();
                        let diamond = Diamond::from(coord.to_string().as_str());
                        game_state.freeze_to_rail(
                            rail,
                            diamond,
                            Ball {
                                ty: ball.to_ball_type(),
                                spec: default_ball_spec.clone(),
                                ..Default::default()
                            },
                        );
                    }
                }
            }
            DslEntry::CueStrike(def) => {
                let name = def.name.clone();
                let cue_strike = build_cue_strike(def)?;
                if cue_strikes.insert(name.clone(), cue_strike).is_some() {
                    return Err(DslBuildError::DuplicateCueStrike(name));
                }
            }
            DslEntry::BallBall(def) => ball_ball_defs.push(def.clone()),
            DslEntry::RailResponse(def) => rail_response_defs.push(def.clone()),
            DslEntry::Rails(def) => rails_defs.push(def.clone()),
            DslEntry::Simulation(def) => simulation_defs.push(def.clone()),
            DslEntry::Shot(def) => shots.push(def.clone()),
        }
    }

    let ball_ball_configs = build_ball_ball_configs(&ball_ball_defs)?;
    let rail_responses = build_rail_responses(&rail_response_defs)?;
    let rail_profiles = build_rail_profiles(&rails_defs, &rail_responses)?;
    let simulations = build_simulations(&simulation_defs, &ball_ball_configs, &rail_profiles)?;
    let shot = match shots.as_slice() {
        [] => None,
        [shot] => Some(build_shot(shot, &cue_strikes, &game_state)?),
        _ => {
            return Err(DslBuildError::MultipleShotsNotSupported { count: shots.len() });
        }
    };

    Ok(DslScenario {
        game_state,
        shot,
        trace_max_events: doc.trace_max_events,
        ball_ball_configs,
        rail_responses,
        rail_profiles,
        simulations,
    })
}

fn resolve_position_expr(
    aliases: &HashMap<String, Position>,
    position: &PositionExpr,
) -> Result<Position, DslBuildError> {
    match position {
        PositionExpr::Diamond { x, y } => {
            validate_coordinate(CoordinateAxis::X, *x, 0.0, 4.0)?;
            validate_coordinate(CoordinateAxis::Y, *y, 0.0, 8.0)?;
            Ok(Position::new(
                Diamond::from(x.to_string().as_str()),
                Diamond::from(y.to_string().as_str()),
            ))
        }
        PositionExpr::Named(named) => Ok(named.to_position()),
        PositionExpr::Alias(name) => aliases
            .get(name)
            .cloned()
            .ok_or_else(|| DslBuildError::UnknownAlias(name.clone())),
    }
}

fn build_cue_strike(def: &CueStrikeDef) -> Result<CueStrikeConfig, DslBuildError> {
    let mut cue_mass_ratio = None;
    let mut collision_energy_loss = None;
    let mut endmass_ratio = None;

    for method in &def.methods {
        match method {
            CueStrikeMethodExpr::MassRatio(value) => {
                set_once(&mut cue_mass_ratio, Scale::from_f64(*value), || {
                    DslBuildError::DuplicateCueStrikeMethod {
                        name: def.name.clone(),
                        method: "mass_ratio".to_string(),
                    }
                })?;
            }
            CueStrikeMethodExpr::EnergyLoss(value) => {
                set_once(&mut collision_energy_loss, Scale::from_f64(*value), || {
                    DslBuildError::DuplicateCueStrikeMethod {
                        name: def.name.clone(),
                        method: "energy_loss".to_string(),
                    }
                })?;
            }
            CueStrikeMethodExpr::EndmassRatio(value) => {
                set_once(&mut endmass_ratio, Scale::from_f64(*value), || {
                    DslBuildError::DuplicateCueStrikeMethod {
                        name: def.name.clone(),
                        method: "endmass_ratio".to_string(),
                    }
                })?;
            }
        }
    }

    let cue_mass_ratio = cue_mass_ratio.ok_or_else(|| DslBuildError::MissingCueStrikeMethod {
        name: def.name.clone(),
        method: "mass_ratio".to_string(),
    })?;
    let collision_energy_loss =
        collision_energy_loss.ok_or_else(|| DslBuildError::MissingCueStrikeMethod {
            name: def.name.clone(),
            method: "energy_loss".to_string(),
        })?;

    match endmass_ratio {
        Some(endmass_ratio) => CueStrikeConfig::new_with_endmass_ratio(
            cue_mass_ratio,
            collision_energy_loss,
            endmass_ratio,
        ),
        None => CueStrikeConfig::new(cue_mass_ratio, collision_energy_loss),
    }
    .map_err(|error| DslBuildError::InvalidCueStrikeConfig {
        name: def.name.clone(),
        error,
    })
}

fn build_ball_ball_configs(
    defs: &[BallBallDef],
) -> Result<HashMap<String, BallBallCollisionConfig>, DslBuildError> {
    let mut configs = HashMap::new();

    for def in defs {
        let name = def.name.clone();
        let config = build_ball_ball_config(def)?;
        if configs.insert(name.clone(), config).is_some() {
            return Err(DslBuildError::DuplicateBallBall(name));
        }
    }

    Ok(configs)
}

fn build_ball_ball_config(def: &BallBallDef) -> Result<BallBallCollisionConfig, DslBuildError> {
    let mut normal_restitution = None;
    let mut tangential_friction = None;

    for method in &def.methods {
        match method {
            BallBallMethodExpr::NormalRestitution(value) => {
                set_once(&mut normal_restitution, *value, || {
                    DslBuildError::DuplicateBallBallMethod {
                        name: def.name.clone(),
                        method: "normal_restitution".to_string(),
                    }
                })?;
            }
            BallBallMethodExpr::TangentialFriction(value) => {
                set_once(&mut tangential_friction, *value, || {
                    DslBuildError::DuplicateBallBallMethod {
                        name: def.name.clone(),
                        method: "tangential_friction".to_string(),
                    }
                })?;
            }
        }
    }

    let normal_restitution =
        normal_restitution.ok_or_else(|| DslBuildError::MissingBallBallMethod {
            name: def.name.clone(),
            method: "normal_restitution".to_string(),
        })?;
    let tangential_friction =
        tangential_friction.ok_or_else(|| DslBuildError::MissingBallBallMethod {
            name: def.name.clone(),
            method: "tangential_friction".to_string(),
        })?;

    Ok(BallBallCollisionConfig::new(
        validate_unit_interval_physics_value(
            PhysicsConfigKind::BallBall,
            &def.name,
            "normal_restitution",
            normal_restitution,
        )?,
        validate_non_negative_physics_value(
            PhysicsConfigKind::BallBall,
            &def.name,
            "tangential_friction",
            tangential_friction,
        )?,
    ))
}

fn build_rail_responses(
    defs: &[RailResponseDef],
) -> Result<HashMap<String, RailCollisionConfig>, DslBuildError> {
    let mut responses = HashMap::new();

    for def in defs {
        let name = def.name.clone();
        let response = build_rail_response(def)?;
        if responses.insert(name.clone(), response).is_some() {
            return Err(DslBuildError::DuplicateRailResponse(name));
        }
    }

    Ok(responses)
}

fn build_rail_response(def: &RailResponseDef) -> Result<RailCollisionConfig, DslBuildError> {
    let mut normal_restitution = None;
    let mut tangential_friction = None;

    for method in &def.methods {
        match method {
            RailResponseMethodExpr::NormalRestitution(value) => {
                set_once(&mut normal_restitution, *value, || {
                    DslBuildError::DuplicateRailResponseMethod {
                        name: def.name.clone(),
                        method: "normal_restitution".to_string(),
                    }
                })?;
            }
            RailResponseMethodExpr::TangentialFriction(value) => {
                set_once(&mut tangential_friction, *value, || {
                    DslBuildError::DuplicateRailResponseMethod {
                        name: def.name.clone(),
                        method: "tangential_friction".to_string(),
                    }
                })?;
            }
        }
    }

    let normal_restitution =
        normal_restitution.ok_or_else(|| DslBuildError::MissingRailResponseMethod {
            name: def.name.clone(),
            method: "normal_restitution".to_string(),
        })?;
    let tangential_friction =
        tangential_friction.ok_or_else(|| DslBuildError::MissingRailResponseMethod {
            name: def.name.clone(),
            method: "tangential_friction".to_string(),
        })?;

    Ok(RailCollisionConfig::new(
        validate_unit_interval_physics_value(
            PhysicsConfigKind::RailResponse,
            &def.name,
            "normal_restitution",
            normal_restitution,
        )?,
        validate_non_negative_physics_value(
            PhysicsConfigKind::RailResponse,
            &def.name,
            "tangential_friction",
            tangential_friction,
        )?,
    ))
}

fn build_rail_profiles(
    defs: &[RailsDef],
    responses: &HashMap<String, RailCollisionConfig>,
) -> Result<HashMap<String, RailCollisionProfile>, DslBuildError> {
    let mut profiles = HashMap::new();

    for def in defs {
        let name = def.name.clone();
        let profile = build_rail_profile(def, responses)?;
        if profiles.insert(name.clone(), profile).is_some() {
            return Err(DslBuildError::DuplicateRails(name));
        }
    }

    Ok(profiles)
}

fn build_rail_profile(
    def: &RailsDef,
    responses: &HashMap<String, RailCollisionConfig>,
) -> Result<RailCollisionProfile, DslBuildError> {
    let mut default_response = None;
    let mut top = None;
    let mut right = None;
    let mut bottom = None;
    let mut left = None;

    for method in &def.methods {
        match method {
            RailsMethodExpr::Default(name) => {
                set_once(&mut default_response, name.clone(), || {
                    DslBuildError::DuplicateRailsMethod {
                        name: def.name.clone(),
                        method: "default".to_string(),
                    }
                })?;
            }
            RailsMethodExpr::Top(name) => {
                set_once(&mut top, name.clone(), || {
                    DslBuildError::DuplicateRailsMethod {
                        name: def.name.clone(),
                        method: "top".to_string(),
                    }
                })?;
            }
            RailsMethodExpr::Right(name) => {
                set_once(&mut right, name.clone(), || {
                    DslBuildError::DuplicateRailsMethod {
                        name: def.name.clone(),
                        method: "right".to_string(),
                    }
                })?;
            }
            RailsMethodExpr::Bottom(name) => {
                set_once(&mut bottom, name.clone(), || {
                    DslBuildError::DuplicateRailsMethod {
                        name: def.name.clone(),
                        method: "bottom".to_string(),
                    }
                })?;
            }
            RailsMethodExpr::Left(name) => {
                set_once(&mut left, name.clone(), || {
                    DslBuildError::DuplicateRailsMethod {
                        name: def.name.clone(),
                        method: "left".to_string(),
                    }
                })?;
            }
        }
    }

    let default_response = default_response.ok_or_else(|| DslBuildError::MissingRailsMethod {
        name: def.name.clone(),
        method: "default".to_string(),
    })?;
    let mut profile =
        RailCollisionProfile::uniform(lookup_rail_response(responses, &default_response)?.clone());

    if let Some(name) = top {
        profile.top = lookup_rail_response(responses, &name)?.clone();
    }
    if let Some(name) = right {
        profile.right = lookup_rail_response(responses, &name)?.clone();
    }
    if let Some(name) = bottom {
        profile.bottom = lookup_rail_response(responses, &name)?.clone();
    }
    if let Some(name) = left {
        profile.left = lookup_rail_response(responses, &name)?.clone();
    }

    Ok(profile)
}

fn lookup_rail_response<'a>(
    responses: &'a HashMap<String, RailCollisionConfig>,
    name: &str,
) -> Result<&'a RailCollisionConfig, DslBuildError> {
    responses
        .get(name)
        .ok_or_else(|| DslBuildError::UnknownRailResponse(name.to_string()))
}

fn build_simulations(
    defs: &[SimulationDef],
    ball_ball_configs: &HashMap<String, BallBallCollisionConfig>,
    rail_profiles: &HashMap<String, RailCollisionProfile>,
) -> Result<HashMap<String, SimulationPreset>, DslBuildError> {
    let mut simulations = HashMap::new();

    for def in defs {
        let name = def.name.clone();
        let simulation = build_simulation(def, ball_ball_configs, rail_profiles)?;
        if simulations.insert(name.clone(), simulation).is_some() {
            return Err(DslBuildError::DuplicateSimulation(name));
        }
    }

    Ok(simulations)
}

fn build_simulation(
    def: &SimulationDef,
    ball_ball_configs: &HashMap<String, BallBallCollisionConfig>,
    rail_profiles: &HashMap<String, RailCollisionProfile>,
) -> Result<SimulationPreset, DslBuildError> {
    let mut collision_model = None;
    let mut ball_ball_name = None;
    let mut rail_model = None;
    let mut rails_name = None;
    let mut conditions_name = None;
    let mut max_events = None;

    for method in &def.methods {
        match method {
            SimulationMethodExpr::CollisionModel(model) => {
                set_once(&mut collision_model, *model, || {
                    DslBuildError::DuplicateSimulationMethod {
                        name: def.name.clone(),
                        method: "collision_model".to_string(),
                    }
                })?;
            }
            SimulationMethodExpr::BallBall(name) => {
                set_once(&mut ball_ball_name, name.clone(), || {
                    DslBuildError::DuplicateSimulationMethod {
                        name: def.name.clone(),
                        method: "ball_ball".to_string(),
                    }
                })?;
            }
            SimulationMethodExpr::RailModel(model) => {
                set_once(&mut rail_model, *model, || {
                    DslBuildError::DuplicateSimulationMethod {
                        name: def.name.clone(),
                        method: "rail_model".to_string(),
                    }
                })?;
            }
            SimulationMethodExpr::Rails(name) => {
                set_once(&mut rails_name, name.clone(), || {
                    DslBuildError::DuplicateSimulationMethod {
                        name: def.name.clone(),
                        method: "rails".to_string(),
                    }
                })?;
            }
            SimulationMethodExpr::Conditions(name) => {
                set_once(&mut conditions_name, name.clone(), || {
                    DslBuildError::DuplicateSimulationMethod {
                        name: def.name.clone(),
                        method: "conditions".to_string(),
                    }
                })?;
            }
            SimulationMethodExpr::MaxEvents(value) => {
                set_once(&mut max_events, *value, || {
                    DslBuildError::DuplicateSimulationMethod {
                        name: def.name.clone(),
                        method: "max_events".to_string(),
                    }
                })?;
            }
        }
    }

    let collision_model =
        collision_model.ok_or_else(|| DslBuildError::MissingSimulationMethod {
            name: def.name.clone(),
            method: "collision_model".to_string(),
        })?;
    let ball_ball_name = ball_ball_name.ok_or_else(|| DslBuildError::MissingSimulationMethod {
        name: def.name.clone(),
        method: "ball_ball".to_string(),
    })?;
    let rail_model = rail_model.ok_or_else(|| DslBuildError::MissingSimulationMethod {
        name: def.name.clone(),
        method: "rail_model".to_string(),
    })?;
    let rails_name = rails_name.ok_or_else(|| DslBuildError::MissingSimulationMethod {
        name: def.name.clone(),
        method: "rails".to_string(),
    })?;
    let conditions = conditions_name
        .as_deref()
        .map(|name| {
            PlayingConditionsPreset::from_name(name)
                .map(PlayingConditions::from)
                .ok_or_else(|| DslBuildError::UnknownPlayingConditionsPreset(name.to_string()))
        })
        .transpose()?
        .unwrap_or_default();

    if !ball_ball_configs.contains_key(&ball_ball_name) {
        return Err(DslBuildError::UnknownBallBallConfig(ball_ball_name));
    }
    if !rail_profiles.contains_key(&rails_name) {
        return Err(DslBuildError::UnknownRailProfile(rails_name));
    }

    Ok(SimulationPreset {
        collision_model,
        ball_ball_name,
        rail_model,
        rails_name,
        conditions,
        max_events,
    })
}

enum ShotAimSpec {
    HeadingDegrees(f64),
    ToPocket {
        object_ball: BallRef,
        pocket: Pocket,
    },
    Cut {
        object_ball: BallRef,
        direction: ShotCutDirection,
        degrees: f64,
    },
}

fn set_shot_aim_once(
    slot: &mut Option<(ShotAimSpec, &'static str)>,
    value: ShotAimSpec,
    method_name: &'static str,
) -> Result<(), DslBuildError> {
    if let Some((_, existing_method)) = slot {
        return Err(if *existing_method == method_name {
            DslBuildError::DuplicateShotMethod {
                method: method_name.to_string(),
            }
        } else {
            DslBuildError::ConflictingShotAimMethods {
                first: existing_method.to_string(),
                second: method_name.to_string(),
            }
        });
    }

    *slot = Some((value, method_name));
    Ok(())
}

fn angle_from_degrees(degrees: f64) -> Angle {
    let radians = degrees.to_radians();
    Angle::from_north(radians.sin(), radians.cos())
}

fn cue_elevation_angle_from_degrees(degrees: f64) -> Result<Angle, DslBuildError> {
    if !degrees.is_finite() || !(0.0..=crate::MAX_CUE_ELEVATION_DEGREES).contains(&degrees) {
        return Err(DslBuildError::CueElevationOutOfRange { degrees });
    }

    Ok(angle_from_degrees(degrees))
}

fn resolve_shot_aiming_ball<'a>(
    game_state: &'a GameState,
    ball_ref: BallRef,
) -> Result<&'a Ball, DslBuildError> {
    if ball_ref == BallRef::Cue {
        return Err(DslBuildError::ShotAimingBallMustNotBeCueBall(ball_ref));
    }

    game_state
        .select_ball(ball_ref.to_ball_type())
        .ok_or(DslBuildError::ShotAimingBallNotPlaced(ball_ref))
}

fn resolve_shot_heading(aim: ShotAimSpec, game_state: &GameState) -> Result<Angle, DslBuildError> {
    let cue_ball = game_state
        .select_ball(BallType::Cue)
        .ok_or(DslBuildError::ShotTargetBallNotPlaced(BallRef::Cue))?;

    match aim {
        ShotAimSpec::HeadingDegrees(degrees) => Ok(angle_from_degrees(degrees)),
        ShotAimSpec::ToPocket {
            object_ball,
            pocket,
        } => {
            let object_ball = resolve_shot_aiming_ball(game_state, object_ball)?;
            Ok(object_ball.aim_angle_to_pocket(pocket, &cue_ball.position, &game_state.table_spec))
        }
        ShotAimSpec::Cut {
            object_ball,
            direction,
            degrees,
        } => {
            if !degrees.is_finite() || !(0.0..=90.0).contains(&degrees) {
                return Err(DslBuildError::CutAngleOutOfRange { degrees });
            }

            let object_ball = resolve_shot_aiming_ball(game_state, object_ball)?;
            let base_heading = cue_ball
                .position
                .angle_to(&object_ball.position)
                .as_degrees();
            let signed_degrees = match direction {
                ShotCutDirection::Left => -degrees,
                ShotCutDirection::Right => degrees,
            };
            let object_heading =
                angle_from_degrees((base_heading + signed_degrees).rem_euclid(360.0));
            let destination = object_ball
                .position
                .translate(Diamond::one(), object_heading);

            Ok(object_ball.aim_angle(&destination, &cue_ball.position, &game_state.table_spec))
        }
    }
}

fn default_cue_elevation_degrees_for_dsl_tip(side_offset_radius: f64) -> Option<f64> {
    if side_offset_radius.abs() > f64::EPSILON {
        Some(DEFAULT_SIDE_ENGLISH_CUE_ELEVATION_DEGREES)
    } else {
        None
    }
}

fn build_shot(
    def: &ShotDef,
    cue_strikes: &HashMap<String, CueStrikeConfig>,
    game_state: &GameState,
) -> Result<ScenarioShot, DslBuildError> {
    if def.ball != BallRef::Cue {
        return Err(DslBuildError::ShotTargetMustBeCueBall(def.ball));
    }

    let ball = def.ball.to_ball_type();
    if game_state.select_ball(ball.clone()).is_none() {
        return Err(DslBuildError::ShotTargetBallNotPlaced(def.ball));
    }

    let mut aim = None;
    let mut cue_ball_launch_speed_ips = None;
    let mut tip = None;
    let mut cue_elevation_degrees = None;
    let mut cue_strike_name = None;

    for method in &def.methods {
        match method {
            ShotMethodExpr::HeadingDegrees(value) => {
                set_shot_aim_once(&mut aim, ShotAimSpec::HeadingDegrees(*value), "heading")?
            }
            ShotMethodExpr::ToPocket {
                object_ball,
                pocket,
            } => set_shot_aim_once(
                &mut aim,
                ShotAimSpec::ToPocket {
                    object_ball: *object_ball,
                    pocket: *pocket,
                },
                "to_pocket",
            )?,
            ShotMethodExpr::Cut {
                object_ball,
                direction,
                degrees,
            } => set_shot_aim_once(
                &mut aim,
                ShotAimSpec::Cut {
                    object_ball: *object_ball,
                    direction: *direction,
                    degrees: *degrees,
                },
                "cut",
            )?,
            ShotMethodExpr::SpeedIps(value) => {
                set_once(&mut cue_ball_launch_speed_ips, *value, || {
                    DslBuildError::DuplicateShotMethod {
                        method: "speed".to_string(),
                    }
                })?;
            }
            ShotMethodExpr::Tip { side, height } => {
                set_once(&mut tip, (*side, *height), || {
                    DslBuildError::DuplicateShotMethod {
                        method: "tip".to_string(),
                    }
                })?;
            }
            ShotMethodExpr::ElevationDegrees(value) => {
                set_once(&mut cue_elevation_degrees, *value, || {
                    DslBuildError::DuplicateShotMethod {
                        method: "elevation".to_string(),
                    }
                })?;
            }
            ShotMethodExpr::Using(name) => {
                set_once(&mut cue_strike_name, name.clone(), || {
                    DslBuildError::DuplicateShotMethod {
                        method: "using".to_string(),
                    }
                })?;
            }
        }
    }

    let (aim, _) = aim.ok_or(DslBuildError::MissingShotAimMethod)?;
    let cue_ball_launch_speed_ips =
        cue_ball_launch_speed_ips.ok_or_else(|| DslBuildError::MissingShotMethod {
            method: "speed".to_string(),
        })?;
    let (side, height) = tip.ok_or_else(|| DslBuildError::MissingShotMethod {
        method: "tip".to_string(),
    })?;
    let cue_strike_name = cue_strike_name.ok_or_else(|| DslBuildError::MissingShotMethod {
        method: "using".to_string(),
    })?;
    let effective_cue_elevation_degrees =
        cue_elevation_degrees.or_else(|| default_cue_elevation_degrees_for_dsl_tip(side));

    let cue_strike = cue_strikes
        .get(&cue_strike_name)
        .cloned()
        .ok_or(DslBuildError::UnknownCueStrike(cue_strike_name))?;
    let tip_contact = CueTipContact::new(Scale::from_f64(side), Scale::from_f64(height))
        .map_err(DslBuildError::InvalidShot)?;
    let mut shot = Shot::new_for_cue_ball_launch_speed(
        resolve_shot_heading(aim, game_state)?,
        InchesPerSecond::new(Inches::from_f64(cue_ball_launch_speed_ips)),
        tip_contact,
        &cue_strike,
    )
    .map_err(DslBuildError::InvalidShot)?;
    if let Some(elevation_degrees) = effective_cue_elevation_degrees {
        shot = shot
            .with_cue_elevation(cue_elevation_angle_from_degrees(elevation_degrees)?)
            .map_err(DslBuildError::InvalidShot)?;
    }

    Ok(ScenarioShot {
        ball_ref: def.ball,
        ball,
        shot,
        cue_strike,
    })
}

fn validate_unit_interval_physics_value(
    kind: PhysicsConfigKind,
    name: &str,
    method: &str,
    value: f64,
) -> Result<Scale, DslBuildError> {
    if (0.0..=1.0).contains(&value) {
        Ok(Scale::from_f64(value))
    } else {
        Err(DslBuildError::InvalidPhysicsConfigValue {
            kind,
            name: name.to_string(),
            method: method.to_string(),
            value,
            expected: "a value in [0, 1]".to_string(),
        })
    }
}

fn validate_non_negative_physics_value(
    kind: PhysicsConfigKind,
    name: &str,
    method: &str,
    value: f64,
) -> Result<Scale, DslBuildError> {
    if value >= 0.0 {
        Ok(Scale::from_f64(value))
    } else {
        Err(DslBuildError::InvalidPhysicsConfigValue {
            kind,
            name: name.to_string(),
            method: method.to_string(),
            value,
            expected: "a non-negative value".to_string(),
        })
    }
}

fn set_once<T>(
    slot: &mut Option<T>,
    value: T,
    duplicate: impl FnOnce() -> DslBuildError,
) -> Result<(), DslBuildError> {
    if slot.is_some() {
        Err(duplicate())
    } else {
        *slot = Some(value);
        Ok(())
    }
}

fn validate_coordinate(
    axis: CoordinateAxis,
    value: f64,
    min: f64,
    max: f64,
) -> Result<(), DslBuildError> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(DslBuildError::CoordinateOutOfRange {
            axis,
            value,
            min,
            max,
        })
    }
}

fn validate_frozen_coordinate(rail: RailSide, value: f64) -> Result<(), DslBuildError> {
    let max = match rail {
        RailSide::Left | RailSide::Right => 8.0,
        RailSide::Top | RailSide::Bottom => 4.0,
    };

    if (0.0..=max).contains(&value) {
        Ok(())
    } else {
        Err(DslBuildError::FrozenCoordinateOutOfRange {
            rail,
            value,
            min: 0.0,
            max,
        })
    }
}

fn parse_error(err: ParseError<'_>) -> DslParseError {
    let offset = match err {
        ErrMode::Backtrack(error) | ErrMode::Cut(error) => {
            let input = error.input;
            Location::current_token_start(&input)
        }
        ErrMode::Incomplete(_) => 0,
    };
    let message = "invalid DSL".to_string();
    DslParseError { message, offset }
}

fn dsl_doc<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ParsedDslDoc> {
    let mut doc = DslDoc {
        table: None,
        game: None,
        trace_max_events: None,
        entries: Vec::new(),
    };
    let mut duplicate_singleton = None;
    let mut shot_source = None;

    repeat(0.., statement)
        .fold(
            || (),
            |(), (entry, entry_start)| match entry {
                DslStatement::Table(table) => {
                    if doc.table.replace(table).is_some() {
                        duplicate_singleton.get_or_insert(entry_start);
                    }
                }
                DslStatement::Game(game) => {
                    if doc.game.replace(game).is_some() {
                        duplicate_singleton.get_or_insert(entry_start);
                    }
                }
                DslStatement::TraceMaxEvents(max_events) => {
                    if doc.trace_max_events.replace(max_events).is_some() {
                        duplicate_singleton.get_or_insert(entry_start);
                    }
                }
                DslStatement::Alias(alias) => doc.entries.push(DslEntry::Alias(alias)),
                DslStatement::Ball(placement) => doc.entries.push(DslEntry::Ball(placement)),
                DslStatement::CueStrike(def) => doc.entries.push(DslEntry::CueStrike(def)),
                DslStatement::BallBall(def) => doc.entries.push(DslEntry::BallBall(def)),
                DslStatement::RailResponse(def) => doc.entries.push(DslEntry::RailResponse(def)),
                DslStatement::Rails(def) => doc.entries.push(DslEntry::Rails(def)),
                DslStatement::Simulation(def) => doc.entries.push(DslEntry::Simulation(def)),
                DslStatement::Shot(parsed) => {
                    shot_source = Some(parsed.source);
                    doc.entries.push(DslEntry::Shot(parsed.def));
                }
                DslStatement::Empty => {}
            },
        )
        .parse_next(input)?;

    if let Some(duplicate_start) = duplicate_singleton {
        return Err(ErrMode::Cut(InputError::at(duplicate_start)));
    }

    let _ = terminated(hws0, eof).parse_next(input)?;

    Ok(ParsedDslDoc { doc, shot_source })
}

#[derive(Debug)]
enum DslStatement {
    Table(TableRef),
    Game(GameRef),
    TraceMaxEvents(usize),
    Alias(AliasDef),
    Ball(BallPlacement),
    CueStrike(CueStrikeDef),
    BallBall(BallBallDef),
    RailResponse(RailResponseDef),
    Rails(RailsDef),
    Simulation(SimulationDef),
    Shot(ParsedShotDef),
    Empty,
}

fn statement<'a>(input: &mut Stream<'a>) -> ParseResult<'a, (DslStatement, Stream<'a>)> {
    let _ = hws0.parse_next(input)?;
    let statement_start = *input;
    let stmt = alt((
        comment_line,
        blank_line,
        preceded(peek("table"), cut_err(table_stmt)),
        preceded(peek("game"), cut_err(game_stmt)),
        preceded(peek("trace"), cut_err(trace_stmt)),
        preceded(peek("pos"), cut_err(alias_stmt)),
        preceded(peek("rail_response"), cut_err(rail_response_stmt)),
        preceded(peek("rails"), cut_err(rails_stmt)),
        preceded(peek("simulation"), cut_err(simulation_stmt)),
        preceded(peek("ball_ball"), cut_err(ball_ball_stmt)),
        preceded(peek("ball"), cut_err(ball_stmt)),
        preceded(peek("cue_strike"), cut_err(cue_strike_stmt)),
        preceded(peek("shot"), cut_err(shot_stmt)),
    ))
    .parse_next(input)?;
    let _ = hws0.parse_next(input)?;
    Ok((stmt, statement_start))
}

fn comment_line<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = ('#', till_line_ending).parse_next(input)?;
    let _ = opt(line_ending).parse_next(input)?;
    Ok(DslStatement::Empty)
}

fn blank_line<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = line_ending.parse_next(input)?;
    Ok(DslStatement::Empty)
}

fn table_stmt<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = "table".parse_next(input)?;
    let _ = ws1.parse_next(input)?;
    let table = table_ref.parse_next(input)?;
    Ok(DslStatement::Table(table))
}

fn game_stmt<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = "game".parse_next(input)?;
    let _ = ws1.parse_next(input)?;
    let game = game_ref.parse_next(input)?;
    Ok(DslStatement::Game(game))
}

fn trace_stmt<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = "trace".parse_next(input)?;
    let max_events = delimited(
        '(',
        delimited(
            hws0,
            preceded(("max_events", hws0, ':', hws0), usize_literal),
            hws0,
        ),
        ')',
    )
    .parse_next(input)?;
    Ok(DslStatement::TraceMaxEvents(max_events))
}

fn alias_stmt<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = "pos".parse_next(input)?;
    let _ = ws1.parse_next(input)?;
    let name = identifier.parse_next(input)?;
    let _ = hws0.parse_next(input)?;
    let _ = '='.parse_next(input)?;
    let _ = hws0.parse_next(input)?;
    let position = position_expr.parse_next(input)?;
    Ok(DslStatement::Alias(AliasDef {
        name: name.to_string(),
        position,
    }))
}

fn ball_stmt<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = "ball".parse_next(input)?;
    let _ = ws1.parse_next(input)?;
    let ball = ball_ref.parse_next(input)?;
    let _ = ws1.parse_next(input)?;
    let placement = alt((ball_frozen, ball_at)).parse_next(input)?;
    Ok(DslStatement::Ball(match placement {
        BallPlacementKind::At(position) => BallPlacement::At { ball, position },
        BallPlacementKind::Frozen { rail, coord } => BallPlacement::Frozen { ball, rail, coord },
    }))
}

fn cue_strike_stmt<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = "cue_strike".parse_next(input)?;
    let name = delimited('(', delimited(hws0, identifier, hws0), ')').parse_next(input)?;
    let methods = repeat(0.., cue_strike_method_segment).parse_next(input)?;
    Ok(DslStatement::CueStrike(CueStrikeDef {
        name: name.to_string(),
        methods,
    }))
}

fn ball_ball_stmt<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = "ball_ball".parse_next(input)?;
    let name = delimited('(', delimited(hws0, identifier, hws0), ')').parse_next(input)?;
    let methods = repeat(0.., ball_ball_method_segment).parse_next(input)?;
    Ok(DslStatement::BallBall(BallBallDef {
        name: name.to_string(),
        methods,
    }))
}

fn rail_response_stmt<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = "rail_response".parse_next(input)?;
    let name = delimited('(', delimited(hws0, identifier, hws0), ')').parse_next(input)?;
    let methods = repeat(0.., rail_response_method_segment).parse_next(input)?;
    Ok(DslStatement::RailResponse(RailResponseDef {
        name: name.to_string(),
        methods,
    }))
}

fn rails_stmt<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = "rails".parse_next(input)?;
    let name = delimited('(', delimited(hws0, identifier, hws0), ')').parse_next(input)?;
    let methods = repeat(0.., rails_method_segment).parse_next(input)?;
    Ok(DslStatement::Rails(RailsDef {
        name: name.to_string(),
        methods,
    }))
}

fn simulation_stmt<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = "simulation".parse_next(input)?;
    let name = delimited('(', delimited(hws0, identifier, hws0), ')').parse_next(input)?;
    let methods = repeat(0.., simulation_method_segment).parse_next(input)?;
    Ok(DslStatement::Simulation(SimulationDef {
        name: name.to_string(),
        methods,
    }))
}

fn shot_stmt<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = "shot".parse_next(input)?;
    let ball = delimited('(', delimited(hws0, ball_ref, hws0), ')').parse_next(input)?;
    let parsed_methods: Vec<ParsedShotMethod> =
        repeat(0.., shot_method_segment).parse_next(input)?;
    let mut methods = Vec::with_capacity(parsed_methods.len());
    let mut source = ShotSourceMetadata::default();

    for parsed in parsed_methods {
        match parsed.source {
            ParsedShotMethodSource::Aim(aim) => source.aim = Some(aim),
            ParsedShotMethodSource::Speed { literal } => source.speed_literal = Some(literal),
            ParsedShotMethodSource::Tip(tip) => source.tip = Some(tip),
            ParsedShotMethodSource::Elevation(elevation) => source.elevation = Some(elevation),
            ParsedShotMethodSource::Other => {}
        }
        methods.push(parsed.expr);
    }

    Ok(DslStatement::Shot(ParsedShotDef {
        def: ShotDef { ball, methods },
        source,
    }))
}

fn hws0<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ()> {
    take_while(0.., |c: char| c == ' ' || c == '\t')
        .void()
        .parse_next(input)
}

fn chain_ws0<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ()> {
    take_while(0.., |c: char| {
        c == ' ' || c == '\t' || c == '\n' || c == '\r'
    })
    .void()
    .parse_next(input)
}

fn ws1<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ()> {
    take_while(1.., |c: char| c == ' ' || c == '\t')
        .void()
        .parse_next(input)
}

fn ball_at<'a>(input: &mut Stream<'a>) -> ParseResult<'a, BallPlacementKind> {
    let _ = "at".parse_next(input)?;
    let _ = ws1.parse_next(input)?;
    let position = position_expr.parse_next(input)?;
    Ok(BallPlacementKind::At(position))
}

fn ball_frozen<'a>(input: &mut Stream<'a>) -> ParseResult<'a, BallPlacementKind> {
    let _ = "frozen".parse_next(input)?;
    let _ = ws1.parse_next(input)?;
    let rail = rail_side.parse_next(input)?;
    let _ = ws1.parse_next(input)?;
    let coord = delimited('(', delimited(hws0, float, hws0), ')').parse_next(input)?;
    Ok(BallPlacementKind::Frozen { rail, coord })
}

fn cue_strike_method_segment<'a>(input: &mut Stream<'a>) -> ParseResult<'a, CueStrikeMethodExpr> {
    let _ = preceded(peek(preceded(chain_ws0, '.')), chain_ws0).parse_next(input)?;
    let _ = '.'.parse_next(input)?;
    cue_strike_method.parse_next(input)
}

fn cue_strike_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, CueStrikeMethodExpr> {
    alt((
        preceded(peek("mass_ratio"), cut_err(cue_strike_mass_ratio_method)),
        preceded(peek("energy_loss"), cut_err(cue_strike_energy_loss_method)),
        preceded(
            peek("endmass_ratio"),
            cut_err(cue_strike_endmass_ratio_method),
        ),
    ))
    .parse_next(input)
}

fn cue_strike_mass_ratio_method<'a>(
    input: &mut Stream<'a>,
) -> ParseResult<'a, CueStrikeMethodExpr> {
    let _ = "mass_ratio".parse_next(input)?;
    let value = delimited('(', delimited(hws0, float, hws0), ')').parse_next(input)?;
    Ok(CueStrikeMethodExpr::MassRatio(value))
}

fn cue_strike_energy_loss_method<'a>(
    input: &mut Stream<'a>,
) -> ParseResult<'a, CueStrikeMethodExpr> {
    let _ = "energy_loss".parse_next(input)?;
    let value = delimited('(', delimited(hws0, float, hws0), ')').parse_next(input)?;
    Ok(CueStrikeMethodExpr::EnergyLoss(value))
}

fn cue_strike_endmass_ratio_method<'a>(
    input: &mut Stream<'a>,
) -> ParseResult<'a, CueStrikeMethodExpr> {
    let _ = "endmass_ratio".parse_next(input)?;
    let value = delimited('(', delimited(hws0, float, hws0), ')').parse_next(input)?;
    Ok(CueStrikeMethodExpr::EndmassRatio(value))
}

fn ball_ball_method_segment<'a>(input: &mut Stream<'a>) -> ParseResult<'a, BallBallMethodExpr> {
    let _ = preceded(peek(preceded(chain_ws0, '.')), chain_ws0).parse_next(input)?;
    let _ = '.'.parse_next(input)?;
    ball_ball_method.parse_next(input)
}

fn ball_ball_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, BallBallMethodExpr> {
    alt((
        preceded(
            peek("normal_restitution"),
            cut_err(ball_ball_normal_restitution_method),
        ),
        preceded(
            peek("tangential_friction"),
            cut_err(ball_ball_tangential_friction_method),
        ),
    ))
    .parse_next(input)
}

fn ball_ball_normal_restitution_method<'a>(
    input: &mut Stream<'a>,
) -> ParseResult<'a, BallBallMethodExpr> {
    let _ = "normal_restitution".parse_next(input)?;
    let value = delimited('(', delimited(hws0, float, hws0), ')').parse_next(input)?;
    Ok(BallBallMethodExpr::NormalRestitution(value))
}

fn ball_ball_tangential_friction_method<'a>(
    input: &mut Stream<'a>,
) -> ParseResult<'a, BallBallMethodExpr> {
    let _ = "tangential_friction".parse_next(input)?;
    let value = delimited('(', delimited(hws0, float, hws0), ')').parse_next(input)?;
    Ok(BallBallMethodExpr::TangentialFriction(value))
}

fn rail_response_method_segment<'a>(
    input: &mut Stream<'a>,
) -> ParseResult<'a, RailResponseMethodExpr> {
    let _ = preceded(peek(preceded(chain_ws0, '.')), chain_ws0).parse_next(input)?;
    let _ = '.'.parse_next(input)?;
    rail_response_method.parse_next(input)
}

fn rail_response_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, RailResponseMethodExpr> {
    alt((
        preceded(
            peek("normal_restitution"),
            cut_err(rail_response_normal_restitution_method),
        ),
        preceded(
            peek("tangential_friction"),
            cut_err(rail_response_tangential_friction_method),
        ),
    ))
    .parse_next(input)
}

fn rail_response_normal_restitution_method<'a>(
    input: &mut Stream<'a>,
) -> ParseResult<'a, RailResponseMethodExpr> {
    let _ = "normal_restitution".parse_next(input)?;
    let value = delimited('(', delimited(hws0, float, hws0), ')').parse_next(input)?;
    Ok(RailResponseMethodExpr::NormalRestitution(value))
}

fn rail_response_tangential_friction_method<'a>(
    input: &mut Stream<'a>,
) -> ParseResult<'a, RailResponseMethodExpr> {
    let _ = "tangential_friction".parse_next(input)?;
    let value = delimited('(', delimited(hws0, float, hws0), ')').parse_next(input)?;
    Ok(RailResponseMethodExpr::TangentialFriction(value))
}

fn rails_method_segment<'a>(input: &mut Stream<'a>) -> ParseResult<'a, RailsMethodExpr> {
    let _ = preceded(peek(preceded(chain_ws0, '.')), chain_ws0).parse_next(input)?;
    let _ = '.'.parse_next(input)?;
    rails_method.parse_next(input)
}

fn rails_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, RailsMethodExpr> {
    alt((
        preceded(peek("default"), cut_err(rails_default_method)),
        preceded(peek("top"), cut_err(rails_top_method)),
        preceded(peek("right"), cut_err(rails_right_method)),
        preceded(peek("bottom"), cut_err(rails_bottom_method)),
        preceded(peek("left"), cut_err(rails_left_method)),
    ))
    .parse_next(input)
}

fn rails_default_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, RailsMethodExpr> {
    let _ = "default".parse_next(input)?;
    let name = delimited('(', delimited(hws0, identifier, hws0), ')').parse_next(input)?;
    Ok(RailsMethodExpr::Default(name.to_string()))
}

fn rails_top_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, RailsMethodExpr> {
    let _ = "top".parse_next(input)?;
    let name = delimited('(', delimited(hws0, identifier, hws0), ')').parse_next(input)?;
    Ok(RailsMethodExpr::Top(name.to_string()))
}

fn rails_right_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, RailsMethodExpr> {
    let _ = "right".parse_next(input)?;
    let name = delimited('(', delimited(hws0, identifier, hws0), ')').parse_next(input)?;
    Ok(RailsMethodExpr::Right(name.to_string()))
}

fn rails_bottom_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, RailsMethodExpr> {
    let _ = "bottom".parse_next(input)?;
    let name = delimited('(', delimited(hws0, identifier, hws0), ')').parse_next(input)?;
    Ok(RailsMethodExpr::Bottom(name.to_string()))
}

fn rails_left_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, RailsMethodExpr> {
    let _ = "left".parse_next(input)?;
    let name = delimited('(', delimited(hws0, identifier, hws0), ')').parse_next(input)?;
    Ok(RailsMethodExpr::Left(name.to_string()))
}

fn simulation_method_segment<'a>(input: &mut Stream<'a>) -> ParseResult<'a, SimulationMethodExpr> {
    let _ = preceded(peek(preceded(chain_ws0, '.')), chain_ws0).parse_next(input)?;
    let _ = '.'.parse_next(input)?;
    simulation_method.parse_next(input)
}

fn simulation_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, SimulationMethodExpr> {
    alt((
        preceded(
            peek("collision_model"),
            cut_err(simulation_collision_model_method),
        ),
        preceded(peek("ball_ball"), cut_err(simulation_ball_ball_method)),
        preceded(peek("rail_model"), cut_err(simulation_rail_model_method)),
        preceded(peek("rails"), cut_err(simulation_rails_method)),
        preceded(peek("conditions"), cut_err(simulation_conditions_method)),
        preceded(peek("max_events"), cut_err(simulation_max_events_method)),
    ))
    .parse_next(input)
}

fn simulation_collision_model_method<'a>(
    input: &mut Stream<'a>,
) -> ParseResult<'a, SimulationMethodExpr> {
    let _ = "collision_model".parse_next(input)?;
    let model =
        delimited('(', delimited(hws0, collision_model_literal, hws0), ')').parse_next(input)?;
    Ok(SimulationMethodExpr::CollisionModel(model))
}

fn simulation_ball_ball_method<'a>(
    input: &mut Stream<'a>,
) -> ParseResult<'a, SimulationMethodExpr> {
    let _ = "ball_ball".parse_next(input)?;
    let name = delimited('(', delimited(hws0, identifier, hws0), ')').parse_next(input)?;
    Ok(SimulationMethodExpr::BallBall(name.to_string()))
}

fn simulation_rail_model_method<'a>(
    input: &mut Stream<'a>,
) -> ParseResult<'a, SimulationMethodExpr> {
    let _ = "rail_model".parse_next(input)?;
    let model = delimited('(', delimited(hws0, rail_model_literal, hws0), ')').parse_next(input)?;
    Ok(SimulationMethodExpr::RailModel(model))
}

fn simulation_rails_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, SimulationMethodExpr> {
    let _ = "rails".parse_next(input)?;
    let name = delimited('(', delimited(hws0, identifier, hws0), ')').parse_next(input)?;
    Ok(SimulationMethodExpr::Rails(name.to_string()))
}

fn simulation_conditions_method<'a>(
    input: &mut Stream<'a>,
) -> ParseResult<'a, SimulationMethodExpr> {
    let _ = "conditions".parse_next(input)?;
    let name = delimited('(', delimited(hws0, identifier, hws0), ')').parse_next(input)?;
    Ok(SimulationMethodExpr::Conditions(name.to_string()))
}

fn simulation_max_events_method<'a>(
    input: &mut Stream<'a>,
) -> ParseResult<'a, SimulationMethodExpr> {
    let _ = "max_events".parse_next(input)?;
    let value = delimited('(', delimited(hws0, usize_literal, hws0), ')').parse_next(input)?;
    Ok(SimulationMethodExpr::MaxEvents(value))
}

fn shot_method_segment<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ParsedShotMethod> {
    let _ = preceded(peek(preceded(chain_ws0, '.')), chain_ws0).parse_next(input)?;
    let _ = '.'.parse_next(input)?;
    shot_method.parse_next(input)
}

fn shot_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ParsedShotMethod> {
    alt((
        preceded(peek("heading"), cut_err(shot_heading_method)),
        preceded(peek("to_pocket"), cut_err(shot_to_pocket_method)),
        preceded(peek("cut_left"), cut_err(shot_cut_left_method)),
        preceded(peek("cut_right"), cut_err(shot_cut_right_method)),
        preceded(peek("pocket"), cut_err(shot_pocket_method)),
        preceded(peek("cut"), cut_err(shot_cut_method)),
        preceded(peek("speed"), cut_err(shot_speed_method)),
        preceded(peek("tip"), cut_err(shot_tip_method)),
        preceded(peek("elevation"), cut_err(shot_elevation_method)),
        preceded(peek("jump"), cut_err(shot_jump_method)),
        preceded(peek("using"), cut_err(shot_using_method)),
    ))
    .parse_next(input)
}

fn shot_heading_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ParsedShotMethod> {
    let method_start = input.current_token_start();
    let _ = "heading".parse_next(input)?;
    let (value, literal) =
        delimited('(', delimited(hws0, located_degrees_literal, hws0), ')').parse_next(input)?;
    Ok(ParsedShotMethod {
        expr: ShotMethodExpr::HeadingDegrees(value),
        source: ParsedShotMethodSource::Aim(ShotAimSource {
            method: ByteSpan::between(method_start, input),
            heading_literal: Some(literal),
        }),
    })
}

fn shot_to_pocket_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ParsedShotMethod> {
    let method_start = input.current_token_start();
    let _ = "to_pocket".parse_next(input)?;
    let expr = shot_pocket_arguments.parse_next(input)?;
    Ok(ParsedShotMethod {
        expr,
        source: ParsedShotMethodSource::Aim(ShotAimSource {
            method: ByteSpan::between(method_start, input),
            heading_literal: None,
        }),
    })
}

fn shot_pocket_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ParsedShotMethod> {
    let method_start = input.current_token_start();
    let _ = "pocket".parse_next(input)?;
    let expr = shot_pocket_arguments.parse_next(input)?;
    Ok(ParsedShotMethod {
        expr,
        source: ParsedShotMethodSource::Aim(ShotAimSource {
            method: ByteSpan::between(method_start, input),
            heading_literal: None,
        }),
    })
}

fn shot_pocket_arguments<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ShotMethodExpr> {
    let (object_ball, pocket) = delimited(
        '(',
        delimited(
            hws0,
            (terminated(ball_ref, (hws0, ',', hws0)), pocket_ref),
            hws0,
        ),
        ')',
    )
    .parse_next(input)?;
    Ok(ShotMethodExpr::ToPocket {
        object_ball,
        pocket,
    })
}

fn shot_cut_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ParsedShotMethod> {
    let method_start = input.current_token_start();
    let _ = "cut".parse_next(input)?;
    let expr = shot_cut_arguments(input)?;
    Ok(ParsedShotMethod {
        expr,
        source: ParsedShotMethodSource::Aim(ShotAimSource {
            method: ByteSpan::between(method_start, input),
            heading_literal: None,
        }),
    })
}

fn shot_cut_left_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ParsedShotMethod> {
    let method_start = input.current_token_start();
    let _ = "cut_left".parse_next(input)?;
    let expr = shot_one_sided_cut_arguments(input, ShotCutDirection::Left)?;
    Ok(ParsedShotMethod {
        expr,
        source: ParsedShotMethodSource::Aim(ShotAimSource {
            method: ByteSpan::between(method_start, input),
            heading_literal: None,
        }),
    })
}

fn shot_cut_right_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ParsedShotMethod> {
    let method_start = input.current_token_start();
    let _ = "cut_right".parse_next(input)?;
    let expr = shot_one_sided_cut_arguments(input, ShotCutDirection::Right)?;
    Ok(ParsedShotMethod {
        expr,
        source: ParsedShotMethodSource::Aim(ShotAimSource {
            method: ByteSpan::between(method_start, input),
            heading_literal: None,
        }),
    })
}

fn shot_cut_arguments<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ShotMethodExpr> {
    let (object_ball, (direction, degrees)) = delimited(
        '(',
        delimited(
            hws0,
            (
                terminated(ball_ref, (hws0, ',', hws0)),
                cut_direction_literal,
            ),
            hws0,
        ),
        ')',
    )
    .parse_next(input)?;
    Ok(ShotMethodExpr::Cut {
        object_ball,
        direction,
        degrees,
    })
}

fn shot_one_sided_cut_arguments<'a>(
    input: &mut Stream<'a>,
    direction: ShotCutDirection,
) -> ParseResult<'a, ShotMethodExpr> {
    let (object_ball, degrees) = delimited(
        '(',
        delimited(
            hws0,
            (
                terminated(ball_ref, (hws0, ',', hws0)),
                cut_angle_degrees_literal,
            ),
            hws0,
        ),
        ')',
    )
    .parse_next(input)?;
    Ok(ShotMethodExpr::Cut {
        object_ball,
        direction,
        degrees,
    })
}

fn shot_speed_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ParsedShotMethod> {
    let _ = "speed".parse_next(input)?;
    let (value, literal) =
        delimited('(', delimited(hws0, located_speed_literal, hws0), ')').parse_next(input)?;
    Ok(ParsedShotMethod {
        expr: ShotMethodExpr::SpeedIps(value),
        source: ParsedShotMethodSource::Speed { literal },
    })
}

fn shot_tip_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ParsedShotMethod> {
    let method_start = input.current_token_start();
    let _ = "tip".parse_next(input)?;
    let ((side, side_literal), (height, height_literal)) = delimited(
        '(',
        delimited(
            hws0,
            (
                preceded(("side", hws0, ':', hws0), located_radius_scale_literal),
                preceded(
                    (hws0, ',', hws0, "height", hws0, ':', hws0),
                    located_radius_scale_literal,
                ),
            ),
            hws0,
        ),
        ')',
    )
    .parse_next(input)?;
    Ok(ParsedShotMethod {
        expr: ShotMethodExpr::Tip { side, height },
        source: ParsedShotMethodSource::Tip(ShotTipSource {
            method: ByteSpan::between(method_start, input),
            side_literal,
            height_literal,
        }),
    })
}

fn shot_elevation_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ParsedShotMethod> {
    let method_start = input.current_token_start();
    let _ = "elevation".parse_next(input)?;
    let (value, literal) =
        delimited('(', delimited(hws0, located_degrees_literal, hws0), ')').parse_next(input)?;
    Ok(ParsedShotMethod {
        expr: ShotMethodExpr::ElevationDegrees(value),
        source: ParsedShotMethodSource::Elevation(ShotElevationSource {
            method: ByteSpan::between(method_start, input),
            literal: Some(literal),
            is_jump: false,
        }),
    })
}

fn shot_jump_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ParsedShotMethod> {
    let method_start = input.current_token_start();
    let _ = "jump".parse_next(input)?;
    let located = delimited(
        '(',
        delimited(hws0, opt(located_degrees_literal), hws0),
        ')',
    )
    .parse_next(input)?;
    let (value, literal) = located
        .map(|(value, literal)| (value, Some(literal)))
        .unwrap_or((DEFAULT_JUMP_CUE_ELEVATION_DEGREES, None));
    Ok(ParsedShotMethod {
        expr: ShotMethodExpr::ElevationDegrees(value),
        source: ParsedShotMethodSource::Elevation(ShotElevationSource {
            method: ByteSpan::between(method_start, input),
            literal,
            is_jump: true,
        }),
    })
}

fn shot_using_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ParsedShotMethod> {
    let _ = "using".parse_next(input)?;
    let name = delimited('(', delimited(hws0, identifier, hws0), ')').parse_next(input)?;
    Ok(ParsedShotMethod {
        expr: ShotMethodExpr::Using(name.to_string()),
        source: ParsedShotMethodSource::Other,
    })
}

fn located_degrees_literal<'a>(input: &mut Stream<'a>) -> ParseResult<'a, (f64, ByteSpan)> {
    let start = input.current_token_start();
    let value = degrees_literal.parse_next(input)?;
    Ok((value, ByteSpan::between(start, input)))
}

fn located_speed_literal<'a>(input: &mut Stream<'a>) -> ParseResult<'a, (f64, ByteSpan)> {
    let start = input.current_token_start();
    let value = speed_literal.parse_next(input)?;
    Ok((value, ByteSpan::between(start, input)))
}

fn located_radius_scale_literal<'a>(input: &mut Stream<'a>) -> ParseResult<'a, (f64, ByteSpan)> {
    let start = input.current_token_start();
    let value = radius_scale_literal.parse_next(input)?;
    Ok((value, ByteSpan::between(start, input)))
}

fn collision_model_literal<'a>(input: &mut Stream<'a>) -> ParseResult<'a, CollisionModel> {
    alt((
        "ideal".map(|_| CollisionModel::Ideal),
        "throw_aware".map(|_| CollisionModel::ThrowAware),
        "spin_friction".map(|_| CollisionModel::SpinFriction),
    ))
    .parse_next(input)
}

fn rail_model_literal<'a>(input: &mut Stream<'a>) -> ParseResult<'a, RailModel> {
    alt((
        "mirror".map(|_| RailModel::Mirror),
        "restitution_only".map(|_| RailModel::RestitutionOnly),
        "spin_aware".map(|_| RailModel::SpinAware),
    ))
    .parse_next(input)
}

fn degrees_literal<'a>(input: &mut Stream<'a>) -> ParseResult<'a, f64> {
    terminated(float, "deg").parse_next(input)
}

fn usize_literal<'a>(input: &mut Stream<'a>) -> ParseResult<'a, usize> {
    let checkpoint = *input;
    let digits: &str = take_while(1.., |c: char| c.is_ascii_digit()).parse_next(input)?;
    digits
        .parse::<usize>()
        .map_err(|_| ErrMode::Backtrack(InputError::at(checkpoint)))
}

fn shot_speed_preset_literal<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ShotSpeedPreset> {
    let checkpoint = *input;
    let name = identifier.parse_next(input)?;
    name.parse::<ShotSpeedPreset>()
        .map_err(|_| ErrMode::Backtrack(InputError::at(checkpoint)))
}

fn speed_literal<'a>(input: &mut Stream<'a>) -> ParseResult<'a, f64> {
    alt((
        terminated(float, "ips"),
        terminated(float, "mph").map(|mph: f64| InchesPerSecond::from_mph(mph).as_f64()),
        terminated(float, "kph")
            .map(|kph: f64| InchesPerSecond::from_mph(kph / 1.609_344).as_f64()),
        shot_speed_preset_literal.map(|preset| preset.inches_per_second().as_f64()),
    ))
    .parse_next(input)
}

fn cut_angle_degrees_literal<'a>(input: &mut Stream<'a>) -> ParseResult<'a, f64> {
    let value = float.parse_next(input)?;
    let _ = opt("deg").parse_next(input)?;
    Ok(value)
}

fn cut_direction_literal<'a>(input: &mut Stream<'a>) -> ParseResult<'a, (ShotCutDirection, f64)> {
    alt((
        preceded(
            peek("left"),
            cut_err(preceded(
                "left",
                delimited('(', delimited(hws0, cut_angle_degrees_literal, hws0), ')')
                    .map(|degrees| (ShotCutDirection::Left, degrees)),
            )),
        ),
        preceded(
            peek("right"),
            cut_err(preceded(
                "right",
                delimited('(', delimited(hws0, cut_angle_degrees_literal, hws0), ')')
                    .map(|degrees| (ShotCutDirection::Right, degrees)),
            )),
        ),
    ))
    .parse_next(input)
}

fn radius_scale_literal<'a>(input: &mut Stream<'a>) -> ParseResult<'a, f64> {
    terminated(float, 'R').parse_next(input)
}

fn rail_side<'a>(input: &mut Stream<'a>) -> ParseResult<'a, RailSide> {
    alt((
        "left".map(|_| RailSide::Left),
        "right".map(|_| RailSide::Right),
        "top".map(|_| RailSide::Top),
        "bottom".map(|_| RailSide::Bottom),
    ))
    .parse_next(input)
}

fn position_expr<'a>(input: &mut Stream<'a>) -> ParseResult<'a, PositionExpr> {
    let _ = hws0.parse_next(input)?;
    let expr = alt((
        coordinate.map(|(x, y)| PositionExpr::Diamond { x, y }),
        identifier.map(|name| match name {
            "center" => PositionExpr::Named(NamedPosition::Center),
            "rack" => PositionExpr::Named(NamedPosition::Rack),
            "top-left" => PositionExpr::Named(NamedPosition::TopLeft),
            "top-right" => PositionExpr::Named(NamedPosition::TopRight),
            "bottom-left" => PositionExpr::Named(NamedPosition::BottomLeft),
            "bottom-right" => PositionExpr::Named(NamedPosition::BottomRight),
            "center-left" => PositionExpr::Named(NamedPosition::CenterLeft),
            "center-right" => PositionExpr::Named(NamedPosition::CenterRight),
            _ => PositionExpr::Alias(name.to_string()),
        }),
    ))
    .parse_next(input)?;
    let _ = hws0.parse_next(input)?;
    Ok(expr)
}

fn coordinate<'a>(input: &mut Stream<'a>) -> ParseResult<'a, (f64, f64)> {
    delimited(
        '(',
        delimited(hws0, (terminated(float, (hws0, ',', hws0)), float), hws0),
        ')',
    )
    .parse_next(input)
}

fn table_ref<'a>(input: &mut Stream<'a>) -> ParseResult<'a, TableRef> {
    alt((
        "brunswick_gc4_9ft".map(|_| TableRef::BrunswickGc4_9ft),
        "three_cushion_carom_10ft".map(|_| TableRef::ThreeCushionCarom10ft),
        "three-cushion-carom-10ft".map(|_| TableRef::ThreeCushionCarom10ft),
    ))
    .parse_next(input)
}

fn game_ref<'a>(input: &mut Stream<'a>) -> ParseResult<'a, GameRef> {
    alt((
        "nine_ball".map(|_| GameRef::NineBall),
        "nine-ball".map(|_| GameRef::NineBall),
        "eight_ball".map(|_| GameRef::EightBall),
        "eight-ball".map(|_| GameRef::EightBall),
        "ten_ball".map(|_| GameRef::TenBall),
        "ten-ball".map(|_| GameRef::TenBall),
        "one_pocket".map(|_| GameRef::OnePocket),
        "one-pocket".map(|_| GameRef::OnePocket),
        "banks".map(|_| GameRef::Banks),
        "three_cushion".map(|_| GameRef::ThreeCushion),
        "three-cushion".map(|_| GameRef::ThreeCushion),
    ))
    .parse_next(input)
}

fn pocket_ref<'a>(input: &mut Stream<'a>) -> ParseResult<'a, Pocket> {
    alt((
        "top-right".map(|_| Pocket::TopRight),
        "center-right".map(|_| Pocket::CenterRight),
        "bottom-right".map(|_| Pocket::BottomRight),
        "bottom-left".map(|_| Pocket::BottomLeft),
        "center-left".map(|_| Pocket::CenterLeft),
        "top-left".map(|_| Pocket::TopLeft),
    ))
    .parse_next(input)
}

fn ball_ref<'a>(input: &mut Stream<'a>) -> ParseResult<'a, BallRef> {
    alt((
        "cue".map(|_| BallRef::Cue),
        "one".map(|_| BallRef::One),
        "two".map(|_| BallRef::Two),
        "three".map(|_| BallRef::Three),
        "four".map(|_| BallRef::Four),
        "five".map(|_| BallRef::Five),
        "six".map(|_| BallRef::Six),
        "seven".map(|_| BallRef::Seven),
        "eight".map(|_| BallRef::Eight),
        "nine".map(|_| BallRef::Nine),
        "yellow".map(|_| BallRef::Yellow),
        "red".map(|_| BallRef::Red),
    ))
    .parse_next(input)
}

fn identifier<'a>(input: &mut Stream<'a>) -> ParseResult<'a, &'a str> {
    take_while(1.., |c: char| {
        c.is_ascii_alphanumeric() || c == '-' || c == '_'
    })
    .parse_next(input)
}

impl std::fmt::Display for CoordinateAxis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoordinateAxis::X => write!(f, "x"),
            CoordinateAxis::Y => write!(f, "y"),
        }
    }
}

impl std::fmt::Display for RailSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RailSide::Left => write!(f, "left"),
            RailSide::Right => write!(f, "right"),
            RailSide::Top => write!(f, "top"),
            RailSide::Bottom => write!(f, "bottom"),
        }
    }
}

impl std::fmt::Display for BallRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BallRef::Cue => write!(f, "cue"),
            BallRef::One => write!(f, "one"),
            BallRef::Two => write!(f, "two"),
            BallRef::Three => write!(f, "three"),
            BallRef::Four => write!(f, "four"),
            BallRef::Five => write!(f, "five"),
            BallRef::Six => write!(f, "six"),
            BallRef::Seven => write!(f, "seven"),
            BallRef::Eight => write!(f, "eight"),
            BallRef::Nine => write!(f, "nine"),
            BallRef::Yellow => write!(f, "yellow"),
            BallRef::Red => write!(f, "red"),
        }
    }
}

impl NamedPosition {
    fn to_position(self) -> Position {
        match self {
            NamedPosition::Center => CENTER_SPOT.clone(),
            NamedPosition::Rack => RACK_SPOT.clone(),
            NamedPosition::TopLeft => TOP_LEFT_DIAMOND.clone(),
            NamedPosition::TopRight => TOP_RIGHT_DIAMOND.clone(),
            NamedPosition::BottomLeft => BOTTOM_LEFT_DIAMOND.clone(),
            NamedPosition::BottomRight => BOTTOM_RIGHT_DIAMOND.clone(),
            NamedPosition::CenterLeft => CENTER_LEFT_DIAMOND.clone(),
            NamedPosition::CenterRight => CENTER_RIGHT_DIAMOND.clone(),
        }
    }
}

impl TableRef {
    fn to_table_spec(self) -> TableSpec {
        match self {
            TableRef::BrunswickGc4_9ft => TableSpec::brunswick_gc4_9ft(),
            TableRef::ThreeCushionCarom10ft => TableSpec::three_cushion_carom_10ft(),
        }
    }
}

impl GameRef {
    fn to_game_type(self) -> GameType {
        match self {
            GameRef::NineBall => GameType::NineBall,
            GameRef::EightBall => GameType::EightBall,
            GameRef::TenBall => GameType::TenBall,
            GameRef::OnePocket => GameType::OnePocket,
            GameRef::Banks => GameType::Banks,
            GameRef::ThreeCushion => GameType::ThreeCushion,
        }
    }
}
impl BallRef {
    fn to_ball_type(self) -> BallType {
        match self {
            BallRef::Cue => BallType::Cue,
            BallRef::One => BallType::One,
            BallRef::Two => BallType::Two,
            BallRef::Three => BallType::Three,
            BallRef::Four => BallType::Four,
            BallRef::Five => BallType::Five,
            BallRef::Six => BallType::Six,
            BallRef::Seven => BallType::Seven,
            BallRef::Eight => BallType::Eight,
            BallRef::Nine => BallType::Nine,
            BallRef::Yellow => BallType::YellowCue,
            BallRef::Red => BallType::Red,
        }
    }
}

impl RailSide {
    fn to_rail(self) -> Rail {
        match self {
            RailSide::Left => Rail::Left,
            RailSide::Right => Rail::Right,
            RailSide::Top => Rail::Top,
            RailSide::Bottom => Rail::Bottom,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MotionPhase;
    use crate::TYPICAL_BALL_RADIUS;

    #[test]
    fn parse_ball_at_coordinate() {
        let dsl = "ball cue at (2, 4)";
        let doc = parse_dsl(dsl).expect("parse");
        assert_eq!(doc.entries.len(), 1);
    }

    #[test]
    fn parse_alias_then_ball() {
        let dsl = "pos spot = (1, 2)\nball eight at spot";
        let doc = parse_dsl(dsl).expect("parse");
        let game_state = build_game_state(&doc).expect("build");
        assert_eq!(game_state.balls().len(), 1);
    }

    #[test]
    fn parse_named_position() {
        let dsl = "ball nine at center";
        let doc = parse_dsl(dsl).expect("parse");
        let game_state = build_game_state(&doc).expect("build");
        assert_eq!(game_state.balls().len(), 1);
    }

    #[test]
    fn parse_frozen_ball() {
        let dsl = "ball cue frozen left (6)";
        let doc = parse_dsl(dsl).expect("parse");
        let game_state = build_game_state(&doc).expect("build");
        assert_eq!(game_state.balls().len(), 1);
    }

    #[test]
    fn parse_shot_scenario() {
        let dsl = "ball cue at center\n\
                   cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
                   shot(cue)\n\
                     .heading(30deg)\n\
                     .speed(128ips)\n\
                     .tip(side: 0.0R, height: 0.4R)\n\
                     .using(default)";
        let scenario = parse_dsl_to_scenario(dsl).expect("build scenario");
        let seeded = scenario
            .strike_shot_on_table(&BallSetPhysicsSpec::default())
            .expect("strike shot")
            .expect("shot present");

        assert_eq!(scenario.game_state.balls().len(), 1);
        assert_eq!(scenario.shot.as_ref().expect("shot").ball_ref, BallRef::Cue);
        assert_eq!(
            seeded
                .as_ball_state()
                .motion_phase(TYPICAL_BALL_RADIUS.clone()),
            MotionPhase::Rolling
        );
    }

    #[test]
    fn parse_ball_ball_config() {
        let scenario = parse_dsl_to_scenario(
            "ball_ball(human).normal_restitution(0.95).tangential_friction(0.06)",
        )
        .expect("build scenario");

        assert_eq!(scenario.ball_ball_configs.len(), 1);
        assert_eq!(
            scenario
                .ball_ball_config_named("human")
                .expect("named config")
                .normal_restitution
                .as_f64(),
            0.95
        );
    }

    #[test]
    fn parse_rail_profile() {
        let scenario = parse_dsl_to_scenario(
            "rail_response(clean).normal_restitution(0.8).tangential_friction(1.0)\n\
             rail_response(dead).normal_restitution(0.6).tangential_friction(1.0)\n\
             rails(bank).default(clean).top(dead)",
        )
        .expect("build scenario");

        assert_eq!(scenario.rail_profiles.len(), 1);
        assert_eq!(
            scenario
                .rail_profile_named("bank")
                .expect("named rail profile")
                .top
                .normal_restitution
                .as_f64(),
            0.6
        );
    }

    #[test]
    fn parse_simulation_preset() {
        let scenario = parse_dsl_to_scenario(
            "ball_ball(ideal).normal_restitution(1.0).tangential_friction(0.06)\n\
             rail_response(clean).normal_restitution(0.8).tangential_friction(1.0)\n\
             rails(table).default(clean)\n\
             simulation(match).collision_model(throw_aware).ball_ball(ideal).rail_model(spin_aware).rails(table)",
        )
        .expect("build scenario");

        let preset = scenario
            .simulation_named("match")
            .expect("named simulation");
        assert_eq!(preset.collision_model, CollisionModel::ThrowAware);
        assert_eq!(preset.ball_ball_name, "ideal");
        assert_eq!(preset.rails_name, "table");
        assert_eq!(preset.rail_model, RailModel::SpinAware);
        assert_eq!(preset.conditions, PlayingConditions::neutral());
    }

    #[test]
    fn rejects_unknown_alias() {
        let dsl = "ball eight at spot";
        let doc = parse_dsl(dsl).expect("parse");
        let err = build_game_state(&doc).expect_err("build");
        assert!(matches!(err, DslBuildError::UnknownAlias(_)));
    }
}
