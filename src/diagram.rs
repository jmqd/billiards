use crate::visualization::{
    DashedLineStyle, EventMarkerStyle, GhostBallStyle, HeadingChevronStyle, LabelOverlayStyle,
    SmoothPolylineStyle, SpinGlyphStyle,
};
use crate::{assets, drawing};
use crate::{
    Angle, AngularVelocity3, BallSpec, BallType, DiagramBackground, DiagramRenderOptions, Inches,
    OverlayLayer, Position, TableKind, TableSpec, Velocity2,
};
use bigdecimal::ToPrimitive;
use image::codecs::png::PngEncoder;
use image::imageops::{overlay, resize, FilterType};
use image::{ImageEncoder, ImageFormat, Rgba, RgbaImage};
use imageproc::drawing::{
    draw_filled_circle_mut, draw_line_segment_mut, draw_polygon_mut,
};
use imageproc::point::Point;

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
const DIAMOND_SIGHT_WIDTH_IN: f32 = 1.35;
const DIAMOND_SIGHT_HEIGHT_IN: f32 = 0.62;
const CORNER_POCKET_MOUTH_IN: f32 = 4.5;
const SIDE_POCKET_MOUTH_IN: f32 = 5.0;
const CORNER_POCKET_SHELF_IN: f32 = 1.75;
const CORNER_POCKET_WELL_DIAMETER_IN: f32 = 6.0;
const CORNER_POCKET_BACK_TANGENT_RATIO: f32 = 0.70;
// These factors affect only the SVG plan-view artwork. They deliberately
// leave the nominal table dimensions above unchanged.
const SIDE_POCKET_DRAWING_MOUTH_SCALE: f32 = 1.0;
const SIDE_POCKET_DRAWING_LIP_DEPTH_IN: f32 = 2.64103;
const SIDE_POCKET_DRAWING_FIRST_CONTROL_RUN_IN: f32 = 1.0 / 3.0;
const SIDE_POCKET_DRAWING_SECOND_CONTROL_DEPTH_IN: f32 = 3.70256;
const SIDE_POCKET_DRAWING_OUTER_ENDPOINT_DEPTH_IN: f32 = 4.95280;
const SIDE_POCKET_DRAWING_OUTER_MID_DEPTH_IN: f32 = 5.30664;
const SIDE_POCKET_DRAWING_SHOULDER_OFFSET_IN: f32 = 1.70;
const SIDE_POCKET_DRAWING_TUCK_OFFSET_IN: f32 = 0.20;
const SIDE_POCKET_DRAWING_SHELF_SAGITTA_IN: f32 = 0.32;
const SIDE_POCKET_DRAWING_SHELF_INNER_SAGITTA_IN: f32 = 0.36;
// Reducing the shelf sagitta to 40% makes its effective radius more than
// twice the previous tight side-pocket shelf curve.
const SIDE_POCKET_DRAWING_SHELF_SAGITTA_SCALE: f32 = 0.40;
const SIDE_POCKET_DRAWING_LINER_SCALE: f32 = 1.37;
// Side-pocket mouth cut angle in the rendered top-view bed-edge frame.
const SIDE_POCKET_CUT_ANGLE_DEG: f32 = 8.0;
const CUSHION_BEVEL_IN: f32 = 1.0;

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
    HeadingChevron {
        tip: Position,
        heading: Angle,
        style: HeadingChevronStyle,
    },
    GhostBall {
        center: Position,
        style: GhostBallStyle,
    },
    OriginMarker {
        center: Position,
        style: LabelOverlayStyle,
    },
    CircleMarker {
        center: Position,
        style: EventMarkerStyle,
        event_label: Option<String>,
        event_title: Option<String>,
    },
    TextLabel {
        anchor: Position,
        text: String,
        style: LabelOverlayStyle,
    },
    SpinGlyph {
        center: Position,
        angular_velocity: AngularVelocity3,
        linear_velocity: Velocity2,
        ball_radius: Inches,
        style: SpinGlyphStyle,
    },
}

