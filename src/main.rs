use bigdecimal::ToPrimitive;
use billiards::*;

mod assets;

fn main() {
    let table_spec = TableSpec::new_9ft_brunswick_gc4();
    let mut game_state = GameState {
        table_spec: table_spec.clone(),
        ball_positions: vec![
            Ball {
                ty: BallType::Cue,
                position: CENTER_SPOT.clone(),
                spec: BallSpec::default(),
            },
            Ball {
                ty: BallType::Nine,
                // TODO: Encode these positions as "hangers" in Position impl.
                position: Position {
                    x: Diamond::from("3.65"),
                    y: Diamond::from("7.625"),
                },
                spec: BallSpec::default(),
            },
        ],
        ty: GameType::NineBall,
        cueball_modifier: CueballModifier::AsItLays,
    };

    game_state.freeze_to_rail(
        Rail::Left,
        Diamond::six(),
        Ball {
            ty: BallType::Eight,
            position: Position::default(),
            spec: BallSpec::default(),
        },
    );

    let cueball = game_state.select_ball(BallType::Cue).unwrap();
    let nine_ball = game_state.select_ball(BallType::Nine).unwrap();

    let displacement = cueball.displacement(nine_ball);
    let distance = displacement.absolute_distance();

    println!("displacement = {:#?}", displacement);
    println!("distance = {:#?}", distance.magnitude.to_f64());

    let img = game_state.draw_2d_diagram();
    write_png_to_file(&img, None);
}
