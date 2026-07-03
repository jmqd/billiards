use crate::visualization::{
    DashedLineStyle, EventMarkerStyle, GhostBallStyle, HeadingChevronStyle, LabelOverlayStyle,
    SmoothPolylineStyle,
};
use crate::{
    assets, drawing, Angle, BallSpec, BallType, DiagramBackground, DiagramRenderOptions, Inches,
    OverlayLayer, Position, TableKind, TableSpec,
};
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
const DIAMOND_SIGHT_WIDTH_IN: f32 = 1.35;
const DIAMOND_SIGHT_HEIGHT_IN: f32 = 0.62;
const CORNER_POCKET_MOUTH_IN: f32 = 4.5;
const SIDE_POCKET_MOUTH_IN: f32 = 5.0;
const CORNER_POCKET_SHELF_IN: f32 = 1.75;
const CORNER_POCKET_WELL_IN: f32 = 4.55;
const SIDE_POCKET_LIP_IN: f32 = 1.6;
const SIDE_POCKET_WELL_IN: f32 = 5.0;
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
        svg.push_str(".overlay-label{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-weight:700;dominant-baseline:central}.origin-marker{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-weight:800;text-anchor:middle;dominant-baseline:central;pointer-events:none}.event-marker[data-event-label]{cursor:help}\n");
        svg.push_str(".table-cloth{fill:url(#tournament-blue-cloth)}.table-cloth-texture{fill:url(#cloth-weave);opacity:.20}");
        svg.push_str(".table-rail{fill:url(#rosewood-rail)}.table-rail-grain{opacity:.62}.table-rail-grain-horizontal{fill:url(#rosewood-grain)}.table-rail-grain-vertical{fill:url(#rosewood-grain-vertical)}.table-rail-inner-shadow{fill:none;stroke:#210b08;stroke-width:10;opacity:.72}");
        svg.push_str(".table-cushion{fill:url(#blue-cushion)}.table-cushion-nose{stroke:#4bd2ea;stroke-width:3;stroke-linecap:round;opacity:.8}.table-cushion-back{stroke:#056a87;stroke-width:3;stroke-linecap:round;opacity:.65}");
        svg.push_str(".table-pocket-well{fill:url(#pocket-well);stroke:none}.table-pocket-leather{fill:none;stroke:url(#pocket-leather);stroke-linecap:round;stroke-linejoin:round;opacity:.98}.table-pocket-leather-highlight{fill:none;stroke:#8d8377;stroke-width:2;stroke-linecap:round;stroke-linejoin:round;opacity:.30}");
        svg.push_str(".table-pocket-shelf{fill:url(#tournament-blue-cloth);stroke:none;opacity:1}.table-pocket-shelf-texture{fill:url(#cloth-weave);stroke:none;opacity:.20;pointer-events:none}");
        svg.push_str(".table-pocket-facing{stroke:#050403;stroke-linecap:round;stroke-linejoin:round}.table-diamond{fill:#f6f0de;stroke:#9b8c63;stroke-width:.75;opacity:.94}\n");
        svg.push_str(".carom-table .table-rail{fill:url(#carom-wood-rail)}.carom-table .table-cloth{fill:url(#heated-carom-cloth)}.carom-table .table-cloth-texture{opacity:.16}.carom-table .table-cushion{fill:url(#heated-carom-cushion)}.carom-table .table-cushion-nose{stroke:#88ecff;stroke-width:3.2}.carom-table .table-cushion-back{stroke:#064f69;stroke-width:3.2}.carom-table .table-rail-inner-shadow{stroke:#0b0705;stroke-width:12;opacity:.58}\n");
        svg.push_str("</style>\n");
        push_svg_table_defs(&mut svg, scene.viewport);

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
                        .ball_diameter_px(&scene.table_spec, &BallSpec::default()),
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
    let side_mouth_y = viewport.y_inches(SIDE_POCKET_MOUTH_IN);
    let side_lip_x = viewport.x_inches(SIDE_POCKET_LIP_IN);
    let side_well_x = viewport.x_inches(SIDE_POCKET_WELL_IN);
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
    svg.push_str(&format!(
        "<rect class=\"table-rail-inner-shadow\" x=\"{left:.3}\" y=\"{top:.3}\" width=\"{cloth_w:.3}\" height=\"{cloth_h:.3}\"/>\n"
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
            side_lip_x,
            side_well_x,
            cushion_x,
            layer,
        );
        push_svg_side_pocket(
            svg,
            right,
            center_y,
            1.0,
            side_mouth_y,
            side_lip_x,
            side_well_x,
            cushion_x,
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

    for layer in [PocketLayer::Shelf, PocketLayer::Facing] {
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
            side_lip_x,
            side_well_x,
            cushion_x,
            layer,
        );
        push_svg_side_pocket(
            svg,
            right,
            center_y,
            1.0,
            side_mouth_y,
            side_lip_x,
            side_well_x,
            cushion_x,
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
    Facing,
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
    let well_scale = CORNER_POCKET_WELL_IN / CORNER_POCKET_SHELF_IN;
    let well_x = shelf_x * well_scale;
    let well_y = shelf_y * well_scale;

    let liner_stroke = (cushion_x.min(cushion_y) * 0.82).clamp(20.0, 32.0);
    let facing_stroke = (cushion_x.min(cushion_y) * 0.32).clamp(7.0, 11.0);
    let liner_clearance = liner_stroke * 0.5;

    let point =
        |inside_x: f32, inside_y: f32| (corner_x - x_sign * inside_x, corner_y - y_sign * inside_y);

    let (mouth_top_x, mouth_top_y) = point(run_x, 0.0);
    let (mouth_side_x, mouth_side_y) = point(0.0, run_y);

    let (top_liner_end_x, top_liner_end_y) = point(
        run_x - cushion_bevel_x * 0.75,
        -(cushion_y + liner_clearance),
    );
    let (side_liner_end_x, side_liner_end_y) = point(
        -(cushion_x + liner_clearance),
        run_y - cushion_bevel_y * 0.75,
    );

    let (upper_ctl1_x, upper_ctl1_y) = point(run_x * 0.72, -(cushion_y + liner_clearance));
    let (upper_ctl2_x, upper_ctl2_y) = point(run_x * 0.08, -well_y * 0.98);
    let (back_crown_x, back_crown_y) = point(-well_x * 0.72, -well_y * 0.72);
    let (lower_ctl1_x, lower_ctl1_y) = point(-well_x * 0.98, run_y * 0.08);
    let (lower_ctl2_x, lower_ctl2_y) = point(-(cushion_x + liner_clearance), run_y * 0.72);

    let back_curve = format!(
        "M {top_liner_end_x:.3} {top_liner_end_y:.3} \
         C {upper_ctl1_x:.3} {upper_ctl1_y:.3} {upper_ctl2_x:.3} {upper_ctl2_y:.3} {back_crown_x:.3} {back_crown_y:.3} \
         C {lower_ctl1_x:.3} {lower_ctl1_y:.3} {lower_ctl2_x:.3} {lower_ctl2_y:.3} {side_liner_end_x:.3} {side_liner_end_y:.3}"
    );

    match layer {
        PocketLayer::Well => {
            svg.push_str(&format!(
                "<path class=\"table-pocket-well\" data-pocket=\"corner\" d=\"M {mouth_top_x:.3} {mouth_top_y:.3} L {top_liner_end_x:.3} {top_liner_end_y:.3} C {upper_ctl1_x:.3} {upper_ctl1_y:.3} {upper_ctl2_x:.3} {upper_ctl2_y:.3} {back_crown_x:.3} {back_crown_y:.3} C {lower_ctl1_x:.3} {lower_ctl1_y:.3} {lower_ctl2_x:.3} {lower_ctl2_y:.3} {side_liner_end_x:.3} {side_liner_end_y:.3} L {mouth_side_x:.3} {mouth_side_y:.3} Z\"/>\n"
            ));
        }
        PocketLayer::Shelf => {
            let (shelf_outer_control_x, shelf_outer_control_y) =
                point(-cushion_x * 0.16, -cushion_y * 0.16);
            let (shelf_inner_control_x, shelf_inner_control_y) = point(run_x * 0.32, run_y * 0.32);
            let shelf_path = format!(
                "M {mouth_top_x:.3} {mouth_top_y:.3} \
                 Q {shelf_outer_control_x:.3} {shelf_outer_control_y:.3} {mouth_side_x:.3} {mouth_side_y:.3} \
                 Q {shelf_inner_control_x:.3} {shelf_inner_control_y:.3} {mouth_top_x:.3} {mouth_top_y:.3} Z"
            );
            svg.push_str(&format!(
                "<path class=\"table-pocket-shelf\" data-pocket=\"corner-shelf\" d=\"{shelf_path}\"/>\n"
            ));
            svg.push_str(&format!(
                "<path class=\"table-pocket-shelf-texture\" data-pocket=\"corner-shelf-texture\" d=\"{shelf_path}\"/>\n"
            ));
        }
        PocketLayer::Liner => {
            svg.push_str(&format!(
                "<path class=\"table-pocket-leather\" data-pocket=\"corner-liner\" stroke-width=\"{liner_stroke:.3}\" d=\"{back_curve}\"/>\n"
            ));
            svg.push_str(&format!(
                "<path class=\"table-pocket-leather-highlight\" d=\"{back_curve}\"/>\n"
            ));
        }
        PocketLayer::Facing => {
            let (top_facing_back_x, top_facing_back_y) =
                point(run_x - cushion_bevel_x * 0.75, -cushion_y);
            let (side_facing_back_x, side_facing_back_y) =
                point(-cushion_x, run_y - cushion_bevel_y * 0.75);

            svg.push_str(&format!(
                "<line class=\"table-pocket-facing\" style=\"stroke-width:{facing_stroke:.3}\" x1=\"{top_facing_back_x:.3}\" y1=\"{top_facing_back_y:.3}\" x2=\"{top_liner_end_x:.3}\" y2=\"{top_liner_end_y:.3}\"/>\n"
            ));
            svg.push_str(&format!(
                "<line class=\"table-pocket-facing\" style=\"stroke-width:{facing_stroke:.3}\" x1=\"{side_liner_end_x:.3}\" y1=\"{side_liner_end_y:.3}\" x2=\"{side_facing_back_x:.3}\" y2=\"{side_facing_back_y:.3}\"/>\n"
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
    lip_depth_x: f32,
    well_depth_x: f32,
    cushion_x: f32,
    layer: PocketLayer,
) {
    let liner_stroke = (cushion_x * 0.78).clamp(20.0, 32.0);
    let facing_stroke = (cushion_x * 0.32).clamp(7.0, 11.0);
    let liner_depth_x = cushion_x + liner_stroke * 0.5;
    let back_depth_x = (well_depth_x - liner_depth_x).max(0.0);

    let point = |depth_x: f32, offset_y: f32| (rail_x + x_sign * depth_x, center_y + offset_y);

    let (top_x, top_y) = point(0.0, -mouth_y * 0.5);
    let (bottom_x, bottom_y) = point(0.0, mouth_y * 0.5);

    let (upper_liner_end_x, upper_liner_end_y) = point(liner_depth_x, -mouth_y * 0.5);
    let (lower_liner_end_x, lower_liner_end_y) = point(liner_depth_x, mouth_y * 0.5);

    let (upper_nipple_x, upper_nipple_y) =
        point(liner_depth_x + back_depth_x * 0.08, -mouth_y * 0.50);
    let (upper_ctl_x, upper_ctl_y) = point(liner_depth_x + back_depth_x * 0.45, -mouth_y * 0.54);
    let (upper_back_x, upper_back_y) = point(liner_depth_x + back_depth_x * 0.98, -mouth_y * 0.30);

    let (back_ctl_upper_x, back_ctl_upper_y) =
        point(liner_depth_x + back_depth_x * 1.13, -mouth_y * 0.16);
    let (back_ctl_lower_x, back_ctl_lower_y) =
        point(liner_depth_x + back_depth_x * 1.13, mouth_y * 0.16);

    let (lower_back_x, lower_back_y) = point(liner_depth_x + back_depth_x * 0.98, mouth_y * 0.30);
    let (lower_ctl_x, lower_ctl_y) = point(liner_depth_x + back_depth_x * 0.45, mouth_y * 0.54);
    let (lower_nipple_x, lower_nipple_y) =
        point(liner_depth_x + back_depth_x * 0.08, mouth_y * 0.50);

    let back_curve = format!(
        "M {upper_liner_end_x:.3} {upper_liner_end_y:.3} \
         C {upper_nipple_x:.3} {upper_nipple_y:.3} {upper_ctl_x:.3} {upper_ctl_y:.3} {upper_back_x:.3} {upper_back_y:.3} \
         C {back_ctl_upper_x:.3} {back_ctl_upper_y:.3} {back_ctl_lower_x:.3} {back_ctl_lower_y:.3} {lower_back_x:.3} {lower_back_y:.3} \
         C {lower_ctl_x:.3} {lower_ctl_y:.3} {lower_nipple_x:.3} {lower_nipple_y:.3} {lower_liner_end_x:.3} {lower_liner_end_y:.3}"
    );

    match layer {
        PocketLayer::Well => {
            svg.push_str(&format!(
                "<path class=\"table-pocket-well\" data-pocket=\"side\" d=\"M {top_x:.3} {top_y:.3} L {upper_liner_end_x:.3} {upper_liner_end_y:.3} C {upper_nipple_x:.3} {upper_nipple_y:.3} {upper_ctl_x:.3} {upper_ctl_y:.3} {upper_back_x:.3} {upper_back_y:.3} C {back_ctl_upper_x:.3} {back_ctl_upper_y:.3} {back_ctl_lower_x:.3} {back_ctl_lower_y:.3} {lower_back_x:.3} {lower_back_y:.3} C {lower_ctl_x:.3} {lower_ctl_y:.3} {lower_nipple_x:.3} {lower_nipple_y:.3} {lower_liner_end_x:.3} {lower_liner_end_y:.3} L {bottom_x:.3} {bottom_y:.3} Z\"/>\n"
            ));
        }
        PocketLayer::Shelf => {
            let (shelf_outer_control_x, shelf_outer_control_y) = point(lip_depth_x * 0.20, 0.0);
            let (shelf_inner_control_x, shelf_inner_control_y) =
                point(-(liner_stroke * 0.24).max(lip_depth_x * 0.20), 0.0);
            let shelf_path = format!(
                "M {top_x:.3} {top_y:.3} \
                 Q {shelf_outer_control_x:.3} {shelf_outer_control_y:.3} {bottom_x:.3} {bottom_y:.3} \
                 Q {shelf_inner_control_x:.3} {shelf_inner_control_y:.3} {top_x:.3} {top_y:.3} Z"
            );
            svg.push_str(&format!(
                "<path class=\"table-pocket-shelf\" data-pocket=\"side-shelf\" d=\"{shelf_path}\"/>\n"
            ));
            svg.push_str(&format!(
                "<path class=\"table-pocket-shelf-texture\" data-pocket=\"side-shelf-texture\" d=\"{shelf_path}\"/>\n"
            ));
        }
        PocketLayer::Liner => {
            svg.push_str(&format!(
                "<path class=\"table-pocket-leather\" data-pocket=\"side-liner\" stroke-width=\"{liner_stroke:.3}\" d=\"{back_curve}\"/>\n"
            ));
            svg.push_str(&format!(
                "<path class=\"table-pocket-leather-highlight\" d=\"{back_curve}\"/>\n"
            ));
        }
        PocketLayer::Facing => {
            let (upper_facing_x, upper_facing_y) = point(cushion_x, -mouth_y * 0.34);
            let (lower_facing_x, lower_facing_y) = point(cushion_x, mouth_y * 0.34);

            svg.push_str(&format!(
                "<line class=\"table-pocket-facing\" style=\"stroke-width:{facing_stroke:.3}\" x1=\"{upper_liner_end_x:.3}\" y1=\"{upper_liner_end_y:.3}\" x2=\"{upper_facing_x:.3}\" y2=\"{upper_facing_y:.3}\"/>\n"
            ));
            svg.push_str(&format!(
                "<line class=\"table-pocket-facing\" style=\"stroke-width:{facing_stroke:.3}\" x1=\"{lower_liner_end_x:.3}\" y1=\"{lower_liner_end_y:.3}\" x2=\"{lower_facing_x:.3}\" y2=\"{lower_facing_y:.3}\"/>\n"
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