impl DiagramElement {
    pub fn layer(&self) -> DiagramLayerId {
        match self {
            Self::DashedLine { style, .. } => style.layer.into(),
            Self::SmoothPolyline { style, .. } => style.layer.into(),
            Self::HeadingChevron { style, .. } => style.layer.into(),
            Self::GhostBall { style, .. } => style.layer.into(),
            Self::OriginMarker { style, .. } | Self::TextLabel { style, .. } => style.layer.into(),
            Self::CircleMarker { style, .. } => style.layer.into(),
            Self::SpinGlyph { style, .. } => style.layer.into(),
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
            let output_width = tw
                .checked_mul(scale_factor)
                .expect("scaled PNG width overflow");
            let output_height = th
                .checked_mul(scale_factor)
                .expect("scaled PNG height overflow");
            resize(
                &table,
                output_width,
                output_height,
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

    fn render(scene: &DiagramScene, options: &DiagramRenderOptions) -> Self::Output {
        let mut svg = String::new();
        let unrotated_width_px = scene.viewport.width_px;
        let unrotated_height_px = scene.viewport.height_px;
        let scale_factor = f64::from(options.scale_factor.max(1));
        let scaled_width_px = f64::from(unrotated_height_px) * scale_factor;
        let scaled_height_px = f64::from(unrotated_width_px) * scale_factor;
        svg.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {:.0} {:.0}\" width=\"{:.0}\" height=\"{:.0}\" role=\"img\" aria-label=\"Billiards diagram\" preserveAspectRatio=\"xMidYMid meet\" data-orientation=\"clockwise\">\n",
            unrotated_height_px, unrotated_width_px, scaled_width_px, scaled_height_px
        ));
        svg.push_str("<style>\n");
        svg.push_str(".diagram-layer{vector-effect:non-scaling-stroke}\n");
        svg.push_str(".ball-label{font-family:Inter,Arial,sans-serif;font-weight:700;text-anchor:middle;dominant-baseline:central;pointer-events:none}\n");
        svg.push_str(".ball-spin-glyph{pointer-events:none}.ball-spin-backplate{fill:#fffaf1;fill-opacity:.98;stroke:#111;stroke-opacity:.9}.ball-spin-vector,.ball-spin-vector-halo,.ball-spin-z,.ball-spin-z-halo,.ball-spin-stun-x-halo,.ball-spin-stun-x-mark{fill:none;stroke-linecap:round;stroke-linejoin:round;vector-effect:non-scaling-stroke}.ball-spin-vector-halo,.ball-spin-z-halo,.ball-spin-stun-x-halo{stroke:#fffaf1;stroke-opacity:1}.ball-spin-arrowhead,.ball-spin-z-head{stroke:#fffaf1;stroke-linejoin:round;vector-effect:non-scaling-stroke}.ball-spin-stun-x-mark{stroke:#7f858c;stroke-opacity:.98}\n");
        svg.push_str(".overlay-label{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-weight:700;dominant-baseline:central}.origin-marker{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-weight:800;text-anchor:middle;dominant-baseline:central;pointer-events:none}.event-marker[data-event-label]{cursor:help}\n");
        svg.push_str(".table-cloth{fill:url(#tournament-blue-cloth)}.table-cloth-texture{fill:url(#cloth-weave);opacity:.20}");
        svg.push_str(".table-rail{fill:url(#rosewood-rail)}.table-rail-grain{opacity:.62}.table-rail-grain-horizontal{fill:url(#rosewood-grain)}.table-rail-grain-vertical{fill:url(#rosewood-grain-vertical)}.table-rail-inner-shadow{fill:none;stroke:#210b08;stroke-width:10;opacity:.72}");
        svg.push_str(".table-cushion{fill:url(#blue-cushion)}.table-cushion-nose{stroke:#4bd2ea;stroke-width:3;stroke-linecap:round;opacity:.8}.table-cushion-back{stroke:#056a87;stroke-width:3;stroke-linecap:round;opacity:.65}");
        svg.push_str(".table-pocket-well{fill:url(#pocket-well);stroke:none}.table-pocket-leather{fill:none;stroke:url(#pocket-leather);stroke-linecap:round;stroke-linejoin:round;opacity:.96}.table-pocket-leather-highlight{fill:none;stroke:#8d8377;stroke-width:2;stroke-linecap:round;stroke-linejoin:round;opacity:.22}");
        svg.push_str(".table-pocket-shelf{fill:url(#tournament-blue-cloth);stroke:none;opacity:1}.table-pocket-shelf-texture{fill:url(#cloth-weave);stroke:none;opacity:.20;pointer-events:none}");
        svg.push_str(".table-diamond{fill:#f6f0de;stroke:#9b8c63;stroke-width:.75;opacity:.94}\n");
        svg.push_str(".carom-table .table-rail{fill:url(#carom-wood-rail)}.carom-table .table-cloth{fill:url(#heated-carom-cloth)}.carom-table .table-cloth-texture{opacity:.16}.carom-table .table-cushion{fill:url(#heated-carom-cushion)}.carom-table .table-cushion-nose{stroke:#88ecff;stroke-width:3.2}.carom-table .table-cushion-back{stroke:#064f69;stroke-width:3.2}.carom-table .table-rail-inner-shadow{stroke:#0b0705;stroke-width:12;opacity:.58}\n");
        svg.push_str("</style>\n");
        push_svg_table_defs(&mut svg, scene.viewport);
        svg.push_str(&format!(
            "<g class=\"diagram-orientation\" transform=\"translate({:.0} 0) rotate(90)\">\n",
            unrotated_height_px
        ));

        svg.push_str(&format!(
            "<g class=\"diagram-layer\" id=\"layer-{}\" data-layer=\"{}\">\n",
            DiagramLayerId::Table.as_str(),
            DiagramLayerId::Table.as_str()
        ));
        if scene.background == DiagramBackground::Table {
            push_svg_table(&mut svg, &scene.table_spec, scene.viewport);
        }
        svg.push_str("</g>\n");

        push_svg_element_layer(&mut svg, scene, DiagramLayerId::OverlaysBelowBalls);
        push_svg_balls(&mut svg, scene);
        push_svg_element_layer(&mut svg, scene, DiagramLayerId::OverlaysAboveBalls);

        svg.push_str("</g>\n");
        svg.push_str("</svg>\n");
        svg
    }
}

fn push_svg_table_defs(svg: &mut String, viewport: DiagramViewport) {
    let left = viewport.playfield_left_px;
    let right = viewport.playfield_right_px;
    let top = viewport.playfield_top_px;
    let bottom = viewport.playfield_bottom_px;

    svg.push_str(&format!(
        r##"<defs>
<linearGradient id="tournament-blue-cloth" gradientUnits="userSpaceOnUse" x1="{left:.3}" y1="{top:.3}" x2="{right:.3}" y2="{bottom:.3}">
<stop offset="0%" stop-color="#02a7d8"/>
<stop offset="48%" stop-color="#058dbc"/>
<stop offset="100%" stop-color="#02749f"/>
</linearGradient>
<pattern id="cloth-weave" patternUnits="userSpaceOnUse" width="14" height="14">
<path d="M0 3.5H14M0 10.5H14" stroke="#4ecbe1" stroke-width=".45" opacity=".55"/>
<path d="M3.5 0V14M10.5 0V14" stroke="#006f95" stroke-width=".45" opacity=".35"/>
</pattern>
<linearGradient id="blue-cushion" x1="0" y1="0" x2="1" y2="1">
<stop offset="0%" stop-color="#20c8e4"/>
<stop offset="52%" stop-color="#0aa1c8"/>
<stop offset="100%" stop-color="#047999"/>
</linearGradient>
<linearGradient id="rosewood-rail" x1="0" y1="0" x2=".35" y2="1">
<stop offset="0%" stop-color="#7b2f22"/>
<stop offset="35%" stop-color="#5a1f17"/>
<stop offset="62%" stop-color="#8d3d27"/>
<stop offset="100%" stop-color="#3b130f"/>
</linearGradient>
<pattern id="rosewood-grain" patternUnits="userSpaceOnUse" width="180" height="64">
<rect width="180" height="64" fill="transparent"/>
<path d="M-18 17C25 5 62 30 105 16C139 5 163 9 198 25" stroke="#2b100c" stroke-width="5" opacity=".48" fill="none"/>
<path d="M-12 31C33 47 74 20 116 38C143 50 165 46 192 34" stroke="#b5663d" stroke-width="3" opacity=".34" fill="none"/>
<path d="M-28 48C16 37 47 55 82 45C124 32 151 60 205 45" stroke="#1d0907" stroke-width="4" opacity=".38" fill="none"/>
<path d="M0 8C40 16 63 4 96 10C127 16 150 3 180 11" stroke="#d0834d" stroke-width="1.5" opacity=".28" fill="none"/>
</pattern>
<pattern id="rosewood-grain-vertical" patternUnits="userSpaceOnUse" width="64" height="180">
<rect width="64" height="180" fill="transparent"/>
<path d="M17 -18C5 25 30 62 16 105C5 139 9 163 25 198" stroke="#2b100c" stroke-width="5" opacity=".48" fill="none"/>
<path d="M31 -12C47 33 20 74 38 116C50 143 46 165 34 192" stroke="#b5663d" stroke-width="3" opacity=".34" fill="none"/>
<path d="M48 -28C37 16 55 47 45 82C32 124 60 151 45 205" stroke="#1d0907" stroke-width="4" opacity=".38" fill="none"/>
<path d="M8 0C16 40 4 63 10 96C16 127 3 150 11 180" stroke="#d0834d" stroke-width="1.5" opacity=".28" fill="none"/>
</pattern>
<radialGradient id="pocket-well" cx="45%" cy="42%" r="75%">
<stop offset="0%" stop-color="#0b0908"/>
<stop offset="62%" stop-color="#020202"/>
<stop offset="100%" stop-color="#000000"/>
</radialGradient>
<linearGradient id="pocket-leather" x1="0" y1="0" x2="1" y2="1">
<stop offset="0%" stop-color="#2a2521"/>
<stop offset="42%" stop-color="#060504"/>
<stop offset="72%" stop-color="#15120f"/>
<stop offset="100%" stop-color="#030303"/>
</linearGradient>
<linearGradient id="heated-carom-cloth" gradientUnits="userSpaceOnUse" x1="{left:.3}" y1="{top:.3}" x2="{right:.3}" y2="{bottom:.3}">
<stop offset="0%" stop-color="#0aa7d0"/>
<stop offset="55%" stop-color="#087da4"/>
<stop offset="100%" stop-color="#075b7c"/>
</linearGradient>
<linearGradient id="heated-carom-cushion" x1="0" y1="0" x2="1" y2="1">
<stop offset="0%" stop-color="#34d7ef"/>
<stop offset="50%" stop-color="#0e97bd"/>
<stop offset="100%" stop-color="#046d8a"/>
</linearGradient>
<linearGradient id="carom-wood-rail" x1="0" y1="0" x2=".35" y2="1">
<stop offset="0%" stop-color="#65311f"/>
<stop offset="38%" stop-color="#3d1a11"/>
<stop offset="68%" stop-color="#7b3e27"/>
<stop offset="100%" stop-color="#24100c"/>
</linearGradient>
</defs>
"##,
    ));
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
            DiagramElement::HeadingChevron {
                tip,
                heading,
                style,
            } => {
                let points =
                    heading_chevron_points(&scene.table_spec, tip, *heading, &style.length_inches);
                drawing::draw_smooth_polyline_mut(table, &points, style.width_px, style.color);
            }
            DiagramElement::GhostBall { center, style } => {
                drawing::draw_ghost_ball_mut(
                    table,
                    center,
                    scene
                        .viewport
                        .ball_diameter_px(&scene.table_spec, &scene.table_spec.default_ball_spec()),
                    style.fill_color,
                    style.outline_color,
                );
            }
            DiagramElement::OriginMarker { center, style } => {
                let scale_px = style.scale_px.max(1);
                drawing::draw_text_label_mut(
                    table,
                    center,
                    "O",
                    -((5 * scale_px as i32) / 2),
                    -((7 * scale_px as i32) / 2),
                    scale_px,
                    style.color,
                );
            }
            DiagramElement::CircleMarker { center, style, .. } => {
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
            DiagramElement::SpinGlyph {
                center,
                angular_velocity,
                linear_velocity,
                ball_radius,
                style,
            } => draw_raster_spin_glyph(
                table,
                scene,
                center,
                angular_velocity,
                linear_velocity,
                ball_radius,
                style,
            ),
        }
    }
}

