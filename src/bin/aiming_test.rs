use billiards::*;
use image::Rgba;

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

    let six = game_state.select_ball(BallType::Six).unwrap();
    let potting_angle = six.position.angle_from_pocket(Pocket::TopRight);
    let mut ghost_ball_pos = six.position.translate_ghost_ball(potting_angle);
    ghost_ball_pos.resolve_shifts(&game_state.table_spec);
    let cue_ball_pos = game_state
        .select_ball(BallType::Cue)
        .unwrap()
        .position
        .clone();

    println!("ghost ball: {:?}", ghost_ball_pos);
    println!("six ball: {:?}", six.position);

    game_state.resolve_positions();
    game_state.add_dotted_line(&cue_ball_pos, &ghost_ball_pos, Rgba([0, 0, 0, 255]));

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
