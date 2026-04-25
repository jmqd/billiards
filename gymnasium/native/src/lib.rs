use billiards::dsl::{BallRef, DslScenario, ScenarioShot, ScenarioTraceRenderOptions};
use billiards::visualization::{BallPathRenderOptions, PathColorMode};
use billiards::{
    human_tuned_preview_motion_config,
    simulate_n_balls_with_physics_and_pockets_on_table_until_rest, strike_resting_ball_on_table,
    Angle, Ball, BallBallCollisionConfig, BallSetPhysicsSpec, BallSpec, BallState, BallType,
    CollisionModel, CueStrikeConfig, CueTipContact, DiagramBackground, DiagramRenderOptions,
    GameState, Inches, InchesPerSecond, NBallSystemEvent, NBallSystemSimulation,
    NBallSystemState, OnTableBallState, Pocket, Position, Rail, RailCollisionProfile, RailModel,
    RestingOnTableBallState, Scale, Seconds, Shot, TableSpec,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const ABSENT_BALL_ID: u8 = 255;
const STANDARD_BALL_COUNT: usize = 10;

#[derive(Debug, Deserialize)]
struct SimRequest {
    balls: Vec<BallInput>,
    shot: ShotInput,
    #[serde(default)]
    config: SimConfig,
}

#[derive(Debug, Deserialize)]
struct RenderBoardRequest {
    balls: Vec<RenderBallInput>,
    #[serde(default)]
    render: RenderOptionsInput,
}

#[derive(Debug, Deserialize)]
struct RenderShotTraceRequest {
    balls: Vec<BallInput>,
    shot: ShotInput,
    #[serde(default)]
    config: SimConfig,
    #[serde(default)]
    render: RenderOptionsInput,
}

#[derive(Debug, Deserialize)]
struct BallInput {
    ball: String,
    x: f64,
    y: f64,
    #[serde(default = "default_units")]
    units: String,
}

#[derive(Debug, Deserialize)]
struct RenderBallInput {
    ball: String,
    x: f64,
    y: f64,
    #[serde(default)]
    state: Option<String>,
    #[serde(default = "default_units")]
    units: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ShotInput {
    heading_degrees: f64,
    speed_ips: f64,
    #[serde(default)]
    tip_side_r: f64,
    #[serde(default)]
    tip_height_r: f64,
    #[serde(default = "default_speed_semantics")]
    speed_semantics: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SimConfig {
    #[serde(default = "default_cue_mass_ratio")]
    cue_mass_ratio: f64,
    #[serde(default = "default_collision_energy_loss")]
    collision_energy_loss: f64,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            cue_mass_ratio: default_cue_mass_ratio(),
            collision_energy_loss: default_collision_energy_loss(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RenderOptionsInput {
    #[serde(default = "default_scale_factor")]
    scale_factor: u32,
    #[serde(default)]
    transparent_background: bool,
    #[serde(default = "default_trace_sample_step_seconds")]
    trace_sample_step_seconds: f64,
    #[serde(default = "default_trace_color_mode")]
    trace_color_mode: String,
    #[serde(default = "default_true")]
    start_ghosts: bool,
    #[serde(default = "default_true")]
    event_markers: bool,
    #[serde(default)]
    labels: bool,
}

impl Default for RenderOptionsInput {
    fn default() -> Self {
        Self {
            scale_factor: default_scale_factor(),
            transparent_background: false,
            trace_sample_step_seconds: default_trace_sample_step_seconds(),
            trace_color_mode: default_trace_color_mode(),
            start_ghosts: true,
            event_markers: true,
            labels: false,
        }
    }
}

#[derive(Debug, Serialize)]
struct SimOutcome {
    elapsed_seconds: f64,
    table_width_inches: f64,
    table_height_inches: f64,
    events: Vec<EventOutput>,
    pocketed: Vec<PocketedOutput>,
    final_balls: Vec<BallStateOutput>,
    cue_pocketed: bool,
    nine_pocketed: bool,
    legal_nine_pocketed: bool,
    first_cue_contact: Option<String>,
    lowest_object_ball: Option<String>,
    first_contact_lowest_object_ball: Option<bool>,
}

#[derive(Debug, Serialize)]
struct EventOutput {
    time_seconds: f64,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ball: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_ball: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    second_ball: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pocket: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jaw: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phase_before: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phase_after: Option<String>,
}

#[derive(Debug, Serialize)]
struct PocketedOutput {
    ball: String,
    pocket: String,
}

#[derive(Debug, Serialize)]
struct BallStateOutput {
    ball: String,
    state: String,
    x: f64,
    y: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pocket: Option<String>,
}

#[derive(Debug)]
struct BatchSimRow {
    elapsed_seconds: f64,
    cue_pocketed: bool,
    nine_pocketed: bool,
    legal_nine_pocketed: bool,
    first_cue_contact: i16,
    lowest_object_ball: i16,
    first_contact_lowest_object_ball: bool,
    event_count: i32,
    pocketed_mask: Vec<bool>,
    final_state: Vec<i16>,
    final_x: Vec<f64>,
    final_y: Vec<f64>,
    final_pocket: Vec<i16>,
}

fn default_units() -> String {
    "inches".to_string()
}

fn default_speed_semantics() -> String {
    "cue_stick_at_impact".to_string()
}

fn default_cue_mass_ratio() -> f64 {
    1.0
}

fn default_collision_energy_loss() -> f64 {
    0.1
}

fn default_scale_factor() -> u32 {
    1
}

fn default_trace_sample_step_seconds() -> f64 {
    0.02
}

fn default_trace_color_mode() -> String {
    "motion_phase".to_string()
}

fn default_true() -> bool {
    true
}

#[pyfunction]
fn simulate_shot_json(request_json: &str) -> PyResult<String> {
    simulate_shot_json_inner(request_json).map_err(PyValueError::new_err)
}

fn simulate_shot_json_inner(request_json: &str) -> Result<String, String> {
    let request: SimRequest = serde_json::from_str(request_json)
        .map_err(|err| format!("invalid simulate_shot request JSON: {err}"))?;
    let outcome = simulate_request(request)?;
    serde_json::to_string(&outcome).map_err(|err| format!("failed to encode outcome JSON: {err}"))
}

#[pyfunction]
fn render_board_png_json<'py>(
    py: Python<'py>,
    request_json: &str,
) -> PyResult<Bound<'py, PyBytes>> {
    let png = render_board_png_json_inner(request_json).map_err(PyValueError::new_err)?;
    Ok(PyBytes::new(py, &png))
}

fn render_board_png_json_inner(request_json: &str) -> Result<Vec<u8>, String> {
    let request: RenderBoardRequest = serde_json::from_str(request_json)
        .map_err(|err| format!("invalid render_board request JSON: {err}"))?;
    render_board_request(request)
}

#[pyfunction]
fn render_shot_trace_png_json<'py>(
    py: Python<'py>,
    request_json: &str,
) -> PyResult<Bound<'py, PyBytes>> {
    let png = render_shot_trace_png_json_inner(request_json).map_err(PyValueError::new_err)?;
    Ok(PyBytes::new(py, &png))
}

fn render_shot_trace_png_json_inner(request_json: &str) -> Result<Vec<u8>, String> {
    let request: RenderShotTraceRequest = serde_json::from_str(request_json)
        .map_err(|err| format!("invalid render_shot_trace request JSON: {err}"))?;
    render_shot_trace_request(request)
}

#[pyfunction]
#[pyo3(signature = (
    ball_ids,
    ball_xs,
    ball_ys,
    shot_values,
    speed_semantics = "cue_ball_launch",
    cue_mass_ratio = 1.0,
    collision_energy_loss = 0.1
))]
fn simulate_shots_batch<'py>(
    py: Python<'py>,
    ball_ids: Vec<Vec<u8>>,
    ball_xs: Vec<Vec<f64>>,
    ball_ys: Vec<Vec<f64>>,
    shot_values: Vec<Vec<f64>>,
    speed_semantics: &str,
    cue_mass_ratio: f64,
    collision_energy_loss: f64,
) -> PyResult<Bound<'py, PyDict>> {
    simulate_shots_batch_inner(
        py,
        ball_ids,
        ball_xs,
        ball_ys,
        shot_values,
        speed_semantics,
        cue_mass_ratio,
        collision_energy_loss,
    )
    .map_err(PyValueError::new_err)
}

