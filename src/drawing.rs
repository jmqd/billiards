use image::{Rgba, RgbaImage};
use imageproc::{
    drawing::{draw_antialiased_line_segment_mut, draw_polygon_mut},
    pixelops::interpolate,
    point::Point,
};

use crate::Position;

#[inline]
fn normal(ux: f32, uy: f32) -> (f32, f32) {
    (uy, -ux)
}

/// Add a scaled vector to a point
#[inline]
fn offset(p: (f32, f32), vx: f32, vy: f32, s: f32) -> Point<i32> {
    Point::new((p.0 + vx * s) as i32, (p.1 + vy * s) as i32)
}

/// A dashed line with adjustable thickness (via `line_width_px`)
pub fn draw_dashed_line_thick_mut(
    img: &mut RgbaImage,
    a: &Position,
    b: &Position,
    dash_px: f32,
    gap_px: f32,
    width_px: f32,
    color: Rgba<u8>,
) {
    let (x0, y0) = crate::assets::diamond_to_pixel(a);
    let (x1, y1) = crate::assets::diamond_to_pixel(b);

    let dx = x1 as f32 - x0 as f32;
    let dy = y1 as f32 - y0 as f32;
    let len = (dx * dx + dy * dy).sqrt();
    if len == 0.0 {
        return;
    }

    // Unit direction and its normal
    let ux = dx / len;
    let uy = dy / len;
    let (nx, ny) = normal(ux, uy);
    let half_w = width_px * 0.5;

    let mut s = 0.0;
    while s < len {
        let e = (s + dash_px).min(len);

        // Centre-line endpoints of this dash
        let p0 = (x0 as f32 + ux * s, y0 as f32 + uy * s);
        let p1 = (x0 as f32 + ux * e, y0 as f32 + uy * e);

        // Four rectangle corners = endpoints +/- half_w along the normal
        let c0 = offset(p0, nx, ny, half_w);
        let c1 = offset(p1, nx, ny, half_w);
        let c2 = offset(p1, -nx, -ny, half_w);
        let c3 = offset(p0, -nx, -ny, half_w);

        draw_polygon_mut(img, &[c0, c1, c2, c3], color);

        s += dash_px + gap_px;
    }
}

fn draw_antialiased_thick_line_segment_mut(
    img: &mut RgbaImage,
    start: (i32, i32),
    end: (i32, i32),
    width_px: f32,
    color: Rgba<u8>,
) {
    let dx = end.0 as f32 - start.0 as f32;
    let dy = end.1 as f32 - start.1 as f32;
    let len = (dx * dx + dy * dy).sqrt();
    if len == 0.0 {
        return;
    }

    let ux = dx / len;
    let uy = dy / len;
    let (nx, ny) = normal(ux, uy);
    let half_w = width_px * 0.5;
    let radius = half_w.ceil() as i32;

    for offset in -radius..=radius {
        let offset_f = offset as f32;
        let coverage = (half_w + 0.5 - offset_f.abs()).clamp(0.0, 1.0);
        if coverage <= 0.0 {
            continue;
        }

        let sx = (start.0 as f32 + nx * offset_f).round() as i32;
        let sy = (start.1 as f32 + ny * offset_f).round() as i32;
        let ex = (end.0 as f32 + nx * offset_f).round() as i32;
        let ey = (end.1 as f32 + ny * offset_f).round() as i32;
        let alpha = ((color[3] as f32) * coverage).round().clamp(0.0, 255.0) as u8;
        let stroke = Rgba([color[0], color[1], color[2], alpha]);

        draw_antialiased_line_segment_mut(img, (sx, sy), (ex, ey), stroke, interpolate);
    }
}

/// Draw a smooth anti-aliased polyline with a first-pass configurable width.
pub fn draw_smooth_polyline_mut(
    img: &mut RgbaImage,
    points: &[Position],
    width_px: f32,
    color: Rgba<u8>,
) {
    if points.len() < 2 {
        return;
    }

    for window in points.windows(2) {
        let start = crate::assets::diamond_to_pixel(&window[0]);
        let end = crate::assets::diamond_to_pixel(&window[1]);
        draw_antialiased_thick_line_segment_mut(img, start, end, width_px, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Position;

    fn changed_pixel_count(image: &RgbaImage) -> usize {
        image
            .pixels()
            .filter(|pixel| **pixel != Rgba([0, 0, 0, 0]))
            .count()
    }

    #[test]
    fn given_a_zero_length_overlay_line_when_drawing_then_no_pixels_are_changed() {
        let mut image = RgbaImage::new(1089, 1938);
        let start = Position::new(2u8, 4u8);

        draw_dashed_line_thick_mut(
            &mut image,
            &start,
            &start,
            3.0,
            12.0,
            2.0,
            Rgba([255, 0, 0, 255]),
        );

        assert_eq!(changed_pixel_count(&image), 0);
    }

    #[test]
    fn given_a_horizontal_overlay_line_when_drawing_then_some_pixels_are_colored() {
        let mut image = RgbaImage::new(1089, 1938);

        draw_dashed_line_thick_mut(
            &mut image,
            &Position::new(1u8, 4u8),
            &Position::new(3u8, 4u8),
            8.0,
            4.0,
            2.0,
            Rgba([0, 255, 0, 255]),
        );

        assert!(changed_pixel_count(&image) > 0);
    }

    #[test]
    fn given_a_smooth_polyline_with_two_points_when_drawing_then_some_pixels_are_colored() {
        let mut image = RgbaImage::new(1089, 1938);

        draw_smooth_polyline_mut(
            &mut image,
            &[Position::new(1u8, 4u8), Position::new(3u8, 4u8)],
            4.0,
            Rgba([0, 255, 0, 255]),
        );

        assert!(changed_pixel_count(&image) > 0);
    }

    #[test]
    fn given_a_smooth_polyline_with_fewer_than_two_points_when_drawing_then_no_pixels_are_changed()
    {
        let mut image = RgbaImage::new(1089, 1938);

        draw_smooth_polyline_mut(
            &mut image,
            &[Position::new(2u8, 4u8)],
            4.0,
            Rgba([255, 0, 0, 255]),
        );

        assert_eq!(changed_pixel_count(&image), 0);
    }
}
