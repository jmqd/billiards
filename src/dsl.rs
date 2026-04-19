use std::collections::HashMap;

use crate::{
    advance_motion_on_table, advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table,
    simulate_n_balls_with_physics_and_pockets_on_table_until_rest, strike_resting_ball_on_table,
    trace_ball_path_with_rail_profile_on_table,
    visualization::{EventMarkerStyle, GhostBallStyle, LabelOverlayStyle, PathColorMode},
    Angle, Ball, BallBallCollisionConfig, BallPath, BallPathSegment, BallPathStop,
    BallSetPhysicsSpec, BallSpec, BallState, BallType, CollisionModel, CueStrikeConfig,
    CueTipContact, Diamond, GameState, HumanShotSpeedValidation, Inches, InchesPerSecond,
    MotionPhase, NBallSystemEvent, NBallSystemSimulation, NBallSystemState, OnTableBallState,
    OnTableMotionConfig, Pocket, Position, Rail, RailCollisionConfig, RailCollisionProfile,
    RailModel, RestingOnTableBallState, Scale, Seconds, Shot, ShotError, TableSpec,
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

#[derive(Debug, Clone, PartialEq)]
pub struct DslDoc {
    pub table: Option<TableRef>,
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
    pub ball_ball_configs: HashMap<String, BallBallCollisionConfig>,
    pub rail_responses: HashMap<String, RailCollisionConfig>,
    pub rail_profiles: HashMap<String, RailCollisionProfile>,
    pub simulations: HashMap<String, SimulationPreset>,
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

    pub fn preferred_simulation_name(&self) -> Option<&str> {
        if self.simulations.contains_key("default") {
            Some("default")
        } else if self.simulations.len() == 1 {
            self.simulations.keys().next().map(String::as_str)
        } else {
            None
        }
    }

    pub fn simulate_shot_system_with_simulation_on_table_until_rest(
        &self,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        simulation_name: &str,
    ) -> Result<Option<NBallSystemSimulation>, DslBuildError> {
        let simulation = self.simulation_named(simulation_name)?;
        self.simulate_shot_system_with_physics_on_table_until_rest(
            ball_set,
            motion,
            simulation.collision_model,
            self.ball_ball_config_named(&simulation.ball_ball_name)?,
            simulation.rail_model,
            self.rail_profile_named(&simulation.rails_name)?,
        )
    }

    pub fn simulate_shot_trace_with_simulation_on_table_until_rest(
        &self,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        simulation_name: &str,
    ) -> Result<Option<ScenarioShotTrace>, DslBuildError> {
        let simulation = self.simulation_named(simulation_name)?;
        self.simulate_shot_trace_with_physics_on_table_until_rest(
            ball_set,
            motion,
            simulation.collision_model,
            self.ball_ball_config_named(&simulation.ball_ball_name)?,
            simulation.rail_model,
            self.rail_profile_named(&simulation.rails_name)?,
        )
    }

    pub fn trace_shot_path_with_simulation_on_table(
        &self,
        stop: BallPathStop,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        simulation_name: &str,
    ) -> Result<Option<BallPath>, DslBuildError> {
        let simulation = self.simulation_named(simulation_name)?;
        self.trace_shot_path_with_rail_profile_on_table(
            stop,
            ball_set,
            motion,
            simulation.rail_model,
            self.rail_profile_named(&simulation.rails_name)?,
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

    pub fn strike_shot_on_table(
        &self,
        ball_set: &BallSetPhysicsSpec,
    ) -> Result<Option<OnTableBallState>, DslBuildError> {
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

        strike_resting_ball_on_table(&resting, &shot.shot, &shot.cue_strike, ball_set)
            .map(Some)
            .map_err(DslBuildError::InvalidShot)
    }

    pub fn initial_shot_system_states_on_table(
        &self,
        ball_set: &BallSetPhysicsSpec,
    ) -> Result<Option<Vec<OnTableBallState>>, DslBuildError> {
        let Some(shot) = &self.shot else {
            return Ok(None);
        };
        let shot_target_index = self
            .game_state
            .balls()
            .iter()
            .position(|ball| ball.ty == shot.ball)
            .ok_or(DslBuildError::ShotTargetBallNotPlaced(shot.ball_ref))?;
        let mut states = Vec::with_capacity(self.game_state.balls().len());

        for (ball_index, game_ball) in self.game_state.balls().iter().enumerate() {
            let resting = RestingOnTableBallState::try_from(BallState::from_position(
                &game_ball.position,
                &self.game_state.table_spec,
            ))
            .expect(
                "game-state ball placements should always correspond to resting on-table states",
            );
            let state = if ball_index == shot_target_index {
                strike_resting_ball_on_table(&resting, &shot.shot, &shot.cue_strike, ball_set)
                    .map_err(DslBuildError::InvalidShot)?
            } else {
                resting.into_on_table_ball_state()
            };
            states.push(state);
        }

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

        Ok(Some(
            simulate_n_balls_with_physics_and_pockets_on_table_until_rest(
                &states,
                ball_set,
                &self.game_state.table_spec,
                motion,
                collision_model,
                collision_config,
                rail_model,
                rail_profile,
            ),
        ))
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
        let Some(initial_states) = self.initial_shot_system_states_on_table(ball_set)? else {
            return Ok(None);
        };
        let simulation = simulate_n_balls_with_physics_and_pockets_on_table_until_rest(
            &initial_states,
            ball_set,
            &self.game_state.table_spec,
            motion,
            collision_model,
            collision_config,
            rail_model,
            rail_profile,
        );
        let initial_system_states = initial_states
            .iter()
            .cloned()
            .map(NBallSystemState::from)
            .collect::<Vec<_>>();
        let event_log = scenario_event_log_from_simulation(&simulation, self.game_state.balls());
        let ball_traces = self.ball_traces_from_simulation(
            &initial_system_states,
            &simulation,
            ball_set,
            motion,
            collision_model,
            collision_config,
            rail_model,
            rail_profile,
        );

        Ok(Some(ScenarioShotTrace {
            simulation,
            event_log,
            ball_traces,
        }))
    }

    pub fn simulate_shot_trace_with_rails_and_pockets_on_table_until_rest(
        &self,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        collision_model: CollisionModel,
        rail_model: RailModel,
    ) -> Result<Option<ScenarioShotTrace>, DslBuildError> {
        self.simulate_shot_trace_with_physics_on_table_until_rest(
            ball_set,
            motion,
            collision_model,
            &BallBallCollisionConfig::human_tuned(),
            rail_model,
            &RailCollisionProfile::default(),
        )
    }

    pub fn simulate_shot_trace_with_preferred_physics_on_table_until_rest(
        &self,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
        collision_model: CollisionModel,
        rail_model: RailModel,
    ) -> Result<Option<ScenarioShotTrace>, DslBuildError> {
        if let Some(simulation_name) = self.preferred_simulation_name() {
            self.simulate_shot_trace_with_simulation_on_table_until_rest(
                ball_set,
                motion,
                simulation_name,
            )
        } else {
            self.simulate_shot_trace_with_rails_and_pockets_on_table_until_rest(
                ball_set,
                motion,
                collision_model,
                rail_model,
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
    ) -> Vec<ScenarioBallTrace> {
        let mut current_states = initial_states.to_vec();
        let mut traces = self
            .game_state
            .balls()
            .iter()
            .zip(initial_states)
            .map(|(ball, state)| ScenarioBallTrace {
                ball: ball.ty.clone(),
                initial_state: state
                    .as_on_table()
                    .expect("initial shot trace states should be on-table")
                    .clone(),
                final_state: state.clone(),
                segments: Vec::new(),
            })
            .collect::<Vec<_>>();

        for event in &simulation.events {
            let step_time = event.time();
            for (trace, state) in traces.iter_mut().zip(&current_states) {
                let Some(start) = state.as_on_table() else {
                    continue;
                };
                let end = OnTableBallState::try_from(
                    advance_motion_on_table(start, step_time, ball_set, motion).state,
                )
                .expect("shot trace sub-advance should preserve on-table invariants");
                push_visible_trace_segment(&mut trace.segments, start, &end, step_time);
            }

            let advanced = advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table(
                &current_states,
                ball_set,
                &self.game_state.table_spec,
                motion,
                collision_model,
                collision_config,
                rail_model,
                rail_profile,
            );
            current_states = advanced.states;
        }

        for (trace, final_state) in traces.iter_mut().zip(&simulation.states) {
            trace.final_state = final_state.clone();
        }

        traces
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

        Ok(Some(trace_ball_path_with_rail_profile_on_table(
            &initial_state,
            stop,
            ball_set,
            &self.game_state.table_spec,
            motion,
            rail_model,
            rail_profile,
        )))
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioTraceRenderOptions {
    pub max_time_step: Seconds,
    pub line_width_px: f32,
    pub start_ghost_balls: bool,
    pub event_markers: bool,
    pub labels: bool,
    pub path_color_mode: PathColorMode,
}

impl Default for ScenarioTraceRenderOptions {
    fn default() -> Self {
        Self {
            max_time_step: Seconds::new(0.02),
            line_width_px: 3.0,
            start_ghost_balls: false,
            event_markers: false,
            labels: false,
            path_color_mode: PathColorMode::Solid,
        }
    }
}

impl ScenarioTraceRenderOptions {
    pub fn rich_defaults() -> Self {
        Self {
            start_ghost_balls: true,
            event_markers: true,
            ..Self::default()
        }
    }
}

impl ScenarioShotTrace {
    pub fn event_lines(&self) -> Vec<String> {
        self.event_log
            .iter()
            .map(ScenarioShotTraceEvent::format_human)
            .collect()
    }

    pub fn rendered_final_layout_with_traces(
        &self,
        scenario: &DslScenario,
        max_time_step: Seconds,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
    ) -> GameState {
        self.rendered_final_layout_with_trace_options(
            scenario,
            &ScenarioTraceRenderOptions {
                max_time_step,
                ..ScenarioTraceRenderOptions::default()
            },
            ball_set,
            motion,
        )
    }

    pub fn rendered_final_layout_with_trace_options(
        &self,
        scenario: &DslScenario,
        options: &ScenarioTraceRenderOptions,
        ball_set: &BallSetPhysicsSpec,
        motion: &OnTableMotionConfig,
    ) -> GameState {
        let mut game_state = scenario.game_state_for_system_states(&self.simulation.states);
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

            if let Some(path) = ball_trace.as_ball_path() {
                game_state.add_smooth_ball_path_styled(
                    &path,
                    options.max_time_step,
                    ball_set,
                    motion,
                    options.line_width_px,
                    &path_style,
                );
            }

            if let Some(pocket_terminal) = ball_trace.pocket_terminal_point() {
                let capture_point = match &ball_trace.final_state {
                    NBallSystemState::Pocketed {
                        state_at_capture, ..
                    } => state_at_capture
                        .as_ball_state()
                        .projected_position(&scenario.game_state.table_spec),
                    NBallSystemState::OnTable(_) => continue,
                };
                if capture_point != pocket_terminal {
                    game_state.add_smooth_polyline_with_width(
                        &[capture_point, pocket_terminal],
                        options.line_width_px,
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
        format!("t={:.3}  {}", self.time.as_f64(), self.kind.format_human())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScenarioShotTraceEventKind {
    BallBallCollision {
        first_ball: BallType,
        second_ball: BallType,
    },
    BallPocketCapture {
        ball: BallType,
        pocket: Pocket,
    },
    BallRailImpact {
        ball: BallType,
        rail: Rail,
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
            ScenarioShotTraceEventKind::BallPocketCapture { ball, pocket } => format!(
                "{} pocketed in {}",
                ball_type_name(ball),
                pocket_name(*pocket)
            ),
            ScenarioShotTraceEventKind::BallRailImpact { ball, rail } => {
                format!("{} rail impact: {}", ball_type_name(ball), rail_name(*rail))
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
    pub initial_state: OnTableBallState,
    pub final_state: NBallSystemState,
    pub segments: Vec<BallPathSegment>,
}

impl ScenarioBallTrace {
    fn pocket_terminal_point(&self) -> Option<Position> {
        match &self.final_state {
            NBallSystemState::Pocketed { pocket, .. } => Some(pocket.aiming_center()),
            NBallSystemState::OnTable(_) => None,
        }
    }

    fn elapsed(&self) -> Seconds {
        Seconds::new(
            self.segments
                .iter()
                .map(|segment| segment.duration.as_f64())
                .sum(),
        )
    }

    fn as_ball_path(&self) -> Option<BallPath> {
        let final_state = match &self.final_state {
            NBallSystemState::OnTable(state) => state.clone(),
            NBallSystemState::Pocketed {
                state_at_capture, ..
            } => state_at_capture.clone(),
        };

        (!self.segments.is_empty()).then(|| BallPath {
            initial_state: self.initial_state.clone(),
            final_state,
            elapsed: self.elapsed(),
            rail_impacts: 0,
            segments: self.segments.clone(),
        })
    }

    pub fn projected_points(&self, table_spec: &TableSpec) -> Vec<Position> {
        let mut points = self.as_ball_path().map_or_else(
            || {
                vec![self
                    .initial_state
                    .as_ball_state()
                    .projected_position(table_spec)]
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
            || {
                vec![self
                    .initial_state
                    .as_ball_state()
                    .projected_position(table_spec)]
            },
            |path| path.sampled_points(max_time_step, ball, motion, table_spec),
        );
        if let Some(pocket_terminal) = self.pocket_terminal_point() {
            if points.last() != Some(&pocket_terminal) {
                points.push(pocket_terminal);
            }
        }
        points
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
) {
    if trace_segment_has_visible_displacement(start, end) {
        segments.push(BallPathSegment {
            start: start.clone(),
            end: end.clone(),
            duration,
        });
    }
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

fn ball_trace_color(ball: &BallType) -> Rgba<u8> {
    match ball {
        BallType::Cue => Rgba([225, 225, 225, 255]),
        BallType::One => Rgba([255, 215, 0, 255]),
        BallType::Two => Rgba([65, 105, 225, 255]),
        BallType::Three => Rgba([220, 20, 60, 255]),
        BallType::Four => Rgba([138, 43, 226, 255]),
        BallType::Five => Rgba([255, 140, 0, 255]),
        BallType::Six => Rgba([34, 139, 34, 255]),
        BallType::Seven => Rgba([128, 0, 0, 255]),
        BallType::Eight => Rgba([32, 32, 32, 255]),
        BallType::Nine => Rgba([255, 215, 0, 255]),
    }
}

fn ball_trace_ghost_style(color: Rgba<u8>) -> GhostBallStyle {
    GhostBallStyle {
        fill_color: Rgba([color[0], color[1], color[2], 64]),
        outline_color: Rgba([color[0], color[1], color[2], 160]),
        ..GhostBallStyle::default()
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimulationPreset {
    pub collision_model: CollisionModel,
    pub ball_ball_name: String,
    pub rail_model: RailModel,
    pub rails_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShotDef {
    pub ball: BallRef,
    pub methods: Vec<ShotMethodExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShotMethodExpr {
    HeadingDegrees(f64),
    SpeedIps(f64),
    Tip { side: f64, height: f64 },
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
    MissingShotMethod {
        method: String,
    },
    UnknownCueStrike(String),
    UnknownBallBallConfig(String),
    UnknownRailResponse(String),
    UnknownRailProfile(String),
    UnknownSimulation(String),
    MultipleShotsNotSupported {
        count: usize,
    },
    ShotTargetMustBeCueBall(BallRef),
    ShotTargetBallNotPlaced(BallRef),
    InvalidCueStrikeConfig {
        name: String,
        error: ShotError,
    },
    InvalidShot(ShotError),
}

impl std::fmt::Display for DslBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownAlias(name) => write!(f, "unknown alias '{name}'"),
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
            Self::MissingShotMethod { method } => {
                write!(f, "shot is missing .{method}(...)")
            }
            Self::UnknownCueStrike(name) => write!(f, "unknown cue_strike '{name}'"),
            Self::UnknownBallBallConfig(name) => write!(f, "unknown ball_ball '{name}'"),
            Self::UnknownRailResponse(name) => write!(f, "unknown rail_response '{name}'"),
            Self::UnknownRailProfile(name) => write!(f, "unknown rails '{name}'"),
            Self::UnknownSimulation(name) => write!(f, "unknown simulation '{name}'"),
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
            Self::InvalidCueStrikeConfig { name, error } => {
                write!(f, "cue_strike '{name}' is invalid: {error:?}")
            }
            Self::InvalidShot(error) => write!(f, "invalid shot: {error:?}"),
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

type Stream<'i> = LocatingSlice<&'i str>;

type ParseError<'i> = ErrMode<InputError<Stream<'i>>>;

type ParseResult<'i, T> = Result<T, ParseError<'i>>;

fn parse_dsl_inner(input: &str) -> ParseResult<'_, DslDoc> {
    let mut stream = LocatingSlice::new(input);
    dsl_doc.parse_next(&mut stream)
}

pub fn parse_dsl(input: &str) -> Result<DslDoc, DslParseError> {
    parse_dsl_inner(input).map_err(parse_error)
}

pub fn parse_dsl_to_game_state(input: &str) -> Result<GameState, DslError> {
    let doc = parse_dsl(input).map_err(DslError::Parse)?;
    build_game_state(&doc).map_err(DslError::Build)
}

pub fn parse_dsl_to_scenario(input: &str) -> Result<DslScenario, DslError> {
    let doc = parse_dsl(input).map_err(DslError::Parse)?;
    build_scenario(&doc).map_err(DslError::Build)
}

pub fn build_game_state(doc: &DslDoc) -> Result<GameState, DslBuildError> {
    build_scenario(doc).map(|scenario| scenario.game_state)
}

pub fn build_scenario(doc: &DslDoc) -> Result<DslScenario, DslBuildError> {
    let table_spec = match doc.table.unwrap_or(TableRef::BrunswickGc4_9ft) {
        TableRef::BrunswickGc4_9ft => TableSpec::brunswick_gc4_9ft(),
    };

    let mut game_state = GameState::new(table_spec);
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
                let resolved = resolve_position_expr(&aliases, &alias.position)?;
                aliases.insert(alias.name.clone(), resolved);
            }
            DslEntry::Ball(placement) => match placement {
                BallPlacement::At { ball, position } => {
                    let pos = resolve_position_expr(&aliases, position)?;
                    game_state.add_ball(Ball {
                        ty: ball.to_ball_type(),
                        position: pos,
                        spec: BallSpec::default(),
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
                            ..Default::default()
                        },
                    );
                }
            },
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

    CueStrikeConfig::new(cue_mass_ratio, collision_energy_loss).map_err(|error| {
        DslBuildError::InvalidCueStrikeConfig {
            name: def.name.clone(),
            error,
        }
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

    Ok(BallBallCollisionConfig {
        normal_restitution: validate_unit_interval_physics_value(
            PhysicsConfigKind::BallBall,
            &def.name,
            "normal_restitution",
            normal_restitution,
        )?,
        tangential_friction_coefficient: validate_non_negative_physics_value(
            PhysicsConfigKind::BallBall,
            &def.name,
            "tangential_friction",
            tangential_friction,
        )?,
    })
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

    Ok(RailCollisionConfig {
        normal_restitution: validate_unit_interval_physics_value(
            PhysicsConfigKind::RailResponse,
            &def.name,
            "normal_restitution",
            normal_restitution,
        )?,
        tangential_friction_coefficient: validate_non_negative_physics_value(
            PhysicsConfigKind::RailResponse,
            &def.name,
            "tangential_friction",
            tangential_friction,
        )?,
    })
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
    })
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

    let mut heading_degrees = None;
    let mut speed_ips = None;
    let mut tip = None;
    let mut cue_strike_name = None;

    for method in &def.methods {
        match method {
            ShotMethodExpr::HeadingDegrees(value) => {
                set_once(&mut heading_degrees, *value, || {
                    DslBuildError::DuplicateShotMethod {
                        method: "heading".to_string(),
                    }
                })?;
            }
            ShotMethodExpr::SpeedIps(value) => {
                set_once(&mut speed_ips, *value, || {
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
            ShotMethodExpr::Using(name) => {
                set_once(&mut cue_strike_name, name.clone(), || {
                    DslBuildError::DuplicateShotMethod {
                        method: "using".to_string(),
                    }
                })?;
            }
        }
    }

    let heading_degrees = heading_degrees.ok_or_else(|| DslBuildError::MissingShotMethod {
        method: "heading".to_string(),
    })?;
    let speed_ips = speed_ips.ok_or_else(|| DslBuildError::MissingShotMethod {
        method: "speed".to_string(),
    })?;
    let (side, height) = tip.ok_or_else(|| DslBuildError::MissingShotMethod {
        method: "tip".to_string(),
    })?;
    let cue_strike_name = cue_strike_name.ok_or_else(|| DslBuildError::MissingShotMethod {
        method: "using".to_string(),
    })?;

    let cue_strike = cue_strikes
        .get(&cue_strike_name)
        .cloned()
        .ok_or(DslBuildError::UnknownCueStrike(cue_strike_name))?;
    let angle_radians = heading_degrees.to_radians();
    let tip_contact = CueTipContact::new(Scale::from_f64(side), Scale::from_f64(height))
        .map_err(DslBuildError::InvalidShot)?;
    let shot = Shot::new(
        Angle::from_north(angle_radians.sin(), angle_radians.cos()),
        InchesPerSecond::new(Inches::from_f64(speed_ips)),
        tip_contact,
    )
    .map_err(DslBuildError::InvalidShot)?;

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

fn dsl_doc<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslDoc> {
    let mut doc = DslDoc {
        table: None,
        entries: Vec::new(),
    };

    repeat(0.., statement)
        .fold(
            || (),
            |(), entry| {
                match entry {
                    DslStatement::Table(table) => doc.table = Some(table),
                    DslStatement::Alias(alias) => doc.entries.push(DslEntry::Alias(alias)),
                    DslStatement::Ball(placement) => doc.entries.push(DslEntry::Ball(placement)),
                    DslStatement::CueStrike(def) => doc.entries.push(DslEntry::CueStrike(def)),
                    DslStatement::BallBall(def) => doc.entries.push(DslEntry::BallBall(def)),
                    DslStatement::RailResponse(def) => {
                        doc.entries.push(DslEntry::RailResponse(def))
                    }
                    DslStatement::Rails(def) => doc.entries.push(DslEntry::Rails(def)),
                    DslStatement::Simulation(def) => doc.entries.push(DslEntry::Simulation(def)),
                    DslStatement::Shot(def) => doc.entries.push(DslEntry::Shot(def)),
                    DslStatement::Empty => {}
                }
                ()
            },
        )
        .parse_next(input)?;

    let _ = terminated(hws0, eof).parse_next(input)?;

    Ok(doc)
}

#[derive(Debug, Clone, PartialEq)]
enum DslStatement {
    Table(TableRef),
    Alias(AliasDef),
    Ball(BallPlacement),
    CueStrike(CueStrikeDef),
    BallBall(BallBallDef),
    RailResponse(RailResponseDef),
    Rails(RailsDef),
    Simulation(SimulationDef),
    Shot(ShotDef),
    Empty,
}

fn statement<'a>(input: &mut Stream<'a>) -> ParseResult<'a, DslStatement> {
    let _ = hws0.parse_next(input)?;
    let stmt = alt((
        comment_line,
        blank_line,
        preceded(peek("table"), cut_err(table_stmt)),
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
    Ok(stmt)
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
    let methods = repeat(0.., shot_method_segment).parse_next(input)?;
    Ok(DslStatement::Shot(ShotDef { ball, methods }))
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

fn shot_method_segment<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ShotMethodExpr> {
    let _ = preceded(peek(preceded(chain_ws0, '.')), chain_ws0).parse_next(input)?;
    let _ = '.'.parse_next(input)?;
    shot_method.parse_next(input)
}

fn shot_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ShotMethodExpr> {
    alt((
        preceded(peek("heading"), cut_err(shot_heading_method)),
        preceded(peek("speed"), cut_err(shot_speed_method)),
        preceded(peek("tip"), cut_err(shot_tip_method)),
        preceded(peek("using"), cut_err(shot_using_method)),
    ))
    .parse_next(input)
}

fn shot_heading_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ShotMethodExpr> {
    let _ = "heading".parse_next(input)?;
    let value = delimited('(', delimited(hws0, degrees_literal, hws0), ')').parse_next(input)?;
    Ok(ShotMethodExpr::HeadingDegrees(value))
}

fn shot_speed_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ShotMethodExpr> {
    let _ = "speed".parse_next(input)?;
    let value = delimited('(', delimited(hws0, speed_literal, hws0), ')').parse_next(input)?;
    Ok(ShotMethodExpr::SpeedIps(value))
}

fn shot_tip_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ShotMethodExpr> {
    let _ = "tip".parse_next(input)?;
    let (side, height) = delimited(
        '(',
        delimited(
            hws0,
            (
                preceded(("side", hws0, ':', hws0), radius_scale_literal),
                preceded(
                    (hws0, ',', hws0, "height", hws0, ':', hws0),
                    radius_scale_literal,
                ),
            ),
            hws0,
        ),
        ')',
    )
    .parse_next(input)?;
    Ok(ShotMethodExpr::Tip { side, height })
}

fn shot_using_method<'a>(input: &mut Stream<'a>) -> ParseResult<'a, ShotMethodExpr> {
    let _ = "using".parse_next(input)?;
    let name = delimited('(', delimited(hws0, identifier, hws0), ')').parse_next(input)?;
    Ok(ShotMethodExpr::Using(name.to_string()))
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

fn speed_literal<'a>(input: &mut Stream<'a>) -> ParseResult<'a, f64> {
    terminated(float, "ips").parse_next(input)
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
        named_position.map(PositionExpr::Named),
        identifier.map(|name| PositionExpr::Alias(name.to_string())),
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

fn named_position<'a>(input: &mut Stream<'a>) -> ParseResult<'a, NamedPosition> {
    alt((
        "center".map(|_| NamedPosition::Center),
        "rack".map(|_| NamedPosition::Rack),
        "top-left".map(|_| NamedPosition::TopLeft),
        "top-right".map(|_| NamedPosition::TopRight),
        "bottom-left".map(|_| NamedPosition::BottomLeft),
        "bottom-right".map(|_| NamedPosition::BottomRight),
        "center-left".map(|_| NamedPosition::CenterLeft),
        "center-right".map(|_| NamedPosition::CenterRight),
    ))
    .parse_next(input)
}

fn table_ref<'a>(input: &mut Stream<'a>) -> ParseResult<'a, TableRef> {
    alt(("brunswick_gc4_9ft".map(|_| TableRef::BrunswickGc4_9ft),)).parse_next(input)
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
    }

    #[test]
    fn rejects_unknown_alias() {
        let dsl = "ball eight at spot";
        let doc = parse_dsl(dsl).expect("parse");
        let err = build_game_state(&doc).expect_err("build");
        assert!(matches!(err, DslBuildError::UnknownAlias(_)));
    }
}
