use billiards::diagram::{
    render_scene_to_bytes, DiagramLayerId, DiagramOutputFormat, DiagramViewport,
};
use billiards::{
    trace_ball_path_with_rails_on_table,
    visualization::{
        AimOverlayStyle, BallPathRenderOptions, BallPathStyle, BallPathWidthMode, DashedLineStyle,
        DashedLineStyleError, EventMarkerStyle, GhostBallStyle, LabelOverlayStyle,
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

fn render_with_viewport(state: &GameState, viewport: DiagramViewport) -> RgbaImage {
    let options = DiagramRenderOptions {
        background: DiagramBackground::Transparent,
        ..DiagramRenderOptions::default()
    };
    render_with_viewport_and_options(state, viewport, &options)
}

fn render_with_viewport_and_options(
    state: &GameState,
    viewport: DiagramViewport,
    options: &DiagramRenderOptions,
) -> RgbaImage {
    let mut scene = state.to_diagram_scene(options);
    scene.viewport = viewport;
    load_from_memory(&render_scene_to_bytes(
        &scene,
        DiagramOutputFormat::Png,
        options,
    ))
    .expect("png decode")
    .into_rgba8()
}

fn viewport_400_by_800() -> DiagramViewport {
    let legacy = DiagramViewport::default();
    let x_scale = 400.0 / legacy.width_px;
    let y_scale = 800.0 / legacy.height_px;
    DiagramViewport {
        width_px: 400.0,
        height_px: 800.0,
        playfield_left_px: legacy.playfield_left_px * x_scale,
        playfield_right_px: legacy.playfield_right_px * x_scale,
        playfield_top_px: legacy.playfield_top_px * y_scale,
        playfield_bottom_px: legacy.playfield_bottom_px * y_scale,
    }
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

fn svg_attr_u64(element: &str, attr: &str) -> u64 {
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

fn svg_attr_str<'a>(element: &'a str, attr: &str) -> &'a str {
    let prefix = format!("{attr}=\"");
    let start = element
        .find(&prefix)
        .unwrap_or_else(|| panic!("missing SVG attribute {attr} in {element}"))
        + prefix.len();
    let end = element[start..]
        .find('"')
        .unwrap_or_else(|| panic!("unterminated SVG attribute {attr} in {element}"))
        + start;
    &element[start..end]
}

fn svg_has_class(element: &str, class_name: &str) -> bool {
    svg_attr_str(element, "class")
        .split_ascii_whitespace()
        .any(|candidate| candidate == class_name)
}

fn svg_element_with_class<'a>(svg: &'a str, class_name: &str, index: usize) -> &'a str {
    svg.lines()
        .filter(|line| line.contains("class=\"") && svg_has_class(line, class_name))
        .nth(index)
        .unwrap_or_else(|| panic!("missing SVG element {index} with class {class_name}"))
}

fn svg_has_element_with_class(svg: &str, class_name: &str) -> bool {
    svg.lines()
        .any(|line| line.contains("class=\"") && svg_has_class(line, class_name))
}

fn svg_css_rule<'a>(svg: &'a str, selector: &str) -> &'a str {
    let prefix = format!("{selector}{{");
    let start = svg
        .find(&prefix)
        .unwrap_or_else(|| panic!("missing SVG CSS rule for {selector}"))
        + prefix.len();
    let end = svg[start..]
        .find('}')
        .unwrap_or_else(|| panic!("unterminated SVG CSS rule for {selector}"))
        + start;
    &svg[start..end]
}

fn svg_rotation_degrees(element: &str) -> f32 {
    let transform_prefix = "transform=\"";
    let transform_start = element
        .find(transform_prefix)
        .unwrap_or_else(|| panic!("missing SVG transform in {element}"))
        + transform_prefix.len();
    let rotate_prefix = "rotate(";
    let rotate_start = element[transform_start..]
        .find(rotate_prefix)
        .unwrap_or_else(|| panic!("missing SVG rotation in {element}"))
        + transform_start
        + rotate_prefix.len();
    let rotate_end = element[rotate_start..]
        .find([' ', ')'])
        .unwrap_or_else(|| panic!("unterminated SVG rotation in {element}"))
        + rotate_start;
    element[rotate_start..rotate_end]
        .parse()
        .unwrap_or_else(|error| panic!("invalid SVG rotation in {element}: {error}"))
}

fn svg_element<'a>(svg: &'a str, marker: &str, index: usize) -> &'a str {
    svg.lines()
        .filter(|line| line.contains(marker))
        .nth(index)
        .unwrap_or_else(|| panic!("missing SVG element {index} matching {marker}"))
}

fn svg_path_numbers(element: &str) -> Vec<f32> {
    let prefix = "d=\"";
    let start = element
        .find(prefix)
        .unwrap_or_else(|| panic!("missing SVG path data in {element}"))
        + prefix.len();
    let end = element[start..]
        .find('"')
        .unwrap_or_else(|| panic!("unterminated SVG path data in {element}"))
        + start;

    element[start..end]
        .split(|ch: char| ch.is_ascii_alphabetic() || ch == ',' || ch.is_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse()
                .unwrap_or_else(|error| panic!("invalid SVG path number {part}: {error}"))
        })
        .collect()
}

fn quadratic_midpoint(start: (f32, f32), control: (f32, f32), end: (f32, f32)) -> (f32, f32) {
    (
        0.25 * start.0 + 0.5 * control.0 + 0.25 * end.0,
        0.25 * start.1 + 0.5 * control.1 + 0.25 * end.1,
    )
}

fn cubic_midpoint(
    start: (f32, f32),
    control_1: (f32, f32),
    control_2: (f32, f32),
    end: (f32, f32),
) -> (f32, f32) {
    (
        0.125 * start.0 + 0.375 * control_1.0 + 0.375 * control_2.0 + 0.125 * end.0,
        0.125 * start.1 + 0.375 * control_1.1 + 0.375 * control_2.1 + 0.125 * end.1,
    )
}

