use billiards::*;

mod assets;

fn main() {
    let mut game_state = GameState {
        table_spec: TableSpec::brunswick_gc4_9ft(),
        ball_positions: rack_9_ball(),
        ..Default::default()
    };

    game_state.freeze_to_rail(
        Rail::Left,
        Diamond::six(),
        Ball {
            ty: BallType::Eight,
            ..Default::default()
        },
    );

    game_state.resolve_positions();

    let img = game_state.draw_2d_diagram();
    write_png_to_file(&img, None);
}