fn draw_raster_spin_glyph(
    table: &mut RgbaImage,
    scene: &DiagramScene,
    center: &Position,
    angular_velocity: &AngularVelocity3,
    linear_velocity: &Velocity2,
    ball_radius: &Inches,
    style: &SpinGlyphStyle,
) {
    let center = scene.viewport.position_to_scene_point(center);
    let radius = scene.viewport.ball_radius_px(
        &scene.table_spec,
        &BallSpec {
            radius: ball_radius.clone(),
        },
    );
    let glyph_radius = (radius * style.glyph_radius_fraction).clamp(8.5, 13.0);
    let badge_offset = radius * 0.72;
    let glyph_center = (center.x + badge_offset, center.y - badge_offset);
    let glyph_center_i32 = (
        glyph_center.0.round() as i32,
        glyph_center.1.round() as i32,
    );
    let stroke_width = (radius * 0.135).clamp(2.4, 4.0);
    let metrics = spin_glyph_metrics(angular_velocity, linear_velocity, ball_radius);

    draw_filled_circle_mut(
        table,
        glyph_center_i32,
        glyph_radius.ceil() as i32 + 1,
        Rgba([17, 17, 17, 230]),
    );
    draw_filled_circle_mut(
        table,
        glyph_center_i32,
        glyph_radius.ceil() as i32,
        Rgba([255, 250, 241, 250]),
    );

    if metrics.total_rps <= SPIN_GLYPH_STUN_RPS {
        let arm = glyph_radius * 0.48;
        for (start, end) in [
            ((-arm, -arm), (arm, arm)),
            ((arm, -arm), (-arm, arm)),
        ] {
            let start = (glyph_center.0 + start.0, glyph_center.1 + start.1);
            let end = (glyph_center.0 + end.0, glyph_center.1 + end.1);
            draw_raster_thick_line(
                table,
                start,
                end,
                stroke_width * 2.7,
                Rgba([255, 250, 241, 255]),
            );
            draw_raster_thick_line(
                table,
                start,
                end,
                stroke_width * 1.35,
                Rgba([SPIN_GLYPH_GREY[0], SPIN_GLYPH_GREY[1], SPIN_GLYPH_GREY[2], 255]),
            );
        }
        return;
    }

    if metrics.planar_rps > SPIN_GLYPH_STUN_RPS {
        let angle = metrics.angle_degrees.to_radians();
        let direction = (angle.cos() as f32, angle.sin() as f32);
        let normal = (-direction.1, direction.0);
        let tail = -glyph_radius * 0.70;
        let tip = glyph_radius * 0.74;
        let head = glyph_radius * 0.36;
        let base = tip - head;
        let line_start = (
            glyph_center.0 + direction.0 * tail,
            glyph_center.1 + direction.1 * tail,
        );
        let line_end = (
            glyph_center.0 + direction.0 * base,
            glyph_center.1 + direction.1 * base,
        );
        let planar = Rgba([
            metrics.planar_color[0],
            metrics.planar_color[1],
            metrics.planar_color[2],
            255,
        ]);
        draw_raster_thick_line(
            table,
            line_start,
            line_end,
            stroke_width * 2.65,
            Rgba([255, 250, 241, 255]),
        );
        draw_raster_thick_line(table, line_start, line_end, stroke_width * 1.28, planar);
        let arrow_tip = (
            glyph_center.0 + direction.0 * tip,
            glyph_center.1 + direction.1 * tip,
        );
        let head_half_width = head * 0.70;
        draw_raster_triangle(
            table,
            arrow_tip,
            (
                line_end.0 + normal.0 * head_half_width,
                line_end.1 + normal.1 * head_half_width,
            ),
            (
                line_end.0 - normal.0 * head_half_width,
                line_end.1 - normal.1 * head_half_width,
            ),
            planar,
        );
    }

    if metrics.wz.abs() > SPIN_GLYPH_STUN_RPS {
        let direction = if metrics.wz >= 0.0 { 1.0 } else { -1.0 };
        let arc_radius = glyph_radius * 0.82;
        let z_color = Rgba([
            metrics.z_color[0],
            metrics.z_color[1],
            metrics.z_color[2],
            255,
        ]);
        draw_raster_spin_arc(
            table,
            glyph_center,
            arc_radius,
            direction,
            stroke_width * 2.25,
            Rgba([255, 250, 241, 255]),
        );
        draw_raster_spin_arc(
            table,
            glyph_center,
            arc_radius,
            direction,
            stroke_width * 1.18,
            z_color,
        );

        let end_angle = 125.0_f32.to_radians();
        let arrow_tip = (
            glyph_center.0 + direction * arc_radius * end_angle.cos(),
            glyph_center.1 + arc_radius * end_angle.sin(),
        );
        let tangent = (-direction * end_angle.sin(), end_angle.cos());
        let tangent_length = tangent.0.hypot(tangent.1);
        let tangent = (tangent.0 / tangent_length, tangent.1 / tangent_length);
        let normal = (-tangent.1, tangent.0);
        let head = glyph_radius * 0.30;
        let base = (
            arrow_tip.0 - tangent.0 * head,
            arrow_tip.1 - tangent.1 * head,
        );
        draw_raster_triangle(
            table,
            arrow_tip,
            (base.0 + normal.0 * head * 0.55, base.1 + normal.1 * head * 0.55),
            (base.0 - normal.0 * head * 0.55, base.1 - normal.1 * head * 0.55),
            z_color,
        );
    }
}

fn draw_raster_thick_line(
    table: &mut RgbaImage,
    start: (f32, f32),
    end: (f32, f32),
    width: f32,
    color: Rgba<u8>,
) {
    let delta = (end.0 - start.0, end.1 - start.1);
    let length = delta.0.hypot(delta.1);
    if length <= f32::EPSILON {
        return;
    }
    let normal = (-delta.1 / length, delta.0 / length);
    let half_width = width * 0.5;
    let offset_limit = half_width.ceil() as i32;
    for offset in -offset_limit..=offset_limit {
        let offset = offset as f32;
        if offset.abs() > half_width + 0.5 {
            continue;
        }
        let shift = (normal.0 * offset, normal.1 * offset);
        draw_line_segment_mut(
            table,
            (start.0 + shift.0, start.1 + shift.1),
            (end.0 + shift.0, end.1 + shift.1),
            color,
        );
    }
}

fn draw_raster_triangle(
    table: &mut RgbaImage,
    first: (f32, f32),
    second: (f32, f32),
    third: (f32, f32),
    color: Rgba<u8>,
) {
    draw_polygon_mut(
        table,
        &[
            Point::new(first.0.round() as i32, first.1.round() as i32),
            Point::new(second.0.round() as i32, second.1.round() as i32),
            Point::new(third.0.round() as i32, third.1.round() as i32),
        ],
        color,
    );
}

