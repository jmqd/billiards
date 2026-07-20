use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShotControlsDto {
    heading_degrees: f64,
    speed_ips: f64,
    speed_hint: &'static str,
    tip_side: f64,
    tip_height: f64,
    tip_max_radius: f64,
    cue_elevation_degrees: f64,
    cue_elevation_explicit: bool,
    speed_max_ips: f64,
    cue_elevation_max_degrees: f64,
}

impl From<crate::dsl::ShotControls> for ShotControlsDto {
    fn from(controls: crate::dsl::ShotControls) -> Self {
        let speed = crate::InchesPerSecond::new(crate::Inches::from_f64(controls.speed_ips));
        Self {
            heading_degrees: controls.heading_degrees,
            speed_ips: controls.speed_ips,
            speed_hint: crate::ShotSpeedPreset::nearest_to_speed(&speed).human_label(),
            tip_side: controls.tip_side,
            tip_height: controls.tip_height,
            tip_max_radius: controls.tip_max_radius,
            cue_elevation_degrees: controls.cue_elevation_degrees,
            cue_elevation_explicit: controls.cue_elevation_explicit,
            speed_max_ips: controls.speed_max_ips,
            cue_elevation_max_degrees: controls.cue_elevation_max_degrees,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShotControlUpdateDto {
    source: String,
    controls: ShotControlsDto,
}

impl From<crate::dsl::ShotControlUpdate> for ShotControlUpdateDto {
    fn from(update: crate::dsl::ShotControlUpdate) -> Self {
        Self {
            source: update.source,
            controls: update.controls.into(),
        }
    }
}

fn serialize_json<T: Serialize>(value: &T) -> Result<String, JsValue> {
    serde_json::to_string(value).map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub fn render_svg_from_dsl(source: &str) -> Result<String, JsValue> {
    crate::svg_generator::render_svg_from_dsl(source).map_err(|error| JsValue::from_str(&error))
}

#[wasm_bindgen]
pub fn render_svg_report_from_dsl(source: &str) -> Result<String, JsValue> {
    crate::svg_generator::render_svg_report_from_dsl(source)
        .map_err(|error| JsValue::from_str(&error))
}

#[wasm_bindgen]
pub fn shot_controls_from_dsl(source: &str) -> Result<String, JsValue> {
    let controls = crate::dsl::shot_controls_from_dsl(source)
        .map_err(|error| JsValue::from_str(&error.to_string()))?
        .map(ShotControlsDto::from);
    serialize_json(&controls)
}

#[wasm_bindgen]
pub fn update_shot_control_in_dsl(
    source: &str,
    control: &str,
    value: f64,
) -> Result<String, JsValue> {
    let control = control
        .parse::<crate::dsl::ShotControl>()
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let update = crate::dsl::update_shot_control_in_dsl(source, control, value)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    serialize_json(&ShotControlUpdateDto::from(update))
}

#[wasm_bindgen]
pub fn update_shot_tip_in_dsl(source: &str, side: f64, height: f64) -> Result<String, JsValue> {
    let update = crate::dsl::update_shot_tip_in_dsl(source, side, height)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    serialize_json(&ShotControlUpdateDto::from(update))
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct RobustControlsDto {
    heading_degrees: f64,
    speed_ips: f64,
    tip_side: f64,
    tip_height: f64,
    cue_elevation_degrees: f64,
}

impl From<crate::RobustShotControls> for RobustControlsDto {
    fn from(controls: crate::RobustShotControls) -> Self {
        Self {
            heading_degrees: controls.heading,
            speed_ips: controls.speed,
            tip_side: controls.tip_side,
            tip_height: controls.tip_height,
            cue_elevation_degrees: controls.elevation,
        }
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct RobustSigmasDto {
    heading_degrees: f64,
    speed_ips: f64,
    tip_side_radii: f64,
    tip_height_radii: f64,
    cue_elevation_degrees: f64,
}

impl From<crate::RobustNoiseSigmas> for RobustSigmasDto {
    fn from(sigmas: crate::RobustNoiseSigmas) -> Self {
        Self {
            heading_degrees: sigmas.heading,
            speed_ips: sigmas.speed,
            tip_side_radii: sigmas.tip_side,
            tip_height_radii: sigmas.tip_height,
            cue_elevation_degrees: sigmas.elevation,
        }
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct RobustOutcomeSummaryDto {
    requested: u32,
    scored: u32,
    missed: u32,
    indeterminate: u32,
    failed: u32,
    success_rate: Option<f64>,
    confidence_low: Option<f64>,
    confidence_high: Option<f64>,
    eligible: bool,
}

impl From<&crate::RobustOutcomeSummary> for RobustOutcomeSummaryDto {
    fn from(summary: &crate::RobustOutcomeSummary) -> Self {
        Self {
            requested: summary.requested,
            scored: summary.scored,
            missed: summary.missed,
            indeterminate: summary.indeterminate,
            failed: summary.failed,
            success_rate: summary.success_rate,
            confidence_low: summary.confidence_low,
            confidence_high: summary.confidence_high,
            eligible: summary.eligible,
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RobustCandidateDto {
    rank: Option<usize>,
    candidate_id: u64,
    controls: RobustControlsDto,
    screening: RobustOutcomeSummaryDto,
    validation: Option<RobustOutcomeSummaryDto>,
}

impl From<&crate::RobustCandidateReport> for RobustCandidateDto {
    fn from(candidate: &crate::RobustCandidateReport) -> Self {
        Self {
            rank: candidate.rank,
            candidate_id: candidate.candidate_id,
            controls: candidate.controls.into(),
            screening: (&candidate.screening).into(),
            validation: candidate.validation.as_ref().map(Into::into),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RobustSearchResponseDto {
    player_level: &'static str,
    player_level_label: &'static str,
    noise_model: &'static str,
    parent_sigma_truncation: f64,
    conditional_standard_deviation_factor: f64,
    shot_inaccuracy_sigmas: RobustSigmasDto,
    source_has_shot: bool,
    physics_profile: &'static str,
    max_events_per_iteration: usize,
    max_events_per_iteration_cap: usize,
    requested_iterations: u32,
    iteration_cap: u32,
    planned_iterations: u32,
    actual_iterations: usize,
    screened_candidate_count: usize,
    validated_finalist_count: usize,
    seed_protocol: &'static str,
    master_seed: String,
    selection_policy: &'static str,
    search_seed_controls: RobustControlsDto,
    winner: Option<RobustCandidateDto>,
    ranked_finalists: Vec<RobustCandidateDto>,
}

fn carom_ball_position(
    scenario: &crate::dsl::DslScenario,
    ball_type: crate::BallType,
    label: &str,
) -> Result<crate::Inches2, JsValue> {
    let ball = scenario
        .game_state
        .select_ball(ball_type)
        .ok_or_else(|| JsValue::from_str(&format!("three-cushion setup is missing the {label}")))?;
    let mut position = ball.position.clone();
    position.resolve_shifts(&scenario.game_state.table_spec);
    Ok(crate::Inches2::new(
        scenario.game_state.table_spec.diamond_to_inches(position.x),
        scenario.game_state.table_spec.diamond_to_inches(position.y),
    ))
}

#[wasm_bindgen]
pub fn robust_three_cushion_shot_from_dsl(
    source: &str,
    iterations: u32,
    player_level: &str,
) -> Result<String, JsValue> {
    let player_level = player_level
        .parse::<crate::ThreeCushionPlayerLevel>()
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let (scenario, controls, preferred_cue) =
        crate::dsl::scenario_controls_and_preferred_cue_from_dsl(source)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
    if scenario.game_state.ty != crate::GameType::ThreeCushion {
        return Err(JsValue::from_str(
            "robust shot search requires game three_cushion",
        ));
    }
    if scenario.game_state.table_spec.kind != crate::TableKind::ThreeCushionCarom {
        return Err(JsValue::from_str(
            "robust shot search requires table three_cushion_carom_10ft",
        ));
    }
    let source_has_shot = controls.is_some();
    let current_controls = if let Some(controls) = controls {
        crate::RobustShotControls {
            heading: controls.heading_degrees,
            speed: controls.speed_ips,
            tip_side: controls.tip_side,
            tip_height: controls.tip_height,
            elevation: controls.cue_elevation_degrees,
        }
    } else {
        // The configured shot is only a search seed. A shotless setup still has
        // a complete search domain and uses this deterministic neutral seed.
        crate::RobustShotControls {
            heading: 0.0,
            speed: 150.0,
            tip_side: 0.0,
            tip_height: 0.0,
            elevation: 0.0,
        }
    };
    let layout = crate::ShotLayout::three_cushion(
        carom_ball_position(&scenario, crate::BallType::Cue, "white cue ball")?,
        carom_ball_position(&scenario, crate::BallType::YellowCue, "yellow cue ball")?,
        carom_ball_position(&scenario, crate::BallType::Red, "red object ball")?,
    )
    .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let cue = if let Some(shot) = scenario.shot.as_ref() {
        shot.cue_strike.clone()
    } else {
        preferred_cue.unwrap_or_else(crate::canonical_three_cushion_cue_config)
    };
    let simulation = scenario
        .preferred_simulation_physics(
            &crate::human_tuned_preview_motion_config(),
            crate::CollisionModel::ThrowAware,
            crate::RailModel::SpinAware,
        )
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    const DEFAULT_MAX_EVENTS: usize = 64;
    const MAX_EVENTS_CAP: usize = 256;
    let configured_max_events = simulation.max_events;
    if configured_max_events == Some(0) {
        return Err(JsValue::from_str(
            "robust shot search requires a positive max-events limit",
        ));
    }
    let max_events = configured_max_events
        .unwrap_or(DEFAULT_MAX_EVENTS)
        .min(MAX_EVENTS_CAP);
    let physics = crate::PhysicsProfile::new(
        scenario.game_state.table_spec.clone(),
        scenario.ball_set_physics_spec(),
        simulation.motion,
        simulation.collision_model,
        simulation.collision_config,
        simulation.rail_model,
        simulation.rail_profile,
    )
    .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let search = crate::run_player_robust_search(&crate::PlayerRobustSearchRequest {
        physics,
        layout,
        cue,
        shooter: crate::ThreeCushionShooter::Cue,
        current_controls,
        requested_evaluations: iterations,
        player_level,
        max_events,
    })
    .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let ranked_finalists = search
        .experiment
        .candidates
        .iter()
        .filter(|candidate| candidate.rank.is_some())
        .map(Into::into)
        .collect::<Vec<_>>();
    let response = RobustSearchResponseDto {
        player_level: search.player_level.key(),
        player_level_label: search.player_level.label(),
        noise_model: "independent-truncated-normal",
        parent_sigma_truncation: crate::ROBUST_NOISE_TRUNCATION_STANDARD_DEVIATIONS,
        conditional_standard_deviation_factor: 0.9866,
        shot_inaccuracy_sigmas: search.shot_inaccuracy.into(),
        source_has_shot,
        requested_iterations: search.budget.requested_evaluations,
        iteration_cap: search.budget.evaluation_cap,
        physics_profile: "dsl-preferred",
        max_events_per_iteration: max_events,
        max_events_per_iteration_cap: MAX_EVENTS_CAP,
        planned_iterations: search.budget.planned_evaluations,
        actual_iterations: search.actual_evaluations(),
        screened_candidate_count: search.experiment.candidates.len(),
        validated_finalist_count: search
            .experiment
            .candidates
            .iter()
            .filter(|candidate| candidate.validation.is_some())
            .count(),
        seed_protocol: crate::ROBUST_SEED_PROTOCOL,
        master_seed: search.master_seed.to_string(),
        selection_policy: "wilson95-lower-then-rate-then-id",
        search_seed_controls: search.search_seed_controls.into(),
        winner: search.winner().map(Into::into),
        ranked_finalists,
    };
    serialize_json(&response)
}
