use billiards::{
    Angle, Ball, BallSpec, BallType, GameState, Pocket, Position, TableSpec, TYPICAL_BALL_RADIUS,
};
use image::{load_from_memory, RgbaImage};

fn render(state: &GameState) -> RgbaImage {
    load_from_memory(&state.draw_2d_diagram())
        .expect("png decode")
        .into_rgba8()
}

fn diff_bbox(a: &RgbaImage, b: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    assert_eq!(a.dimensions(), b.dimensions());

    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut changed = false;

    for y in 0..a.height() {
        for x in 0..a.width() {
            if a.get_pixel(x, y) != b.get_pixel(x, y) {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                changed = true;
            }
        }
    }

    changed.then_some((min_x, min_y, max_x, max_y))
}

fn cue_ball_at(x: &str, y: &str) -> GameState {
    GameState::with_balls(
        TableSpec::default(),
        [Ball {
            ty: BallType::Cue,
            position: Position::new(x, y),
            spec: BallSpec::default(),
        }],
    )
}

#[test]
fn rendered_ball_uses_the_table_geometry_diameter() {
    let empty = render(&GameState::default());
    let with_ball = render(&cue_ball_at("2", "4"));

    let (min_x, min_y, max_x, max_y) = diff_bbox(&empty, &with_ball).expect("ball diff bbox");

    assert_eq!(max_x - min_x + 1, 39);
    assert_eq!(max_y - min_y + 1, 39);
}

#[test]
fn rendered_ball_is_centered_on_the_requested_table_position() {
    let empty = render(&GameState::default());
    let with_ball = render(&cue_ball_at("2", "4"));

    let (min_x, min_y, max_x, max_y) = diff_bbox(&empty, &with_ball).expect("ball diff bbox");

    assert_eq!((min_x + max_x) / 2, 539);
    assert_eq!((min_y + max_y) / 2, 969);
}

#[test]
fn out_of_range_ball_positions_still_render_a_full_sprite_inside_the_image() {
    let empty = render(&GameState::default());
    let with_ball = render(&cue_ball_at("5", "-1"));

    let (min_x, min_y, max_x, max_y) = diff_bbox(&empty, &with_ball).expect("ball diff bbox");

    assert_eq!(max_x - min_x + 1, 39);
    assert_eq!(max_y - min_y + 1, 39);
    assert_eq!(max_x, with_ball.width() - 1);
    assert_eq!(max_y, with_ball.height() - 1);
}

#[test]
fn drawing_resolves_pending_inches_shifts_before_rendering() {
    let table_spec = TableSpec::default();
    let shifted = Position::new(2u8, 4u8)
        .translate_inches(TYPICAL_BALL_RADIUS.clone(), Angle::from_north(0.0, 1.0));

    let unresolved = GameState::with_balls(
        table_spec.clone(),
        [Ball {
            ty: BallType::Cue,
            position: shifted.clone(),
            spec: BallSpec::default(),
        }],
    );

    let mut resolved = GameState::with_balls(
        table_spec,
        [Ball {
            ty: BallType::Cue,
            position: shifted,
            spec: BallSpec::default(),
        }],
    );
    resolved.resolve_positions();

    assert_eq!(render(&unresolved), render(&resolved));
}

#[test]
fn adding_a_dotted_potting_line_to_a_pocket_matches_a_manually_computed_ghost_ball_line() {
    let table_spec = TableSpec::default();
    let object_ball = Ball {
        ty: BallType::Eight,
        position: Position::new(2u8, 6u8),
        spec: BallSpec::default(),
    };
    let shooting_position = Position::new(2u8, 4u8)
        .translate_inches(TYPICAL_BALL_RADIUS.clone(), Angle::from_north(1.0, 0.0));
    let color = image::Rgba([0, 0, 0, 255]);

    let mut manual = GameState::new(table_spec.clone());
    let mut resolved_shooting_position = shooting_position.clone();
    resolved_shooting_position.resolve_shifts(&table_spec);
    let ghost_ball =
        object_ball.center_to_center_ghost_ball_to_pocket(Pocket::TopRight, &table_spec);
    manual.add_dotted_line(&resolved_shooting_position, &ghost_ball, color);

    let mut helper = GameState::new(table_spec);
    helper.add_dotted_potting_line_to_pocket(
        &object_ball,
        Pocket::TopRight,
        &shooting_position,
        color,
    );

    assert_eq!(render(&helper), render(&manual));
}
