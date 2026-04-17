use crate::{Inches, OverlayLayer, TYPICAL_BALL_RADIUS};
use image::Rgba;

#[derive(Clone, Debug, PartialEq)]
pub struct DashedLineStyle {
    pub color: Rgba<u8>,
    pub dash_px: f32,
    pub gap_px: f32,
    pub width_px: f32,
    pub layer: OverlayLayer,
}

impl DashedLineStyle {
    pub fn new(color: Rgba<u8>) -> Self {
        Self {
            color,
            dash_px: 3.0,
            gap_px: 12.0,
            width_px: 2.0,
            layer: OverlayLayer::BelowBalls,
        }
    }

    pub fn on_layer(mut self, layer: OverlayLayer) -> Self {
        self.layer = layer;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SmoothPolylineStyle {
    pub color: Rgba<u8>,
    pub width_px: f32,
    pub layer: OverlayLayer,
}

impl SmoothPolylineStyle {
    pub fn new(color: Rgba<u8>) -> Self {
        Self {
            color,
            width_px: 4.0,
            layer: OverlayLayer::BelowBalls,
        }
    }

    pub fn on_layer(mut self, layer: OverlayLayer) -> Self {
        self.layer = layer;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GhostBallStyle {
    pub fill_color: Rgba<u8>,
    pub outline_color: Rgba<u8>,
    pub layer: OverlayLayer,
}

impl Default for GhostBallStyle {
    fn default() -> Self {
        Self {
            fill_color: Rgba([255, 255, 255, 64]),
            outline_color: Rgba([0, 0, 0, 96]),
            layer: OverlayLayer::BelowBalls,
        }
    }
}

impl GhostBallStyle {
    pub fn on_layer(mut self, layer: OverlayLayer) -> Self {
        self.layer = layer;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathColorMode {
    Solid,
    FadeByTime,
    MotionPhase,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LabelOverlayStyle {
    pub enabled: bool,
    pub color: Rgba<u8>,
    pub layer: OverlayLayer,
}

impl Default for LabelOverlayStyle {
    fn default() -> Self {
        Self {
            enabled: false,
            color: Rgba([0, 0, 0, 255]),
            layer: OverlayLayer::AboveBalls,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EventMarkerStyle {
    pub enabled: bool,
    pub color: Rgba<u8>,
    pub radius_px: f32,
    pub layer: OverlayLayer,
}

impl Default for EventMarkerStyle {
    fn default() -> Self {
        Self {
            enabled: false,
            color: Rgba([0, 0, 0, 192]),
            radius_px: 5.0,
            layer: OverlayLayer::AboveBalls,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AimOverlayStyle {
    pub line: DashedLineStyle,
    pub ghost_ball: Option<GhostBallStyle>,
    pub clip_endpoints_to_ball_radius: Option<Inches>,
}

impl AimOverlayStyle {
    pub fn new(color: Rgba<u8>) -> Self {
        Self {
            line: DashedLineStyle::new(color),
            ghost_ball: Some(GhostBallStyle::default()),
            clip_endpoints_to_ball_radius: Some(TYPICAL_BALL_RADIUS.clone()),
        }
    }

    pub fn without_endpoint_clipping(mut self) -> Self {
        self.clip_endpoints_to_ball_radius = None;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BallPathStyle {
    pub line: DashedLineStyle,
    pub start_ghost_ball: Option<GhostBallStyle>,
    pub clip_endpoints_to_ball_radius: Option<Inches>,
    pub color_mode: PathColorMode,
    pub event_markers: EventMarkerStyle,
    pub labels: LabelOverlayStyle,
}

impl BallPathStyle {
    pub fn new(color: Rgba<u8>) -> Self {
        Self {
            line: DashedLineStyle::new(color),
            start_ghost_ball: None,
            clip_endpoints_to_ball_radius: Some(TYPICAL_BALL_RADIUS.clone()),
            color_mode: PathColorMode::Solid,
            event_markers: EventMarkerStyle::default(),
            labels: LabelOverlayStyle::default(),
        }
    }

    pub fn with_start_ghost(mut self, ghost_ball: GhostBallStyle) -> Self {
        self.start_ghost_ball = Some(ghost_ball);
        self
    }

    pub fn without_endpoint_clipping(mut self) -> Self {
        self.clip_endpoints_to_ball_radius = None;
        self
    }
}