fn simulate_shots_batch_inner<'py>(
    py: Python<'py>,
    ball_ids: Vec<Vec<u8>>,
    ball_xs: Vec<Vec<f64>>,
    ball_ys: Vec<Vec<f64>>,
    shot_values: Vec<Vec<f64>>,
    speed_semantics: &str,
    cue_mass_ratio: f64,
    collision_energy_loss: f64,
) -> Result<Bound<'py, PyDict>, String> {
    let batch_size = ball_ids.len();
    let max_balls = ball_ids.first().map_or(0, Vec::len);
    if max_balls == 0 && batch_size > 0 {
        return Err("ball_ids must have at least one ball slot per row".to_string());
    }
    require_rectangular_u8(&ball_ids, "ball_ids", max_balls)?;
    require_rectangular_f64(&ball_xs, "ball_xs", batch_size, max_balls)?;
    require_rectangular_f64(&ball_ys, "ball_ys", batch_size, max_balls)?;
    if shot_values.len() != batch_size {
        return Err(format!(
            "shot_values row count {} must match ball_ids batch size {batch_size}",
            shot_values.len()
        ));
    }
    let shot_cols = shot_values.first().map_or(0, Vec::len);
    if shot_cols < 2 {
        return Err("shot_values must have at least heading_degrees and speed_ips columns".into());
    }
    require_rectangular_f64(&shot_values, "shot_values", batch_size, shot_cols)?;

    let config = SimConfig {
        cue_mass_ratio,
        collision_energy_loss,
    };
    let speed_semantics = speed_semantics.to_string();
    let rows = py.allow_threads(move || {
        (0..batch_size)
            .into_par_iter()
            .map(|batch_index| {
                simulate_batch_row(
                    batch_index,
                    max_balls,
                    shot_cols,
                    &ball_ids,
                    &ball_xs,
                    &ball_ys,
                    &shot_values,
                    &speed_semantics,
                    &config,
                )
            })
            .collect::<Result<Vec<_>, _>>()
    })?;

    let mut elapsed_seconds = Vec::with_capacity(batch_size);
    let mut cue_pocketed = Vec::with_capacity(batch_size);
    let mut nine_pocketed = Vec::with_capacity(batch_size);
    let mut legal_nine_pocketed = Vec::with_capacity(batch_size);
    let mut first_cue_contact = Vec::with_capacity(batch_size);
    let mut lowest_object_ball = Vec::with_capacity(batch_size);
    let mut first_contact_lowest_object_ball = Vec::with_capacity(batch_size);
    let mut event_count = Vec::with_capacity(batch_size);
    let mut pocketed_mask = Vec::with_capacity(batch_size);
    let mut final_state = Vec::with_capacity(batch_size);
    let mut final_x = Vec::with_capacity(batch_size);
    let mut final_y = Vec::with_capacity(batch_size);
    let mut final_pocket = Vec::with_capacity(batch_size);

    for row in rows {
        elapsed_seconds.push(row.elapsed_seconds);
        cue_pocketed.push(row.cue_pocketed);
        nine_pocketed.push(row.nine_pocketed);
        legal_nine_pocketed.push(row.legal_nine_pocketed);
        first_cue_contact.push(row.first_cue_contact);
        lowest_object_ball.push(row.lowest_object_ball);
        first_contact_lowest_object_ball.push(row.first_contact_lowest_object_ball);
        event_count.push(row.event_count);
        pocketed_mask.push(row.pocketed_mask);
        final_state.push(row.final_state);
        final_x.push(row.final_x);
        final_y.push(row.final_y);
        final_pocket.push(row.final_pocket);
    }

    let numpy = py.import("numpy").map_err(|err| err.to_string())?;
    let output = PyDict::new(py);
    set_numpy_array(&output, &numpy, "elapsed_seconds", elapsed_seconds)?;
    set_numpy_array(&output, &numpy, "cue_pocketed", cue_pocketed)?;
    set_numpy_array(&output, &numpy, "nine_pocketed", nine_pocketed)?;
    set_numpy_array(&output, &numpy, "legal_nine_pocketed", legal_nine_pocketed)?;
    set_numpy_array(&output, &numpy, "first_cue_contact", first_cue_contact)?;
    set_numpy_array(&output, &numpy, "lowest_object_ball", lowest_object_ball)?;
    set_numpy_array(
        &output,
        &numpy,
        "first_contact_lowest_object_ball",
        first_contact_lowest_object_ball,
    )?;
    set_numpy_array(&output, &numpy, "event_count", event_count)?;
    set_numpy_array(&output, &numpy, "pocketed_mask", pocketed_mask)?;
    set_numpy_array(&output, &numpy, "final_state", final_state)?;
    set_numpy_array(&output, &numpy, "final_x", final_x)?;
    set_numpy_array(&output, &numpy, "final_y", final_y)?;
    set_numpy_array(&output, &numpy, "final_pocket", final_pocket)?;
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
fn simulate_batch_row(
    batch_index: usize,
    max_balls: usize,
    shot_cols: usize,
    ball_ids: &[Vec<u8>],
    ball_xs: &[Vec<f64>],
    ball_ys: &[Vec<f64>],
    shot_values: &[Vec<f64>],
    speed_semantics: &str,
    config: &SimConfig,
) -> Result<BatchSimRow, String> {
    let mut balls = Vec::new();
    for ball_slot in 0..max_balls {
        let ball_id = ball_ids[batch_index][ball_slot];
        if ball_id == ABSENT_BALL_ID {
            continue;
        }
        balls.push(BallInput {
            ball: ball_name_from_id(ball_id)?.to_string(),
            x: ball_xs[batch_index][ball_slot],
            y: ball_ys[batch_index][ball_slot],
            units: "inches".to_string(),
        });
    }

    let shot = ShotInput {
        heading_degrees: shot_values[batch_index][0],
        speed_ips: shot_values[batch_index][1],
        tip_side_r: if shot_cols > 2 {
            shot_values[batch_index][2]
        } else {
            0.0
        },
        tip_height_r: if shot_cols > 3 {
            shot_values[batch_index][3]
        } else {
            0.0
        },
        speed_semantics: speed_semantics.to_string(),
    };
    let outcome = simulate_request(SimRequest {
        balls,
        shot,
        config: config.clone(),
    })?;

    let mut pocketed_mask = vec![false; STANDARD_BALL_COUNT];
    let mut final_state = vec![0i16; STANDARD_BALL_COUNT];
    let mut final_x = vec![f64::NAN; STANDARD_BALL_COUNT];
    let mut final_y = vec![f64::NAN; STANDARD_BALL_COUNT];
    let mut final_pocket = vec![-1i16; STANDARD_BALL_COUNT];
    for ball in outcome.final_balls {
        let ball_id = optional_ball_name_to_id(Some(&ball.ball));
        if ball_id < 0 {
            continue;
        }
        let index = ball_id as usize;
        final_state[index] = if ball.state == "pocketed" { 2 } else { 1 };
        final_x[index] = ball.x;
        final_y[index] = ball.y;
        if ball.state == "pocketed" {
            pocketed_mask[index] = true;
        }
        if let Some(pocket) = ball.pocket.as_deref() {
            final_pocket[index] = pocket_id_from_name(pocket);
        }
    }

    Ok(BatchSimRow {
        elapsed_seconds: outcome.elapsed_seconds,
        cue_pocketed: outcome.cue_pocketed,
        nine_pocketed: outcome.nine_pocketed,
        legal_nine_pocketed: outcome.legal_nine_pocketed,
        first_cue_contact: optional_ball_name_to_id(outcome.first_cue_contact.as_deref()),
        lowest_object_ball: optional_ball_name_to_id(outcome.lowest_object_ball.as_deref()),
        first_contact_lowest_object_ball: outcome.first_contact_lowest_object_ball.unwrap_or(false),
        event_count: outcome.events.len() as i32,
        pocketed_mask,
        final_state,
        final_x,
        final_y,
        final_pocket,
    })
}

