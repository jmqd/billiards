use billiards::{GameState, TableSpec};

fn main() {
    let state = GameState::new(TableSpec::default());

    let _ = state.ball_positions.len();
    let _ = state.lines_to_draw.len();
}
