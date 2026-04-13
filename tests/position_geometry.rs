use bigdecimal::ToPrimitive;
use billiards::{
    translate_inwards, Angle, Ball, BallSpec, BallType, Diamond, GameState, Position, Rail,
    TableSpec, TYPICAL_BALL_RADIUS, TOP_RIGHT_DIAMOND,
};

fn diamond_value(diamond: &Diamond) -> f64 {
    diamond.magnitude.to_f64().expect("diamond magnitude")
}

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

#[test]
fn given_a_position_when_translating_in_cardinal_directions_then_the_expected_coordinates_are_reached() {
    let origin = Position::new(2u8, 4u8);

    let up = origin.translate(Diamond::one(), Angle::from_north(0.0, 1.0));
    let right = origin.translate(Diamond::one(), Angle::from_north(1.0, 0.0));
    let down = origin.translate(Diamond::one(), Angle::from_north(0.0, -1.0));
    let left = origin.translate(Diamond::one(), Angle::from_north(-1.0, 0.0));

    assert_close(diamond_value(&up.x), 2.0);
    assert_close(diamond_value(&up.y), 5.0);
    assert_close(diamond_value(&right.x), 3.0);
    assert_close(diamond_value(&right.y), 4.0);
    assert_close(diamond_value(&down.x), 2.0);
    assert_close(diamond_value(&down.y), 3.0);
    assert_close(diamond_value(&left.x), 1.0);
    assert_close(diamond_value(&left.y), 4.0);
}

#[test]
fn given_a_position_when_translating_inches_and_resolving_then_the_equivalent_diamond_shift_is_applied() {
    let table = TableSpec::default();
    let mut shifted = Position::new(2u8, 4u8)
        .translate_inches(TYPICAL_BALL_RADIUS.clone(), Angle::from_north(1.0, 0.0));

    shifted.resolve_shifts(&table);

    assert_close(diamond_value(&shifted.x), 2.09);
    assert_close(diamond_value(&shifted.y), 4.0);
}

#[test]
fn given_a_position_when_translating_to_the_ghost_ball_then_it_moves_by_one_ball_diameter() {
    let table = TableSpec::default();
    let mut ghost = Position::new(2u8, 4u8).translate_ghost_ball(Angle::from_north(0.0, 1.0));

    ghost.resolve_shifts(&table);

    assert_close(diamond_value(&ghost.x), 2.0);
    assert_close(diamond_value(&ghost.y), 4.18);
}

#[test]
fn given_the_top_right_corner_when_translating_inwards_then_both_coordinates_move_toward_center() {
    let moved = translate_inwards(&TOP_RIGHT_DIAMOND, Diamond::from("0.07"), Diamond::from("0.07"));

    assert_close(diamond_value(&moved.x), 3.93);
    assert_close(diamond_value(&moved.y), 7.93);
}

#[test]
fn given_each_rail_when_freezing_a_ball_then_the_ball_center_sits_one_radius_in_from_the_cushion() {
    let mut state = GameState::new(TableSpec::default());

    state.freeze_to_rail(
        Rail::Top,
        Diamond::from("1.5"),
        Ball {
            ty: BallType::One,
            position: Position::zeroed(),
            spec: BallSpec::default(),
        },
    );
    state.freeze_to_rail(
        Rail::Right,
        Diamond::from("6.5"),
        Ball {
            ty: BallType::Two,
            position: Position::zeroed(),
            spec: BallSpec::default(),
        },
    );
    state.freeze_to_rail(
        Rail::Bottom,
        Diamond::from("2.5"),
        Ball {
            ty: BallType::Three,
            position: Position::zeroed(),
            spec: BallSpec::default(),
        },
    );
    state.freeze_to_rail(
        Rail::Left,
        Diamond::from("5.5"),
        Ball {
            ty: BallType::Four,
            position: Position::zeroed(),
            spec: BallSpec::default(),
        },
    );

    let top = state.select_ball(BallType::One).expect("top ball");
    let right = state.select_ball(BallType::Two).expect("right ball");
    let bottom = state.select_ball(BallType::Three).expect("bottom ball");
    let left = state.select_ball(BallType::Four).expect("left ball");

    assert_close(diamond_value(&top.position.x), 1.5);
    assert_close(diamond_value(&top.position.y), 7.91);
    assert_close(diamond_value(&right.position.x), 3.91);
    assert_close(diamond_value(&right.position.y), 6.5);
    assert_close(diamond_value(&bottom.position.x), 2.5);
    assert_close(diamond_value(&bottom.position.y), 0.09);
    assert_close(diamond_value(&left.position.x), 0.09);
    assert_close(diamond_value(&left.position.y), 5.5);
}
