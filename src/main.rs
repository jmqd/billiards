use billiards::*;

mod assets;

fn main() {
    let table_spec = TableSpec::new_9ft_brunswick_gc4();
    let game_state = GameState {
        table_spec: table_spec.clone(),
        ball_positions: vec![
            Ball {
                ty: BallType::Cue,
                position: CENTER_SPOT.clone(),
                spec: BallSpec::default(),
            },
            Ball {
                ty: BallType::Nine,
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

    let img = game_state.draw_2d_diagram();
    write_png_to_file(&img, None);
}
