use billiards::*;

fn main() {
    let table_spec = TableSpec::brunswick_gc4_9ft();

    let mut game_state = GameState {
        table_spec: table_spec.clone(),
        ball_positions: vec![
            Ball {
                ty: BallType::Cue,
                position: TOP_RIGHT_DIAMOND.clone(),
                spec: BallSpec::default(),
            },
            Ball {
                ty: BallType::Cue,
                position: CENTER_RIGHT_DIAMOND.clone(),
                spec: BallSpec::default(),
            },
            Ball {
                ty: BallType::Cue,
                position: BOTTOM_RIGHT_DIAMOND.clone(),
                spec: BallSpec::default(),
            },
            Ball {
                ty: BallType::Cue,
                position: BOTTOM_LEFT_DIAMOND.clone(),
                spec: BallSpec::default(),
            },
            Ball {
                ty: BallType::Cue,
                position: CENTER_LEFT_DIAMOND.clone(),
                spec: BallSpec::default(),
            },
            Ball {
                ty: BallType::Cue,
                position: TOP_LEFT_DIAMOND.clone(),
                spec: BallSpec::default(),
            },
            Ball {
                ty: BallType::Nine,
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
                    x: Diamond::from(0),
                    y: Diamond::from(2),
                    ..Default::default()
                },
                spec: BallSpec::default(),
            },
            Ball {
                ty: BallType::Five,
                position: Position {
                    x: Diamond::from(3),
                    y: Diamond::from(0),
                    ..Default::default()
                },
                spec: BallSpec::default(),
            },
        ],
        ty: GameType::NineBall,
        cueball_modifier: CueballModifier::AsItLays,
        ..Default::default()
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
}
