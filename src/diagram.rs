use crate::visualization::{
    DashedLineStyle, EventMarkerStyle, GhostBallStyle, LabelOverlayStyle, SmoothPolylineStyle,
};
use crate::{
    assets, drawing, BallSpec, BallType, DiagramBackground, DiagramRenderOptions, OverlayLayer,
};
use crate::{Position, TableSpec};
use bigdecimal::ToPrimitive;
use image::codecs::png::PngEncoder;
use image::imageops::{overlay, resize, FilterType};
use image::{ImageEncoder, ImageFormat, Rgba, RgbaImage};

const LEGACY_WIDTH_PX: f32 = 1089.0;
const LEGACY_HEIGHT_PX: f32 = 1938.0;
const PLAYFIELD_LEFT_PX: f32 = 110.0;
const PLAYFIELD_RIGHT_PX: f32 = 968.0;
const PLAYFIELD_TOP_PX: f32 = 110.0;
const PLAYFIELD_BOTTOM_PX: f32 = 1828.0;
const TABLE_DIAMONDS_X: f32 = 4.0;
const TABLE_DIAMONDS_Y: f32 = 8.0;
const PLAYFIELD_WIDTH_IN: f32 = 50.0;
const PLAYFIELD_LENGTH_IN: f32 = 100.0;
const CUSHION_WIDTH_IN: f32 = 1.9;
const DIAMOND_SIGHT_SETBACK_IN: f32 = 3.6875;
const DIAMOND_SIGHT_WIDTH_IN: f32 = 1.125;
const DIAMOND_SIGHT_HEIGHT_IN: f32 = 0.5;
const CORNER_POCKET_MOUTH_IN: f32 = 4.5;
const SIDE_POCKET_MOUTH_IN: f32 = 5.0;
const CORNER_POCKET_SHELF_IN: f32 = 1.75;
const SIDE_POCKET_SHELF_IN: f32 = 0.375;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagramOutputFormat {
    Png,
    Svg,
}

impl DiagramOutputFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Svg => "svg",
        }
    }

    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "png" => Some(Self::Png),
            "svg" => Some(Self::Svg),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagramLayerId {
    Table,
    OverlaysBelowBalls,
    Balls,
    OverlaysAboveBalls,
}

impl DiagramLayerId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Table => "table",
            Self::OverlaysBelowBalls => "overlays-below-balls",
            Self::Balls => "balls",
            Self::OverlaysAboveBalls => "overlays-above-balls",
        }
    }
}

