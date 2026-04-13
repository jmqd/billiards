use billiards::*;
use image::Rgba;

fn main() {
    let table_spec = TableSpec::brunswick_gc4_9ft();

    let mut game_state = GameState::with_balls(
        table_spec.clone(),
        [
            Ball {
                ty: BallType::Cue,
                position: Position::new(2u8, 4u8),
                spec: BallSpec::default(),
            },
            Ball {
                ty: BallType::Eight,
                position: Position::new(2u8, 6u8),
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

    let six = game_state.select_ball(BallType::Six).unwrap().clone();
    let cue_ball_pos = game_state
        .select_ball(BallType::Cue)
        .unwrap()
        .position
        .clone();
    let potting_angle = six.center_to_center_potting_angle_to_pocket(
        Pocket::TopRight,
        &cue_ball_pos,
        &game_state.table_spec,
    );
    let ghost_ball_pos =
        six.center_to_center_ghost_ball_to_pocket(Pocket::TopRight, &game_state.table_spec);

    println!("potting angle: {}", potting_angle);
    println!("ghost ball: {:?}", ghost_ball_pos);
    println!("six ball: {:?}", six.position);

    game_state.add_dotted_potting_line_to_pocket(
        &six,
        Pocket::TopRight,
        &cue_ball_pos,
        Rgba([0, 0, 0, 255]),
    );

    let img = game_state.draw_2d_diagram();
    write_png_to_file(&img, None);

    println!("{:#?}", table_spec);
    println!("{:#?}", game_state);

    for ball in game_state.balls().iter() {
        println!(
            "Angle to top-right pocket from {:?}: {}",
            ball.ty,
            ball.position.angle_to_pocket(Pocket::TopRight)
        );
    }
}
