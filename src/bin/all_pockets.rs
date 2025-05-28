use billiards::*;

fn main() {
    let table_spec = TableSpec::new_9ft_brunswick_gc4();

    let game_state = GameState {
        table_spec: table_spec.clone(),
        ball_positions: vec![
            Ball {
                ty: BallType::Cue,
                position: TOP_RIGHT_DIAMOND.clone(),
                spec: BallSpec::default(),
            },
            Ball {
                ty: BallType::Cue,
                position: SIDE_RIGHT_DIAMOND.clone(),
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
                position: SIDE_LEFT_DIAMOND.clone(),
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
                },
                spec: BallSpec::default(),
            },
        ],
        ty: GameType::NineBall,
        cueball_modifier: CueballModifier::AsItLays,
    };

    let img = game_state.draw_2d_diagram();

    write_png_to_file(&img, None);

    println!("{:#?}", table_spec);
    println!("{:#?}", game_state);
}