fn draw_raster_spin_arc(
    table: &mut RgbaImage,
    center: (f32, f32),
    radius: f32,
    direction: f32,
    width: f32,
    color: Rgba<u8>,
) {
    const ARC_SEGMENTS: u32 = 28;
    let start_angle = -155.0_f32.to_radians();
    let sweep = 280.0_f32.to_radians();
    let mut previous = None;
    for step in 0..=ARC_SEGMENTS {
        let angle = start_angle + sweep * step as f32 / ARC_SEGMENTS as f32;
        let point = (
            center.0 + direction * radius * angle.cos(),
            center.1 + radius * angle.sin(),
        );
        if let Some(previous) = previous {
            draw_raster_thick_line(table, previous, point, width, color);
        }
        previous = Some(point);
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

fn push_svg_table(svg: &mut String, table_spec: &TableSpec, viewport: DiagramViewport) {
    match table_spec.kind {
        TableKind::Pool => push_svg_pool_table(svg, viewport),
        TableKind::ThreeCushionCarom => {
            push_svg_three_cushion_carom_table(svg, table_spec, viewport)
        }
    }
}

fn push_svg_pool_table(svg: &mut String, viewport: DiagramViewport) {
    // WPA tournament dimensions used by Diamond-style 9 ft tables:
    // 100 x 50 in playing surface, sights 3 11/16 in from cushion nose,
    // 4.5 in corner mouths, 5.0 in side mouths. Diamond's current product
    // photos show flush black leather/liner pockets cut into the rail cap, so
    // SVG pockets are shaped wells and facings rather than circular holes.
    let w = viewport.width_px;
    let h = viewport.height_px;
    let left = viewport.playfield_left_px;
    let right = viewport.playfield_right_px;
    let top = viewport.playfield_top_px;
    let bottom = viewport.playfield_bottom_px;
    let cloth_w = right - left;
    let cloth_h = bottom - top;
    let bottom_rail_h = h - bottom;
    let right_rail_w = w - right;
    let center_y = (top + bottom) * 0.5;

    let cushion_x = viewport.x_inches(CUSHION_WIDTH_IN);
    let cushion_y = viewport.y_inches(CUSHION_WIDTH_IN);
    let corner_run_x = viewport.x_inches(CORNER_POCKET_MOUTH_IN / 2.0_f32.sqrt());
    let corner_run_y = viewport.y_inches(CORNER_POCKET_MOUTH_IN / 2.0_f32.sqrt());
    let corner_shelf_x = viewport.x_inches(CORNER_POCKET_SHELF_IN);
    let corner_shelf_y = viewport.y_inches(CORNER_POCKET_SHELF_IN);
    let side_mouth_y = viewport.y_inches(SIDE_POCKET_MOUTH_IN * SIDE_POCKET_DRAWING_MOUTH_SCALE);
    let cushion_bevel_x = viewport.x_inches(CUSHION_BEVEL_IN);
    let cushion_bevel_y = viewport.y_inches(CUSHION_BEVEL_IN);
    svg.push_str(&format!(
        "<rect class=\"table-rail\" x=\"0\" y=\"0\" width=\"{w:.3}\" height=\"{h:.3}\" rx=\"58\"/>\n"
    ));
    svg.push_str(&format!(
        "<clipPath id=\"table-rail-clip\"><rect x=\"0\" y=\"0\" width=\"{w:.3}\" height=\"{h:.3}\" rx=\"58\"/></clipPath>\n"
    ));
    svg.push_str(&format!(
        "<g clip-path=\"url(#table-rail-clip)\"><rect class=\"table-rail-grain table-rail-grain-horizontal\" x=\"0\" y=\"0\" width=\"{w:.3}\" height=\"{top:.3}\"/><rect class=\"table-rail-grain table-rail-grain-horizontal\" x=\"0\" y=\"{bottom:.3}\" width=\"{w:.3}\" height=\"{bottom_rail_h:.3}\"/><rect class=\"table-rail-grain table-rail-grain-vertical\" x=\"0\" y=\"{top:.3}\" width=\"{left:.3}\" height=\"{cloth_h:.3}\"/><rect class=\"table-rail-grain table-rail-grain-vertical\" x=\"{right:.3}\" y=\"{top:.3}\" width=\"{right_rail_w:.3}\" height=\"{cloth_h:.3}\"/></g>\n"
    ));
    svg.push_str(&format!(
        "<rect class=\"table-cloth\" x=\"{left:.3}\" y=\"{top:.3}\" width=\"{cloth_w:.3}\" height=\"{cloth_h:.3}\"/>\n"
    ));
    svg.push_str(&format!(
        "<rect class=\"table-cloth-texture\" x=\"{left:.3}\" y=\"{top:.3}\" width=\"{cloth_w:.3}\" height=\"{cloth_h:.3}\"/>\n"
    ));

    let corner_pockets = [
        (left, top, -1.0, -1.0),
        (right, top, 1.0, -1.0),
        (left, bottom, -1.0, 1.0),
        (right, bottom, 1.0, 1.0),
    ];

    for layer in [PocketLayer::Well, PocketLayer::Liner] {
        for (corner_x, corner_y, x_sign, y_sign) in corner_pockets {
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
                cushion_x,
                cushion_y,
                cushion_bevel_x,
                cushion_bevel_y,
                layer,
            );
        }
        push_svg_side_pocket(
            svg,
            left,
            center_y,
            -1.0,
            side_mouth_y,
            cushion_x,
            cushion_bevel_y,
            layer,
        );
        push_svg_side_pocket(
            svg,
            right,
            center_y,
            1.0,
            side_mouth_y,
            cushion_x,
            cushion_bevel_y,
            layer,
        );
    }

    push_svg_horizontal_cushion(
        svg,
        left + corner_run_x,
        top - cushion_y,
        cloth_w - 2.0 * corner_run_x,
        cushion_y,
        -1.0,
        cushion_bevel_x,
    );
    push_svg_horizontal_cushion(
        svg,
        left + corner_run_x,
        bottom,
        cloth_w - 2.0 * corner_run_x,
        cushion_y,
        1.0,
        cushion_bevel_x,
    );
    push_svg_vertical_cushion(
        svg,
        left - cushion_x,
        top + corner_run_y,
        cushion_x,
        center_y - side_mouth_y * 0.5 - top - corner_run_y,
        -1.0,
        cushion_bevel_y,
    );
    push_svg_vertical_cushion(
        svg,
        left - cushion_x,
        center_y + side_mouth_y * 0.5,
        cushion_x,
        bottom - corner_run_y - center_y - side_mouth_y * 0.5,
        -1.0,
        cushion_bevel_y,
    );
    push_svg_vertical_cushion(
        svg,
        right,
        top + corner_run_y,
        cushion_x,
        center_y - side_mouth_y * 0.5 - top - corner_run_y,
        1.0,
        cushion_bevel_y,
    );
    push_svg_vertical_cushion(
        svg,
        right,
        center_y + side_mouth_y * 0.5,
        cushion_x,
        bottom - corner_run_y - center_y - side_mouth_y * 0.5,
        1.0,
        cushion_bevel_y,
    );

    for layer in [PocketLayer::Shelf] {
        for (corner_x, corner_y, x_sign, y_sign) in corner_pockets {
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
                cushion_x,
                cushion_y,
                cushion_bevel_x,
                cushion_bevel_y,
                layer,
            );
        }
        push_svg_side_pocket(
            svg,
            left,
            center_y,
            -1.0,
            side_mouth_y,
            cushion_x,
            cushion_bevel_y,
            layer,
        );
        push_svg_side_pocket(
            svg,
            right,
            center_y,
            1.0,
            side_mouth_y,
            cushion_x,
            cushion_bevel_y,
            layer,
        );
    }

    push_svg_table_sights(svg, viewport);
}

fn push_svg_three_cushion_carom_table(
    svg: &mut String,
    table_spec: &TableSpec,
    viewport: DiagramViewport,
) {
    let w = viewport.width_px;
    let h = viewport.height_px;
    let left = viewport.playfield_left_px;
    let right = viewport.playfield_right_px;
    let top = viewport.playfield_top_px;
    let bottom = viewport.playfield_bottom_px;
    let cloth_w = right - left;
    let cloth_h = bottom - top;
    let bottom_rail_h = h - bottom;
    let right_rail_w = w - right;
    let cushion_x = viewport.x_inches_for_table(table_spec, 2.25);
    let cushion_y = viewport.y_inches_for_table(table_spec, 2.25);

    svg.push_str("<g class=\"carom-table\">\n");
    svg.push_str(&format!(
        "<rect class=\"table-rail\" x=\"0\" y=\"0\" width=\"{w:.3}\" height=\"{h:.3}\" rx=\"58\"/>\n"
    ));
    svg.push_str(&format!(
        "<clipPath id=\"carom-table-rail-clip\"><rect x=\"0\" y=\"0\" width=\"{w:.3}\" height=\"{h:.3}\" rx=\"58\"/></clipPath>\n"
    ));
    svg.push_str(&format!(
        "<g clip-path=\"url(#carom-table-rail-clip)\"><rect class=\"table-rail-grain table-rail-grain-horizontal\" x=\"0\" y=\"0\" width=\"{w:.3}\" height=\"{top:.3}\"/><rect class=\"table-rail-grain table-rail-grain-horizontal\" x=\"0\" y=\"{bottom:.3}\" width=\"{w:.3}\" height=\"{bottom_rail_h:.3}\"/><rect class=\"table-rail-grain table-rail-grain-vertical\" x=\"0\" y=\"{top:.3}\" width=\"{left:.3}\" height=\"{cloth_h:.3}\"/><rect class=\"table-rail-grain table-rail-grain-vertical\" x=\"{right:.3}\" y=\"{top:.3}\" width=\"{right_rail_w:.3}\" height=\"{cloth_h:.3}\"/></g>\n"
    ));
    svg.push_str(&format!(
        "<rect class=\"table-cloth\" x=\"{left:.3}\" y=\"{top:.3}\" width=\"{cloth_w:.3}\" height=\"{cloth_h:.3}\"/>\n"
    ));
    svg.push_str(&format!(
        "<rect class=\"table-cloth-texture\" x=\"{left:.3}\" y=\"{top:.3}\" width=\"{cloth_w:.3}\" height=\"{cloth_h:.3}\"/>\n"
    ));
    svg.push_str(&format!(
        "<rect class=\"table-rail-inner-shadow\" x=\"{left:.3}\" y=\"{top:.3}\" width=\"{cloth_w:.3}\" height=\"{cloth_h:.3}\"/>\n"
    ));

    svg.push_str(&format!(
        "<path class=\"table-cushion\" d=\"M {left:.3} {top:.3} L {right:.3} {top:.3} L {:.3} {:.3} L {:.3} {:.3} Z\"/>\n",
        right + cushion_x,
        top - cushion_y,
        left - cushion_x,
        top - cushion_y
    ));
    svg.push_str(&format!(
        "<path class=\"table-cushion\" d=\"M {left:.3} {bottom:.3} L {right:.3} {bottom:.3} L {:.3} {:.3} L {:.3} {:.3} Z\"/>\n",
        right + cushion_x,
        bottom + cushion_y,
        left - cushion_x,
        bottom + cushion_y
    ));
    svg.push_str(&format!(
        "<path class=\"table-cushion\" d=\"M {left:.3} {top:.3} L {left:.3} {bottom:.3} L {:.3} {:.3} L {:.3} {:.3} Z\"/>\n",
        left - cushion_x,
        bottom + cushion_y,
        left - cushion_x,
        top - cushion_y
    ));
    svg.push_str(&format!(
        "<path class=\"table-cushion\" d=\"M {right:.3} {top:.3} L {right:.3} {bottom:.3} L {:.3} {:.3} L {:.3} {:.3} Z\"/>\n",
        right + cushion_x,
        bottom + cushion_y,
        right + cushion_x,
        top - cushion_y
    ));
    svg.push_str(&format!(
        "<line class=\"table-cushion-nose\" x1=\"{left:.3}\" y1=\"{top:.3}\" x2=\"{right:.3}\" y2=\"{top:.3}\"/>\n"
    ));
    svg.push_str(&format!(
        "<line class=\"table-cushion-nose\" x1=\"{left:.3}\" y1=\"{bottom:.3}\" x2=\"{right:.3}\" y2=\"{bottom:.3}\"/>\n"
    ));
    svg.push_str(&format!(
        "<line class=\"table-cushion-nose\" x1=\"{left:.3}\" y1=\"{top:.3}\" x2=\"{left:.3}\" y2=\"{bottom:.3}\"/>\n"
    ));
    svg.push_str(&format!(
        "<line class=\"table-cushion-nose\" x1=\"{right:.3}\" y1=\"{top:.3}\" x2=\"{right:.3}\" y2=\"{bottom:.3}\"/>\n"
    ));
    svg.push_str(&format!(
        "<line class=\"table-cushion-back\" x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\"/>\n",
        left - cushion_x,
        top - cushion_y,
        right + cushion_x,
        top - cushion_y
    ));
    svg.push_str(&format!(
        "<line class=\"table-cushion-back\" x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\"/>\n",
        left - cushion_x,
        bottom + cushion_y,
        right + cushion_x,
        bottom + cushion_y
    ));
    svg.push_str(&format!(
        "<line class=\"table-cushion-back\" x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\"/>\n",
        left - cushion_x,
        top - cushion_y,
        left - cushion_x,
        bottom + cushion_y
    ));
    svg.push_str(&format!(
        "<line class=\"table-cushion-back\" x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\"/>\n",
        right + cushion_x,
        top - cushion_y,
        right + cushion_x,
        bottom + cushion_y
    ));

    push_svg_three_cushion_carom_sights(svg, table_spec, viewport);
    svg.push_str("</g>\n");
}

