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
