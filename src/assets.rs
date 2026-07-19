use crate::BallType;

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
pub fn ball_img(ball: BallType) -> &'static [u8] {
    match ball {
        BallType::Cue => BALL_IMGS[0],
        BallType::One | BallType::YellowCue => BALL_IMGS[1],
        BallType::Two => BALL_IMGS[2],
        BallType::Three | BallType::Red => BALL_IMGS[3],
        BallType::Four => BALL_IMGS[4],
        BallType::Five => BALL_IMGS[5],
        BallType::Six => BALL_IMGS[6],
        BallType::Seven => BALL_IMGS[7],
        BallType::Eight => BALL_IMGS[8],
        BallType::Nine => BALL_IMGS[9],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ball_img_returns_the_borrowed_embedded_sprite_for_each_representative_mapping() {
        let cases = [
            (BallType::Cue, 0),
            (BallType::One, 1),
            (BallType::YellowCue, 1),
            (BallType::Three, 3),
            (BallType::Red, 3),
        ];

        for (ball, sprite_index) in cases {
            let actual: &'static [u8] = ball_img(ball);

            assert_eq!(actual, BALL_IMGS[sprite_index]);
        }
    }
}
