use billiards::{
    human_tuned_preview_motion_config,
    simulate_n_balls_with_physics_and_pockets_on_table_until_rest, strike_resting_ball_on_table,
    Angle, Ball, BallBallCollisionConfig, BallSetPhysicsSpec, BallSpec, BallState, BallType,
    CollisionModel, CueStrikeConfig, CueTipContact, Inches, InchesPerSecond, NBallSystemEvent,
    NBallSystemSimulation, NBallSystemState, Pocket, Position, Rail, RailCollisionProfile,
    RailModel, RestingOnTableBallState, Scale, Shot, TableSpec,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct SimRequest {
    balls: Vec<BallInput>,
    shot: ShotInput,
    #[serde(default)]
    config: SimConfig,
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

#[derive(Debug, Deserialize)]
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

fn simulate_request(request: SimRequest) -> Result<SimOutcome, String> {
    if request.balls.is_empty() {
        return Err("at least one ball is required".to_string());
    }

    let table = TableSpec::brunswick_gc4_9ft();
    let ball_set = BallSetPhysicsSpec::default();
    let cue_strike = CueStrikeConfig::new(
        Scale::from_f64(request.config.cue_mass_ratio),
        Scale::from_f64(request.config.collision_energy_loss),
    )
    .map_err(|err| format!("invalid cue strike config: {err:?}"))?;
    let tip_contact = CueTipContact::new(
        Scale::from_f64(request.shot.tip_side_r),
        Scale::from_f64(request.shot.tip_height_r),
    )
    .map_err(|err| format!("invalid cue-tip contact: {err:?}"))?;
    let shot_speed = InchesPerSecond::new(Inches::from_f64(request.shot.speed_ips));
    let shot = match normalize_token(&request.shot.speed_semantics).as_str() {
        "cue-stick-at-impact" | "cue-stick" | "stick" => Shot::new(
            angle_from_degrees(request.shot.heading_degrees),
            shot_speed,
            tip_contact,
        ),
        "cue-ball-launch" | "cue-ball" | "launch" => Shot::new_for_cue_ball_launch_speed(
            angle_from_degrees(request.shot.heading_degrees),
            shot_speed,
            tip_contact,
            &cue_strike,
        ),
        other => {
            return Err(format!(
                "unknown speed_semantics '{other}'; expected cue_stick_at_impact or cue_ball_launch"
            ))
        }
    }
    .map_err(|err| format!("invalid shot: {err:?}"))?;

    let mut balls = Vec::with_capacity(request.balls.len());
    let mut ball_types = Vec::with_capacity(request.balls.len());
    let mut cue_indices = Vec::new();
    for (index, input) in request.balls.iter().enumerate() {
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
        let position = position_from_inches(&table, input.x, input.y);
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

    let mut states = Vec::with_capacity(balls.len());
    for (index, ball) in balls.iter().enumerate() {
        let resting = RestingOnTableBallState::try_from(BallState::resting_at_position(
            &ball.position,
            &table,
        ))
        .map_err(|err| {
            format!(
                "ball '{}' is not a resting on-table state: {err:?}",
                ball_type_name(&ball.ty)
            )
        })?;
        let state = if index == cue_index {
            strike_resting_ball_on_table(&resting, &shot, &cue_strike, &ball_set)
                .map_err(|err| format!("failed to strike cue ball: {err:?}"))?
        } else {
            resting.into_on_table_ball_state()
        };
        states.push(state);
    }

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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
