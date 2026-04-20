use bigdecimal::ToPrimitive;
use billiards::{
    rack_9_ball, racked_ball_positions, Ball, BallSpec, BallType, Displacement, Position, TableSpec,
};

fn diamond_value(position: &Position) -> (f64, f64) {
    (
        position.x.magnitude.to_f64().expect("x magnitude"),
        position.y.magnitude.to_f64().expect("y magnitude"),
    )
}

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

#[test]
fn given_a_three_four_five_displacement_when_measuring_distance_then_the_pythagorean_length_is_returned(
) {
    let displacement = Displacement::new("3", "4");

    let distance = displacement.absolute_distance();

    assert_close(
        distance.magnitude.to_f64().expect("distance magnitude"),
        5.0,
    );
}

#[test]
fn given_racked_ball_positions_when_returned_then_adjacent_balls_are_already_one_diameter_apart() {
    let positions = racked_ball_positions();
    let table = TableSpec::default();
    let expected_diameter = table
        .inches_to_diamond(table.diamond_to_inches(billiards::Diamond::from("0.18")))
        .magnitude
        .to_f64()
        .expect("diameter");

    let head_ball = Ball {
        ty: BallType::One,
        position: positions[0].clone(),
        spec: BallSpec::default(),
    };
    let second_row_left = Ball {
        ty: BallType::Two,
        position: positions[1].clone(),
        spec: BallSpec::default(),
    };

    let distance = head_ball.distance(&second_row_left);

    assert_close(
        distance.magnitude.to_f64().expect("distance magnitude"),
        expected_diameter,
    );
}

#[test]
fn given_a_nine_ball_rack_when_built_then_the_nine_ball_sits_behind_the_head_ball_in_the_middle_of_the_triangle(
) {
    let rack = rack_9_ball();

    let head_ball = rack
        .iter()
        .find(|ball| ball.ty == BallType::One)
        .expect("head ball");
    let nine_ball = rack
        .iter()
        .find(|ball| ball.ty == BallType::Nine)
        .expect("nine ball");

    let (head_x, head_y) = diamond_value(&head_ball.position);
    let (nine_x, nine_y) = diamond_value(&nine_ball.position);

    assert_close(head_x, 2.0);
    assert_close(head_y, 2.0);
    assert_close(nine_x, 2.0);
    assert!(nine_y < head_y, "expected nine-ball behind the head ball");
}

#[test]
fn given_racked_ball_positions_when_checked_then_the_triangle_is_frozen_without_gaps_or_overlaps() {
    let positions = racked_ball_positions();
    let table = TableSpec::default();
    let expected_diameter_inches = BallSpec::default().radius.as_f64() * 2.0;
    let mut touching_pairs = 0usize;

    let balls = positions
        .into_iter()
        .enumerate()
        .map(|(idx, position)| Ball {
            ty: match idx {
                0 => BallType::One,
                1 => BallType::Two,
                2 => BallType::Three,
                3 => BallType::Four,
                4 => BallType::Nine,
                5 => BallType::Five,
                6 => BallType::Six,
                7 => BallType::Seven,
                _ => BallType::Eight,
            },
            position,
            spec: BallSpec::default(),
        })
        .collect::<Vec<_>>();

    for first in 0..balls.len() {
        for second in (first + 1)..balls.len() {
            let distance_inches = table
                .diamond_to_inches(balls[first].distance(&balls[second]))
                .as_f64();
            assert!(
                distance_inches + 1e-9 >= expected_diameter_inches,
                "rack pair ({first}, {second}) overlaps: distance {distance_inches} < diameter {expected_diameter_inches}"
            );
            if (distance_inches - expected_diameter_inches).abs() < 1e-9 {
                touching_pairs += 1;
            }
        }
    }

    assert_eq!(
        touching_pairs, 16,
        "expected the frozen nine-ball triangle to contain the standard 16 touching pairs"
    );
}
