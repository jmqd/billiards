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