impl DiagramViewport {
    fn x_inches(self, inches: f32) -> f32 {
        inches * (self.playfield_right_px - self.playfield_left_px) / PLAYFIELD_WIDTH_IN
    }

    fn y_inches(self, inches: f32) -> f32 {
        inches * (self.playfield_bottom_px - self.playfield_top_px) / PLAYFIELD_LENGTH_IN
    }

    fn x_inches_for_table(self, table_spec: &TableSpec, inches: f32) -> f32 {
        let table_width_in = table_spec.diamond_length.as_f64() as f32 * TABLE_DIAMONDS_X;
        inches * (self.playfield_right_px - self.playfield_left_px) / table_width_in
    }

    fn y_inches_for_table(self, table_spec: &TableSpec, inches: f32) -> f32 {
        let table_length_in = table_spec.diamond_length.as_f64() as f32 * TABLE_DIAMONDS_Y;
        inches * (self.playfield_bottom_px - self.playfield_top_px) / table_length_in
    }
}

fn push_svg_horizontal_cushion(
    svg: &mut String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    y_sign: f32,
    bevel: f32,
) {
    let nose_y = if y_sign < 0.0 { y + height } else { y };
    let back_y = if y_sign < 0.0 { y } else { y + height };
    let back_left_x = x - bevel;
    let back_right_x = x + width + bevel;
    let right_x = x + width;

    svg.push_str(&format!(
        "<path class=\"table-cushion\" d=\"M {x:.3} {nose_y:.3} L {right_x:.3} {nose_y:.3} L {back_right_x:.3} {back_y:.3} L {back_left_x:.3} {back_y:.3} Z\"/>\n"
    ));
    svg.push_str(&format!(
        "<line class=\"table-cushion-nose\" x1=\"{x:.3}\" y1=\"{nose_y:.3}\" x2=\"{right_x:.3}\" y2=\"{nose_y:.3}\"/>\n"
    ));
    svg.push_str(&format!(
        "<line class=\"table-cushion-back\" x1=\"{back_left_x:.3}\" y1=\"{back_y:.3}\" x2=\"{back_right_x:.3}\" y2=\"{back_y:.3}\"/>\n"
    ));
}

fn push_svg_vertical_cushion(
    svg: &mut String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    x_sign: f32,
    bevel: f32,
) {
    let nose_x = if x_sign < 0.0 { x + width } else { x };
    let back_x = if x_sign < 0.0 { x } else { x + width };
    let back_top_y = y - bevel;
    let back_bottom_y = y + height + bevel;
    let bottom_y = y + height;

    svg.push_str(&format!(
        "<path class=\"table-cushion\" d=\"M {nose_x:.3} {y:.3} L {nose_x:.3} {bottom_y:.3} L {back_x:.3} {back_bottom_y:.3} L {back_x:.3} {back_top_y:.3} Z\"/>\n"
    ));
    svg.push_str(&format!(
        "<line class=\"table-cushion-nose\" x1=\"{nose_x:.3}\" y1=\"{y:.3}\" x2=\"{nose_x:.3}\" y2=\"{bottom_y:.3}\"/>\n"
    ));
    svg.push_str(&format!(
        "<line class=\"table-cushion-back\" x1=\"{back_x:.3}\" y1=\"{back_top_y:.3}\" x2=\"{back_x:.3}\" y2=\"{back_bottom_y:.3}\"/>\n"
    ));
}

#[derive(Clone, Copy)]
enum PocketLayer {
    Well,
    Shelf,
    Liner,
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
    cushion_x: f32,
    cushion_y: f32,
    cushion_bevel_x: f32,
    cushion_bevel_y: f32,
    layer: PocketLayer,
) {
    let well_radius_scale = (CORNER_POCKET_WELL_DIAMETER_IN * 0.5) / CORNER_POCKET_SHELF_IN;
    let well_radius_x = shelf_x * well_radius_scale;
    let well_radius_y = shelf_y * well_radius_scale;
    let liner_stroke = (cushion_x.min(cushion_y) * 0.57).clamp(17.5, 20.0);

    let point =
        |inside_x: f32, inside_y: f32| (corner_x - x_sign * inside_x, corner_y - y_sign * inside_y);

    let top_liner_inside = (run_x - cushion_bevel_x, -cushion_y);
    let side_liner_inside = (-cushion_x, run_y - cushion_bevel_y);
    let (top_liner_end_x, top_liner_end_y) = point(top_liner_inside.0, top_liner_inside.1);
    let (side_liner_end_x, side_liner_end_y) = point(side_liner_inside.0, side_liner_inside.1);

    // Separated controls keep the configured midpoint while giving the
    // padded leather upper a broad circular rear arc instead of a tight cusp.
    let back_control_center_inside_x =
        (-well_radius_x - 0.125 * (top_liner_inside.0 + side_liner_inside.0)) / 0.75;
    let back_control_center_inside_y =
        (-well_radius_y - 0.125 * (top_liner_inside.1 + side_liner_inside.1)) / 0.75;
    let back_tangent_x = well_radius_x * CORNER_POCKET_BACK_TANGENT_RATIO;
    let back_tangent_y = well_radius_y * CORNER_POCKET_BACK_TANGENT_RATIO;
    let (top_back_control_x, top_back_control_y) = point(
        back_control_center_inside_x + back_tangent_x,
        back_control_center_inside_y - back_tangent_y,
    );
    let (side_back_control_x, side_back_control_y) = point(
        back_control_center_inside_x - back_tangent_x,
        back_control_center_inside_y + back_tangent_y,
    );

    // Shelf depth is measured normally from the line between the cushion
    // noses to the cloth drop edge. The quadratic control is solved so its
    // midpoint lands at that physical depth rather than using a visual guess.
    let shelf_mid_inside_x = run_x * 0.5 - shelf_x / 2.0_f32.sqrt();
    let shelf_mid_inside_y = run_y * 0.5 - shelf_y / 2.0_f32.sqrt();
    let drop_control_inside_x =
        2.0 * shelf_mid_inside_x - 0.5 * (top_liner_inside.0 + side_liner_inside.0);
    let drop_control_inside_y =
        2.0 * shelf_mid_inside_y - 0.5 * (top_liner_inside.1 + side_liner_inside.1);
    let (drop_control_x, drop_control_y) = point(drop_control_inside_x, drop_control_inside_y);

    match layer {
        PocketLayer::Well => {
            svg.push_str(&format!(
                "<path class=\"table-pocket-well\" data-pocket=\"corner\" d=\"M {top_liner_end_x:.3} {top_liner_end_y:.3} C {top_back_control_x:.3} {top_back_control_y:.3} {side_back_control_x:.3} {side_back_control_y:.3} {side_liner_end_x:.3} {side_liner_end_y:.3} Q {drop_control_x:.3} {drop_control_y:.3} {top_liner_end_x:.3} {top_liner_end_y:.3} Z\"/>\n"
            ));
        }
        PocketLayer::Shelf => {
            // Tuck the mouth edge under the bed and cushion ends. The shelf
            // and well share the exact drop curve, leaving no exposed sliver.
            let shelf_bed_overlap = 1.05;
            let (shelf_top_x, shelf_top_y) = point(run_x * shelf_bed_overlap, 0.0);
            let (shelf_side_x, shelf_side_y) = point(0.0, run_y * shelf_bed_overlap);
            let (shelf_inner_control_x, shelf_inner_control_y) = point(run_x * 0.62, run_y * 0.62);
            let shelf_path = format!(
                "M {shelf_top_x:.3} {shelf_top_y:.3} \
                 L {top_liner_end_x:.3} {top_liner_end_y:.3} \
                 Q {drop_control_x:.3} {drop_control_y:.3} {side_liner_end_x:.3} {side_liner_end_y:.3} \
                 L {shelf_side_x:.3} {shelf_side_y:.3} \
                 Q {shelf_inner_control_x:.3} {shelf_inner_control_y:.3} {shelf_top_x:.3} {shelf_top_y:.3} Z"
            );
            svg.push_str(&format!(
                "<path class=\"table-pocket-shelf\" data-pocket=\"corner-shelf\" d=\"{shelf_path}\"/>\n"
            ));
            svg.push_str(&format!(
                "<path class=\"table-pocket-shelf-texture\" data-pocket=\"corner-shelf-texture\" d=\"{shelf_path}\"/>\n"
            ));
        }
        PocketLayer::Liner => {
            let back_curve = format!(
                "M {top_liner_end_x:.3} {top_liner_end_y:.3} \
                 C {top_back_control_x:.3} {top_back_control_y:.3} {side_back_control_x:.3} {side_back_control_y:.3} {side_liner_end_x:.3} {side_liner_end_y:.3}"
            );
            svg.push_str(&format!(
                "<path class=\"table-pocket-leather\" data-pocket=\"corner-liner\" stroke-width=\"{liner_stroke:.3}\" d=\"{back_curve}\"/>\n"
            ));
            svg.push_str(&format!(
                "<path class=\"table-pocket-leather-highlight\" d=\"{back_curve}\"/>\n"
            ));
        }
    }
}