fn require_rectangular_u8(
    values: &[Vec<u8>],
    name: &str,
    expected_cols: usize,
) -> Result<(), String> {
    for (row_index, row) in values.iter().enumerate() {
        if row.len() != expected_cols {
            return Err(format!(
                "{name} row {row_index} has {} columns, expected {expected_cols}",
                row.len()
            ));
        }
    }
    Ok(())
}

fn require_rectangular_f64(
    values: &[Vec<f64>],
    name: &str,
    expected_rows: usize,
    expected_cols: usize,
) -> Result<(), String> {
    if values.len() != expected_rows {
        return Err(format!(
            "{name} has {} rows, expected {expected_rows}",
            values.len()
        ));
    }
    for (row_index, row) in values.iter().enumerate() {
        if row.len() != expected_cols {
            return Err(format!(
                "{name} row {row_index} has {} columns, expected {expected_cols}",
                row.len()
            ));
        }
    }
    Ok(())
}

fn set_numpy_array<'py, T>(
    dict: &Bound<'py, PyDict>,
    numpy: &Bound<'py, PyAny>,
    key: &str,
    values: T,
) -> Result<(), String>
where
    T: IntoPyObject<'py>,
{
    let array = numpy
        .call_method1("array", (values,))
        .map_err(|err| err.to_string())?;
    dict.set_item(key, array).map_err(|err| err.to_string())
}

