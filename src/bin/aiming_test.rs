use billiards::*;

fn main() {
    let table_spec = TableSpec::brunswick_gc4_9ft();

    let mut game_state = GameState {
        table_spec: table_spec.clone(),
        ball_positions: vec![
            Ball {
                ty: BallType::Cue,
                position: Position {
                    x: Diamond::from(2),
                    y: Diamond::from(4),
                    ..Default::default()
                },
                spec: BallSpec::default(),
            },
            Ball {
                ty: BallType::Eight,
                position: Position {
                    x: Diamond::from(2),
                    y: Diamond::from(6),
                    ..Default::default()
                },
                spec: BallSpec::default(),
            },
        ],
        ty: GameType::NineBall,
        cueball_modifier: CueballModifier::AsItLays,
    };

    game_state.freeze_to_rail(
        Rail::Bottom,
        Diamond::one(),
        Ball {
            ty: BallType::One,
            position: Position::zeroed(),
            spec: BallSpec::default(),
        },
    );

    game_state.freeze_to_rail(
        Rail::Right,
        Diamond::six(),
        Ball {
            ty: BallType::Six,
            position: Position::zeroed(),
            spec: BallSpec::default(),
        },
    );

    let img = game_state.draw_2d_diagram();
    write_png_to_file(&img, None);

    println!("{:#?}", table_spec);
    println!("{:#?}", game_state);

    for ball in game_state.ball_positions.iter() {
        println!(
            "Angle to top-right pocket from {:?}: {}",
            ball.ty,
            ball.position.angle_to_pocket(Pocket::TopRight)
        );
    }
}
