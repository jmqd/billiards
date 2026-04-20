use billiards::{
    trace_ball_path_with_rails_on_table,
    visualization::{
        AimOverlayStyle, BallPathRenderOptions, BallPathStyle, BallPathWidthMode, EventMarkerStyle,
        GhostBallStyle, LabelOverlayStyle,
    },
    Angle, AngularVelocity3, Ball, BallPathStop, BallSetPhysicsSpec, BallSpec, BallState, BallType,
    DiagramBackground, DiagramRenderOptions, Diamond, GameState, Inches, Inches2, InchesPerSecond,
    InchesPerSecondSq, MotionPhaseConfig, MotionTransitionConfig, OnTableBallState,
    OnTableMotionConfig, OverlayLayer, Pocket, Position, RadiansPerSecondSq, Rail,
    RailAngleReference, RailModel, RailTangentDirection, RollingResistanceModel, Seconds,
    SlidingFrictionModel, SpinDecayModel, TableSpec, Velocity2, TYPICAL_BALL_RADIUS,
};
use image::{load_from_memory, RgbaImage};

fn render(state: &GameState) -> RgbaImage {
    load_from_memory(&state.draw_2d_diagram())
        .expect("png decode")
        .into_rgba8()
}

fn render_with_options(state: &GameState, options: &DiagramRenderOptions) -> RgbaImage {
    load_from_memory(&state.draw_2d_diagram_with_options(options))
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

fn visible_pixel_count_in_row(image: &RgbaImage, y: u32) -> usize {
    (0..image.width())
        .filter(|&x| image.get_pixel(x, y)[3] > 0)
        .count()
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

fn clip_to_ball_edges(
    table_spec: &TableSpec,
    from: &Position,
    to: &Position,
) -> (Position, Position) {
    let mut from = from.clone();
    from.resolve_shifts(table_spec);
    let mut to = to.clone();
    to.resolve_shifts(table_spec);

    let from_x = table_spec.diamond_to_inches(from.x.clone()).as_f64();
    let from_y = table_spec.diamond_to_inches(from.y.clone()).as_f64();
    let to_x = table_spec.diamond_to_inches(to.x.clone()).as_f64();
    let to_y = table_spec.diamond_to_inches(to.y.clone()).as_f64();
    let distance = (to_x - from_x).hypot(to_y - from_y);
    let radius = TYPICAL_BALL_RADIUS.clone();
    if distance <= 2.0 * radius.as_f64() + 1e-9 {
        return (from, to);
    }

    let angle = from.angle_to(&to);
    let mut clipped_from = from.translate_inches(radius.clone(), angle);
    clipped_from.resolve_shifts(table_spec);
    let mut clipped_to = to.translate_inches(radius, angle.flipped());
    clipped_to.resolve_shifts(table_spec);
    (clipped_from, clipped_to)
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
fn drawing_with_scale_factor_two_doubles_the_output_dimensions() {
    let state = cue_ball_at("2", "4");
    let baseline = render(&state);
    let scaled = render_with_options(
        &state,
        &DiagramRenderOptions {
            scale_factor: 2,
            ..DiagramRenderOptions::default()
        },
    );

    assert_eq!(scaled.width(), baseline.width() * 2);
    assert_eq!(scaled.height(), baseline.height() * 2);
}

#[test]
fn drawing_with_a_transparent_background_leaves_an_empty_table_fully_transparent() {
    let rendered = render_with_options(
        &GameState::default(),
        &DiagramRenderOptions {
            background: DiagramBackground::Transparent,
            ..DiagramRenderOptions::default()
        },
    );

    assert!(rendered.pixels().all(|pixel| pixel[3] == 0));
}

#[test]
fn drawing_with_a_transparent_background_still_renders_visible_balls() {
    let rendered = render_with_options(
        &cue_ball_at("2", "4"),
        &DiagramRenderOptions {
            background: DiagramBackground::Transparent,
            ..DiagramRenderOptions::default()
        },
    );

    assert!(rendered.pixels().any(|pixel| pixel[3] > 0));
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
    let (clipped_start, clipped_end) =
        clip_to_ball_edges(&table_spec, &resolved_shooting_position, &ghost_ball);
    manual.add_dotted_line(&clipped_start, &clipped_end, color);

    let mut helper = GameState::new(table_spec.clone());
    helper.add_dotted_aim_line_to_pocket(&object_ball, Pocket::TopRight, &shooting_position, color);

    let mut styled = GameState::new(table_spec.clone());
    styled.add_dotted_aim_line_to_pocket_styled(
        &object_ball,
        Pocket::TopRight,
        &shooting_position,
        &AimOverlayStyle::new(color),
    );

    let mut unclipped = GameState::new(table_spec);
    unclipped.add_dotted_aim_line_to_pocket_styled(
        &object_ball,
        Pocket::TopRight,
        &shooting_position,
        &AimOverlayStyle::new(color).without_endpoint_clipping(),
    );

    assert_eq!(render(&helper), render(&manual));
    assert_eq!(render(&styled), render(&manual));
    assert_ne!(render(&unclipped), render(&manual));
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
    let (first_start, first_end) = clip_to_ball_edges(&table_spec, &points[0], &points[1]);
    manual.add_dotted_line(&first_start, &first_end, color);
    let (second_start, second_end) = clip_to_ball_edges(&table_spec, &points[1], &points[2]);
    manual.add_dotted_line(&second_start, &second_end, color);

    let mut helper = GameState::new(table_spec.clone());
    helper.add_dotted_ball_path(&path, color);

    assert_eq!(render(&helper), render(&manual));

    let start = path
        .initial_state
        .as_ball_state()
        .projected_position(&table_spec);

    let mut manual_with_ghost = GameState::new(table_spec.clone());
    manual_with_ghost.add_ghost_ball(&start, ghost_fill_color(), ghost_outline_color());
    manual_with_ghost.add_dotted_line(&first_start, &first_end, color);
    manual_with_ghost.add_dotted_line(&second_start, &second_end, color);

    let mut helper_with_ghost = GameState::new(table_spec.clone());
    helper_with_ghost.add_dotted_ball_path_with_start_ghost(
        &path,
        color,
        ghost_fill_color(),
        ghost_outline_color(),
    );

    assert_eq!(render(&helper_with_ghost), render(&manual_with_ghost));

    let marker_style = EventMarkerStyle::enabled(image::Rgba([255, 0, 0, 192]));
    let label_style = LabelOverlayStyle::enabled(image::Rgba([0, 0, 0, 255]));
    let mut manual_with_markers = GameState::new(table_spec.clone());
    manual_with_markers.add_ghost_ball(&start, ghost_fill_color(), ghost_outline_color());
    manual_with_markers.add_dotted_line(&first_start, &first_end, color);
    manual_with_markers.add_dotted_line(&second_start, &second_end, color);
    manual_with_markers.add_event_marker_styled(&points[1], marker_style.clone());
    manual_with_markers.add_event_marker_styled(&points[2], marker_style.clone());
    manual_with_markers.add_text_label_styled(&points[1], "1", label_style.clone());
    manual_with_markers.add_text_label_styled(&points[2], "2", label_style.clone());

    let mut helper_with_markers = GameState::new(table_spec.clone());
    helper_with_markers.add_dotted_ball_path_styled(
        &path,
        &BallPathStyle::new(color)
            .with_start_ghost(GhostBallStyle {
                fill_color: ghost_fill_color(),
                outline_color: ghost_outline_color(),
                ..Default::default()
            })
            .with_event_markers(marker_style.clone())
            .with_labels(label_style.clone()),
    );

    assert_eq!(render(&helper_with_markers), render(&manual_with_markers));

    let mut solid = GameState::new(table_spec.clone());
    solid.add_dotted_ball_path_styled(
        &path,
        &BallPathStyle::new(color).with_start_ghost(GhostBallStyle {
            fill_color: ghost_fill_color(),
            outline_color: ghost_outline_color(),
            ..Default::default()
        }),
    );
    let solid_image = render(&solid);

    let mut faded = GameState::new(table_spec.clone());
    faded.add_dotted_ball_path_styled(
        &path,
        &BallPathStyle::new(color)
            .with_start_ghost(GhostBallStyle {
                fill_color: ghost_fill_color(),
                outline_color: ghost_outline_color(),
                ..Default::default()
            })
            .with_color_mode(billiards::visualization::PathColorMode::FadeByTime),
    );
    let faded_image = render(&faded);
    assert_ne!(faded_image, solid_image);

    let mut phase_colored = GameState::new(table_spec.clone());
    phase_colored.add_dotted_ball_path_styled(
        &path,
        &BallPathStyle::new(color)
            .with_start_ghost(GhostBallStyle {
                fill_color: ghost_fill_color(),
                outline_color: ghost_outline_color(),
                ..Default::default()
            })
            .with_color_mode(billiards::visualization::PathColorMode::MotionPhase),
    );
    let phase_colored_image = render(&phase_colored);
    assert_ne!(phase_colored_image, solid_image);

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

#[test]
fn rendered_ball_paths_can_use_one_shared_renderer_for_fixed_and_speed_scaled_widths() {
    let table_spec = TableSpec::default();
    let ball_set = BallSetPhysicsSpec::default();
    let motion = motion_config();
    let start = on_table(BallState::on_table(
        inches2(
            table_spec.diamond_to_inches(Diamond::two()).as_f64(),
            table_spec.diamond_to_inches(Diamond::one()).as_f64(),
        ),
        Velocity2::new("0", "24"),
        AngularVelocity3::zero(),
    ));
    let path = trace_ball_path_with_rails_on_table(
        &start,
        BallPathStop::UntilRest,
        &ball_set,
        &table_spec,
        &motion,
        RailModel::SpinAware,
    );
    let style = BallPathStyle::new(image::Rgba([255, 255, 255, 255])).without_endpoint_clipping();
    let transparent = DiagramRenderOptions {
        scale_factor: 1,
        background: DiagramBackground::Transparent,
    };
    let empty = render_with_options(&GameState::new(table_spec.clone()), &transparent);

    let mut uniform = GameState::new(table_spec.clone());
    uniform.add_rendered_ball_path_styled(
        &path,
        &ball_set,
        &motion,
        &BallPathRenderOptions {
            max_time_step: Seconds::new(0.02),
            width_px: 8.0,
            width_mode: BallPathWidthMode::Fixed,
        },
        &style,
    );
    let uniform_image = render_with_options(&uniform, &transparent);

    let mut tapered = GameState::new(table_spec);
    tapered.add_rendered_ball_path_styled(
        &path,
        &ball_set,
        &motion,
        &BallPathRenderOptions {
            max_time_step: Seconds::new(0.02),
            width_px: 8.0,
            width_mode: BallPathWidthMode::ScaleBySpeed,
        },
        &style,
    );
    let tapered_image = render_with_options(&tapered, &transparent);

    assert_ne!(tapered_image, uniform_image);

    let (_, min_y, _, max_y) = diff_bbox(&empty, &tapered_image).expect("speed-scaled path bbox");
    let height = max_y - min_y;
    assert!(
        height > 40,
        "expected a clearly visible vertical trace, got height {height}"
    );

    let fast_row = max_y - height / 4;
    let slow_row = min_y + height / 4;
    let fast_width = visible_pixel_count_in_row(&tapered_image, fast_row);
    let slow_width = visible_pixel_count_in_row(&tapered_image, slow_row);

    assert!(
        fast_width > slow_width,
        "expected the faster early cue-ball path to render thicker than the slower late path; got fast row width {fast_width} and slow row width {slow_width}"
    );
}