fn simulate_request(request: SimRequest) -> Result<SimOutcome, String> {
    if request.balls.is_empty() {
        return Err("at least one ball is required".to_string());
    }

    let table = TableSpec::brunswick_gc4_9ft();
    let ball_set = BallSetPhysicsSpec::default();
    let cue_strike = cue_strike_from_config(&request.config)?;
    let shot = shot_from_input(&request.shot, &cue_strike)?;
    let (balls, ball_types, cue_index) = parse_initial_balls(&request.balls, &table)?;
    let states = initial_states_from_balls(&balls, cue_index, &shot, &cue_strike, &ball_set, &table)?;

    let motion = human_tuned_preview_motion_config();
    let collision_config = BallBallCollisionConfig::human_tuned();
    let rail_profile = RailCollisionProfile::default();
    let simulation = simulate_n_balls_with_physics_and_pockets_on_table_until_rest(
        &states,
        &ball_set,
        &table,
        &motion,
        CollisionModel::ThrowAware,
        &collision_config,
        RailModel::SpinAware,
        &rail_profile,
    );

    Ok(outcome_from_simulation(
        &simulation,
        &ball_types,
        cue_index,
        &table,
    ))
}

fn cue_strike_from_config(config: &SimConfig) -> Result<CueStrikeConfig, String> {
    CueStrikeConfig::new(
        Scale::from_f64(config.cue_mass_ratio),
        Scale::from_f64(config.collision_energy_loss),
    )
    .map_err(|err| format!("invalid cue strike config: {err:?}"))
}

fn shot_from_input(input: &ShotInput, cue_strike: &CueStrikeConfig) -> Result<Shot, String> {
    let tip_contact = CueTipContact::new(
        Scale::from_f64(input.tip_side_r),
        Scale::from_f64(input.tip_height_r),
    )
    .map_err(|err| format!("invalid cue-tip contact: {err:?}"))?;
    let shot_speed = InchesPerSecond::new(Inches::from_f64(input.speed_ips));
    match normalize_token(&input.speed_semantics).as_str() {
        "cue-stick-at-impact" | "cue-stick" | "stick" => Shot::new(
            angle_from_degrees(input.heading_degrees),
            shot_speed,
            tip_contact,
        ),
        "cue-ball-launch" | "cue-ball" | "launch" => Shot::new_for_cue_ball_launch_speed(
            angle_from_degrees(input.heading_degrees),
            shot_speed,
            tip_contact,
            cue_strike,
        ),
        other => {
            return Err(format!(
                "unknown speed_semantics '{other}'; expected cue_stick_at_impact or cue_ball_launch"
            ))
        }
    }
    .map_err(|err| format!("invalid shot: {err:?}"))
}

