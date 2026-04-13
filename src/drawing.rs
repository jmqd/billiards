use image::{Rgba, RgbaImage};
use imageproc::{drawing::draw_polygon_mut, point::Point};

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
}