impl From<OverlayLayer> for DiagramLayerId {
    fn from(value: OverlayLayer) -> Self {
        match value {
            OverlayLayer::BelowBalls => Self::OverlaysBelowBalls,
            OverlayLayer::AboveBalls => Self::OverlaysAboveBalls,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScenePoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiagramViewport {
    pub width_px: f32,
    pub height_px: f32,
    pub playfield_left_px: f32,
    pub playfield_right_px: f32,
    pub playfield_top_px: f32,
    pub playfield_bottom_px: f32,
}

impl Default for DiagramViewport {
    fn default() -> Self {
        Self {
            width_px: LEGACY_WIDTH_PX,
            height_px: LEGACY_HEIGHT_PX,
            playfield_left_px: PLAYFIELD_LEFT_PX,
            playfield_right_px: PLAYFIELD_RIGHT_PX,
            playfield_top_px: PLAYFIELD_TOP_PX,
            playfield_bottom_px: PLAYFIELD_BOTTOM_PX,
        }
    }
}

impl DiagramViewport {
    pub fn position_to_scene_point(&self, position: &Position) -> ScenePoint {
        let x_diamond = position
            .x
            .magnitude
            .to_f32()
            .expect("diagram x diamond should fit in f32");
        let y_diamond = position
            .y
            .magnitude
            .to_f32()
            .expect("diagram y diamond should fit in f32");

        ScenePoint {
            x: self.playfield_left_px
                + (x_diamond / TABLE_DIAMONDS_X)
                    * (self.playfield_right_px - self.playfield_left_px),
            y: self.playfield_bottom_px
                - (y_diamond / TABLE_DIAMONDS_Y)
                    * (self.playfield_bottom_px - self.playfield_top_px),
        }
    }

    pub fn ball_radius_px(&self, table_spec: &TableSpec, ball_spec: &BallSpec) -> f32 {
        let radius_diamonds = table_spec
            .inches_to_diamond(ball_spec.radius.clone())
            .magnitude
            .to_f32()
            .expect("ball radius diamond value should fit in f32");
        let px_per_diamond_x =
            (self.playfield_right_px - self.playfield_left_px) / TABLE_DIAMONDS_X;
        let px_per_diamond_y =
            (self.playfield_bottom_px - self.playfield_top_px) / TABLE_DIAMONDS_Y;
        radius_diamonds * px_per_diamond_x.min(px_per_diamond_y)
    }

    fn ball_diameter_px(&self, table_spec: &TableSpec, ball_spec: &BallSpec) -> u32 {
        (2.0 * self.ball_radius_px(table_spec, ball_spec))
            .round()
            .max(1.0) as u32
    }
}

#[derive(Clone, Debug)]
pub struct DiagramBall {
    pub ty: BallType,
    pub position: Position,
    pub spec: BallSpec,
}

#[derive(Clone, Debug)]
pub enum DiagramElement {
    DashedLine {
        start: Position,
        end: Position,
        style: DashedLineStyle,
    },
    SmoothPolyline {
        points: Vec<Position>,
        style: SmoothPolylineStyle,
    },
    GhostBall {
        center: Position,
        style: GhostBallStyle,
    },
    CircleMarker {
        center: Position,
        style: EventMarkerStyle,
    },
    TextLabel {
        anchor: Position,
        text: String,
        style: LabelOverlayStyle,
    },
}

impl DiagramElement {
    pub fn layer(&self) -> DiagramLayerId {
        match self {
            Self::DashedLine { style, .. } => style.layer.into(),
            Self::SmoothPolyline { style, .. } => style.layer.into(),
            Self::GhostBall { style, .. } => style.layer.into(),
            Self::CircleMarker { style, .. } => style.layer.into(),
            Self::TextLabel { style, .. } => style.layer.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DiagramScene {
    pub table_spec: TableSpec,
    pub viewport: DiagramViewport,
    pub background: DiagramBackground,
    pub balls: Vec<DiagramBall>,
    pub elements: Vec<DiagramElement>,
}

impl DiagramScene {
    pub fn elements_for_layer(
        &self,
        layer: DiagramLayerId,
    ) -> impl Iterator<Item = &DiagramElement> {
        self.elements
            .iter()
            .filter(move |element| element.layer() == layer)
    }
}

pub trait DiagramBackend {
    type Output;

    fn render(scene: &DiagramScene, options: &DiagramRenderOptions) -> Self::Output;
}

pub struct PngBackend;

impl DiagramBackend for PngBackend {
    type Output = Vec<u8>;

    fn render(scene: &DiagramScene, options: &DiagramRenderOptions) -> Self::Output {
        let table_asset: RgbaImage =
            image::load_from_memory_with_format(assets::TABLE_DIAGRAM, ImageFormat::Png)
                .expect("broken table asset")
                .into_rgba8();
        let (tw, th) = table_asset.dimensions();
        let mut table = match scene.background {
            DiagramBackground::Table => table_asset,
            DiagramBackground::Transparent => RgbaImage::new(tw, th),
        };

        draw_raster_elements_for_layer(scene, DiagramLayerId::OverlaysBelowBalls, &mut table);
        draw_raster_balls(scene, &mut table, tw, th);
        draw_raster_elements_for_layer(scene, DiagramLayerId::OverlaysAboveBalls, &mut table);

        let scale_factor = options.scale_factor.max(1);
        let output = if scale_factor == 1 {
            table
        } else {
            resize(
                &table,
                tw * scale_factor,
                th * scale_factor,
                FilterType::CatmullRom,
            )
        };
        let (ow, oh) = output.dimensions();

        let mut buf = Vec::new();
        PngEncoder::new(&mut buf)
            .write_image(&output, ow, oh, image::ColorType::Rgba8.into())
            .expect("PNG encode failed");
        buf
    }
}

pub struct SvgBackend;

impl DiagramBackend for SvgBackend {
    type Output = String;

    fn render(scene: &DiagramScene, _options: &DiagramRenderOptions) -> Self::Output {
        let mut svg = String::new();
        svg.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {:.0} {:.0}\" width=\"{:.0}\" height=\"{:.0}\" role=\"img\" aria-label=\"Billiards diagram\" preserveAspectRatio=\"xMidYMid meet\">\n",
            scene.viewport.width_px,
            scene.viewport.height_px,
            scene.viewport.width_px,
            scene.viewport.height_px
        ));
        svg.push_str("<style>\n");
        svg.push_str(".diagram-layer{vector-effect:non-scaling-stroke}\n");
        svg.push_str(".ball-label{font-family:Inter,Arial,sans-serif;font-weight:700;text-anchor:middle;dominant-baseline:central;pointer-events:none}\n");
        svg.push_str(".overlay-label{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-weight:700;dominant-baseline:central}\n");
        svg.push_str(".table-cloth{fill:#176640}.table-rail{fill:#4a2d18}.table-cushion{fill:#0d5638}.table-pocket{fill:#020202}.table-pocket-facing{stroke:#211914;stroke-width:4;stroke-linecap:round}.table-diamond{fill:#f3ead1;stroke:#8f7f55;stroke-width:.75;opacity:.94}\n");
        svg.push_str("</style>\n");

        svg.push_str(&format!(
            "<g class=\"diagram-layer\" id=\"layer-{}\" data-layer=\"{}\">\n",
            DiagramLayerId::Table.as_str(),
            DiagramLayerId::Table.as_str()
        ));
        if scene.background == DiagramBackground::Table {
            push_svg_table(&mut svg, scene.viewport);
        }
        svg.push_str("</g>\n");

        push_svg_element_layer(&mut svg, scene, DiagramLayerId::OverlaysBelowBalls);
        push_svg_balls(&mut svg, scene);
        push_svg_element_layer(&mut svg, scene, DiagramLayerId::OverlaysAboveBalls);

        svg.push_str("</svg>\n");
        svg
    }
}

pub fn render_scene_to_bytes(
    scene: &DiagramScene,
    format: DiagramOutputFormat,
    options: &DiagramRenderOptions,
) -> Vec<u8> {
    match format {
        DiagramOutputFormat::Png => PngBackend::render(scene, options),
        DiagramOutputFormat::Svg => SvgBackend::render(scene, options).into_bytes(),
    }
}

fn draw_raster_elements_for_layer(
    scene: &DiagramScene,
    layer: DiagramLayerId,
    table: &mut RgbaImage,
) {
    for element in scene.elements_for_layer(layer) {
        match element {
            DiagramElement::DashedLine { start, end, style } => {
                drawing::draw_dashed_line_thick_mut(
                    table,
                    start,
                    end,
                    style.dash_px,
                    style.gap_px,
                    style.width_px,
                    style.color,
                );
            }
            DiagramElement::SmoothPolyline { points, style } => {
                drawing::draw_smooth_polyline_mut(table, points, style.width_px, style.color);
            }
            DiagramElement::GhostBall { center, style } => {
                drawing::draw_ghost_ball_mut(
                    table,
                    center,
                    scene
                        .viewport
                        .ball_diameter_px(&scene.table_spec, &BallSpec::default()),
                    style.fill_color,
                    style.outline_color,
                );
            }
            DiagramElement::CircleMarker { center, style } => {
                drawing::draw_filled_circle_marker_mut(table, center, style.radius_px, style.color);
            }
            DiagramElement::TextLabel {
                anchor,
                text,
                style,
            } => {
                drawing::draw_text_label_mut(
                    table,
                    anchor,
                    text,
                    style.offset_x_px,
                    style.offset_y_px,
                    style.scale_px,
                    style.color,
                );
            }
        }
    }
}

fn draw_raster_balls(scene: &DiagramScene, table: &mut RgbaImage, tw: u32, th: u32) {
    for ball in &scene.balls {
        let ball_png = assets::ball_img(ball.ty.clone());
        let mut ball_img: RgbaImage =
            image::load_from_memory_with_format(&ball_png, ImageFormat::Png)
                .expect("bad ball image")
                .into_rgba8();
        let ball_diameter_px = scene
            .viewport
            .ball_diameter_px(&scene.table_spec, &ball.spec);
        ball_img = resize(
            &ball_img,
            ball_diameter_px,
            ball_diameter_px,
            FilterType::CatmullRom,
        );
        let (bw, bh) = ball_img.dimensions();
        let center = scene.viewport.position_to_scene_point(&ball.position);
        let px = center.x.round() as i32;
        let py = center.y.round() as i32;
        let mut px_shifted = px - (bw as i32 / 2);
        let mut py_shifted = py - (bh as i32 / 2);
        px_shifted = px_shifted.clamp(0, (tw - bw) as i32);
        py_shifted = py_shifted.clamp(0, (th - bh) as i32);
        overlay(&mut *table, &ball_img, px_shifted.into(), py_shifted.into());
    }
}

fn push_svg_table(svg: &mut String, viewport: DiagramViewport) {
    // WPA tournament dimensions used by Diamond-style 9 ft tables:
    // 100 x 50 in playing surface, sights 3 11/16 in from cushion nose,
    // 4.5 in corner mouths, 5.0 in side mouths, and cut pockets instead of
    // circular holes drawn on the playfield.
    let w = viewport.width_px;
    let h = viewport.height_px;
    let left = viewport.playfield_left_px;
    let right = viewport.playfield_right_px;
    let top = viewport.playfield_top_px;
    let bottom = viewport.playfield_bottom_px;
    let cloth_w = right - left;
    let cloth_h = bottom - top;
    let center_y = (top + bottom) * 0.5;

    let cushion_x = viewport.x_inches(CUSHION_WIDTH_IN);
    let cushion_y = viewport.y_inches(CUSHION_WIDTH_IN);
    let corner_run_x = viewport.x_inches(CORNER_POCKET_MOUTH_IN / 2.0_f32.sqrt());
    let corner_run_y = viewport.y_inches(CORNER_POCKET_MOUTH_IN / 2.0_f32.sqrt());
    let corner_shelf_x = viewport.x_inches(CORNER_POCKET_SHELF_IN);
    let corner_shelf_y = viewport.y_inches(CORNER_POCKET_SHELF_IN);
    let side_mouth_y = viewport.y_inches(SIDE_POCKET_MOUTH_IN);
    let side_shelf_x = viewport.x_inches(SIDE_POCKET_SHELF_IN);

    svg.push_str(&format!(
        "<rect class=\"table-rail\" x=\"0\" y=\"0\" width=\"{w:.3}\" height=\"{h:.3}\" rx=\"58\"/>\n"
    ));
    svg.push_str(&format!(
        "<rect class=\"table-cloth\" x=\"{left:.3}\" y=\"{top:.3}\" width=\"{cloth_w:.3}\" height=\"{cloth_h:.3}\"/>\n"
    ));

    push_svg_cushion_rect(
        svg,
        left + corner_run_x,
        top - cushion_y,
        cloth_w - 2.0 * corner_run_x,
        cushion_y,
    );
    push_svg_cushion_rect(
        svg,
        left + corner_run_x,
        bottom,
        cloth_w - 2.0 * corner_run_x,
        cushion_y,
    );
    push_svg_cushion_rect(
        svg,
        left - cushion_x,
        top + corner_run_y,
        cushion_x,
        center_y - side_mouth_y * 0.5 - top - corner_run_y,
    );
    push_svg_cushion_rect(
        svg,
        left - cushion_x,
        center_y + side_mouth_y * 0.5,
        cushion_x,
        bottom - corner_run_y - center_y - side_mouth_y * 0.5,
    );
    push_svg_cushion_rect(
        svg,
        right,
        top + corner_run_y,
        cushion_x,
        center_y - side_mouth_y * 0.5 - top - corner_run_y,
    );
    push_svg_cushion_rect(
        svg,
        right,
        center_y + side_mouth_y * 0.5,
        cushion_x,
        bottom - corner_run_y - center_y - side_mouth_y * 0.5,
    );

    for (corner_x, corner_y, x_sign, y_sign) in [
        (left, top, -1.0, -1.0),
        (right, top, 1.0, -1.0),
        (left, bottom, -1.0, 1.0),
        (right, bottom, 1.0, 1.0),
    ] {
        push_svg_corner_pocket(
            svg,
            corner_x,
            corner_y,
            x_sign,
            y_sign,
            corner_run_x,
            corner_run_y,
            corner_shelf_x,
            corner_shelf_y,
        );
    }
    push_svg_side_pocket(
        svg,
        left,
        center_y,
        -1.0,
        side_mouth_y,
        side_shelf_x,
        cushion_x,
    );
    push_svg_side_pocket(
        svg,
        right,
        center_y,
        1.0,
        side_mouth_y,
        side_shelf_x,
        cushion_x,
    );

    push_svg_table_sights(svg, viewport);
}

impl DiagramViewport {
    fn x_inches(self, inches: f32) -> f32 {
        inches * (self.playfield_right_px - self.playfield_left_px) / PLAYFIELD_WIDTH_IN
    }

    fn y_inches(self, inches: f32) -> f32 {
        inches * (self.playfield_bottom_px - self.playfield_top_px) / PLAYFIELD_LENGTH_IN
    }
}

fn push_svg_cushion_rect(svg: &mut String, x: f32, y: f32, width: f32, height: f32) {
    svg.push_str(&format!(
        "<rect class=\"table-cushion\" x=\"{x:.3}\" y=\"{y:.3}\" width=\"{width:.3}\" height=\"{height:.3}\"/>\n"
    ));
}

fn push_svg_corner_pocket(
    svg: &mut String,
    corner_x: f32,
    corner_y: f32,
    x_sign: f32,
    y_sign: f32,
    run_x: f32,
    run_y: f32,
    shelf_x: f32,
    shelf_y: f32,
) {
    let horizontal_x = corner_x - x_sign * run_x;
    let horizontal_y = corner_y;
    let vertical_x = corner_x;
    let vertical_y = corner_y - y_sign * run_y;
    let inner_x = corner_x - x_sign * shelf_x;
    let inner_y = corner_y - y_sign * shelf_y;
    let well_x = corner_x + x_sign * shelf_x * 1.45;
    let well_y = corner_y + y_sign * shelf_y * 1.45;

    svg.push_str(&format!(
        "<path class=\"table-pocket\" data-pocket=\"corner\" d=\"M {horizontal_x:.3} {horizontal_y:.3} Q {inner_x:.3} {inner_y:.3} {vertical_x:.3} {vertical_y:.3} Q {well_x:.3} {well_y:.3} {horizontal_x:.3} {horizontal_y:.3} Z\"/>\n"
    ));
    svg.push_str(&format!(
        "<line class=\"table-pocket-facing\" x1=\"{horizontal_x:.3}\" y1=\"{horizontal_y:.3}\" x2=\"{inner_x:.3}\" y2=\"{inner_y:.3}\"/>\n"
    ));
    svg.push_str(&format!(
        "<line class=\"table-pocket-facing\" x1=\"{vertical_x:.3}\" y1=\"{vertical_y:.3}\" x2=\"{inner_x:.3}\" y2=\"{inner_y:.3}\"/>\n"
    ));
}

fn push_svg_side_pocket(
    svg: &mut String,
    rail_x: f32,
    center_y: f32,
    x_sign: f32,
    mouth_y: f32,
    shelf_x: f32,
    cushion_x: f32,
) {
    let top_y = center_y - mouth_y * 0.5;
    let bottom_y = center_y + mouth_y * 0.5;
    let throat_x = rail_x + x_sign * (shelf_x + cushion_x * 1.15);
    let lip_x = rail_x + x_sign * shelf_x;

    svg.push_str(&format!(
        "<path class=\"table-pocket\" data-pocket=\"side\" d=\"M {rail_x:.3} {top_y:.3} Q {lip_x:.3} {center_y:.3} {rail_x:.3} {bottom_y:.3} Q {throat_x:.3} {center_y:.3} {rail_x:.3} {top_y:.3} Z\"/>\n"
    ));
    svg.push_str(&format!(
        "<line class=\"table-pocket-facing\" x1=\"{rail_x:.3}\" y1=\"{top_y:.3}\" x2=\"{lip_x:.3}\" y2=\"{center_y:.3}\"/>\n"
    ));
    svg.push_str(&format!(
        "<line class=\"table-pocket-facing\" x1=\"{rail_x:.3}\" y1=\"{bottom_y:.3}\" x2=\"{lip_x:.3}\" y2=\"{center_y:.3}\"/>\n"
    ));
}

fn push_svg_table_sights(svg: &mut String, viewport: DiagramViewport) {
    let left = viewport.playfield_left_px;
    let right = viewport.playfield_right_px;
    let top = viewport.playfield_top_px;
    let bottom = viewport.playfield_bottom_px;
    let cloth_w = right - left;
    let cloth_h = bottom - top;
    let sight_setback_x = viewport.x_inches(DIAMOND_SIGHT_SETBACK_IN);
    let sight_setback_y = viewport.y_inches(DIAMOND_SIGHT_SETBACK_IN);
    let sight_half_along_x = viewport.x_inches(DIAMOND_SIGHT_WIDTH_IN) * 0.5;
    let sight_half_along_y = viewport.y_inches(DIAMOND_SIGHT_WIDTH_IN) * 0.5;
    let sight_half_cross_x = viewport.x_inches(DIAMOND_SIGHT_HEIGHT_IN) * 0.5;
    let sight_half_cross_y = viewport.y_inches(DIAMOND_SIGHT_HEIGHT_IN) * 0.5;

    for fraction in [0.25, 0.5, 0.75] {
        let x = left + fraction * cloth_w;
        push_svg_horizontal_sight(
            svg,
            x,
            top - sight_setback_y,
            sight_half_along_x,
            sight_half_cross_y,
        );
        push_svg_horizontal_sight(
            svg,
            x,
            bottom + sight_setback_y,
            sight_half_along_x,
            sight_half_cross_y,
        );
    }
    for fraction in [0.125, 0.25, 0.375, 0.625, 0.75, 0.875] {
        let y = bottom - fraction * cloth_h;
        push_svg_vertical_sight(
            svg,
            left - sight_setback_x,
            y,
            sight_half_along_y,
            sight_half_cross_x,
        );
        push_svg_vertical_sight(
            svg,
            right + sight_setback_x,
            y,
            sight_half_along_y,
            sight_half_cross_x,
        );
    }
}

fn push_svg_horizontal_sight(svg: &mut String, cx: f32, cy: f32, half_along: f32, half_cross: f32) {
    svg.push_str(&format!(
        "<polygon class=\"table-diamond\" points=\"{:.3},{:.3} {:.3},{:.3} {:.3},{:.3} {:.3},{:.3}\"/>\n",
        cx,
        cy - half_cross,
        cx + half_along,
        cy,
        cx,
        cy + half_cross,
        cx - half_along,
        cy
    ));
}

fn push_svg_vertical_sight(svg: &mut String, cx: f32, cy: f32, half_along: f32, half_cross: f32) {
    svg.push_str(&format!(
        "<polygon class=\"table-diamond\" points=\"{:.3},{:.3} {:.3},{:.3} {:.3},{:.3} {:.3},{:.3}\"/>\n",
        cx,
        cy - half_along,
        cx + half_cross,
        cy,
        cx,
        cy + half_along,
        cx - half_cross,
        cy
    ));
}

fn push_svg_element_layer(svg: &mut String, scene: &DiagramScene, layer: DiagramLayerId) {
    svg.push_str(&format!(
        "<g class=\"diagram-layer\" id=\"layer-{}\" data-layer=\"{}\">\n",
        layer.as_str(),
        layer.as_str()
    ));
    for element in scene.elements_for_layer(layer) {
        push_svg_element(svg, scene, element);
    }
    svg.push_str("</g>\n");
}

fn push_svg_element(svg: &mut String, scene: &DiagramScene, element: &DiagramElement) {
    match element {
        DiagramElement::DashedLine { start, end, style } => {
            let start = scene.viewport.position_to_scene_point(start);
            let end = scene.viewport.position_to_scene_point(end);
            let (stroke, opacity) = svg_color(style.color);
            svg.push_str(&format!(
                "<line class=\"overlay dashed-line\" x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{:.3}\" stroke-linecap=\"round\" stroke-dasharray=\"{:.3} {:.3}\" fill=\"none\"/>\n",
                start.x,
                start.y,
                end.x,
                end.y,
                stroke,
                opacity,
                style.width_px,
                style.dash_px,
                style.gap_px
            ));
        }
        DiagramElement::SmoothPolyline { points, style } => {
            if points.len() < 2 {
                return;
            }
            let (stroke, opacity) = svg_color(style.color);
            let points = points
                .iter()
                .map(|point| {
                    let point = scene.viewport.position_to_scene_point(point);
                    format!("{:.3},{:.3}", point.x, point.y)
                })
                .collect::<Vec<_>>()
                .join(" ");
            svg.push_str(&format!(
                "<polyline class=\"overlay smooth-polyline\" points=\"{}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{:.3}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" fill=\"none\"/>\n",
                points, stroke, opacity, style.width_px
            ));
        }
        DiagramElement::GhostBall { center, style } => {
            let center = scene.viewport.position_to_scene_point(center);
            let radius = scene
                .viewport
                .ball_radius_px(&scene.table_spec, &BallSpec::default());
            let (fill, fill_opacity) = svg_color(style.fill_color);
            let (stroke, stroke_opacity) = svg_color(style.outline_color);
            svg.push_str(&format!(
                "<circle class=\"overlay ghost-ball\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"{:.3}\" fill=\"{}\" fill-opacity=\"{:.3}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"2\" stroke-dasharray=\"3 8\"/>\n",
                center.x, center.y, radius, fill, fill_opacity, stroke, stroke_opacity
            ));
        }
        DiagramElement::CircleMarker { center, style } => {
            let center = scene.viewport.position_to_scene_point(center);
            let (fill, opacity) = svg_color(style.color);
            svg.push_str(&format!(
                "<circle class=\"overlay event-marker\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"{:.3}\" fill=\"{}\" fill-opacity=\"{:.3}\"/>\n",
                center.x, center.y, style.radius_px, fill, opacity
            ));
        }
        DiagramElement::TextLabel {
            anchor,
            text,
            style,
        } => {
            let anchor = scene.viewport.position_to_scene_point(anchor);
            let (fill, opacity) = svg_color(style.color);
            svg.push_str(&format!(
                "<text class=\"overlay overlay-label\" x=\"{:.3}\" y=\"{:.3}\" fill=\"{}\" fill-opacity=\"{:.3}\" font-size=\"{}\">{}</text>\n",
                anchor.x + style.offset_x_px as f32,
                anchor.y + style.offset_y_px as f32,
                fill,
                opacity,
                style.scale_px.max(1) * 7,
                escape_xml(text)
            ));
        }
    }
}

fn push_svg_balls(svg: &mut String, scene: &DiagramScene) {
    svg.push_str(&format!(
        "<g class=\"diagram-layer\" id=\"layer-{}\" data-layer=\"{}\">\n",
        DiagramLayerId::Balls.as_str(),
        DiagramLayerId::Balls.as_str()
    ));
    for ball in &scene.balls {
        let center = scene.viewport.position_to_scene_point(&ball.position);
        let radius = scene.viewport.ball_radius_px(&scene.table_spec, &ball.spec);
        let visual = ball_visual(&ball.ty);
        let label = ball_label(&ball.ty);
        svg.push_str(&format!(
            "<g class=\"ball ball-{}\" data-ball=\"{}\" transform=\"translate({:.3} {:.3})\">\n",
            visual.class_name, visual.class_name, center.x, center.y
        ));
        svg.push_str(&format!(
            "<circle r=\"{radius:.3}\" fill=\"{}\" stroke=\"#111\" stroke-width=\"1.5\"/>\n",
            visual.fill
        ));
        svg.push_str(&format!(
            "<circle r=\"{:.3}\" fill=\"none\" stroke=\"rgba(255,255,255,.45)\" stroke-width=\"2\"/>\n",
            radius * 0.72
        ));
        if let Some(label) = label {
            let label_radius = (radius * 0.42).max(7.0);
            svg.push_str(&format!(
                "<circle r=\"{label_radius:.3}\" fill=\"#f8f4e8\" stroke=\"#111\" stroke-width=\".75\"/>\n"
            ));
            svg.push_str(&format!(
                "<text class=\"ball-label\" y=\".5\" fill=\"#111\" font-size=\"{:.3}\">{}</text>\n",
                (radius * 0.58).max(10.0),
                label
            ));
        }
        svg.push_str("</g>\n");
    }
    svg.push_str("</g>\n");
}

struct BallVisual {
    fill: &'static str,
    class_name: &'static str,
}

fn ball_visual(ball_type: &BallType) -> BallVisual {
    match ball_type {
        BallType::Cue => BallVisual {
            fill: "#f8f4e8",
            class_name: "cue",
        },
        BallType::One | BallType::Nine => BallVisual {
            fill: "#f1c232",
            class_name: if matches!(ball_type, BallType::One) {
                "one"
            } else {
                "nine"
            },
        },
        BallType::Two => BallVisual {
            fill: "#2458c8",
            class_name: "two",
        },
        BallType::Three => BallVisual {
            fill: "#c82828",
            class_name: "three",
        },
        BallType::Four => BallVisual {
            fill: "#6f3fa8",
            class_name: "four",
        },
        BallType::Five => BallVisual {
            fill: "#e27a22",
            class_name: "five",
        },
        BallType::Six => BallVisual {
            fill: "#25834b",
            class_name: "six",
        },
        BallType::Seven => BallVisual {
            fill: "#8f2d20",
            class_name: "seven",
        },
        BallType::Eight => BallVisual {
            fill: "#111111",
            class_name: "eight",
        },
    }
}

fn ball_label(ball_type: &BallType) -> Option<&'static str> {
    match ball_type {
        BallType::Cue => None,
        BallType::One => Some("1"),
        BallType::Two => Some("2"),
        BallType::Three => Some("3"),
        BallType::Four => Some("4"),
        BallType::Five => Some("5"),
        BallType::Six => Some("6"),
        BallType::Seven => Some("7"),
        BallType::Eight => Some("8"),
        BallType::Nine => Some("9"),
    }
}

fn svg_color(color: Rgba<u8>) -> (String, f32) {
    (
        format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2]),
        (color[3] as f32 / 255.0).clamp(0.0, 1.0),
    )
}

fn escape_xml(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}
