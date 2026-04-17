use billiards::{
    trace_ball_path_with_rails_on_table, visualization::AimOverlayStyle, Angle,
    AngularVelocity3, Ball, BallPathStop, BallSetPhysicsSpec, BallSpec, BallState, BallType,
    Diamond, GameState, Inches, Inches2, InchesPerSecond, InchesPerSecondSq,
    MotionPhaseConfig, MotionTransitionConfig, OnTableBallState, OnTableMotionConfig,
    OverlayLayer, Pocket, Position, RadiansPerSecondSq, Rail, RailAngleReference, RailModel,
    RailTangentDirection, RollingResistanceModel, SlidingFrictionModel, SpinDecayModel,
    TableSpec, Velocity2, TYPICAL_BALL_RADIUS,
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

fn ghost_fill_color() -> image::Rgba<u8> {
    image::Rgba([255, 255, 255, 64])
}

fn ghost_outline_color() -> image::Rgba<u8> {
    image::Rgba([0, 0, 0, 96])
}

fn motion_config() -> OnTableMotionConfig {
    MotionTransitionConfig {
        phase: MotionPhaseConfig::default(),
        sliding_friction: SlidingFrictionModel::ConstantAcceleration {
            acceleration_magnitude: InchesPerSecondSq::new("5"),
        },
        spin_decay: SpinDecayModel::ConstantAngularDeceleration {
            angular_deceleration: RadiansPerSecondSq::new(2.0),
        },
        rolling_resistance: RollingResistanceModel::ConstantDeceleration {
            linear_deceleration: InchesPerSecondSq::new("5"),
        },
    }
}

fn on_table(state: BallState) -> OnTableBallState {
    OnTableBallState::try_from(state).expect("test states should validate as on-table")
}

fn inches2(x: f64, y: f64) -> Inches2 {
    Inches2::new(Inches::from_f64(x), Inches::from_f64(y))
}

fn thirty_degree_top_rail_bank_state(table: &TableSpec) -> OnTableBallState {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let heading = Rail::Top.bank_heading_toward(
        30.0,
        RailAngleReference::FromNormal,
        RailTangentDirection::Positive,
    );
    let speed = InchesPerSecond::new("10");
    let velocity = Velocity2::from_polar(speed, heading);
    let impact_time = 0.5;
    let along_path_distance_to_impact = 10.0 * impact_time - 0.5 * 5.0 * impact_time * impact_time;
    let radians = heading.as_degrees().to_radians();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;

    on_table(BallState::on_table(
        inches2(
            10.0,
            top_plane - along_path_distance_to_impact * radians.cos(),
        ),
        velocity,
        AngularVelocity3::new(
            -10.0 * radians.cos() / radius,
            10.0 * radians.sin() / radius,
            0.0,
        ),
    ))
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
fn adding_a_dotted_aim_line_to_a_pocket_matches_a_manually_computed_ghost_ball_overlay() {
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
    let ghost_ball = object_ball.ghost_ball_to_pocket(Pocket::TopRight, &table_spec);
    manual.add_ghost_ball(&ghost_ball, ghost_fill_color(), ghost_outline_color());
    manual.add_dotted_line(&resolved_shooting_position, &ghost_ball, color);

    let mut helper = GameState::new(table_spec.clone());
    helper.add_dotted_aim_line_to_pocket(&object_ball, Pocket::TopRight, &shooting_position, color);

    let mut styled = GameState::new(table_spec);
    styled.add_dotted_aim_line_to_pocket_styled(
        &object_ball,
        Pocket::TopRight,
        &shooting_position,
        &AimOverlayStyle::new(color),
    );

    assert_eq!(render(&helper), render(&manual));
    assert_eq!(render(&styled), render(&manual));
}

#[test]
fn adding_a_ghost_ball_renders_a_ball_sized_overlay_centered_on_the_requested_position() {
    let empty = render(&GameState::default());
    let mut ghosted = GameState::new(TableSpec::default());
    ghosted.add_ghost_ball(
        &Position::new(2u8, 4u8),
        ghost_fill_color(),
        ghost_outline_color(),
    );

    let (min_x, min_y, max_x, max_y) =
        diff_bbox(&empty, &render(&ghosted)).expect("ghost ball diff bbox");

    assert_eq!(max_x - min_x + 1, 39);
    assert_eq!(max_y - min_y + 1, 39);
    assert_eq!((min_x + max_x) / 2, 539);
    assert_eq!((min_y + max_y) / 2, 969);
}

#[test]
fn overlays_can_be_drawn_above_balls_when_requested() {
    let baseline = cue_ball_at("2", "4");

    let mut below = cue_ball_at("2", "4");
    below.add_ghost_ball(
        &Position::new(2u8, 4u8),
        ghost_fill_color(),
        ghost_outline_color(),
    );

    let mut above = cue_ball_at("2", "4");
    above.add_ghost_ball_on_layer(
        &Position::new(2u8, 4u8),
        ghost_fill_color(),
        ghost_outline_color(),
        OverlayLayer::AboveBalls,
    );

    let baseline_image = render(&baseline);
    let below_image = render(&below);
    let above_image = render(&above);

    assert!(diff_bbox(&baseline_image, &above_image).is_some());
    assert!(diff_bbox(&below_image, &above_image).is_some());
}

#[test]
fn adding_a_dotted_ball_path_matches_manually_drawing_its_projected_segments() {
    let table_spec = TableSpec::default();
    let color = image::Rgba([0, 0, 0, 255]);
    let path = trace_ball_path_with_rails_on_table(
        &thirty_degree_top_rail_bank_state(&table_spec),
        BallPathStop::Duration(billiards::Seconds::new(1.0)),
        &BallSetPhysicsSpec::default(),
        &table_spec,
        &motion_config(),
        RailModel::Mirror,
    );
    let points = path.projected_points(&table_spec);
    assert_eq!(
        points.len(),
        3,
        "the traced bank path should yield a one-bank polyline"
    );

    let mut manual = GameState::new(table_spec.clone());
    manual.add_dotted_line(&points[0], &points[1], color);
    manual.add_dotted_line(&points[1], &points[2], color);

    let mut helper = GameState::new(table_spec.clone());
    helper.add_dotted_ball_path(&path, color);

    assert_eq!(render(&helper), render(&manual));

    let start = path.initial_state.as_ball_state().projected_position(&table_spec);

    let mut manual_with_ghost = GameState::new(table_spec.clone());
    manual_with_ghost.add_ghost_ball(&start, ghost_fill_color(), ghost_outline_color());
    manual_with_ghost.add_dotted_line(&points[0], &points[1], color);
    manual_with_ghost.add_dotted_line(&points[1], &points[2], color);

    let mut helper_with_ghost = GameState::new(table_spec.clone());
    helper_with_ghost.add_dotted_ball_path_with_start_ghost(
        &path,
        color,
        ghost_fill_color(),
        ghost_outline_color(),
    );

    assert_eq!(render(&helper_with_ghost), render(&manual_with_ghost));

    let sampled = path.sampled_points(
        billiards::Seconds::new(0.02),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        &table_spec,
    );
    let empty = render(&GameState::new(table_spec.clone()));

    let mut dotted = GameState::new(table_spec.clone());
    dotted.add_dotted_polyline(&sampled, color);

    let mut smooth = GameState::new(table_spec);
    smooth.add_smooth_polyline(&sampled, color);

    let dotted_image = render(&dotted);
    let smooth_image = render(&smooth);

    assert_ne!(smooth_image, dotted_image);
    assert!(diff_bbox(&empty, &dotted_image).is_some());
    assert!(diff_bbox(&empty, &smooth_image).is_some());
}