fn parse_initial_balls(
    inputs: &[BallInput],
    table: &TableSpec,
) -> Result<(Vec<Ball>, Vec<BallType>, usize), String> {
    let mut balls = Vec::with_capacity(inputs.len());
    let mut ball_types = Vec::with_capacity(inputs.len());
    let mut cue_indices = Vec::new();
    for (index, input) in inputs.iter().enumerate() {
        if normalize_token(&input.units) != "inches" {
            return Err(format!(
                "unsupported units '{}' for ball '{}'; only inches are supported",
                input.units, input.ball
            ));
        }
        let ball_type = parse_ball_type(&input.ball)?;
        if ball_type == BallType::Cue {
            cue_indices.push(index);
        }
        let position = position_from_inches(table, input.x, input.y);
        balls.push(Ball {
            ty: ball_type.clone(),
            position,
            spec: BallSpec::default(),
        });
        ball_types.push(ball_type);
    }

    let cue_index = match cue_indices.as_slice() {
        [index] => *index,
        [] => return Err("exactly one cue ball is required, but none were supplied".to_string()),
        _ => {
            return Err("exactly one cue ball is required, but multiple were supplied".to_string())
        }
    };

    Ok((balls, ball_types, cue_index))
}

fn initial_states_from_balls(
    balls: &[Ball],
    cue_index: usize,
    shot: &Shot,
    cue_strike: &CueStrikeConfig,
    ball_set: &BallSetPhysicsSpec,
    table: &TableSpec,
) -> Result<Vec<OnTableBallState>, String> {
    let mut states = Vec::with_capacity(balls.len());
    for (index, ball) in balls.iter().enumerate() {
        let resting = RestingOnTableBallState::try_from(BallState::resting_at_position(
            &ball.position,
            table,
        ))
        .map_err(|err| {
            format!(
                "ball '{}' is not a resting on-table state: {err:?}",
                ball_type_name(&ball.ty)
            )
        })?;
        let state = if index == cue_index {
            strike_resting_ball_on_table(&resting, shot, cue_strike, ball_set)
                .map_err(|err| format!("failed to strike cue ball: {err:?}"))?
        } else {
            resting.into_on_table_ball_state()
        };
        states.push(state);
    }
    Ok(states)
}

fn render_board_request(request: RenderBoardRequest) -> Result<Vec<u8>, String> {
    let table = TableSpec::brunswick_gc4_9ft();
    let balls = parse_render_balls(&request.balls, &table)?;
    let game_state = GameState::with_balls(table, balls);
    Ok(game_state.draw_2d_diagram_with_options(&diagram_render_options(&request.render)))
}

fn render_shot_trace_request(request: RenderShotTraceRequest) -> Result<Vec<u8>, String> {
    if request.balls.is_empty() {
        return Err("at least one ball is required".to_string());
    }

    let table = TableSpec::brunswick_gc4_9ft();
    let ball_set = BallSetPhysicsSpec::default();
    let cue_strike = cue_strike_from_config(&request.config)?;
    let shot = shot_from_input(&request.shot, &cue_strike)?;
    let (balls, _, _) = parse_initial_balls(&request.balls, &table)?;
    let mut scenario = DslScenario {
        game_state: GameState::with_balls(table, balls),
        shot: Some(ScenarioShot {
            ball_ref: BallRef::Cue,
            ball: BallType::Cue,
            shot,
            cue_strike,
        }),
        ball_ball_configs: HashMap::new(),
        rail_responses: HashMap::new(),
        rail_profiles: HashMap::new(),
        simulations: HashMap::new(),
    };
    scenario.game_state.resolve_positions();

    let motion = human_tuned_preview_motion_config();
    let collision_config = BallBallCollisionConfig::human_tuned();
    let rail_profile = RailCollisionProfile::default();
    let trace = scenario
        .simulate_shot_trace_with_physics_on_table_until_rest(
            &ball_set,
            &motion,
            CollisionModel::ThrowAware,
            &collision_config,
            RailModel::SpinAware,
            &rail_profile,
        )
        .map_err(|err| format!("failed to simulate shot trace: {err:?}"))?
        .ok_or_else(|| "shot trace request did not include a shot".to_string())?;
    let render_state = trace.rendered_final_layout_with_trace_options(
        &scenario,
        &scenario_trace_render_options(&request.render)?,
    );
    Ok(render_state.draw_2d_diagram_with_options(&diagram_render_options(&request.render)))
}

fn parse_render_balls(inputs: &[RenderBallInput], table: &TableSpec) -> Result<Vec<Ball>, String> {
    let mut balls = Vec::with_capacity(inputs.len());
    for input in inputs {
        if normalize_token(&input.units) != "inches" {
            return Err(format!(
                "unsupported units '{}' for ball '{}'; only inches are supported",
                input.units, input.ball
            ));
        }
        let state = input
            .state
            .as_deref()
            .map(normalize_token)
            .unwrap_or_else(|| "on-table".to_string());
        if matches!(state.as_str(), "pocketed" | "off-table") {
            continue;
        }
        if !matches!(state.as_str(), "on-table" | "resting") {
            return Err(format!(
                "unsupported render state '{}' for ball '{}'; expected on_table or pocketed",
                state, input.ball
            ));
        }
        balls.push(Ball {
            ty: parse_ball_type(&input.ball)?,
            position: position_from_inches(table, input.x, input.y),
            spec: BallSpec::default(),
        });
    }
    Ok(balls)
}