fn push_svg_side_pocket(
    svg: &mut String,
    rail_x: f32,
    center_y: f32,
    x_sign: f32,
    mouth_y: f32,
    cushion_x: f32,
    cushion_bevel_y: f32,
    layer: PocketLayer,
) {
    let px_per_inch = cushion_x / CUSHION_WIDTH_IN;
    let drawing_scale = mouth_y / (px_per_inch * SIDE_POCKET_MOUTH_IN);
    let liner_stroke = (cushion_x * 0.57 * SIDE_POCKET_DRAWING_LINER_SCALE).clamp(24.0, 26.0);
    let mouth_half_y = mouth_y * 0.5;
    let point = |depth_x: f32, offset_y: f32| (rail_x + x_sign * depth_x, center_y + offset_y);

    let rail_top = point(0.0, -mouth_half_y);
    let rail_bottom = point(0.0, mouth_half_y);
    let lip_top = point(
        px_per_inch * SIDE_POCKET_DRAWING_LIP_DEPTH_IN * drawing_scale,
        -mouth_half_y,
    );
    let lip_bottom = point(
        px_per_inch * SIDE_POCKET_DRAWING_LIP_DEPTH_IN * drawing_scale,
        mouth_half_y,
    );
    // Keep the installed leather profile rounded like the reference/before
    // silhouette. The 8-degree cut controls only the tangent leaving each
    // mouth lip; the remaining controls form the broad, symmetric rear bowl.
    let first_control_depth_in = SIDE_POCKET_DRAWING_LIP_DEPTH_IN
        + SIDE_POCKET_DRAWING_FIRST_CONTROL_RUN_IN * SIDE_POCKET_CUT_ANGLE_DEG.to_radians().tan();
    let first_control_run_y =
        cushion_bevel_y * SIDE_POCKET_DRAWING_FIRST_CONTROL_RUN_IN * drawing_scale;
    let first_control = point(
        px_per_inch * first_control_depth_in * drawing_scale,
        -mouth_half_y + first_control_run_y,
    );
    let second_control = point(
        px_per_inch * SIDE_POCKET_DRAWING_SECOND_CONTROL_DEPTH_IN * drawing_scale,
        -mouth_half_y - px_per_inch * SIDE_POCKET_DRAWING_TUCK_OFFSET_IN * drawing_scale,
    );
    let outer_top = point(
        px_per_inch * SIDE_POCKET_DRAWING_OUTER_ENDPOINT_DEPTH_IN * drawing_scale,
        -mouth_half_y + cushion_bevel_y * drawing_scale,
    );
    let outer_first_control = point(
        px_per_inch * SIDE_POCKET_DRAWING_OUTER_MID_DEPTH_IN * drawing_scale,
        -mouth_half_y + px_per_inch * SIDE_POCKET_DRAWING_SHOULDER_OFFSET_IN * drawing_scale,
    );
    let outer_second_control = point(
        px_per_inch * SIDE_POCKET_DRAWING_OUTER_MID_DEPTH_IN * drawing_scale,
        mouth_half_y - px_per_inch * SIDE_POCKET_DRAWING_SHOULDER_OFFSET_IN * drawing_scale,
    );
    let outer_bottom = point(
        px_per_inch * SIDE_POCKET_DRAWING_OUTER_ENDPOINT_DEPTH_IN * drawing_scale,
        mouth_half_y - cushion_bevel_y * drawing_scale,
    );
    let third_control = point(
        px_per_inch * SIDE_POCKET_DRAWING_SECOND_CONTROL_DEPTH_IN * drawing_scale,
        mouth_half_y + px_per_inch * SIDE_POCKET_DRAWING_TUCK_OFFSET_IN * drawing_scale,
    );
    let fourth_control = point(
        px_per_inch * first_control_depth_in * drawing_scale,
        mouth_half_y - first_control_run_y,
    );
    let liner_curve = format!(
        "C {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} C {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} C {:.3} {:.3} {:.3} {:.3} {:.3} {:.3}",
        first_control.0,
        first_control.1,
        second_control.0,
        second_control.1,
        outer_top.0,
        outer_top.1,
        outer_first_control.0,
        outer_first_control.1,
        outer_second_control.0,
        outer_second_control.1,
        outer_bottom.0,
        outer_bottom.1,
        third_control.0,
        third_control.1,
        fourth_control.0,
        fourth_control.1,
        lip_bottom.0,
        lip_bottom.1,
    );
    let shelf_control = point(
        px_per_inch
            * SIDE_POCKET_DRAWING_SHELF_SAGITTA_IN
            * drawing_scale
            * SIDE_POCKET_DRAWING_SHELF_SAGITTA_SCALE,
        0.0,
    );
    let shelf_inner_control = point(
        -px_per_inch
            * SIDE_POCKET_DRAWING_SHELF_INNER_SAGITTA_IN
            * drawing_scale
            * SIDE_POCKET_DRAWING_SHELF_SAGITTA_SCALE,
        0.0,
    );
    let shelf_path = format!(
        "M {:.3} {:.3} Q {:.3} {:.3} {:.3} {:.3} Q {:.3} {:.3} {:.3} {:.3} Z",
        rail_top.0,
        rail_top.1,
        shelf_control.0,
        shelf_control.1,
        rail_bottom.0,
        rail_bottom.1,
        shelf_inner_control.0,
        shelf_inner_control.1,
        rail_top.0,
        rail_top.1,
    );
    match layer {
        PocketLayer::Well => {
            svg.push_str(&format!(
                "<path class=\"table-pocket-well\" data-pocket=\"side\" d=\"M {rail_top_x:.3} {rail_top_y:.3} L {lip_top_x:.3} {lip_top_y:.3} {liner_curve} L {rail_bottom_x:.3} {rail_bottom_y:.3} Q {shelf_control_x:.3} {shelf_control_y:.3} {rail_top_x:.3} {rail_top_y:.3} Z\"/>\n",
                rail_top_x = rail_top.0,
                rail_top_y = rail_top.1,
                lip_top_x = lip_top.0,
                lip_top_y = lip_top.1,
                rail_bottom_x = rail_bottom.0,
                rail_bottom_y = rail_bottom.1,
                shelf_control_x = shelf_control.0,
                shelf_control_y = shelf_control.1,
            ));
        }
        PocketLayer::Shelf => {
            svg.push_str(&format!(
                "<path class=\"table-pocket-shelf\" data-pocket=\"side-shelf\" d=\"{shelf_path}\"/>\n"
            ));
            svg.push_str(&format!(
                "<path class=\"table-pocket-shelf-texture\" data-pocket=\"side-shelf-texture\" d=\"{shelf_path}\"/>\n"
            ));
        }
        PocketLayer::Liner => {
            let liner_path = format!("M {:.3} {:.3} {liner_curve}", lip_top.0, lip_top.1);
            svg.push_str(&format!(
                "<path class=\"table-pocket-leather\" data-pocket=\"side-liner\" stroke-width=\"{liner_stroke:.3}\" d=\"{liner_path}\"/>\n"
            ));
            svg.push_str(&format!(
                "<path class=\"table-pocket-leather-highlight\" d=\"{liner_path}\"/>\n"
            ));
        }
    }
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

fn push_svg_three_cushion_carom_sights(
    svg: &mut String,
    table_spec: &TableSpec,
    viewport: DiagramViewport,
) {
    let left = viewport.playfield_left_px;
    let right = viewport.playfield_right_px;
    let top = viewport.playfield_top_px;
    let bottom = viewport.playfield_bottom_px;
    let cloth_w = right - left;
    let cloth_h = bottom - top;
    let sight_setback_x = viewport.x_inches_for_table(table_spec, DIAMOND_SIGHT_SETBACK_IN);
    let sight_setback_y = viewport.y_inches_for_table(table_spec, DIAMOND_SIGHT_SETBACK_IN);
    let sight_half_along_x = viewport.x_inches_for_table(table_spec, DIAMOND_SIGHT_WIDTH_IN) * 0.5;
    let sight_half_along_y = viewport.y_inches_for_table(table_spec, DIAMOND_SIGHT_WIDTH_IN) * 0.5;
    let sight_half_cross_x = viewport.x_inches_for_table(table_spec, DIAMOND_SIGHT_HEIGHT_IN) * 0.5;
    let sight_half_cross_y = viewport.y_inches_for_table(table_spec, DIAMOND_SIGHT_HEIGHT_IN) * 0.5;

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

    for fraction in [0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875] {
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

fn angle_from_degrees(degrees: f64) -> Angle {
    let radians = degrees.to_radians();
    Angle::from_north(radians.sin(), radians.cos())
}

fn heading_chevron_points(
    table_spec: &TableSpec,
    tip: &Position,
    heading: Angle,
    length_inches: &Inches,
) -> [Position; 3] {
    let mut left = tip.translate_inches(
        length_inches.clone(),
        angle_from_degrees(heading.as_degrees() + 150.0),
    );
    let mut tip = tip.clone();
    let mut right = tip.translate_inches(
        length_inches.clone(),
        angle_from_degrees(heading.as_degrees() - 150.0),
    );

    left.resolve_shifts(table_spec);
    tip.resolve_shifts(table_spec);
    right.resolve_shifts(table_spec);

    [left, tip, right]
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
        DiagramElement::HeadingChevron {
            tip,
            heading,
            style,
        } => {
            let points =
                heading_chevron_points(&scene.table_spec, tip, *heading, &style.length_inches);
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
                "<polyline class=\"overlay heading-chevron\" points=\"{}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{:.3}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" fill=\"none\" data-heading-deg=\"{:.3}\"/>\n",
                points,
                stroke,
                opacity,
                style.width_px,
                heading.as_degrees()
            ));
        }
        DiagramElement::GhostBall { center, style } => {
            let center = scene.viewport.position_to_scene_point(center);
            let ball_spec = scene.table_spec.default_ball_spec();
            let radius = scene.viewport.ball_radius_px(&scene.table_spec, &ball_spec);
            let (fill, fill_opacity) = svg_color(style.fill_color);
            let (stroke, stroke_opacity) = svg_color(style.outline_color);
            svg.push_str(&format!(
                "<circle class=\"overlay ghost-ball\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"{:.3}\" fill=\"{}\" fill-opacity=\"{:.3}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"2\" stroke-dasharray=\"3 8\"/>\n",
                center.x, center.y, radius, fill, fill_opacity, stroke, stroke_opacity
            ));
        }
        DiagramElement::OriginMarker { center, style } => {
            let center = scene.viewport.position_to_scene_point(center);
            let (fill, opacity) = svg_color(style.color);
            svg.push_str(&format!(
                "<text class=\"overlay origin-marker\" x=\"{:.3}\" y=\"{:.3}\" fill=\"{}\" fill-opacity=\"{:.3}\" font-size=\"{}\">O</text>\n",
                center.x,
                center.y,
                fill,
                opacity,
                style.scale_px.max(1) * 7
            ));
        }
        DiagramElement::CircleMarker {
            center,
            style,
            event_label,
            event_title,
        } => {
            let center = scene.viewport.position_to_scene_point(center);
            let (fill, opacity) = svg_color(style.color);
            if let Some(event_label) = event_label {
                let event_title = event_title.as_ref().unwrap_or(event_label);
                let event_label = escape_xml(event_label);
                let event_title = escape_xml(event_title);
                svg.push_str(&format!(
                    "<circle class=\"overlay event-marker\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"{:.3}\" fill=\"{}\" fill-opacity=\"{:.3}\" data-event-label=\"{}\"><title>{}</title></circle>\n",
                    center.x, center.y, style.radius_px, fill, opacity, event_label, event_title
                ));
            } else {
                svg.push_str(&format!(
                    "<circle class=\"overlay event-marker\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"{:.3}\" fill=\"{}\" fill-opacity=\"{:.3}\"/>\n",
                    center.x, center.y, style.radius_px, fill, opacity
                ));
            }
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
        DiagramElement::SpinGlyph {
            center,
            angular_velocity,
            linear_velocity,
            ball_radius,
            style,
        } => {
            push_svg_spin_glyph(
                svg,
                scene,
                center,
                angular_velocity,
                linear_velocity,
                ball_radius,
                style,
            );
        }
    }
}

struct SpinGlyphMetrics {
    vx: f64,
    vy: f64,
    wx: f64,
    wy: f64,
    wz: f64,
    planar_rps: f64,
    total_rps: f64,
    roll_ratio: f64,
    roll_alignment: f64,
    roll_slip_ips: f64,
    angle_degrees: f64,
    kind: &'static str,
    planar_color: [u8; 3],
    z_color: [u8; 3],
}

fn push_svg_spin_glyph(
    svg: &mut String,
    scene: &DiagramScene,
    center: &Position,
    angular_velocity: &AngularVelocity3,
    linear_velocity: &Velocity2,
    ball_radius: &Inches,
    style: &SpinGlyphStyle,
) {
    let center = scene.viewport.position_to_scene_point(center);
    let radius = scene.viewport.ball_radius_px(
        &scene.table_spec,
        &BallSpec {
            radius: ball_radius.clone(),
        },
    );
    let glyph_radius = (radius * style.glyph_radius_fraction).clamp(8.5, 13.0);
    let badge_offset = radius * 0.72;
    let stroke_width = (radius * 0.135).clamp(2.4, 4.0);
    let metrics = spin_glyph_metrics(angular_velocity, linear_velocity, ball_radius);
    let planar_color = svg_rgb(metrics.planar_color);
    let z_color = svg_rgb(metrics.z_color);
    let title = escape_xml(&format!(
        "spin: v=({:.1}, {:.1}) ips; omega=({:.1}, {:.1}, {:.1}) rad/s; roll slip={:.1} ips; roll ratio={:.2}; side={:.1} rad/s",
        metrics.vx,
        metrics.vy,
        metrics.wx,
        metrics.wy,
        metrics.wz,
        metrics.roll_slip_ips,
        metrics.roll_ratio,
        metrics.wz
    ));
    svg.push_str(&format!(
        "<g class=\"overlay ball-spin-glyph\" transform=\"translate({:.3} {:.3})\" role=\"img\" aria-label=\"{}\" data-spin-kind=\"{}\" data-spin-angle-deg=\"{:.3}\" data-spin-rps=\"{:.3}\" data-spin-planar-rps=\"{:.3}\" data-spin-z-rps=\"{:.3}\" data-spin-roll-ratio=\"{:.3}\" data-spin-roll-alignment=\"{:.3}\" data-spin-slip-ips=\"{:.3}\" data-spin-vx=\"{:.3}\" data-spin-vy=\"{:.3}\" data-spin-wx=\"{:.3}\" data-spin-wy=\"{:.3}\" data-spin-wz=\"{:.3}\"><title>{}</title>\n",
        center.x + badge_offset,
        center.y - badge_offset,
        title,
        metrics.kind,
        metrics.angle_degrees,
        metrics.total_rps,
        metrics.planar_rps,
        metrics.wz,
        metrics.roll_ratio,
        metrics.roll_alignment,
        metrics.roll_slip_ips,
        metrics.vx,
        metrics.vy,
        metrics.wx,
        metrics.wy,
        metrics.wz,
        title
    ));
    svg.push_str(&format!(
        "<circle class=\"ball-spin-backplate\" r=\"{:.3}\" stroke-width=\"{:.3}\"/>\n",
        glyph_radius,
        stroke_width * 0.75
    ));

    if metrics.total_rps <= SPIN_GLYPH_STUN_RPS {
        let arm = glyph_radius * 0.48;
        svg.push_str(&format!(
            "<path class=\"ball-spin-stun-x-halo\" d=\"M {:.3} {:.3} L {:.3} {:.3} M {:.3} {:.3} L {:.3} {:.3}\" stroke-width=\"{:.3}\"/>\n",
            -arm,
            -arm,
            arm,
            arm,
            arm,
            -arm,
            -arm,
            arm,
            stroke_width * 2.7
        ));
        svg.push_str(&format!(
            "<path class=\"ball-spin-stun-x-mark\" d=\"M {:.3} {:.3} L {:.3} {:.3} M {:.3} {:.3} L {:.3} {:.3}\" stroke-width=\"{:.3}\"/>\n",
            -arm,
            -arm,
            arm,
            arm,
            arm,
            -arm,
            -arm,
            arm,
            stroke_width * 1.35
        ));
    } else {
        if metrics.planar_rps > SPIN_GLYPH_STUN_RPS {
            svg.push_str(&format!(
                "<g transform=\"rotate({:.3})\">\n",
                metrics.angle_degrees
            ));
            let tail = -glyph_radius * 0.70;
            let tip = glyph_radius * 0.74;
            let head = glyph_radius * 0.36;
            let base = tip - head;
            svg.push_str(&format!(
                "<path class=\"ball-spin-vector-halo\" d=\"M {:.3} 0 L {:.3} 0\" stroke-width=\"{:.3}\"/>\n",
                tail,
                base,
                stroke_width * 2.65
            ));
            svg.push_str(&format!(
                "<path class=\"ball-spin-vector\" d=\"M {:.3} 0 L {:.3} 0\" stroke=\"{}\" stroke-opacity=\".98\" stroke-width=\"{:.3}\"/>\n",
                tail,
                base,
                planar_color,
                stroke_width * 1.28
            ));
            svg.push_str(&format!(
                "<path class=\"ball-spin-arrowhead\" d=\"M {:.3} 0 L {:.3} {:.3} L {:.3} {:.3} Z\" fill=\"{}\" fill-opacity=\".98\" stroke-width=\"{:.3}\"/>\n",
                tip,
                base,
                -head * 0.70,
                base,
                head * 0.70,
                planar_color,
                stroke_width * 0.55
            ));
            svg.push_str("</g>\n");
        }
        if metrics.wz.abs() > SPIN_GLYPH_STUN_RPS {
            let arc = glyph_radius * 0.82;
            let z_opacity = (metrics.wz.abs() / metrics.total_rps).clamp(0.66, 1.0);
            let z_direction = if metrics.wz >= 0.0 { 1.0 } else { -1.0 };
            let head = glyph_radius * 0.30;
            svg.push_str(&format!("<g transform=\"scale({:.1} 1)\">\n", z_direction));
            svg.push_str(&format!(
                "<path class=\"ball-spin-z-halo\" d=\"M {:.3} {:.3} A {:.3} {:.3} 0 1 1 {:.3} {:.3}\" stroke-width=\"{:.3}\"/>\n",
                -arc,
                -arc * 0.42,
                arc,
                arc,
                arc,
                arc * 0.42,
                stroke_width * 2.25
            ));
            svg.push_str(&format!(
                "<path class=\"ball-spin-z\" d=\"M {:.3} {:.3} A {:.3} {:.3} 0 1 1 {:.3} {:.3}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{:.3}\"/>\n",
                -arc,
                -arc * 0.42,
                arc,
                arc,
                arc,
                arc * 0.42,
                z_color,
                z_opacity,
                stroke_width * 1.18
            ));
            svg.push_str(&format!(
                "<path class=\"ball-spin-z-head\" d=\"M {:.3} {:.3} L {:.3} {:.3} L {:.3} {:.3} Z\" fill=\"{}\" fill-opacity=\"{:.3}\" stroke-width=\"{:.3}\"/>\n",
                arc,
                arc * 0.42,
                arc - head * 0.72,
                arc * 0.42 - head * 0.78,
                arc - head * 0.12,
                arc * 0.42 + head * 0.90,
                z_color,
                z_opacity,
                stroke_width * 0.55
            ));
            svg.push_str("</g>\n");
        }
    }
    svg.push_str("</g>\n");
}

const SPIN_GLYPH_STUN_RPS: f64 = 1e-6;
const SPIN_GLYPH_GREY: [u8; 3] = [0x7f, 0x85, 0x8c];
const SPIN_GLYPH_GREEN: [u8; 3] = [0x2d, 0xa4, 0x4e];
const SPIN_GLYPH_BLUE: [u8; 3] = [0x09, 0x6b, 0xd8];
const SPIN_GLYPH_ORANGE: [u8; 3] = [0xfb, 0x85, 0x1e];
const SPIN_GLYPH_AMBER: [u8; 3] = [0xbf, 0x87, 0x00];
const SPIN_GLYPH_VIOLET: [u8; 3] = [0x8b, 0x5c, 0xf6];

fn spin_glyph_metrics(
    angular_velocity: &AngularVelocity3,
    linear_velocity: &Velocity2,
    ball_radius: &Inches,
) -> SpinGlyphMetrics {
    let vx = finite_or_zero(linear_velocity.x().as_f64());
    let vy = finite_or_zero(linear_velocity.y().as_f64());
    let wx = finite_or_zero(angular_velocity.x().as_f64());
    let wy = finite_or_zero(angular_velocity.y().as_f64());
    let wz = finite_or_zero(angular_velocity.z().as_f64());
    let planar_rps = wx.hypot(wy);
    let total_rps = planar_rps.hypot(wz);
    let linear_speed_ips = vx.hypot(vy);
    let radius_inches = finite_or_zero(ball_radius.as_f64()).max(SPIN_GLYPH_STUN_RPS);
    let rolling_target_rps = (linear_speed_ips / radius_inches).max(0.0);
    let roll_ratio = if rolling_target_rps > SPIN_GLYPH_STUN_RPS {
        planar_rps / rolling_target_rps
    } else {
        0.0
    };
    let roll_vx = radius_inches * wy;
    let roll_vy = -radius_inches * wx;
    let roll_speed_ips = roll_vx.hypot(roll_vy);
    let roll_slip_ips = (vx - roll_vx).hypot(vy - roll_vy);
    let roll_alignment =
        if linear_speed_ips > SPIN_GLYPH_STUN_RPS && roll_speed_ips > SPIN_GLYPH_STUN_RPS {
            ((vx * roll_vx + vy * roll_vy) / (linear_speed_ips * roll_speed_ips)).clamp(-1.0, 1.0)
        } else {
            0.0
        };
    let spin_vector_x = roll_vx;
    let spin_vector_scene_y = -roll_vy;
    let angle_degrees = if spin_vector_x.hypot(spin_vector_scene_y) > SPIN_GLYPH_STUN_RPS {
        spin_vector_scene_y.atan2(spin_vector_x).to_degrees()
    } else {
        0.0
    };
    let rolling_slip_limit = (linear_speed_ips * 0.12).max(0.75);
    let is_rolling = linear_speed_ips > SPIN_GLYPH_STUN_RPS
        && planar_rps > SPIN_GLYPH_STUN_RPS
        && roll_slip_ips <= rolling_slip_limit;
    let has_prominent_side_spin = wz.abs() > planar_rps.max(rolling_target_rps) * 0.25;
    let kind = if total_rps <= SPIN_GLYPH_STUN_RPS {
        "stun"
    } else if is_rolling && has_prominent_side_spin {
        "rolling-english"
    } else if is_rolling {
        "rolling"
    } else if roll_alignment <= -0.5 {
        "draw"
    } else if roll_alignment >= 0.5 && roll_ratio > 1.15 {
        "follow"
    } else if wz.abs() >= planar_rps {
        "english"
    } else {
        "spin"
    };
    let planar_color = match kind {
        "stun" | "english" => SPIN_GLYPH_GREY,
        "rolling" | "rolling-english" => SPIN_GLYPH_GREEN,
        "draw" => SPIN_GLYPH_ORANGE,
        "follow" => SPIN_GLYPH_BLUE,
        _ => {
            let reference = rolling_target_rps.max(120.0);
            let t = (planar_rps / reference).clamp(0.0, 1.0);
            interpolate_rgb(SPIN_GLYPH_GREY, SPIN_GLYPH_AMBER, t)
        }
    };
    let z_color = SPIN_GLYPH_VIOLET;

    SpinGlyphMetrics {
        vx,
        vy,
        wx,
        wy,
        wz,
        planar_rps,
        total_rps,
        roll_ratio,
        roll_alignment,
        roll_slip_ips,
        angle_degrees,
        kind,
        planar_color,
        z_color,
    }
}

fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}
fn interpolate_rgb(start: [u8; 3], end: [u8; 3], t: f64) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        interpolate_channel(start[0], end[0], t),
        interpolate_channel(start[1], end[1], t),
        interpolate_channel(start[2], end[2], t),
    ]
}

fn interpolate_channel(start: u8, end: u8, t: f64) -> u8 {
    (start as f64 + (end as f64 - start as f64) * t).round() as u8
}

fn svg_rgb(color: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2])
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
                "<text class=\"ball-label\" y=\".5\" fill=\"#111\" font-size=\"{:.3}\" transform=\"rotate(-90)\">{}</text>\n",
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
        BallType::YellowCue => BallVisual {
            fill: "#f1c232",
            class_name: "yellow",
        },
        BallType::Red => BallVisual {
            fill: "#c82828",
            class_name: "red",
        },
    }
}

fn ball_label(ball_type: &BallType) -> Option<&'static str> {
    match ball_type {
        BallType::Cue | BallType::YellowCue | BallType::Red => None,
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
