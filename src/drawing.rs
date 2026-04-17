use image::{Pixel, Rgba, RgbaImage};
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

fn blend_pixel(img: &mut RgbaImage, x: i32, y: i32, color: Rgba<u8>) {
    if x < 0 || y < 0 {
        return;
    }

    let x = x as u32;
    let y = y as u32;
    if x >= img.width() || y >= img.height() {
        return;
    }

    img.get_pixel_mut(x, y).blend(&color);
}

fn draw_filled_circle_alpha_mut(
    img: &mut RgbaImage,
    center: (i32, i32),
    radius_px: f32,
    color: Rgba<u8>,
) {
    if radius_px <= 0.0 || color[3] == 0 {
        return;
    }

    let radius_sq = radius_px * radius_px;
    let min_x = (center.0 as f32 - radius_px).floor() as i32;
    let max_x = (center.0 as f32 + radius_px).ceil() as i32;
    let min_y = (center.1 as f32 - radius_px).floor() as i32;
    let max_y = (center.1 as f32 + radius_px).ceil() as i32;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = (x - center.0) as f32;
            let dy = (y - center.1) as f32;
            if dx * dx + dy * dy <= radius_sq {
                blend_pixel(img, x, y, color);
            }
        }
    }
}

pub fn draw_filled_circle_marker_mut(
    img: &mut RgbaImage,
    center: &Position,
    radius_px: f32,
    color: Rgba<u8>,
) {
    draw_filled_circle_alpha_mut(img, crate::assets::diamond_to_pixel(center), radius_px, color);
}

fn digit_bitmap(ch: char) -> Option<[u8; 7]> {
    Some(match ch {
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b11100],
        _ => return None,
    })
}

pub fn draw_text_label_mut(
    img: &mut RgbaImage,
    anchor: &Position,
    text: &str,
    offset_x_px: i32,
    offset_y_px: i32,
    scale_px: u32,
    color: Rgba<u8>,
) {
    if scale_px == 0 || color[3] == 0 {
        return;
    }

    let (anchor_x, anchor_y) = crate::assets::diamond_to_pixel(anchor);
    let glyph_advance = 6 * scale_px as i32;

    for (index, ch) in text.chars().enumerate() {
        let Some(bitmap) = digit_bitmap(ch) else {
            continue;
        };

        let glyph_x = anchor_x + offset_x_px + index as i32 * glyph_advance;
        let glyph_y = anchor_y + offset_y_px;

        for (row, bits) in bitmap.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) == 0 {
                    continue;
                }
                for dy in 0..scale_px as i32 {
                    for dx in 0..scale_px as i32 {
                        blend_pixel(
                            img,
                            glyph_x + col * scale_px as i32 + dx,
                            glyph_y + row as i32 * scale_px as i32 + dy,
                            color,
                        );
                    }
                }
            }
        }
    }
}

/// Draw a translucent ghost-ball marker with a dotted outline at a table position.
pub fn draw_ghost_ball_mut(
    img: &mut RgbaImage,
    center: &Position,
    diameter_px: u32,
    fill_color: Rgba<u8>,
    outline_color: Rgba<u8>,
) {
    if diameter_px == 0 {
        return;
    }

    let center = crate::assets::diamond_to_pixel(center);
    let radius_px = diameter_px as f32 * 0.5;
    draw_filled_circle_alpha_mut(img, center, radius_px, fill_color);

    if outline_color[3] == 0 {
        return;
    }

    let dot_radius_px = (radius_px * 0.1).max(1.0);
    let orbit_radius_px = (radius_px - dot_radius_px).max(0.0);
    let circumference_px = 2.0 * std::f32::consts::PI * orbit_radius_px.max(1.0);
    let dot_spacing_px = (dot_radius_px * 3.5).max(6.0);
    let dot_count = ((circumference_px / dot_spacing_px).round() as usize).max(12);

    for dot in 0..dot_count {
        let theta = 2.0 * std::f32::consts::PI * dot as f32 / dot_count as f32;
        let x = center.0 as f32 + orbit_radius_px * theta.cos();
        let y = center.1 as f32 + orbit_radius_px * theta.sin();
        draw_filled_circle_alpha_mut(
            img,
            (x.round() as i32, y.round() as i32),
            dot_radius_px,
            outline_color,
        );
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

    #[test]
    fn given_a_ghost_ball_overlay_when_drawing_then_some_pixels_are_colored() {
        let mut image = RgbaImage::new(1089, 1938);

        draw_ghost_ball_mut(
            &mut image,
            &Position::new(2u8, 4u8),
            39,
            Rgba([255, 255, 255, 64]),
            Rgba([0, 0, 0, 96]),
        );

        assert!(changed_pixel_count(&image) > 0);
    }

    #[test]
    fn given_a_circle_marker_overlay_when_drawing_then_some_pixels_are_colored() {
        let mut image = RgbaImage::new(1089, 1938);

        draw_filled_circle_marker_mut(
            &mut image,
            &Position::new(2u8, 4u8),
            5.0,
            Rgba([255, 0, 0, 192]),
        );

        assert!(changed_pixel_count(&image) > 0);
    }

    #[test]
    fn given_a_numeric_text_label_when_drawing_then_some_pixels_are_colored() {
        let mut image = RgbaImage::new(1089, 1938);

        draw_text_label_mut(
            &mut image,
            &Position::new(2u8, 4u8),
            "12",
            8,
            -8,
            2,
            Rgba([0, 0, 0, 255]),
        );

        assert!(changed_pixel_count(&image) > 0);
    }
}
