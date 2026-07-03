use billiards::diagram::{DiagramLayerId, DiagramOutputFormat};
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

fn render_svg_with_options(state: &GameState, options: &DiagramRenderOptions) -> String {
    String::from_utf8(state.render_2d_diagram_with_options(DiagramOutputFormat::Svg, options))
        .expect("svg should be utf-8")
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

fn svg_attr_f32(element: &str, attr: &str) -> f32 {
    let prefix = format!("{attr}=\"");
    let start = element
        .find(&prefix)
        .unwrap_or_else(|| panic!("missing SVG attribute {attr} in {element}"))
        + prefix.len();
    let end = element[start..]
        .find('"')
        .unwrap_or_else(|| panic!("unterminated SVG attribute {attr} in {element}"))
        + start;
    element[start..end]
        .parse()
        .unwrap_or_else(|error| panic!("invalid SVG attribute {attr} in {element}: {error}"))
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
fn diagram_scene_exposes_backend_neutral_balls_and_overlay_layers() {
    let mut state = cue_ball_at("2", "4");
    state.add_event_marker_styled(
        &Position::new(2u8, 4u8),
        EventMarkerStyle::enabled(image::Rgba([255, 0, 0, 255])),
    );

    let scene = state.to_diagram_scene(&DiagramRenderOptions {
        background: DiagramBackground::Transparent,
        ..DiagramRenderOptions::default()
    });

    assert_eq!(scene.background, DiagramBackground::Transparent);
    assert_eq!(scene.balls.len(), 1);
    assert_eq!(
        scene
            .elements_for_layer(DiagramLayerId::OverlaysAboveBalls)
            .count(),
        1
    );
    assert_eq!(scene.elements_for_layer(DiagramLayerId::Balls).count(), 0);
}

#[test]
fn svg_backend_emits_layered_scalable_markup_for_a_ball_layout() {
    let svg = render_svg_with_options(&cue_ball_at("2", "4"), &DiagramRenderOptions::default());

    assert!(svg.starts_with("<svg "));
    assert!(svg.contains("viewBox=\"0 0 1089 1938\""));
    assert!(svg.contains("data-layer=\"table\""));
    assert!(svg.contains("data-layer=\"balls\""));
    assert!(svg.contains("class=\"ball ball-cue\""));
}

#[test]
fn svg_table_uses_cut_pockets_eighteen_sights_and_diamond_style_materials() {
    let svg = render_svg_with_options(&cue_ball_at("2", "4"), &DiagramRenderOptions::default());

    assert_eq!(svg.matches("class=\"table-diamond\"").count(), 18);
    assert_eq!(svg.matches("data-pocket=\"corner\"").count(), 4);
    assert_eq!(svg.matches("data-pocket=\"side\"").count(), 2);
    assert!(svg.contains("<polygon class=\"table-diamond\""));
    assert!(!svg.contains("<circle class=\"table-pocket\""));
    assert_eq!(svg.matches("class=\"table-pocket-well\"").count(), 6);
    assert_eq!(svg.matches("class=\"table-pocket-leather\"").count(), 6);
    assert_eq!(
        svg.matches("class=\"table-pocket-leather-highlight\"")
            .count(),
        6
    );
    assert_eq!(
        svg.matches("class=\"table-pocket-mouth-shadow\"").count(),
        0
    );
    assert_eq!(svg.matches("class=\"table-pocket-shelf\"").count(), 6);
    assert_eq!(
        svg.matches("class=\"table-pocket-shelf-texture\"").count(),
        6
    );
    assert_eq!(
        svg.matches("class=\"table-pocket-shelf-shadow\"").count(),
        0
    );
    assert_eq!(svg.matches("class=\"table-pocket-facing\"").count(), 12);
    assert_eq!(svg.matches("data-pocket=\"corner-liner\"").count(), 4);
    assert_eq!(svg.matches("data-pocket=\"side-liner\"").count(), 2);
    let first_cushion = svg
        .find("<path class=\"table-cushion\"")
        .expect("table cushion should render");
    let first_shelf = svg
        .find("<path class=\"table-pocket-shelf\"")
        .expect("pocket shelf should render");
    let first_facing = svg
        .find("<line class=\"table-pocket-facing\"")
        .expect("pocket facing should render");
    assert!(first_cushion < first_shelf);
    assert!(first_shelf < first_facing);
    assert!(!svg.contains("stroke-width:0"));
    assert!(svg.contains("id=\"tournament-blue-cloth\" gradientUnits=\"userSpaceOnUse\""));
    assert!(svg.contains("id=\"rosewood-grain\""));
    assert!(svg.contains("id=\"pocket-well\""));
    assert!(svg.contains("id=\"pocket-leather\""));
    assert!(
        svg.contains(".table-pocket-shelf{fill:url(#tournament-blue-cloth);stroke:none;opacity:1}")
    );
    assert!(
        svg.contains(".table-pocket-shelf-texture{fill:url(#cloth-weave);stroke:none;opacity:.20")
    );
    assert!(svg.contains("class=\"table-cloth-texture\""));
    assert!(svg.contains("class=\"table-cushion-nose\""));
}

#[test]
fn svg_three_cushion_table_is_pocketless_with_carom_sights_and_balls() {
    let table = TableSpec::three_cushion_carom_10ft();
    let ball_spec = table.default_ball_spec();
    let state = GameState::with_balls(
        table,
        [
            Ball {
                ty: BallType::Cue,
                position: Position::new("1", "1"),
                spec: ball_spec.clone(),
            },
            Ball {
                ty: BallType::YellowCue,
                position: Position::new("2", "4"),
                spec: ball_spec.clone(),
            },
            Ball {
                ty: BallType::Red,
                position: Position::new("3", "7"),
                spec: ball_spec,
            },
        ],
    );
    let svg = render_svg_with_options(&state, &DiagramRenderOptions::default());

    assert!(svg.contains("class=\"carom-table\""));
    assert_eq!(svg.matches("class=\"table-diamond\"").count(), 20);
    assert_eq!(svg.matches("data-pocket=").count(), 0);
    assert!(svg.contains("id=\"heated-carom-cloth\""));
    assert!(svg.contains("id=\"carom-wood-rail\""));
    assert!(svg.contains("class=\"ball ball-yellow\""));
    assert!(svg.contains("class=\"ball ball-red\""));
}

#[test]
fn svg_table_uses_shaped_leather_pocket_wells_and_pronounced_facing_noses() {
    let svg = render_svg_with_options(&cue_ball_at("2", "4"), &DiagramRenderOptions::default());

    assert!(svg.contains("<path class=\"table-pocket-well\" data-pocket=\"corner\""));
    assert!(svg.contains("<path class=\"table-pocket-well\" data-pocket=\"side\""));
    assert!(svg.contains("<path class=\"table-pocket-leather\" data-pocket=\"corner-liner\""));
    assert!(svg.contains("<path class=\"table-pocket-leather\" data-pocket=\"side-liner\""));
    assert!(svg.contains("<path class=\"table-pocket-leather-highlight\""));
    assert!(!svg.contains("<path class=\"table-pocket-mouth-shadow\""));
    assert!(svg.contains("<path class=\"table-pocket-shelf\" data-pocket=\"corner-shelf\""));
    assert!(svg.contains("<path class=\"table-pocket-shelf\" data-pocket=\"side-shelf\""));
    assert!(svg.contains(
        "<path class=\"table-pocket-shelf\" data-pocket=\"corner-shelf\" d=\"M 164.603 110.000 Q "
    ));
    assert!(svg.contains("110.000 164.666 Q 127.473 127.493 164.603 110.000 Z"));
    assert!(svg.contains(
        "<path class=\"table-pocket-shelf\" data-pocket=\"side-shelf\" d=\"M 110.000 926.050 Q 104.509 969.000 110.000 1011.950 Q 116.103 969.000 110.000 926.050 Z"
    ));
    assert!(!svg.contains("<path class=\"table-pocket-shelf-shadow\""));
    assert!(!svg.contains("<circle class=\"table-pocket\""));
}

#[test]
fn svg_trace_event_markers_carry_event_labels_for_tooltips_without_visible_text() {
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
        BallPathStop::RailImpacts(1),
        &ball_set,
        &table_spec,
        &motion,
        RailModel::SpinAware,
    );
    let mut state = GameState::new(table_spec);
    state.add_dotted_ball_path_styled(
        &path,
        &BallPathStyle::new(image::Rgba([255, 255, 255, 255]))
            .with_event_markers(EventMarkerStyle::enabled(image::Rgba([0, 0, 0, 255]))),
    );

    let svg = render_svg_with_options(
        &state,
        &DiagramRenderOptions {
            background: DiagramBackground::Transparent,
            ..DiagramRenderOptions::default()
        },
    );

    assert!(svg.contains("class=\"overlay event-marker\""));
    assert!(svg.contains("data-event-label=\"(1)\""));
    assert!(svg.contains("<title>(1)</title>"));
    assert!(!svg.contains(">(1)</text>"));
    assert!(!svg.contains("<text class=\"overlay overlay-label\""));
}

#[test]
fn svg_transparent_background_omits_table_art_but_keeps_balls() {
    let svg = render_svg_with_options(
        &cue_ball_at("2", "4"),
        &DiagramRenderOptions {
            background: DiagramBackground::Transparent,
            ..DiagramRenderOptions::default()
        },
    );

    assert!(!svg.contains("class=\"table-cloth\""));
    assert!(svg.contains("class=\"ball ball-cue\""));
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
fn adding_a_speed_aware_dotted_aim_line_to_a_pocket_uses_the_effective_target_center() {
    let table_spec = TableSpec::default();
    let object_ball = Ball {
        ty: BallType::Eight,
        position: Position::new(2u8, 5u8),
        spec: BallSpec::default(),
    };
    let shooting_position = Position::new(1u8, 5u8)
        .translate_inches(TYPICAL_BALL_RADIUS.clone(), Angle::from_north(1.0, 0.0));
    let speed = InchesPerSecond::new("120");
    let color = image::Rgba([0, 0, 0, 255]);

    let mut manual = GameState::new(table_spec.clone());
    let mut resolved_shooting_position = shooting_position.clone();
    resolved_shooting_position.resolve_shifts(&table_spec);
    let ghost_ball = object_ball.ghost_ball_to_pocket_with_speed(
        Pocket::CenterRight,
        speed.clone(),
        &table_spec,
    );
    manual.add_ghost_ball(&ghost_ball, ghost_fill_color(), ghost_outline_color());
    let (clipped_start, clipped_end) =
        clip_to_ball_edges(&table_spec, &resolved_shooting_position, &ghost_ball);
    manual.add_dotted_line(&clipped_start, &clipped_end, color);

    let mut helper = GameState::new(table_spec.clone());
    let helper_ghost_ball = helper.add_dotted_aim_line_to_pocket_with_speed(
        &object_ball,
        Pocket::CenterRight,
        &shooting_position,
        speed,
        color,
    );

    let mut slow_default = GameState::new(table_spec);
    let slow_default_ghost_ball = slow_default.add_dotted_aim_line_to_pocket(
        &object_ball,
        Pocket::CenterRight,
        &shooting_position,
        color,
    );

    assert_eq!(helper_ghost_ball, ghost_ball);
    assert_ne!(slow_default_ghost_ball, ghost_ball);
    assert_eq!(render(&helper), render(&manual));
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
    manual_with_markers.add_text_label_styled(&points[1], "(1)", label_style.clone());
    manual_with_markers.add_text_label_styled(&points[2], "(2)", label_style.clone());

    let mut helper_with_markers = GameState::new(table_spec.clone());
    helper_with_markers.add_dotted_ball_path_styled(
        &path,
        &BallPathStyle::new(color)
            .with_start_ghost(GhostBallStyle {
                fill_color: ghost_fill_color(),
                outline_color: ghost_outline_color(),
                ..Default::default()
            })
            .with_event_markers(marker_style)
            .with_labels(label_style),
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
            heading_chevrons: false,
            ..BallPathRenderOptions::default()
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
            heading_chevrons: false,
            ..BallPathRenderOptions::default()
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

#[test]
fn rendered_ball_paths_emit_speed_scaled_heading_chevrons_in_svg() {
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

    let mut state = GameState::new(table_spec);
    state.add_rendered_ball_path_styled(
        &path,
        &ball_set,
        &motion,
        &BallPathRenderOptions {
            max_time_step: Seconds::new(0.02),
            width_px: 8.0,
            width_mode: BallPathWidthMode::ScaleBySpeed,
            heading_chevrons: true,
            heading_chevron_spacing: Seconds::new(0.08),
            ..BallPathRenderOptions::default()
        },
        &style,
    );

    let svg = render_svg_with_options(
        &state,
        &DiagramRenderOptions {
            scale_factor: 1,
            background: DiagramBackground::Transparent,
        },
    );
    let chevrons = svg
        .lines()
        .filter(|line| line.contains("heading-chevron"))
        .collect::<Vec<_>>();
    assert!(
        chevrons.len() >= 3,
        "expected multiple instantaneous heading chevrons, got {} in {svg}",
        chevrons.len()
    );
    assert!(
        chevrons
            .iter()
            .all(|line| line.contains("data-heading-deg")),
        "heading chevrons should expose their table heading in SVG"
    );

    let min_width = chevrons
        .iter()
        .map(|line| svg_attr_f32(line, "stroke-width"))
        .fold(f32::INFINITY, f32::min);
    let max_width = chevrons
        .iter()
        .map(|line| svg_attr_f32(line, "stroke-width"))
        .fold(0.0, f32::max);
    assert!(
        max_width > min_width,
        "speed-scaled heading chevrons should narrow as the ball slows; got min {min_width} max {max_width}"
    );

    let min_opacity = chevrons
        .iter()
        .map(|line| svg_attr_f32(line, "stroke-opacity"))
        .fold(f32::INFINITY, f32::min);
    let max_opacity = chevrons
        .iter()
        .map(|line| svg_attr_f32(line, "stroke-opacity"))
        .fold(0.0, f32::max);
    assert!(
        max_opacity > min_opacity,
        "speed-scaled heading chevrons should fade as the ball slows; got min {min_opacity} max {max_opacity}"
    );
}