fn diagram_render_options(input: &RenderOptionsInput) -> DiagramRenderOptions {
    DiagramRenderOptions {
        scale_factor: input.scale_factor.max(1),
        background: if input.transparent_background {
            DiagramBackground::Transparent
        } else {
            DiagramBackground::Table
        },
    }
}

fn scenario_trace_render_options(
    input: &RenderOptionsInput,
) -> Result<ScenarioTraceRenderOptions, String> {
    if !input.trace_sample_step_seconds.is_finite() || input.trace_sample_step_seconds <= 0.0 {
        return Err("trace_sample_step_seconds must be positive and finite".to_string());
    }
    Ok(ScenarioTraceRenderOptions {
        path_render: BallPathRenderOptions {
            max_time_step: Seconds::new(input.trace_sample_step_seconds),
            ..BallPathRenderOptions::default()
        },
        start_ghost_balls: input.start_ghosts,
        event_markers: input.event_markers,
        labels: input.labels,
        path_color_mode: parse_path_color_mode(&input.trace_color_mode)?,
    })
}

fn parse_path_color_mode(input: &str) -> Result<PathColorMode, String> {
    match normalize_token(input).as_str() {
        "solid" => Ok(PathColorMode::Solid),
        "fade-by-time" | "fade" | "time" => Ok(PathColorMode::FadeByTime),
        "motion-phase" | "phase" => Ok(PathColorMode::MotionPhase),
        other => Err(format!(
            "unknown trace_color_mode '{other}'; expected solid, fade_by_time, or motion_phase"
        )),
    }
}

fn outcome_from_simulation(
    simulation: &NBallSystemSimulation,
    ball_types: &[BallType],
    cue_index: usize,
    table: &TableSpec,
) -> SimOutcome {
    let mut elapsed = 0.0;
    let mut events = Vec::with_capacity(simulation.events.len());
    let mut first_cue_contact = None;

    for event in &simulation.events {
        elapsed += event.time().as_f64();
        if first_cue_contact.is_none() {
            if let NBallSystemEvent::BallBallCollision {
                first_ball_index,
                second_ball_index,
                ..
            } = event
            {
                if *first_ball_index == cue_index && *second_ball_index != cue_index {
                    first_cue_contact = Some(ball_types[*second_ball_index].clone());
                } else if *second_ball_index == cue_index && *first_ball_index != cue_index {
                    first_cue_contact = Some(ball_types[*first_ball_index].clone());
                }
            }
        }
        events.push(event_output(event, ball_types, elapsed));
    }

    let lowest_object_ball = ball_types.iter().filter_map(ball_number).min();
    let first_contact_lowest_object_ball = first_cue_contact
        .as_ref()
        .map(|ball| ball_number(ball).is_some() && ball_number(ball) == lowest_object_ball);

    let mut pocketed = Vec::new();
    let mut final_balls = Vec::with_capacity(simulation.states.len());
    for (ball_type, state) in ball_types.iter().zip(&simulation.states) {
        match state {
            NBallSystemState::OnTable(on_table) => {
                let pos = &on_table.as_ball_state().position;
                final_balls.push(BallStateOutput {
                    ball: ball_type_name(ball_type).to_string(),
                    state: "on_table".to_string(),
                    x: pos.x().as_f64(),
                    y: pos.y().as_f64(),
                    pocket: None,
                });
            }
            NBallSystemState::Pocketed {
                pocket,
                state_at_capture,
            } => {
                let pos = &state_at_capture.as_ball_state().position;
                let pocket_name = pocket_name(*pocket).to_string();
                pocketed.push(PocketedOutput {
                    ball: ball_type_name(ball_type).to_string(),
                    pocket: pocket_name.clone(),
                });
                final_balls.push(BallStateOutput {
                    ball: ball_type_name(ball_type).to_string(),
                    state: "pocketed".to_string(),
                    x: pos.x().as_f64(),
                    y: pos.y().as_f64(),
                    pocket: Some(pocket_name),
                });
            }
        }
    }

    let cue_pocketed = pocketed.iter().any(|p| p.ball == "cue");
    let nine_pocketed = pocketed.iter().any(|p| p.ball == "nine");
    let legal_nine_pocketed =
        nine_pocketed && !cue_pocketed && first_contact_lowest_object_ball.unwrap_or(false);

    SimOutcome {
        elapsed_seconds: simulation.elapsed.as_f64(),
        table_width_inches: table.diamond_length.as_f64() * 4.0,
        table_height_inches: table.diamond_length.as_f64() * 8.0,
        events,
        pocketed,
        final_balls,
        cue_pocketed,
        nine_pocketed,
        legal_nine_pocketed,
        first_cue_contact: first_cue_contact
            .as_ref()
            .map(|ball| ball_type_name(ball).to_string()),
        lowest_object_ball: lowest_object_ball
            .map(|number| ball_type_name(&ball_type_from_number(number)).to_string()),
        first_contact_lowest_object_ball,
    }
}