fn cubic_radius_at_midpoint(
    start: (f32, f32),
    control_1: (f32, f32),
    control_2: (f32, f32),
    end: (f32, f32),
) -> f32 {
    let velocity = (
        3.0 * (0.25 * (control_1.0 - start.0)
            + 0.5 * (control_2.0 - control_1.0)
            + 0.25 * (end.0 - control_2.0)),
        3.0 * (0.25 * (control_1.1 - start.1)
            + 0.5 * (control_2.1 - control_1.1)
            + 0.25 * (end.1 - control_2.1)),
    );
    let acceleration = (
        6.0 * (0.5 * (control_2.0 - 2.0 * control_1.0 + start.0)
            + 0.5 * (end.0 - 2.0 * control_2.0 + control_1.0)),
        6.0 * (0.5 * (control_2.1 - 2.0 * control_1.1 + start.1)
            + 0.5 * (end.1 - 2.0 * control_2.1 + control_1.1)),
    );
    let speed_squared = velocity.0 * velocity.0 + velocity.1 * velocity.1;
    let cross = (velocity.0 * acceleration.1 - velocity.1 * acceleration.0).abs();
    speed_squared.powf(1.5) / cross
}

fn assert_point_close(actual: (f32, f32), expected: (f32, f32)) {
    assert!(
        (actual.0 - expected.0).abs() < 0.002 && (actual.1 - expected.1).abs() < 0.002,
        "point {actual:?} != {expected:?}"
    );
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
fn dashed_line_style_accepts_a_zero_gap() {
    let result = DashedLineStyle::new(image::Rgba([255, 0, 255, 255])).with_pattern(4.0, 0.0);

    assert!(
        result.is_ok(),
        "a zero gap should produce a continuous dash pattern"
    );
}

#[test]
fn dashed_line_style_rejects_invalid_patterns() {
    let color = image::Rgba([255, 0, 255, 255]);
    let cases = [
        ("zero dash", 0.0, 1.0, DashedLineStyleError::DashLength),
        ("negative dash", -1.0, 1.0, DashedLineStyleError::DashLength),
        ("NaN dash", f32::NAN, 1.0, DashedLineStyleError::DashLength),
        (
            "positive-infinite dash",
            f32::INFINITY,
            1.0,
            DashedLineStyleError::DashLength,
        ),
        (
            "negative-infinite dash",
            f32::NEG_INFINITY,
            1.0,
            DashedLineStyleError::DashLength,
        ),
        ("negative gap", 1.0, -1.0, DashedLineStyleError::GapLength),
        ("NaN gap", 1.0, f32::NAN, DashedLineStyleError::GapLength),
        (
            "positive-infinite gap",
            1.0,
            f32::INFINITY,
            DashedLineStyleError::GapLength,
        ),
        (
            "negative-infinite gap",
            1.0,
            f32::NEG_INFINITY,
            DashedLineStyleError::GapLength,
        ),
        (
            "finite pattern with overflowing span",
            f32::MAX,
            f32::MAX,
            DashedLineStyleError::PatternSpanOverflow,
        ),
    ];

    for (name, dash_px, gap_px, expected) in cases {
        let error = DashedLineStyle::new(color)
            .with_pattern(dash_px, gap_px)
            .expect_err(name);

        assert_eq!(error, expected, "{name}");
    }
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
fn default_viewport_preserves_legacy_table_anchor_pixels() {
    let viewport = DiagramViewport::default();
    let cases = [
        (Position::new(0u8, 0u8), (110.0, 1828.0)),
        (Position::new(2u8, 4u8), (539.0, 969.0)),
        (Position::new(4u8, 8u8), (968.0, 110.0)),
    ];

    for (position, expected) in cases {
        let actual = viewport.position_to_scene_point(&position);
        assert_eq!((actual.x, actual.y), expected);
    }
}

#[test]
fn png_output_dimensions_follow_custom_viewport_for_every_background() {
    let viewport = viewport_400_by_800();
    let cases = [
        (
            "transparent",
            TableSpec::brunswick_gc4_9ft(),
            DiagramBackground::Transparent,
        ),
        (
            "pool table",
            TableSpec::brunswick_gc4_9ft(),
            DiagramBackground::Table,
        ),
        (
            "three-cushion table",
            TableSpec::three_cushion_carom_10ft(),
            DiagramBackground::Table,
        ),
    ];

    for (name, table_spec, background) in cases {
        let image = render_with_viewport_and_options(
            &GameState::new(table_spec),
            viewport,
            &DiagramRenderOptions {
                scale_factor: 1,
                background,
            },
        );

        assert_eq!(image.dimensions(), (400, 800), "{name}");
    }
}

#[test]
fn png_scale_factor_multiplies_custom_viewport_dimensions() {
    let image = render_with_viewport_and_options(
        &GameState::default(),
        viewport_400_by_800(),
        &DiagramRenderOptions {
            scale_factor: 2,
            background: DiagramBackground::Transparent,
        },
    );

    assert_eq!(image.dimensions(), (800, 1600));
}

#[test]
fn png_cue_ball_in_one_pixel_viewport_is_center_clipped_without_panicking() {
    let viewport = DiagramViewport {
        width_px: 1.0,
        height_px: 1.0,
        playfield_left_px: 100.0,
        playfield_right_px: 200.0,
        playfield_top_px: 100.0,
        playfield_bottom_px: 300.0,
    };
    let image = render_with_viewport(&cue_ball_at("2", "4"), viewport);

    assert_eq!(image.dimensions(), (1, 1));
    assert_eq!(
        image.get_pixel(0, 0)[3],
        0,
        "the off-canvas ball must be clipped at its physical center, not moved into the viewport"
    );
}

#[test]
fn png_marker_uses_custom_viewport_playfield_coordinates() {
    let viewport = viewport_400_by_800();
    let anchor = Position::new(1u8, 6u8);
    let expected = viewport.position_to_scene_point(&anchor);
    let options = DiagramRenderOptions {
        background: DiagramBackground::Transparent,
        ..DiagramRenderOptions::default()
    };
    let empty = render_with_viewport_and_options(&GameState::default(), viewport, &options);
    let mut marked = GameState::default();
    marked.add_event_marker_styled(
        &anchor,
        EventMarkerStyle {
            enabled: true,
            color: image::Rgba([255, 0, 255, 255]),
            radius_px: 7.0,
            layer: OverlayLayer::AboveBalls,
        },
    );
    let marked = render_with_viewport_and_options(&marked, viewport, &options);

    let (min_x, min_y, max_x, max_y) =
        diff_bbox(&empty, &marked).expect("custom-viewport marker should render");
    let actual = ((min_x + max_x) as f32 / 2.0, (min_y + max_y) as f32 / 2.0);

    assert!(
        (actual.0 - expected.x).abs() <= 1.0 && (actual.1 - expected.y).abs() <= 1.0,
        "marker center {actual:?} should align with custom viewport point ({}, {})",
        expected.x,
        expected.y,
    );
}

#[test]
fn png_rejects_invalid_viewport_dimensions() {
    const TOO_LARGE_FOR_U32: f32 = 4_294_967_296.0;
    let cases = [
        ("zero width", 0.0, 800.0),
        ("zero height", 400.0, 0.0),
        ("negative width", -1.0, 800.0),
        ("negative height", 400.0, -1.0),
        ("fractional width", 400.5, 800.0),
        ("fractional height", 400.0, 800.5),
        ("NaN width", f32::NAN, 800.0),
        ("NaN height", 400.0, f32::NAN),
        ("infinite width", f32::INFINITY, 800.0),
        ("infinite height", 400.0, f32::INFINITY),
        ("unrepresentable width", TOO_LARGE_FOR_U32, 800.0),
        ("unrepresentable height", 400.0, TOO_LARGE_FOR_U32),
    ];
    let options = DiagramRenderOptions {
        background: DiagramBackground::Transparent,
        ..DiagramRenderOptions::default()
    };

    for (name, width_px, height_px) in cases {
        let mut scene = GameState::default().to_diagram_scene(&options);
        scene.viewport = DiagramViewport {
            width_px,
            height_px,
            ..viewport_400_by_800()
        };

        let result = std::panic::catch_unwind(|| {
            render_scene_to_bytes(&scene, DiagramOutputFormat::Png, &options)
        });

        assert!(result.is_err(), "PNG rendering should reject {name}");
    }
}

#[test]
fn png_pool_table_moves_cloth_boundaries_with_non_proportional_viewport() {
    fn is_pool_rail(pixel: &image::Rgba<u8>) -> bool {
        let [red, green, blue, alpha] = pixel.0;
        alpha >= 240 && red <= 170 && green <= 195 && blue <= 100
    }

    fn is_pool_cloth(pixel: &image::Rgba<u8>) -> bool {
        let [red, green, blue, alpha] = pixel.0;
        alpha >= 240 && red >= 180 && green >= 195 && blue >= 100
    }

    let viewport = DiagramViewport {
        width_px: 1089.0,
        height_px: 1938.0,
        playfield_left_px: 250.0,
        playfield_right_px: 1070.0,
        playfield_top_px: 200.0,
        playfield_bottom_px: 1840.0,
    };
    let image = render_with_viewport_and_options(
        &GameState::new(TableSpec::brunswick_gc4_9ft()),
        viewport,
        &DiagramRenderOptions {
            scale_factor: 1,
            background: DiagramBackground::Table,
        },
    );
    let samples = [
        ("left", (242, 1073), (258, 1073)),
        ("top", (713, 192), (713, 208)),
    ];

    for (edge, rail_point, cloth_point) in samples {
        assert!(
            is_pool_rail(image.get_pixel(rail_point.0, rail_point.1)),
            "{edge} sample immediately outside the requested playfield should be rail"
        );
        assert!(
            is_pool_cloth(image.get_pixel(cloth_point.0, cloth_point.1)),
            "{edge} sample immediately inside the requested playfield should be cloth"
        );
    }
}

#[test]
fn raster_primitives_share_a_non_default_viewport_anchor() {
    let viewport = DiagramViewport {
        width_px: 1089.0,
        height_px: 1938.0,
        playfield_left_px: 250.0,
        playfield_right_px: 1070.0,
        playfield_top_px: 200.0,
        playfield_bottom_px: 1840.0,
    };
    let anchor = Position::new(2u8, 4u8);
    let expected_center = (660.0, 1020.0);
    let color = image::Rgba([255, 0, 255, 255]);

    let ball = cue_ball_at("2", "4");

    let mut line = GameState::default();
    let mut line_style = DashedLineStyle::new(color)
        .with_pattern(1000.0, 1.0)
        .expect("finite positive dash and finite positive gap should be valid");
    line_style.width_px = 3.0;
    line.add_dotted_line_styled(
        &Position::new(1u8, 4u8),
        &Position::new(3u8, 4u8),
        line_style,
    );

    let mut marker = GameState::default();
    marker.add_event_marker_styled(
        &anchor,
        EventMarkerStyle {
            enabled: true,
            color,
            radius_px: 7.0,
            layer: OverlayLayer::AboveBalls,
        },
    );

    let mut ghost = GameState::default();
    ghost.add_ghost_ball(&anchor, color, image::Rgba([0, 0, 0, 0]));

    let mut label = GameState::default();
    label.add_text_label_styled(
        &anchor,
        "8",
        LabelOverlayStyle {
            enabled: true,
            color,
            layer: OverlayLayer::AboveBalls,
            offset_x_px: -5,
            offset_y_px: -7,
            scale_px: 2,
        },
    );

    for (name, state) in [
        ("ball", &ball),
        ("line", &line),
        ("marker", &marker),
        ("ghost", &ghost),
        ("label", &label),
    ] {
        let image = render_with_viewport(state, viewport);
        let blank = RgbaImage::new(image.width(), image.height());
        let (min_x, min_y, max_x, max_y) =
            diff_bbox(&blank, &image).unwrap_or_else(|| panic!("{name} should render"));
        let actual_center = ((min_x + max_x) as f32 / 2.0, (min_y + max_y) as f32 / 2.0);

        assert!(
            (actual_center.0 - expected_center.0).abs() <= 1.0
                && (actual_center.1 - expected_center.1).abs() <= 1.0,
            "{name} center {actual_center:?} should align with viewport anchor {expected_center:?}",
        );
    }
}

#[test]
fn fully_out_of_view_ball_is_clipped_instead_of_relocated_to_the_image_edge() {
    let empty = render(&GameState::default());
    let with_ball = render(&cue_ball_at("5", "-1"));

    assert!(
        diff_bbox(&empty, &with_ball).is_none(),
        "a ball whose physical sprite is outside the viewport must not be clamped into view"
    );
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
fn svg_scale_factor_changes_intrinsic_dimensions_without_changing_view_box() {
    let svg = render_svg_with_options(
        &cue_ball_at("2", "4"),
        &DiagramRenderOptions {
            scale_factor: 2,
            ..DiagramRenderOptions::default()
        },
    );

    assert!(svg.contains("viewBox=\"0 0 1938 1089\""));
    assert!(svg.contains("width=\"3876\" height=\"2178\""));
}

#[test]
fn svg_scale_factor_above_f32_integer_precision_keeps_exact_intrinsic_dimensions() {
    const SCALE_FACTOR: u32 = (1 << 24) + 1;
    let svg = render_svg_with_options(
        &cue_ball_at("2", "4"),
        &DiagramRenderOptions {
            scale_factor: SCALE_FACTOR,
            ..DiagramRenderOptions::default()
        },
    );
    let root = svg.lines().next().expect("SVG root element");

    assert_eq!(svg_attr_u64(root, "width"), 1_938 * u64::from(SCALE_FACTOR));
    assert_eq!(
        svg_attr_u64(root, "height"),
        1_089 * u64::from(SCALE_FACTOR)
    );
    assert_eq!(root.matches("viewBox=\"0 0 1938 1089\"").count(), 1);
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
fn png_backend_renders_spin_glyphs_without_requiring_a_ball_sprite() {
    let table_spec = TableSpec::default();
    let ball_spec = BallSpec::default();
    let radius = ball_spec.radius.as_f64();
    let rolling = on_table(BallState::on_table(
        inches2(24.0, 48.0),
        Velocity2::new("0", "24"),
        AngularVelocity3::new(-24.0 / radius, 0.0, 0.0),
    ));
    let mut game = GameState::new(table_spec);
    game.add_spin_glyph_for_on_table_state(&rolling, &ball_spec);

    let rendered = render_with_options(
        &game,
        &DiagramRenderOptions {
            background: DiagramBackground::Transparent,
            ..DiagramRenderOptions::default()
        },
    );
    let visible_pixels = rendered.pixels().filter(|pixel| pixel[3] > 0).count();
    let bbox = diff_bbox(
        &RgbaImage::new(rendered.width(), rendered.height()),
        &rendered,
    )
    .expect("spin glyph should produce visible raster pixels");

    assert!(
        visible_pixels > 20,
        "spin glyph should have a visible filled footprint"
    );
    assert!(bbox.2 - bbox.0 >= 10 && bbox.3 - bbox.1 >= 10);
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
    assert!(svg.contains("viewBox=\"0 0 1938 1089\""));
    assert!(svg.contains("data-orientation=\"clockwise\""));
    assert!(
        svg.contains("class=\"diagram-orientation\" transform=\"translate(1938 0) rotate(90)\"")
    );
    assert!(svg.contains("data-layer=\"table\""));
    assert!(svg.contains("data-layer=\"balls\""));
    assert!(svg.contains("class=\"ball ball-cue\""));
}

#[test]
fn svg_ball_number_is_counter_rotated_to_remain_upright_on_screen() {
    let state = GameState::with_balls(
        TableSpec::default(),
        [Ball {
            ty: BallType::Eight,
            position: Position::new("2", "4"),
            spec: BallSpec::default(),
        }],
    );
    let svg = render_svg_with_options(&state, &DiagramRenderOptions::default());
    let label = svg_element_with_class(&svg, "ball-label", 0);

    let screen_rotation_degrees = 90.0 + svg_rotation_degrees(label);
    assert!(screen_rotation_degrees.abs() < 1e-6);
}

#[test]
fn svg_pool_numbered_solid_has_centered_large_bold_number_portrait() {
    let state = GameState::with_balls(
        TableSpec::default(),
        [Ball {
            ty: BallType::Eight,
            position: Position::new("2", "4"),
            spec: BallSpec::default(),
        }],
    );
    let svg = render_svg_with_options(&state, &DiagramRenderOptions::default());

    let outer = svg_element_with_class(&svg, "ball-eight", 0);
    assert!(svg_has_class(outer, "ball"));
    assert_eq!(svg_attr_str(outer, "data-ball"), "eight");
    assert_eq!(svg_attr_str(outer, "data-ball-style"), "solid");

    let shell = svg_element_with_class(&svg, "ball-shell", 0);
    let portrait = svg_element_with_class(&svg, "ball-number-medallion", 0);
    let label = svg_element_with_class(&svg, "ball-number-label", 0);
    let shell_radius = svg_attr_f32(shell, "r");
    let portrait_radius = svg_attr_f32(portrait, "r");

    assert_eq!(svg_attr_str(shell, "data-fill"), "black");
    assert_eq!(svg_attr_str(portrait, "data-fill"), "ivory");
    assert!((svg_attr_f32(portrait, "cx")).abs() < 1e-6);
    assert!((svg_attr_f32(portrait, "cy")).abs() < 1e-6);
    assert!(
        ((portrait_radius / shell_radius) - 0.47).abs() < 0.02,
        "number portrait radius {portrait_radius} was not about 47% of shell radius {shell_radius}"
    );
    assert!(!svg_has_element_with_class(&svg, "ball-stripe-band"));

    assert!(svg_has_class(label, "ball-label"));
    assert!((svg_attr_f32(label, "x")).abs() < 1e-6);
    assert!((svg_attr_f32(label, "y")).abs() < 1e-6);
    let label_size_ratio = svg_attr_f32(label, "font-size") / shell_radius;
    assert!(
        (0.72..=0.80).contains(&label_size_ratio),
        "number label size was {label_size_ratio:.3} times the shell radius"
    );
    assert!(label.contains(">8</text>"));

    let label_rule = svg_css_rule(&svg, ".ball-number-label");
    assert!(label_rule.split(';').any(|item| item == "font-weight:800"));
    let base_label_rule = svg_css_rule(&svg, ".ball-label");
    assert!(base_label_rule
        .split(';')
        .any(|item| item == "text-anchor:middle"));
    assert!(base_label_rule
        .split(';')
        .any(|item| item == "dominant-baseline:central"));
}

#[test]
fn svg_pool_nine_ball_uses_ivory_shell_with_yellow_stripe_band() {
    let state = GameState::with_balls(
        TableSpec::default(),
        [Ball {
            ty: BallType::Nine,
            position: Position::new("2", "4"),
            spec: BallSpec::default(),
        }],
    );
    let svg = render_svg_with_options(&state, &DiagramRenderOptions::default());

    let outer = svg_element_with_class(&svg, "ball-nine", 0);
    assert!(svg_has_class(outer, "ball"));
    assert_eq!(svg_attr_str(outer, "data-ball"), "nine");
    assert_eq!(svg_attr_str(outer, "data-ball-style"), "stripe");

    let shell = svg_element_with_class(&svg, "ball-shell", 0);
    let band = svg_element_with_class(&svg, "ball-stripe-band", 0);
    let portrait = svg_element_with_class(&svg, "ball-number-medallion", 0);
    let label = svg_element_with_class(&svg, "ball-number-label", 0);

    assert_eq!(svg_attr_str(shell, "data-fill"), "ivory");
    assert_eq!(svg_attr_str(band, "data-fill"), "yellow");
    assert_eq!(svg_attr_str(portrait, "data-fill"), "ivory");
    assert!((svg_attr_f32(portrait, "cx")).abs() < 1e-6);
    assert!((svg_attr_f32(portrait, "cy")).abs() < 1e-6);
    assert!(svg_has_class(label, "ball-label"));
    assert!(label.contains(">9</text>"));
}

#[test]
fn svg_three_cushion_balls_do_not_use_pool_number_or_stripe_artwork() {
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

    assert!(svg_has_element_with_class(&svg, "ball-cue"));
    assert!(svg_has_element_with_class(&svg, "ball-yellow"));
    assert!(svg_has_element_with_class(&svg, "ball-red"));
    assert!(!svg_has_element_with_class(&svg, "ball-number-medallion"));
    assert!(!svg_has_element_with_class(&svg, "ball-number-label"));
    assert!(!svg_has_element_with_class(&svg, "ball-stripe-band"));
}

#[test]
fn svg_backend_emits_compact_spin_glyphs_with_angle_and_spin_speed_data() {
    let table_spec = TableSpec::default();
    let ball_spec = BallSpec::default();
    let radius = ball_spec.radius.as_f64();
    let rolling = on_table(BallState::on_table(
        inches2(18.0, 30.0),
        Velocity2::new("0", "24"),
        AngularVelocity3::new(-24.0 / radius, 0.0, 0.0),
    ));
    let draw = on_table(BallState::on_table(
        inches2(24.0, 36.0),
        Velocity2::new("0", "24"),
        AngularVelocity3::new(24.0 / radius, 0.0, 0.0),
    ));
    let follow = on_table(BallState::on_table(
        inches2(30.0, 42.0),
        Velocity2::new("0", "352"),
        AngularVelocity3::new(-704.0 / radius, 0.0, 0.0),
    ));
    let english = on_table(BallState::on_table(
        inches2(36.0, 48.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 240.0),
    ));
    let stun = on_table(BallState::on_table(
        inches2(42.0, 54.0),
        Velocity2::zero(),
        AngularVelocity3::zero(),
    ));
    let states = [&rolling, &draw, &follow, &english, &stun];
    let balls = states.iter().enumerate().map(|(index, state)| Ball {
        ty: match index {
            0 => BallType::Cue,
            1 => BallType::One,
            2 => BallType::Two,
            3 => BallType::Three,
            _ => BallType::Four,
        },
        position: state.as_ball_state().projected_position(&table_spec),
        spec: ball_spec.clone(),
    });
    let mut game = GameState::with_balls(table_spec.clone(), balls);
    for state in states {
        game.add_spin_glyph_for_on_table_state(state, &ball_spec);
    }
    let trajectory_start = rolling.as_ball_state().projected_position(&table_spec);
    let trajectory_end =
        trajectory_start.translate_inches(Inches::from_f64(12.0), Angle::from_north(0.0, 1.0));
    game.add_dotted_line(
        &trajectory_start,
        &trajectory_end,
        image::Rgba([255, 255, 255, 255]),
    );

    let svg = render_svg_with_options(
        &game,
        &DiagramRenderOptions {
            background: DiagramBackground::Transparent,
            ..DiagramRenderOptions::default()
        },
    );

    assert_eq!(svg.matches("class=\"overlay ball-spin-glyph\"").count(), 5);
    assert!(svg.contains("data-spin-kind=\"rolling\""));
    assert!(svg.contains("data-spin-kind=\"draw\""));
    assert!(svg.contains("data-spin-kind=\"follow\""));
    assert!(svg.contains("data-spin-kind=\"english\""));
    assert!(svg.contains("data-spin-kind=\"stun\""));
    assert!(svg.contains("data-spin-rps=\"240.000\""));
    assert!(svg.contains("data-spin-angle-deg=\"-90.000\""));
    assert!(svg.contains("data-spin-roll-ratio=\"1.000\""));
    assert!(svg.contains("class=\"ball-spin-vector-halo\""));
    assert!(svg.contains("class=\"ball-spin-vector\""));
    assert!(svg.contains("class=\"ball-spin-z-halo\""));
    assert!(svg.contains("class=\"ball-spin-z\""));
    assert!(svg.contains("class=\"ball-spin-stun-x-mark\""));
    assert!(!svg.contains("class=\"ball-spin-dot\""));
    assert!(svg.contains("#2da44e"));
    assert!(svg.contains("#8b5cf6"));
    assert!(svg.contains("#fb851e"));
    assert!(svg.contains("#096bd8"));
    assert!(svg.contains("role=\"img\" aria-label=\"spin:"));
    assert!(svg.contains("#7f858c"));
    assert!(svg.contains("omega="));
    let follow_glyph = svg_element(&svg, "data-spin-kind=\"follow\"", 0);
    assert!(follow_glyph.contains("data-spin-slip-ips=\"352.000\""));
    let follow_aria = follow_glyph
        .split_once("aria-label=\"")
        .expect("follow spin glyph should have an accessible label")
        .1
        .split_once('"')
        .expect("follow spin glyph accessible label should be terminated")
        .0;
    let follow_title = follow_glyph
        .split_once("<title>")
        .expect("follow spin glyph should have a title")
        .1
        .split_once("</title>")
        .expect("follow spin glyph title should be terminated")
        .0;
    assert_eq!(follow_title, follow_aria);
    assert!(follow_title.contains("v=(0.0, 32.2) km/h"));
    assert!(follow_title.contains("roll slip=32.2 km/h"));
    let trajectory = svg_element(&svg, "class=\"overlay dashed-line\"", 0);
    let trajectory_dx = svg_attr_f32(trajectory, "x2") - svg_attr_f32(trajectory, "x1");
    let trajectory_dy = svg_attr_f32(trajectory, "y2") - svg_attr_f32(trajectory, "y1");
    let trajectory_length = trajectory_dx.hypot(trajectory_dy);
    let rolling_glyph = svg_element(&svg, "data-spin-kind=\"rolling\"", 0);
    let arrow_angle = svg_attr_f32(rolling_glyph, "data-spin-angle-deg").to_radians();
    let alignment =
        (trajectory_dx * arrow_angle.cos() + trajectory_dy * arrow_angle.sin()) / trajectory_length;
    assert!(
        alignment > 0.999,
        "rolling spin arrow should point along the rendered trajectory, got dot {alignment}"
    );
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
    assert_eq!(svg.matches("class=\"table-pocket-facing\"").count(), 0);
    assert_eq!(svg.matches(".table-pocket-facing").count(), 0);
    assert_eq!(svg.matches("data-pocket=\"corner-liner\"").count(), 4);
    assert_eq!(svg.matches("data-pocket=\"side-liner\"").count(), 2);
    let first_cushion = svg
        .find("<path class=\"table-cushion\"")
        .expect("table cushion should render");
    let first_shelf = svg
        .find("<path class=\"table-pocket-shelf\"")
        .expect("pocket shelf should render");
    assert!(first_cushion < first_shelf);
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
    assert!(!svg.contains("<rect class=\"table-rail-inner-shadow\""));
}

#[test]
fn svg_pool_pocket_shelves_are_depth_calibrated_and_share_drop_edges() {
    let svg = render_svg_with_options(&cue_ball_at("2", "4"), &DiagramRenderOptions::default());
    let cloth = svg_element(&svg, "<rect class=\"table-cloth\"", 0);
    let x_px_per_inch = svg_attr_f32(cloth, "width") / 50.0;
    let y_px_per_inch = svg_attr_f32(cloth, "height") / 100.0;

    let corner_well = svg_path_numbers(svg_element(
        &svg,
        "class=\"table-pocket-well\" data-pocket=\"corner\"",
        0,
    ));
    let corner_shelf = svg_path_numbers(svg_element(
        &svg,
        "class=\"table-pocket-shelf\" data-pocket=\"corner-shelf\"",
        0,
    ));
    let corner_drop_start = (corner_well[0], corner_well[1]);
    let corner_drop_end = (corner_well[6], corner_well[7]);
    let corner_drop_control = (corner_well[8], corner_well[9]);
    assert_point_close(corner_drop_start, (corner_shelf[2], corner_shelf[3]));
    assert_point_close(corner_drop_control, (corner_shelf[4], corner_shelf[5]));
    assert_point_close(corner_drop_end, (corner_shelf[6], corner_shelf[7]));

    let top_cushion_nose = svg_element(&svg, "<line class=\"table-cushion-nose\"", 0);
    let corner = (svg_attr_f32(cloth, "x"), svg_attr_f32(cloth, "y"));
    let corner_back_mid = cubic_midpoint(
        corner_drop_start,
        (corner_well[2], corner_well[3]),
        (corner_well[4], corner_well[5]),
        corner_drop_end,
    );
    let corner_back_setback_x_in = (corner.0 - corner_back_mid.0) / x_px_per_inch;
    let corner_back_setback_y_in = (corner.1 - corner_back_mid.1) / y_px_per_inch;
    assert!(
        (corner_back_setback_x_in - 3.0).abs() < 0.002
            && (corner_back_setback_y_in - 3.0).abs() < 0.002,
        "corner liner midpoint setbacks were {corner_back_setback_x_in} x {corner_back_setback_y_in} in"
    );
    let corner_back_radius_in = cubic_radius_at_midpoint(
        corner_drop_start,
        (corner_well[2], corner_well[3]),
        (corner_well[4], corner_well[5]),
        corner_drop_end,
    ) / ((x_px_per_inch + y_px_per_inch) * 0.5);
    assert!(
        (2.0..=2.3).contains(&corner_back_radius_in),
        "corner liner rear radius was {corner_back_radius_in} in"
    );
    let mouth_top = (
        svg_attr_f32(top_cushion_nose, "x1"),
        svg_attr_f32(top_cushion_nose, "y1"),
    );
    let corner_drop_mid =
        quadratic_midpoint(corner_drop_start, corner_drop_control, corner_drop_end);
    let mouth_line_in =
        (mouth_top.0 - corner.0) / x_px_per_inch + (mouth_top.1 - corner.1) / y_px_per_inch;
    let drop_line_in = (corner_drop_mid.0 - corner.0) / x_px_per_inch
        + (corner_drop_mid.1 - corner.1) / y_px_per_inch;
    let corner_shelf_depth_in = (mouth_line_in - drop_line_in) / 2.0_f32.sqrt();
    assert!(
        (corner_shelf_depth_in - 1.75).abs() < 0.002,
        "corner shelf depth was {corner_shelf_depth_in} in"
    );

    let side_well = svg_path_numbers(svg_element(
        &svg,
        "class=\"table-pocket-well\" data-pocket=\"side\"",
        0,
    ));
    let side_shelf = svg_path_numbers(svg_element(
        &svg,
        "class=\"table-pocket-shelf\" data-pocket=\"side-shelf\"",
        0,
    ));
    let side_drop_start = (side_well[0], side_well[1]);
    let side_drop_end = (side_well[22], side_well[23]);
    let side_drop_control = (side_well[24], side_well[25]);
    assert_point_close(side_drop_start, (side_shelf[0], side_shelf[1]));
    assert_point_close(side_drop_control, (side_shelf[2], side_shelf[3]));
    assert_point_close(side_drop_end, (side_shelf[4], side_shelf[5]));
    assert_point_close(
        (side_well[26], side_well[27]),
        (side_shelf[8], side_shelf[9]),
    );

    let side_liner = svg_path_numbers(svg_element(
        &svg,
        "class=\"table-pocket-leather\" data-pocket=\"side-liner\"",
        0,
    ));
    let side_mouth_x = side_liner[0];
    // The output rotates raw x/y clockwise, so the depth-over-bed-run
    // tangent leaving the lip is the visible top-view cut angle.
    let side_cut_angle_deg = (side_liner[2] - side_liner[0])
        .abs()
        .atan2((side_liner[3] - side_liner[1]).abs())
        .to_degrees();
    assert!(
        (side_cut_angle_deg - 8.0).abs() < 0.25,
        "side pocket cut tangent was {side_cut_angle_deg} degrees"
    );
    let side_mouth_width_in = (side_well[23] - side_well[1]).abs() / y_px_per_inch;
    assert!(
        (side_mouth_width_in - 5.0).abs() < 0.002,
        "side pocket opening was {side_mouth_width_in} in"
    );

    let side_rear_x = side_liner
        .iter()
        .step_by(2)
        .copied()
        .fold(f32::INFINITY, f32::min);
    let side_rear_depth_in = (side_mouth_x - side_rear_x) / x_px_per_inch;
    assert!(
        (2.6..=2.8).contains(&side_rear_depth_in),
        "side liner rear depth was {side_rear_depth_in} in"
    );
    let side_rear_radius_in = cubic_radius_at_midpoint(
        (side_liner[6], side_liner[7]),
        (side_liner[8], side_liner[9]),
        (side_liner[10], side_liner[11]),
        (side_liner[12], side_liner[13]),
    ) / x_px_per_inch;
    assert!(
        (5.0..=6.2).contains(&side_rear_radius_in),
        "side liner rear radius was {side_rear_radius_in} in"
    );

    let side_shelf_sagitta_in = (side_drop_start.0 - side_drop_control.0).abs() / x_px_per_inch;
    assert!(
        (side_shelf_sagitta_in - 0.128).abs() < 0.002,
        "side shelf sagitta was {side_shelf_sagitta_in} in"
    );
    let side_chord_in = (side_drop_end.1 - side_drop_start.1).abs() / y_px_per_inch;
    let side_shelf_radius_in =
        side_chord_in * side_chord_in / (8.0 * side_shelf_sagitta_in) + side_shelf_sagitta_in * 0.5;
    assert!(
        side_shelf_radius_in > 20.0,
        "side shelf radius was {side_shelf_radius_in} in"
    );
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
fn png_three_cushion_table_replaces_pool_pocket_wells_with_continuous_surfaces() {
    fn neighborhood_counts(
        image: &RgbaImage,
        center: (u32, u32),
        radius: u32,
    ) -> (usize, usize, usize) {
        let min_x = center.0.saturating_sub(radius);
        let max_x = center.0.saturating_add(radius).min(image.width() - 1);
        let min_y = center.1.saturating_sub(radius);
        let max_y = center.1.saturating_add(radius).min(image.height() - 1);
        let mut dark = 0;
        let mut opaque_colored_surface = 0;
        let mut total = 0;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let [red, green, blue, alpha] = image.get_pixel(x, y).0;
                let min_channel = red.min(green).min(blue);
                let max_channel = red.max(green).max(blue);
                let is_colored = max_channel.saturating_sub(min_channel) >= 24;
                let is_opaque = alpha >= 240;
                let is_dark = is_opaque && max_channel <= 64;
                dark += usize::from(is_dark);
                opaque_colored_surface += usize::from(is_opaque && !is_dark && is_colored);
                total += 1;
            }
        }

        (dark, opaque_colored_surface, total)
    }

    let options = DiagramRenderOptions {
        background: DiagramBackground::Table,
        ..DiagramRenderOptions::default()
    };
    let pool = render_with_options(&GameState::new(TableSpec::brunswick_gc4_9ft()), &options);
    let carom = render_with_options(
        &GameState::new(TableSpec::three_cushion_carom_10ft()),
        &options,
    );
    let viewport = DiagramViewport::default();
    let center_y = (viewport.playfield_top_px + viewport.playfield_bottom_px) * 0.5;
    let pocket_locations = [
        (
            "top-left corner",
            viewport.playfield_left_px,
            viewport.playfield_top_px,
        ),
        (
            "top-right corner",
            viewport.playfield_right_px,
            viewport.playfield_top_px,
        ),
        ("left side", viewport.playfield_left_px, center_y),
        ("right side", viewport.playfield_right_px, center_y),
        (
            "bottom-left corner",
            viewport.playfield_left_px,
            viewport.playfield_bottom_px,
        ),
        (
            "bottom-right corner",
            viewport.playfield_right_px,
            viewport.playfield_bottom_px,
        ),
    ];
    const NEIGHBORHOOD_RADIUS_PX: u32 = 30;

    for (name, x, y) in pocket_locations {
        let center = (x.round() as u32, y.round() as u32);
        let (pool_dark, _, pool_total) = neighborhood_counts(&pool, center, NEIGHBORHOOD_RADIUS_PX);
        let (carom_dark, carom_surface, carom_total) =
            neighborhood_counts(&carom, center, NEIGHBORHOOD_RADIUS_PX);

        assert!(
            pool_dark * 3 >= pool_total,
            "pool {name} neighborhood contained only {pool_dark}/{pool_total} dark pixels; expected a broad pocket well"
        );
        assert!(
            carom_dark * 5 <= carom_total,
            "carom {name} neighborhood contained {carom_dark}/{carom_total} dark pixels; expected no broad pocket well"
        );
        assert!(
            carom_surface * 5 >= carom_total * 4,
            "carom {name} neighborhood contained only {carom_surface}/{carom_total} opaque cloth/cushion/rail pixels"
        );
    }
}

#[test]
fn svg_table_uses_installed_leather_rims_and_cloth_shelves() {
    let svg = render_svg_with_options(&cue_ball_at("2", "4"), &DiagramRenderOptions::default());

    assert!(svg.contains("<path class=\"table-pocket-well\" data-pocket=\"corner\""));
    assert!(svg.contains("<path class=\"table-pocket-well\" data-pocket=\"side\""));
    assert!(svg.contains("<path class=\"table-pocket-leather\" data-pocket=\"corner-liner\""));
    assert!(svg.contains("<path class=\"table-pocket-leather\" data-pocket=\"side-liner\""));
    assert!(svg.contains("<path class=\"table-pocket-leather-highlight\""));
    assert!(!svg.contains("<path class=\"table-pocket-mouth-shadow\""));
    assert!(svg.contains("<path class=\"table-pocket-shelf\" data-pocket=\"corner-shelf\""));
    assert!(svg.contains("<path class=\"table-pocket-shelf\" data-pocket=\"side-shelf\""));
    assert!(!svg.contains("table-pocket-facing"));
    assert!(!svg.contains(".table-pocket-facing"));
    let corner_liner = svg_element(&svg, "data-pocket=\"corner-liner\"", 0);
    let side_liner = svg_element(&svg, "data-pocket=\"side-liner\"", 0);
    let corner_width = svg_attr_f32(corner_liner, "stroke-width");
    assert!(
        (18.0..=19.5).contains(&corner_width),
        "corner top-plane rim was {corner_width}px wide"
    );
    let side_width = svg_attr_f32(side_liner, "stroke-width");
    assert!(
        (24.0..=26.0).contains(&side_width),
        "side top-plane rim was {side_width}px wide"
    );
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
    )
    .expect("event-marker path should trace");
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
fn carom_ghost_ball_uses_the_table_ball_size() {
    let table_spec = TableSpec::three_cushion_carom_10ft();
    let empty = render(&GameState::new(table_spec.clone()));
    let mut ghosted = GameState::new(table_spec.clone());
    ghosted.add_ghost_ball(
        &Position::new(2u8, 4u8),
        ghost_fill_color(),
        ghost_outline_color(),
    );

    let (min_x, min_y, max_x, max_y) =
        diff_bbox(&empty, &render(&ghosted)).expect("ghost ball diff bbox");
    let expected_diameter = (2.0
        * DiagramViewport::default().ball_radius_px(&table_spec, &table_spec.default_ball_spec()))
    .round() as u32;

    assert_eq!(max_x - min_x + 1, expected_diameter);
    assert_eq!(max_y - min_y + 1, expected_diameter);
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
    )
    .expect("mirror-bank path should trace");
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
    )
    .expect("speed-scaled raster path should trace");
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
    )
    .expect("speed-scaled SVG path should trace");
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
