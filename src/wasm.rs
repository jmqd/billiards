use wasm_bindgen::prelude::*;

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
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    Ok(match controls {
        Some(controls) => format!(
            concat!(
                "{{\"headingDegrees\":{},",
                "\"speedIps\":{},",
                "\"tipSide\":{},",
                "\"tipHeight\":{},",
                "\"tipMaxRadius\":{},",
                "\"cueElevationDegrees\":{},",
                "\"cueElevationExplicit\":{},",
                "\"speedMaxIps\":{},",
                "\"cueElevationMaxDegrees\":{}}}"
            ),
            controls.heading_degrees,
            controls.speed_ips,
            controls.tip_side,
            controls.tip_height,
            controls.tip_max_radius,
            controls.cue_elevation_degrees,
            controls.cue_elevation_explicit,
            controls.speed_max_ips,
            controls.cue_elevation_max_degrees,
        ),
        None => "null".to_string(),
    })
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
    crate::dsl::update_shot_control_in_dsl(source, control, value)
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub fn update_shot_tip_in_dsl(source: &str, side: f64, height: f64) -> Result<String, JsValue> {
    crate::dsl::update_shot_tip_in_dsl(source, side, height)
        .map_err(|error| JsValue::from_str(&error.to_string()))
}
