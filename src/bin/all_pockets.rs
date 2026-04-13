use billiards::*;

fn main() {
    let table_spec = TableSpec::brunswick_gc4_9ft();

    let mut game_state = GameState::with_balls(
        table_spec.clone(),
        [
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
                position: Position::new(2u8, 4u8),
                spec: BallSpec::default(),
            },
            Ball {
                ty: BallType::Eight,
                position: Position::new(0u8, 2u8),
                spec: BallSpec::default(),
            },
            Ball {
                ty: BallType::Five,
                position: Position::new(3u8, 0u8),
                spec: BallSpec::default(),
            },
        ],
    );
    game_state.ty = GameType::NineBall;
    game_state.cueball_modifier = CueballModifier::AsItLays;

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