fn event_output(
    event: &NBallSystemEvent,
    ball_types: &[BallType],
    time_seconds: f64,
) -> EventOutput {
    match event {
        NBallSystemEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            ..
        } => EventOutput {
            time_seconds,
            kind: "ball_ball_collision".to_string(),
            ball: None,
            first_ball: Some(ball_type_name(&ball_types[*first_ball_index]).to_string()),
            second_ball: Some(ball_type_name(&ball_types[*second_ball_index]).to_string()),
            pocket: None,
            rail: None,
            jaw: None,
            phase_before: None,
            phase_after: None,
        },
        NBallSystemEvent::UnsupportedSharedBallBallContact {
            ball_indices,
            ball_ball_pairs,
            ..
        } => EventOutput {
            time_seconds,
            kind: "unsupported_shared_ball_ball_contact".to_string(),
            ball: Some(
                ball_indices
                    .iter()
                    .map(|index| ball_type_name(&ball_types[*index]))
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            first_ball: None,
            second_ball: None,
            pocket: None,
            rail: None,
            jaw: Some(
                ball_ball_pairs
                    .iter()
                    .map(|(first, second)| {
                        format!(
                            "{}-{}",
                            ball_type_name(&ball_types[*first]),
                            ball_type_name(&ball_types[*second])
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            phase_before: None,
            phase_after: None,
        },
        NBallSystemEvent::BallJawImpact { ball_index, impact } => EventOutput {
            time_seconds,
            kind: "ball_jaw_impact".to_string(),
            ball: Some(ball_type_name(&ball_types[*ball_index]).to_string()),
            first_ball: None,
            second_ball: None,
            pocket: Some(pocket_name(impact.pocket).to_string()),
            rail: None,
            jaw: Some(jaw_name(impact.jaw).to_string()),
            phase_before: None,
            phase_after: None,
        },
        NBallSystemEvent::BallPocketCapture {
            ball_index,
            capture,
        } => EventOutput {
            time_seconds,
            kind: "ball_pocket_capture".to_string(),
            ball: Some(ball_type_name(&ball_types[*ball_index]).to_string()),
            first_ball: None,
            second_ball: None,
            pocket: Some(pocket_name(capture.pocket).to_string()),
            rail: None,
            jaw: None,
            phase_before: None,
            phase_after: None,
        },
        NBallSystemEvent::BallRailImpact { ball_index, impact } => EventOutput {
            time_seconds,
            kind: "ball_rail_impact".to_string(),
            ball: Some(ball_type_name(&ball_types[*ball_index]).to_string()),
            first_ball: None,
            second_ball: None,
            pocket: None,
            rail: Some(rail_name(impact.rail).to_string()),
            jaw: None,
            phase_before: None,
            phase_after: None,
        },
        NBallSystemEvent::MotionTransition {
            ball_index,
            transition,
        } => EventOutput {
            time_seconds,
            kind: "motion_transition".to_string(),
            ball: Some(ball_type_name(&ball_types[*ball_index]).to_string()),
            first_ball: None,
            second_ball: None,
            pocket: None,
            rail: None,
            jaw: None,
            phase_before: Some(format!("{:?}", transition.phase_before)),
            phase_after: Some(format!("{:?}", transition.phase_after)),
        },
    }
}

fn angle_from_degrees(degrees: f64) -> Angle {
    let radians = degrees.to_radians();
    Angle::from_north(radians.sin(), radians.cos())
}

fn position_from_inches(table: &TableSpec, x: f64, y: f64) -> Position {
    Position::new(
        table.inches_to_diamond(Inches::from_f64(x)),
        table.inches_to_diamond(Inches::from_f64(y)),
    )
}

fn parse_ball_type(input: &str) -> Result<BallType, String> {
    match normalize_token(input).as_str() {
        "cue" | "cue-ball" | "cueball" | "cb" | "0" => Ok(BallType::Cue),
        "one" | "1" => Ok(BallType::One),
        "two" | "2" => Ok(BallType::Two),
        "three" | "3" => Ok(BallType::Three),
        "four" | "4" => Ok(BallType::Four),
        "five" | "5" => Ok(BallType::Five),
        "six" | "6" => Ok(BallType::Six),
        "seven" | "7" => Ok(BallType::Seven),
        "eight" | "8" => Ok(BallType::Eight),
        "nine" | "9" => Ok(BallType::Nine),
        other => Err(format!("unknown ball '{other}'")),
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

fn ball_id(ball: &BallType) -> u8 {
    match ball {
        BallType::Cue => 0,
        BallType::One => 1,
        BallType::Two => 2,
        BallType::Three => 3,
        BallType::Four => 4,
        BallType::Five => 5,
        BallType::Six => 6,
        BallType::Seven => 7,
        BallType::Eight => 8,
        BallType::Nine => 9,
    }
}

fn ball_name_from_id(ball_id: u8) -> Result<&'static str, String> {
    let ball_type = match ball_id {
        0 => BallType::Cue,
        1 => BallType::One,
        2 => BallType::Two,
        3 => BallType::Three,
        4 => BallType::Four,
        5 => BallType::Five,
        6 => BallType::Six,
        7 => BallType::Seven,
        8 => BallType::Eight,
        9 => BallType::Nine,
        other => {
            return Err(format!(
                "unknown ball id {other}; expected 0..9 or {ABSENT_BALL_ID} for absent"
            ))
        }
    };
    Ok(ball_type_name(&ball_type))
}

fn optional_ball_name_to_id(name: Option<&str>) -> i16 {
    name.and_then(|name| parse_ball_type(name).ok())
        .map(|ball_type| ball_id(&ball_type) as i16)
        .unwrap_or(-1)
}

fn ball_number(ball: &BallType) -> Option<u8> {
    match ball {
        BallType::Cue => None,
        BallType::One => Some(1),
        BallType::Two => Some(2),
        BallType::Three => Some(3),
        BallType::Four => Some(4),
        BallType::Five => Some(5),
        BallType::Six => Some(6),
        BallType::Seven => Some(7),
        BallType::Eight => Some(8),
        BallType::Nine => Some(9),
    }
}

fn ball_type_from_number(number: u8) -> BallType {
    match number {
        1 => BallType::One,
        2 => BallType::Two,
        3 => BallType::Three,
        4 => BallType::Four,
        5 => BallType::Five,
        6 => BallType::Six,
        7 => BallType::Seven,
        8 => BallType::Eight,
        9 => BallType::Nine,
        _ => unreachable!("lowest object-ball number should be in 1..=9"),
    }
}

fn pocket_name(pocket: Pocket) -> &'static str {
    match pocket {
        Pocket::TopRight => "top_right",
        Pocket::CenterRight => "center_right",
        Pocket::BottomRight => "bottom_right",
        Pocket::BottomLeft => "bottom_left",
        Pocket::CenterLeft => "center_left",
        Pocket::TopLeft => "top_left",
    }
}

fn pocket_id_from_name(name: &str) -> i16 {
    match normalize_token(name).as_str() {
        "top-right" => 0,
        "center-right" => 1,
        "bottom-right" => 2,
        "bottom-left" => 3,
        "center-left" => 4,
        "top-left" => 5,
        _ => -1,
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

fn jaw_name(jaw: billiards::PocketJaw) -> &'static str {
    match jaw {
        billiards::PocketJaw::First => "first",
        billiards::PocketJaw::Second => "second",
    }
}

fn normalize_token(input: &str) -> String {
    input.trim().to_ascii_lowercase().replace('_', "-")
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(simulate_shot_json, m)?)?;
    m.add_function(wrap_pyfunction!(simulate_shots_batch, m)?)?;
    m.add_function(wrap_pyfunction!(render_board_png_json, m)?)?;
    m.add_function(wrap_pyfunction!(render_shot_trace_png_json, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

    #[test]
    fn straight_combo_reports_legal_nine_pocket() {
        let request = json!({
            "balls": [
                {"ball": "cue", "x": 10.0, "y": 50.0},
                {"ball": "one", "x": 25.0, "y": 50.0},
                {"ball": "nine", "x": 37.5, "y": 50.0}
            ],
            "shot": {
                "heading_degrees": 90.0,
                "speed_ips": 180.0,
                "speed_semantics": "cue_ball_launch"
            }
        });

        let response =
            simulate_shot_json_inner(&request.to_string()).expect("shot should simulate");
        let outcome: serde_json::Value = serde_json::from_str(&response).expect("valid JSON");

        assert_eq!(outcome["first_cue_contact"], "one");
        assert_eq!(outcome["lowest_object_ball"], "one");
        assert_eq!(outcome["first_contact_lowest_object_ball"], true);
        assert_eq!(outcome["nine_pocketed"], true);
        assert_eq!(outcome["cue_pocketed"], false);
        assert_eq!(outcome["legal_nine_pocketed"], true);
        assert!(outcome["events"]
            .as_array()
            .expect("events")
            .iter()
            .any(|event| {
                event["kind"] == "ball_ball_collision"
                    && event["first_ball"] == "cue"
                    && event["second_ball"] == "one"
            }));
    }

    #[test]
    fn board_renderer_returns_png_and_skips_pocketed_final_balls() {
        let request = json!({
            "balls": [
                {"ball": "cue", "state": "on_table", "x": 10.0, "y": 50.0},
                {"ball": "one", "state": "pocketed", "x": 25.0, "y": 50.0}
            ],
            "render": {"scale_factor": 1}
        });

        let png = render_board_png_json_inner(&request.to_string()).expect("board should render");
        assert!(png.starts_with(PNG_SIGNATURE));
    }

    #[test]
    fn shot_trace_renderer_returns_png() {
        let request = json!({
            "balls": [
                {"ball": "cue", "x": 10.0, "y": 50.0},
                {"ball": "one", "x": 25.0, "y": 50.0}
            ],
            "shot": {
                "heading_degrees": 90.0,
                "speed_ips": 128.0,
                "speed_semantics": "cue_ball_launch"
            },
            "render": {"trace_color_mode": "motion_phase", "event_markers": true}
        });

        let png =
            render_shot_trace_png_json_inner(&request.to_string()).expect("trace should render");
        assert!(png.starts_with(PNG_SIGNATURE));
    }
}
