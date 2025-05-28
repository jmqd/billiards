
use crate::{BallType, Position};
use bigdecimal::ToPrimitive;

const TOPMOST: f32 = 42.;
const RIGHTMOST: f32 = 1040.;
const BOTTOMMOST: f32 = 1884.;
const LEFTMOST: f32 = 56.;
const BALL_TO_DIAMOND: f32 = 2.25 / 12.5;

#[allow(unused)]
pub fn ideal_ball_size_px() -> u32 {
    let px_diam_x = (RIGHTMOST - LEFTMOST) / 4.0;
    let px_diam_y = (BOTTOMMOST - TOPMOST) / 8.0;
    let px_ball = px_diam_x.min(px_diam_y) * BALL_TO_DIAMOND;
    px_ball.round() as u32
}

/// All of our ball sprites.
#[allow(unused)]
pub const BALL_IMGS: [&[u8]; 10] = [
    include_bytes!("assets/ball_cue.png"),
    include_bytes!("assets/ball_1.png"),
    include_bytes!("assets/ball_2.png"),
    include_bytes!("assets/ball_3.png"),
    include_bytes!("assets/ball_4.png"),
    include_bytes!("assets/ball_5.png"),
    include_bytes!("assets/ball_6.png"),
    include_bytes!("assets/ball_7.png"),
    include_bytes!("assets/ball_8.png"),
    include_bytes!("assets/ball_9.png"),
];

/// This image is 1089 × 1938 pixels.
#[allow(unused)]
pub const TABLE_DIAGRAM: &[u8] = include_bytes!("assets/table_diagram_head_top.png");

/// Retrieve the sprite for a given ball.
#[allow(unused)]
pub fn ball_img(ball: BallType) -> Vec<u8> {
    match ball {
        BallType::Cue => BALL_IMGS[0].to_vec(),
        BallType::One => BALL_IMGS[1].to_vec(),
        BallType::Two => BALL_IMGS[2].to_vec(),
        BallType::Three => BALL_IMGS[3].to_vec(),
        BallType::Four => BALL_IMGS[4].to_vec(),
        BallType::Five => BALL_IMGS[5].to_vec(),
        BallType::Six => BALL_IMGS[6].to_vec(),
        BallType::Seven => BALL_IMGS[7].to_vec(),
        BallType::Eight => BALL_IMGS[8].to_vec(),
        BallType::Nine => BALL_IMGS[9].to_vec(),
    }
}

/// Maps a diamond-grid position (x∈0‥4, y∈0‥8) to fractional coordinates inside
/// the playing surface of the pool table. This is useful to do pixel math.
#[allow(unused)]
pub fn diamond_to_pixel(pos: &Position) -> (i32, i32) {
    let x_px = LEFTMOST + (pos.x.magnitude.to_f32().unwrap() / 4.0) * (RIGHTMOST - LEFTMOST);

    let y_px = BOTTOMMOST - (pos.y.magnitude.to_f32().unwrap() / 8.0) * (BOTTOMMOST - TOPMOST);

    (x_px.round() as i32, y_px.round() as i32)
}
