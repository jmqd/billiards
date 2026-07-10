use bigdecimal::ToPrimitive;
use billiards::dsl::{
    parse_dsl, parse_dsl_to_game_state, parse_dsl_to_scenario, CoordinateAxis, DslBuildError,
    DslError, DslParseError, RailSide, ScenarioBallTimelineSegment, ScenarioBallTrace,
    ScenarioShotTrace, ScenarioTraceRenderOptions,
};
use billiards::{
    advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table,
    human_tuned_preview_motion_config,
    visualization::{BallPathRenderOptions, BallPathStyle, PathColorMode, SmoothPolylineStyle},
    Angle, AngularVelocity3, BallPathStop, BallSetPhysicsSpec, BallState, BallType, CollisionModel,
    Diamond, GameType, HumanShotSpeedBand, Inches2, InchesPerSecondSq, MotionPhase,
    MotionPhaseConfig, MotionTransitionConfig, NBallSystemEvent, NBallSystemSimulation,
    NBallSystemState, OnTableBallState, OnTableMotionConfig, PlayingConditions, Pocket,
    RadiansPerSecondSq, RailCollisionProfile, RailModel, RollingResistanceModel, Seconds,
    ShotSpeedPreset, SlidingFrictionModel, SpinDecayModel, TableKind, Velocity2, CAROM_BALL_RADIUS,
    TYPICAL_BALL_RADIUS,
};
use image::{load_from_memory, Rgba};

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

fn assert_parse_error(input: &str) {
    let err = parse_dsl_to_game_state(input).expect_err("expected parse failure");
    assert!(matches!(err, DslError::Parse(_)), "unexpected error: {err}");
}

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

fn render_png(state: &billiards::GameState) -> image::RgbaImage {
    load_from_memory(&state.draw_2d_diagram())
        .expect("png decode")
        .into_rgba8()
}

#[test]
fn parse_dsl_returns_a_crate_owned_error_with_a_byte_offset() {
    let err = parse_dsl("ball cue nope").expect_err("expected parse failure");

    assert_eq!(
        err,
        DslParseError {
            message: "invalid DSL".to_string(),
            offset: 9,
        }
    );
}

#[test]
fn given_comments_blank_lines_aliases_and_frozen_balls_when_building_then_positions_match_table_space(
) {
    let state = parse_dsl_to_game_state(
        "# comment\n\n\
         pos hanger = (3.93, 7.93)\n\
         ball cue at center\n\
         ball nine at hanger\n\
         ball eight frozen left (6.0)\n",
    )
    .expect("expected DSL to build");

    let cue = state.select_ball(BallType::Cue).expect("cue ball");
    let nine = state.select_ball(BallType::Nine).expect("nine ball");
    let eight = state.select_ball(BallType::Eight).expect("eight ball");

    assert_close(cue.position.x.magnitude.to_f64().expect("cue x"), 2.0);
    assert_close(cue.position.y.magnitude.to_f64().expect("cue y"), 4.0);
    assert_close(nine.position.x.magnitude.to_f64().expect("nine x"), 3.93);
    assert_close(nine.position.y.magnitude.to_f64().expect("nine y"), 7.93);
    assert_close(eight.position.x.magnitude.to_f64().expect("eight x"), 0.09);
    assert_close(eight.position.y.magnitude.to_f64().expect("eight y"), 6.0);
}

#[test]
fn carom_table_dsl_builds_pocketless_table_game_and_carom_balls() {
    let scenario = parse_dsl_to_scenario(
        "table three_cushion_carom_10ft\n\
         game three_cushion\n\
         ball cue at (1.0, 1.0)\n\
         ball yellow at (2.0, 4.0)\n\
         ball red at (3.0, 7.0)\n",
    )
    .expect("expected carom DSL to build");

    assert_eq!(
        scenario.game_state.table_spec.kind,
        TableKind::ThreeCushionCarom
    );
    assert!(!scenario.game_state.table_spec.has_pockets());
    assert_eq!(scenario.game_state.ty, GameType::ThreeCushion);
    assert_close(
        scenario.ball_set_physics_spec().radius.as_f64(),
        CAROM_BALL_RADIUS.as_f64(),
    );

    for ball_type in [BallType::Cue, BallType::YellowCue, BallType::Red] {
        let ball = scenario
            .game_state
            .select_ball(ball_type)
            .expect("carom ball placement");
        assert_close(ball.spec.radius.as_f64(), CAROM_BALL_RADIUS.as_f64());
    }
}


#[test]
fn given_an_invalid_second_statement_when_parsing_then_the_error_offset_points_at_the_bad_token() {
    let err = parse_dsl("ball cue at center\nball nine nope").expect_err("expected parse failure");

    assert_eq!(
        err,
        DslParseError {
            message: "invalid DSL".to_string(),
            offset: 29,
        }
    );
}

#[test]
fn rejects_alias_values_on_the_next_line() {
    assert_parse_error("pos spot =\ncenter");
}

#[test]
fn rejects_multiline_coordinates() {
    assert_parse_error("ball cue at (2,\n4)");
}

#[test]
fn rejects_coordinates_outside_the_table_bounds() {
    let err = parse_dsl_to_game_state("ball cue at (4.01, 2)").expect_err("expected build failure");

    assert!(matches!(
        err,
        DslError::Build(DslBuildError::CoordinateOutOfRange {
            axis: CoordinateAxis::X,
            value,
            min: 0.0,
            max: 4.0,
        }) if (value - 4.01).abs() < f64::EPSILON
    ));
}

#[test]
fn rejects_frozen_coordinates_past_the_end_of_a_rail() {
    let err =
        parse_dsl_to_game_state("ball cue frozen top (4.01)").expect_err("expected build failure");

    assert!(matches!(
        err,
        DslError::Build(DslBuildError::FrozenCoordinateOutOfRange {
            rail: RailSide::Top,
            value,
            min: 0.0,
            max: 4.0,
        }) if (value - 4.01).abs() < f64::EPSILON
    ));
}

#[test]
fn a_chained_shot_scenario_builds_validated_domain_types_and_can_seed_the_engine() {
    let scenario = parse_dsl_to_scenario(
        "ball cue at center\n\
         cue_strike(default)\n\
           .mass_ratio(1.0)\n\
           .energy_loss(0.1)\n\
           .endmass_ratio(29.158)\n\
         shot(cue)\n\
           .heading(30deg)\n\
           .speed(128ips)\n\
           .tip(side: 0.0R, height: 0.4R)\n\
           .using(default)\n",
    )
    .expect("expected shot DSL to build");

    let cue = scenario
        .game_state
        .select_ball(BallType::Cue)
        .expect("cue ball placement");
    let shot = scenario.shot.as_ref().expect("scenario shot");
    let seeded = scenario
        .strike_shot_on_table(&BallSetPhysicsSpec::default())
        .expect("expected strike to succeed")
        .expect("scenario should contain a shot");

    assert_close(cue.position.x.magnitude.to_f64().expect("cue x"), 2.0);
    assert_close(cue.position.y.magnitude.to_f64().expect("cue y"), 4.0);
    assert_eq!(shot.ball, BallType::Cue);
    assert_close(shot.shot.heading().as_degrees(), 30.0);
    assert!(
        shot.shot.cue_speed().as_f64() > 128.0,
        "DSL speed is cue-ball launch speed, so off-center hits require more cue-stick speed"
    );
    assert_close(seeded.as_ball_state().speed().as_f64(), 128.0);
    assert_close(shot.shot.tip_contact().side_offset().as_f64(), 0.0);
    assert_close(shot.shot.tip_contact().height_offset().as_f64(), 0.4);
    assert_close(shot.cue_strike.cue_mass_ratio().as_f64(), 1.0);
    assert_close(shot.cue_strike.collision_energy_loss().as_f64(), 0.1);
    assert_close(shot.cue_strike.cue_ball_to_endmass_ratio().as_f64(), 29.158);
    assert_eq!(
        seeded
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Rolling
    );
}

#[test]
fn shot_speed_literals_accept_mph_and_kph() {
    let mph = parse_dsl_to_scenario(
        "ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(90deg).speed(10mph).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected mph shot DSL to build");
    let kph = parse_dsl_to_scenario(
        "ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(90deg).speed(16.09344kph).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected kph shot DSL to build");

    assert_close(
        mph.strike_shot_on_table(&BallSetPhysicsSpec::default())
            .expect("mph shot should strike")
            .expect("mph scenario should contain a shot")
            .as_ball_state()
            .speed()
            .as_f64(),
        176.0,
    );
    assert_close(
        kph.strike_shot_on_table(&BallSetPhysicsSpec::default())
            .expect("kph shot should strike")
            .expect("kph scenario should contain a shot")
            .as_ball_state()
            .speed()
            .as_f64(),
        176.0,
    );
}

#[test]
fn shot_speed_literals_accept_dr_dave_named_and_numbered_presets() {
    let named = parse_dsl_to_scenario(
        "ball cue at (0, 0)\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(90deg).speed(medium-fast).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected named speed preset to build");
    let numbered = parse_dsl_to_scenario(
        "ball cue at (0, 0)\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(90deg).speed(3).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected numbered speed preset to build");

    assert_close(
        named
            .strike_shot_on_table(&BallSetPhysicsSpec::default())
            .expect("named speed shot should strike")
            .expect("named scenario should contain a shot")
            .as_ball_state()
            .speed()
            .as_f64(),
        ShotSpeedPreset::MediumFast.inches_per_second().as_f64(),
    );
    assert_close(
        numbered
            .strike_shot_on_table(&BallSetPhysicsSpec::default())
            .expect("numbered speed shot should strike")
            .expect("numbered scenario should contain a shot")
            .as_ball_state()
            .speed()
            .as_f64(),
        ShotSpeedPreset::Fast.inches_per_second().as_f64(),
    );
}

#[test]
fn shot_scenarios_can_derive_heading_with_to_pocket() {
    let scenario = parse_dsl_to_scenario(
        "ball cue at center\n\
         ball nine at (2.0, 6.0)\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).to_pocket(nine, top-right).speed(64ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected to_pocket shot DSL to build");

    let cue = scenario
        .game_state
        .select_ball(BallType::Cue)
        .expect("cue ball placement");
    let nine = scenario
        .game_state
        .select_ball(BallType::Nine)
        .expect("nine ball placement");
    let expected = nine.aim_angle_to_pocket(
        Pocket::TopRight,
        &cue.position,
        &scenario.game_state.table_spec,
    );

    assert_close(
        scenario
            .shot
            .as_ref()
            .expect("shot")
            .shot
            .heading()
            .as_degrees(),
        expected.as_degrees(),
    );
}

#[test]
fn shot_scenarios_can_derive_heading_with_pocket_alias() {
    let via_to_pocket = parse_dsl_to_scenario(
        "ball cue at center\n\
         ball nine at (2.0, 6.0)\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).to_pocket(nine, top-right).speed(64ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected to_pocket shot DSL to build");
    let via_pocket = parse_dsl_to_scenario(
        "ball cue at center\n\
         ball nine at (2.0, 6.0)\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).pocket(nine, top-right).speed(64ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected pocket alias shot DSL to build");

    assert_close(
        via_pocket
            .shot
            .as_ref()
            .expect("shot")
            .shot
            .heading()
            .as_degrees(),
        via_to_pocket
            .shot
            .as_ref()
            .expect("shot")
            .shot
            .heading()
            .as_degrees(),
    );
}

#[test]
fn shot_scenarios_can_derive_heading_with_cut_helpers() {
    let scenario = parse_dsl_to_scenario(
        "ball cue at center\n\
         ball nine at (2.0, 6.0)\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).cut(nine, left(32)).speed(64ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected cut shot DSL to build");

    let cue = scenario
        .game_state
        .select_ball(BallType::Cue)
        .expect("cue ball placement");
    let nine = scenario
        .game_state
        .select_ball(BallType::Nine)
        .expect("nine ball placement");
    let object_heading_degrees = cue.position.angle_to(&nine.position).as_degrees() - 32.0;
    let object_heading = Angle::from_north(
        object_heading_degrees.to_radians().sin(),
        object_heading_degrees.to_radians().cos(),
    );
    let destination = nine.position.translate(Diamond::one(), object_heading);
    let expected = nine.aim_angle(&destination, &cue.position, &scenario.game_state.table_spec);

    assert_close(
        scenario
            .shot
            .as_ref()
            .expect("shot")
            .shot
            .heading()
            .as_degrees(),
        expected.as_degrees(),
    );
}

#[test]
fn shot_scenarios_can_derive_heading_with_cut_left_and_cut_right_aliases() {
    let via_cut_left = parse_dsl_to_scenario(
        "ball cue at center\n\
         ball nine at (2.0, 6.0)\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).cut_left(nine, 32).speed(64ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected cut_left shot DSL to build");
    let via_cut = parse_dsl_to_scenario(
        "ball cue at center\n\
         ball nine at (2.0, 6.0)\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).cut(nine, left(32deg)).speed(64ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected cut shot DSL to build");
    let via_cut_right = parse_dsl_to_scenario(
        "ball cue at center\n\
         ball nine at (2.0, 6.0)\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).cut_right(nine, 18).speed(64ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected cut_right shot DSL to build");

    let cue = via_cut_right
        .game_state
        .select_ball(BallType::Cue)
        .expect("cue ball placement");
    let nine = via_cut_right
        .game_state
        .select_ball(BallType::Nine)
        .expect("nine ball placement");
    let object_heading_degrees = cue.position.angle_to(&nine.position).as_degrees() + 18.0;
    let object_heading = Angle::from_north(
        object_heading_degrees.to_radians().sin(),
        object_heading_degrees.to_radians().cos(),
    );
    let destination = nine.position.translate(Diamond::one(), object_heading);
    let expected_right = nine.aim_angle(
        &destination,
        &cue.position,
        &via_cut_right.game_state.table_spec,
    );

    assert_close(
        via_cut_left
            .shot
            .as_ref()
            .expect("shot")
            .shot
            .heading()
            .as_degrees(),
        via_cut
            .shot
            .as_ref()
            .expect("shot")
            .shot
            .heading()
            .as_degrees(),
    );
    assert_close(
        via_cut_right
            .shot
            .as_ref()
            .expect("shot")
            .shot
            .heading()
            .as_degrees(),
        expected_right.as_degrees(),
    );
}

#[test]
fn shot_scenarios_can_report_human_speed_validation() {
    let scenario = parse_dsl_to_scenario(
        "ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(90deg).speed(128ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected shot DSL to build");

    let validation = scenario
        .validate_shot_human_speed()
        .expect("human speed validation should succeed")
        .expect("scenario should contain a shot");

    assert_eq!(validation.cue_speed_band, HumanShotSpeedBand::MediumFast);
    assert_eq!(
        validation.cue_ball_speed_band,
        HumanShotSpeedBand::MediumFast
    );
    assert_close(
        validation.estimated_cue_ball_speed_after_impact.as_f64(),
        128.0,
    );
    assert!(validation.is_typical_table_shot());
}

#[test]
fn shot_scenarios_can_trace_a_preview_path_through_the_engine() {
    let scenario = parse_dsl_to_scenario(
        "ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(30deg).speed(128ips).tip(side: 0.0R, height: 0.4R).using(default)\n",
    )
    .expect("expected shot DSL to build");

    let path = scenario
        .trace_shot_path_until_rest_with_rails_on_table(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            RailModel::SpinAware,
        )
        .expect("expected shot path trace to succeed")
        .expect("scenario should contain a shot");

    assert!(!path.segments.is_empty(), "expected a visible preview path");
    assert!(path.projected_points(&scenario.game_state.table_spec).len() >= 2);
    assert_eq!(
        path.final_state
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Rest
    );
}

#[test]
fn traced_side_spin_render_paths_sample_within_phase_curvature() {
    let table = billiards::TableSpec::default();
    let ball_set = BallSetPhysicsSpec::default();
    let motion = human_tuned_preview_motion_config();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let state = OnTableBallState::try_from(BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 6.0),
    ))
    .expect("test state should be on-table");
    let path = billiards::trace_ball_path_with_rails_on_table(
        &state,
        BallPathStop::Duration(Seconds::new(2.0)),
        &ball_set,
        &table,
        &motion,
        RailModel::SpinAware,
    );

    let projected = path.projected_points(&table);
    let sampled = path.sampled_points(Seconds::new(0.02), &ball_set, &motion, &table);
    assert!(
        sampled.len() > projected.len(),
        "phase-aware trace sampling should insert within-segment points"
    );

    let point_xy = |point: &billiards::Position| {
        (
            point.x.magnitude.to_f64().expect("point x"),
            point.y.magnitude.to_f64().expect("point y"),
        )
    };
    let distance_to_segment =
        |point: &billiards::Position, start: &billiards::Position, end: &billiards::Position| {
            let (px, py) = point_xy(point);
            let (sx, sy) = point_xy(start);
            let (ex, ey) = point_xy(end);
            let dx = ex - sx;
            let dy = ey - sy;
            let length_squared = dx * dx + dy * dy;
            if length_squared <= f64::EPSILON {
                return ((px - sx).powi(2) + (py - sy).powi(2)).sqrt();
            }
            let u = (((px - sx) * dx + (py - sy) * dy) / length_squared).clamp(0.0, 1.0);
            let closest_x = sx + u * dx;
            let closest_y = sy + u * dy;
            ((px - closest_x).powi(2) + (py - closest_y).powi(2)).sqrt()
        };
    let max_endpoint_chord_deviation = sampled
        .iter()
        .map(|sample| {
            projected
                .windows(2)
                .map(|segment| distance_to_segment(sample, &segment[0], &segment[1]))
                .fold(f64::INFINITY, f64::min)
        })
        .fold(0.0_f64, f64::max);
    assert!(
        max_endpoint_chord_deviation > 1.0e-5,
        "sampled trace points should deviate from endpoint-only chords; max deviation was {max_endpoint_chord_deviation}"
    );

    let path_render = BallPathRenderOptions {
        max_time_step: Seconds::new(0.02),
        ..BallPathRenderOptions::default().with_heading_chevrons(false)
    };
    let path_style = BallPathStyle::new(Rgba([225, 225, 225, 255]));
    let mut rendered_with_sampling = billiards::GameState::new(table.clone());
    rendered_with_sampling.add_rendered_ball_path_styled(
        &path,
        &ball_set,
        &motion,
        &path_render,
        &path_style,
    );

    let mut endpoint_only = billiards::GameState::new(table.clone());
    for segment in &path.segments {
        let points = [
            segment.start.as_ball_state().projected_position(&table),
            segment.end.as_ball_state().projected_position(&table),
        ];
        endpoint_only.add_smooth_polyline_styled(
            &points,
            SmoothPolylineStyle {
                color: Rgba([225, 225, 225, 255]),
                width_px: path_render.width_px,
                ..SmoothPolylineStyle::new(Rgba([225, 225, 225, 255]))
            },
        );
    }

    assert_ne!(
        render_png(&rendered_with_sampling),
        render_png(&endpoint_only),
        "rendered trace path should not collapse to endpoint-only segment chords"
    );
}

#[test]
fn shot_scenarios_can_use_named_ball_ball_configs_defined_in_dsl() {
    let scenario = parse_dsl_to_scenario(
        "ball cue at (2.0, 3.0)\n\
         ball one at (2.0, 4.0)\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         ball_ball(ideal).normal_restitution(1.0).tangential_friction(0.06)\n\
         ball_ball(human).normal_restitution(0.95).tangential_friction(0.06)\n\
         shot(cue).heading(0deg).speed(128ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected shot DSL to build");
    let initial = scenario
        .initial_shot_system_states_on_table(&BallSetPhysicsSpec::default())
        .expect("expected initial shot states to build")
        .expect("scenario should contain a shot");
    let ideal = advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table(
        &initial,
        &BallSetPhysicsSpec::default(),
        &scenario.game_state.table_spec,
        &motion_config(),
        CollisionModel::Ideal,
        scenario
            .ball_ball_config_named("ideal")
            .expect("ideal ball-ball config should exist"),
        RailModel::SpinAware,
        &RailCollisionProfile::default(),
    )
    .expect("DSL scenario geometry should validate");
    let damped = advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table(
        &initial,
        &BallSetPhysicsSpec::default(),
        &scenario.game_state.table_spec,
        &motion_config(),
        CollisionModel::Ideal,
        scenario
            .ball_ball_config_named("human")
            .expect("human ball-ball config should exist"),
        RailModel::SpinAware,
        &RailCollisionProfile::default(),
    )
    .expect("DSL scenario geometry should validate");

    let ideal_object_speed = match &ideal.states[1] {
        NBallSystemState::OnTable(state) => state.as_ball_state().speed().as_f64(),
        other => panic!("expected object ball to remain on-table, got {other:?}"),
    };
    let damped_object_speed = match &damped.states[1] {
        NBallSystemState::OnTable(state) => state.as_ball_state().speed().as_f64(),
        other => panic!("expected object ball to remain on-table, got {other:?}"),
    };

    assert!(
        damped_object_speed < ideal_object_speed,
        "lower ball-ball restitution should reduce the struck ball's immediate post-collision speed"
    );
}

#[test]
fn overlapping_shot_balls_report_named_dsl_geometry_error() {
    let scenario = parse_dsl_to_scenario(
        "ball cue at (2.0, 3.0)\n\
         ball one at (2.0, 3.0)\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(0deg).speed(16ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected overlapping shot scenario DSL to build");

    let error = scenario
        .initial_shot_system_states_on_table(&BallSetPhysicsSpec::default())
        .expect_err("overlapping shot balls should be rejected");
    let DslBuildError::InvalidNBallGeometry {
        first_ball,
        second_ball,
        ..
    } = error
    else {
        panic!("expected named N-ball geometry error, got {error:?}");
    };

    assert_eq!((first_ball, second_ball), (BallType::Cue, BallType::One));
}

#[test]
fn rejects_out_of_range_ball_ball_restitution() {
    let err = parse_dsl_to_scenario(
        "ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         ball_ball(human).normal_restitution(1.1).tangential_friction(0.06)\n\
         shot(cue).heading(90deg).speed(128ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect_err("expected build failure");

    assert!(matches!(
        err,
        DslError::Build(DslBuildError::InvalidPhysicsConfigValue {
            name,
            method,
            ..
        }) if name == "human" && method == "normal_restitution"
    ));
}

#[test]
fn shot_scenarios_can_use_named_rail_profiles_defined_in_dsl() {
    let scenario = parse_dsl_to_scenario(
        "ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         rail_response(clean).normal_restitution(0.8).tangential_friction(1.0)\n\
         rail_response(dead).normal_restitution(0.6).tangential_friction(1.0)\n\
         rails(lively).default(clean)\n\
         rails(dead_banks).default(clean).right(dead).top(dead)\n\
         shot(cue).heading(30deg).speed(128ips).tip(side: 0.0R, height: 0.4R).using(default)\n",
    )
    .expect("expected shot DSL to build");
    let default_path = scenario
        .trace_shot_path_with_rail_profile_on_table(
            billiards::BallPathStop::Duration(billiards::Seconds::new(1.0)),
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            RailModel::SpinAware,
            scenario
                .rail_profile_named("lively")
                .expect("lively rail profile should exist"),
        )
        .expect("default profile trace should succeed")
        .expect("scenario should contain a shot");
    let dead_path = scenario
        .trace_shot_path_with_rail_profile_on_table(
            billiards::BallPathStop::Duration(billiards::Seconds::new(1.0)),
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            RailModel::SpinAware,
            scenario
                .rail_profile_named("dead_banks")
                .expect("dead_banks rail profile should exist"),
        )
        .expect("dead profile trace should succeed")
        .expect("scenario should contain a shot");

    assert!(
        dead_path.final_state.as_ball_state().speed().as_f64()
            < default_path.final_state.as_ball_state().speed().as_f64(),
        "deader rails should leave the cue ball carrying less rebound speed"
    );
    assert!(
        dead_path.rail_impacts <= default_path.rail_impacts,
        "deader rails should not create more rail contacts within the same preview horizon"
    );
}

#[test]
fn rejects_rails_profiles_that_reference_unknown_rail_responses() {
    let err = parse_dsl_to_scenario(
        "ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         rails(dead_banks).default(clean).right(dead)\n\
         shot(cue).heading(30deg).speed(128ips).tip(side: 0.0R, height: 0.4R).using(default)\n",
    )
    .expect_err("expected build failure");

    assert!(matches!(
        err,
        DslError::Build(DslBuildError::UnknownRailResponse(name)) if name == "clean"
    ));
}

#[test]
fn shot_scenarios_can_use_named_simulations_defined_in_dsl() {
    let scenario = parse_dsl_to_scenario(
        "ball cue at (2.0, 3.0)\n\
         ball one at (2.0, 4.0)\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         ball_ball(ideal).normal_restitution(1.0).tangential_friction(0.06)\n\
         ball_ball(human).normal_restitution(0.95).tangential_friction(0.06)\n\
         rail_response(clean).normal_restitution(0.8).tangential_friction(1.0)\n\
         rails(table).default(clean)\n\
         simulation(ideal_table).collision_model(ideal).ball_ball(ideal).rail_model(spin_aware).rails(table)\n\
         simulation(human_table).collision_model(ideal).ball_ball(human).rail_model(spin_aware).rails(table)\n\
         shot(cue).heading(0deg).speed(128ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected shot DSL to build");
    let ideal = scenario
        .simulate_shot_system_with_simulation_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            "ideal_table",
        )
        .expect("expected ideal simulation to succeed")
        .expect("scenario should contain a shot");
    let damped = scenario
        .simulate_shot_system_with_simulation_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            "human_table",
        )
        .expect("expected damped simulation to succeed")
        .expect("scenario should contain a shot");

    let preset = scenario
        .simulation_named("human_table")
        .expect("named simulation");
    assert_eq!(preset.ball_ball_name, "human");
    assert_eq!(preset.conditions, PlayingConditions::neutral());

    let ideal_object_y = match &ideal.states[1] {
        NBallSystemState::OnTable(state) => state.as_ball_state().position.y().as_f64(),
        other => panic!("expected object ball to remain on-table, got {other:?}"),
    };
    let damped_object_y = match &damped.states[1] {
        NBallSystemState::OnTable(state) => state.as_ball_state().position.y().as_f64(),
        other => panic!("expected object ball to remain on-table, got {other:?}"),
    };

    assert!(
        (damped_object_y - ideal_object_y).abs() > 1.0,
        "the simulation preset should thread the named ball-ball config into the engine"
    );
}

#[test]
fn shot_scenarios_can_apply_named_playing_conditions_from_simulation_presets() {
    let scenario = parse_dsl_to_scenario(
        "ball cue at (2.0, 3.0)\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         ball_ball(human).normal_restitution(0.95).tangential_friction(0.06)\n\
         rail_response(clean).normal_restitution(0.8).tangential_friction(1.0)\n\
         rails(table).default(clean)\n\
         simulation(neutral_table).collision_model(throw_aware).ball_ball(human).rail_model(spin_aware).rails(table)\n\
         simulation(humid_table).collision_model(throw_aware).ball_ball(human).rail_model(spin_aware).rails(table).conditions(humid_dirty)\n\
         shot(cue).heading(0deg).speed(20ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected shot DSL to build");
    let neutral = scenario
        .simulate_shot_system_with_simulation_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            "neutral_table",
        )
        .expect("expected neutral simulation to succeed")
        .expect("scenario should contain a shot");
    let humid = scenario
        .simulate_shot_system_with_simulation_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            "humid_table",
        )
        .expect("expected humid simulation to succeed")
        .expect("scenario should contain a shot");

    assert_eq!(
        scenario
            .simulation_named("humid_table")
            .expect("named simulation")
            .conditions,
        PlayingConditions::humid_dirty()
    );

    let neutral_cue_y = match &neutral.states[0] {
        NBallSystemState::OnTable(state) => state.as_ball_state().position.y().as_f64(),
        other => panic!("expected cue ball to remain on-table, got {other:?}"),
    };
    let humid_cue_y = match &humid.states[0] {
        NBallSystemState::OnTable(state) => state.as_ball_state().position.y().as_f64(),
        other => panic!("expected cue ball to remain on-table, got {other:?}"),
    };

    assert!(
        humid_cue_y < neutral_cue_y - 0.5,
        "humid conditions should shorten cue-ball travel before any rail contact; got neutral y {neutral_cue_y} vs humid y {humid_cue_y}"
    );
}

#[test]
fn a_single_named_simulation_becomes_the_preferred_cli_physics_path() {
    let scenario = parse_dsl_to_scenario(
        "ball cue at (2.0, 3.0)\n\
         ball one at (2.18, 4.12)\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         ball_ball(human).normal_restitution(0.95).tangential_friction(0.06)\n\
         rail_response(clean).normal_restitution(0.7).tangential_friction(0.17)\n\
         rails(table).default(clean)\n\
         simulation(human_table).collision_model(throw_aware).ball_ball(human).rail_model(spin_aware).rails(table)\n\
         shot(cue).heading(9deg).speed(128ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected shot DSL to build");
    let explicit = scenario
        .simulate_shot_trace_with_simulation_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            "human_table",
        )
        .expect("explicit named simulation should succeed");
    let preferred = scenario
        .simulate_shot_trace_with_preferred_physics_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        )
        .expect("preferred simulation should succeed");

    assert_eq!(scenario.preferred_simulation_name(), Some("human_table"));
    assert_eq!(preferred, explicit);
}

#[test]
fn preferred_trace_can_stop_at_an_event_limit() {
    let scenario = parse_dsl_to_scenario(
        "ball cue at (2.0, 3.0)\n\
         ball one at (2.18, 4.12)\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         ball_ball(human).normal_restitution(0.95).tangential_friction(0.06)\n\
         rail_response(clean).normal_restitution(0.7).tangential_friction(0.17)\n\
         rails(table).default(clean)\n\
         simulation(human_table).collision_model(throw_aware).ball_ball(human).rail_model(spin_aware).rails(table)\n\
         shot(cue).heading(9deg).speed(128ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected shot DSL to build");
    let full = scenario
        .simulate_shot_trace_with_preferred_physics_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        )
        .expect("full preferred trace should succeed")
        .expect("scenario should contain a shot");
    let limited = scenario
        .simulate_shot_trace_with_preferred_physics_on_table_until_event_limit(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
            1,
        )
        .expect("limited preferred trace should succeed")
        .expect("scenario should contain a shot");

    assert!(
        full.simulation.events.len() > limited.simulation.events.len(),
        "test fixture should produce more than one full simulation event"
    );
    assert_eq!(limited.simulation.events.len(), 1);
    assert_eq!(limited.event_log.len(), 1);
    assert!(limited.simulation.elapsed.as_f64() <= full.simulation.elapsed.as_f64());
}

#[test]
fn named_simulation_can_default_preferred_trace_to_an_event_limit() {
    let scenario = parse_dsl_to_scenario(
        "ball cue at (2.0, 3.0)\n\
         ball one at (2.18, 4.12)\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         ball_ball(human).normal_restitution(0.95).tangential_friction(0.06)\n\
         rail_response(clean).normal_restitution(0.7).tangential_friction(0.17)\n\
         rails(table).default(clean)\n\
         simulation(human_table).collision_model(throw_aware).ball_ball(human).rail_model(spin_aware).rails(table).max_events(1)\n\
         shot(cue).heading(9deg).speed(128ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected shot DSL to build");
    let trace = scenario
        .simulate_shot_trace_with_preferred_physics_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        )
        .expect("preferred trace should succeed")
        .expect("scenario should contain a shot");

    assert_eq!(
        scenario
            .simulation_named("human_table")
            .expect("named simulation")
            .max_events,
        Some(1)
    );
    assert_eq!(trace.simulation.events.len(), 1);
    assert_eq!(trace.event_log.len(), 1);
}

#[test]
fn rejects_unknown_playing_conditions_presets_in_simulations() {
    let err = parse_dsl_to_scenario(
        "ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         ball_ball(human).normal_restitution(0.95).tangential_friction(0.06)\n\
         rail_response(clean).normal_restitution(0.8).tangential_friction(1.0)\n\
         rails(table).default(clean)\n\
         simulation(trace).collision_model(throw_aware).ball_ball(human).rail_model(spin_aware).rails(table).conditions(swampy)\n\
         shot(cue).heading(90deg).speed(128ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect_err("expected build failure");

    assert!(matches!(
        err,
        DslError::Build(DslBuildError::UnknownPlayingConditionsPreset(name)) if name == "swampy"
    ));
}

#[test]
fn rejects_simulations_that_reference_unknown_named_physics() {
    let err = parse_dsl_to_scenario(
        "ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         simulation(trace).collision_model(throw_aware).ball_ball(ideal).rail_model(spin_aware).rails(table)\n\
         shot(cue).heading(90deg).speed(128ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect_err("expected build failure");

    assert!(matches!(
        err,
        DslError::Build(DslBuildError::UnknownBallBallConfig(name)) if name == "ideal"
    ));
}

#[test]
fn shot_scenarios_can_build_a_typed_trace_and_render_the_final_layout_with_ball_traces() {
    let scenario = parse_dsl_to_scenario(
        "ball cue at center\n\
         ball one at rack\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         ball_ball(ideal).normal_restitution(1.0).tangential_friction(0.06)\n\
         rail_response(clean).normal_restitution(0.8).tangential_friction(1.0)\n\
         rails(table).default(clean)\n\
         simulation(trace).collision_model(throw_aware).ball_ball(ideal).rail_model(spin_aware).rails(table)\n\
         shot(cue).heading(90deg).speed(128ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected shot DSL to build");

    let trace = scenario
        .simulate_shot_trace_with_simulation_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            "trace",
        )
        .expect("expected full traced system simulation to succeed")
        .expect("scenario should contain a shot");
    let rendered =
        trace.rendered_final_layout_with_traces(&scenario, billiards::Seconds::new(0.02));
    let rendered_via_default_options = trace.rendered_final_layout_with_trace_options(
        &scenario,
        &ScenarioTraceRenderOptions {
            path_render: BallPathRenderOptions {
                max_time_step: billiards::Seconds::new(0.02),
                ..ScenarioTraceRenderOptions::default().path_render
            },
            ..ScenarioTraceRenderOptions::default()
        },
    );
    let rendered_with_rich_overlays = trace.rendered_final_layout_with_trace_options(
        &scenario,
        &ScenarioTraceRenderOptions {
            labels: true,
            path_color_mode: PathColorMode::FadeByTime,
            ..ScenarioTraceRenderOptions::rich_defaults()
        },
    );

    assert_eq!(
        render_png(&rendered),
        render_png(&rendered_via_default_options)
    );
    assert_ne!(
        render_png(&rendered),
        render_png(&rendered_with_rich_overlays)
    );

    assert!(matches!(
        trace.event_log.as_slice(),
        [billiards::dsl::ScenarioShotTraceEvent {
            kind: billiards::dsl::ScenarioShotTraceEventKind::BallPocketCapture { ball, pocket },
            ..
        }] if *ball == BallType::Cue && *pocket == billiards::Pocket::CenterRight
    ));
    assert_eq!(trace.event_lines().len(), 1);
    assert!(trace.event_lines()[0].contains("cue pocketed in center-right"));
    assert!(trace.ball_traces[0].segments.iter().any(|segment| {
        segment.event_marker_at_end && segment.event_marker_label.as_deref() == Some("(1)")
    }));
    assert_eq!(trace.ball_traces.len(), 2);
    assert!(!trace.ball_traces[0].segments.is_empty());
    assert!(trace.ball_traces[1].segments.is_empty());
    let projected = trace.ball_traces[0].projected_points(&scenario.game_state.table_spec);
    let sampled = trace.ball_traces[0].sampled_points(
        billiards::Seconds::new(0.02),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        &scenario.game_state.table_spec,
    );
    let pocket_center = Pocket::CenterRight.aiming_center();
    assert!(projected.len() >= 3);
    assert!(sampled.len() >= 3);
    assert_close(
        projected
            .last()
            .expect("projected points should not be empty")
            .x
            .magnitude
            .to_f64()
            .expect("projected x"),
        pocket_center.x.magnitude.to_f64().expect("pocket x"),
    );
    assert_close(
        projected
            .last()
            .expect("projected points should not be empty")
            .y
            .magnitude
            .to_f64()
            .expect("projected y"),
        pocket_center.y.magnitude.to_f64().expect("pocket y"),
    );
    assert_close(
        sampled
            .last()
            .expect("sampled points should not be empty")
            .x
            .magnitude
            .to_f64()
            .expect("sampled x"),
        pocket_center.x.magnitude.to_f64().expect("pocket x"),
    );
    assert_close(
        sampled
            .last()
            .expect("sampled points should not be empty")
            .y
            .magnitude
            .to_f64()
            .expect("sampled y"),
        pocket_center.y.magnitude.to_f64().expect("pocket y"),
    );
    assert!(trace.simulation.events.iter().any(|event| matches!(
        event,
        NBallSystemEvent::BallPocketCapture {
            ball_index: 0,
            capture,
        } if capture.pocket == billiards::Pocket::CenterRight
    )));
    match &trace.simulation.states[0] {
        NBallSystemState::Pocketed { pocket, .. } => {
            assert_eq!(*pocket, billiards::Pocket::CenterRight)
        }
        other => panic!("expected cue ball to be pocketed, got {other:?}"),
    }
    match &trace.simulation.states[1] {
        NBallSystemState::OnTable(state) => assert_eq!(
            state
                .as_ball_state()
                .motion_phase(TYPICAL_BALL_RADIUS.clone()),
            MotionPhase::Rest
        ),
        other => panic!("expected the object ball to remain on the table, got {other:?}"),
    }
    assert!(
        rendered.select_ball(BallType::Cue).is_none(),
        "pocketed balls should not appear in the rendered final layout"
    );
    assert!(rendered.select_ball(BallType::One).is_some());
    assert_eq!(rendered.balls().len(), 1);
}

#[test]
fn shot_scenarios_still_build_plain_game_state_views() {
    let state = parse_dsl_to_game_state(
        "ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(30deg).speed(128ips).tip(side: 0.0R, height: 0.4R).using(default)\n",
    )
    .expect("expected shot DSL to still build a game-state view");

    assert!(state.select_ball(BallType::Cue).is_some());
}

#[test]
fn scenario_trace_can_limit_rendered_simulation_events() {
    let scenario = parse_dsl_to_scenario(
        "trace(max_events: 8)\n\
         ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(0deg).speed(64ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected shot DSL to build");

    assert_eq!(scenario.trace_max_events, Some(8));
}

#[test]
fn playback_frames_snap_to_logged_event_times_and_sample_between_them() {
    let scenario = parse_dsl_to_scenario(
        "ball cue at (2.0, 3.0)\n\
         ball one at (2.18, 4.12)\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         ball_ball(human).normal_restitution(0.95).tangential_friction(0.06)\n\
         rail_response(clean).normal_restitution(0.7).tangential_friction(0.17)\n\
         rails(table).default(clean)\n\
         simulation(human_table).collision_model(throw_aware).ball_ball(human).rail_model(spin_aware).rails(table).max_events(4)\n\
         shot(cue).heading(9deg).speed(128ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected shot DSL to build");
    let trace = scenario
        .simulate_shot_trace_with_simulation_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            "human_table",
        )
        .expect("preferred trace should succeed")
        .expect("scenario should contain a shot");

    let max_time_step = Seconds::new(0.02);
    let frames = trace.playback_frames(max_time_step);

    assert!(!frames.is_empty());
    assert_close(frames[0].time.as_f64(), 0.0);
    assert_close(
        frames.last().expect("final playback frame").time.as_f64(),
        trace.simulation.elapsed.as_f64(),
    );
    for event in &trace.event_log {
        assert!(
            frames
                .iter()
                .any(|frame| (frame.time.as_f64() - event.time.as_f64()).abs() < 1e-9),
            "playback frames should include logged event time {:.9}",
            event.time.as_f64()
        );
    }
    for window in frames.windows(2) {
        let gap = window[1].time.as_f64() - window[0].time.as_f64();
        assert!(
            gap <= max_time_step.as_f64() + 1e-9,
            "playback frame gap {gap} should not exceed configured step"
        );
    }

    let initial_cue = &trace
        .ball_traces
        .iter()
        .find(|ball_trace| ball_trace.ball == BallType::Cue)
        .expect("cue trace")
        .initial_state;
    let moved_cue = &frames
        .iter()
        .find(|frame| frame.time.as_f64() > 0.0)
        .and_then(|frame| frame.balls.iter().find(|ball| ball.ball == BallType::Cue))
        .expect("sampled cue after shot starts")
        .state;
    assert!(
        (moved_cue.position.x().as_f64() - initial_cue.position.x().as_f64()).abs() > 1e-9
            || (moved_cue.position.y().as_f64() - initial_cue.position.y().as_f64()).abs() > 1e-9,
        "playback should expose sub-event physics samples, not only event vertices"
    );
}

#[test]
fn playback_frames_omit_pocketed_balls_after_their_capture_time() {
    let initial_state = OnTableBallState::try_from(BallState::on_table(
        Inches2::new("20", "20"),
        Velocity2::new("10", "0"),
        AngularVelocity3::new(0.0, 0.0, 0.0),
    ))
    .expect("initial state should be on-table");
    let captured_state = OnTableBallState::try_from(BallState::on_table(
        Inches2::new("30", "20"),
        Velocity2::new("0", "0"),
        AngularVelocity3::new(0.0, 0.0, 0.0),
    ))
    .expect("captured state should be on-table");
    let final_state = NBallSystemState::Pocketed {
        pocket: Pocket::CenterRight,
        state_at_capture: captured_state.clone(),
    };
    let trace = ScenarioShotTrace {
        simulation: NBallSystemSimulation {
            states: vec![final_state.clone()],
            elapsed: Seconds::new(2.0),
            events: Vec::new(),
        },
        event_log: Vec::new(),
        ball_traces: vec![ScenarioBallTrace {
            ball: BallType::Cue,
            initial_state: initial_state.clone().into(),
            final_state,
            segments: Vec::new(),
            timeline_segments: vec![ScenarioBallTimelineSegment {
                start_time: Seconds::zero(),
                start: initial_state.into(),
                end: captured_state.into(),
                duration: Seconds::new(1.0),
            }],
        }],
        ball_set: BallSetPhysicsSpec::default(),
        motion: motion_config(),
    };

    let frames = trace.playback_frames(Seconds::new(0.5));
    let frame_before_capture = frames
        .iter()
        .find(|frame| (frame.time.as_f64() - 0.5).abs() < 1e-9)
        .expect("pre-capture frame");
    let frame_after_capture = frames
        .iter()
        .find(|frame| (frame.time.as_f64() - 2.0).abs() < 1e-9)
        .expect("post-capture frame");

    assert!(frame_before_capture
        .balls
        .iter()
        .any(|ball| ball.ball == BallType::Cue));
    assert!(
        frame_after_capture.balls.is_empty(),
        "pocketed balls should disappear after capture instead of snapping back onto the table"
    );
}

#[test]
fn parses_elevated_cue_method_and_jump_alias() {
    let scenario = parse_dsl_to_scenario(
        "ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(30deg).speed(128ips).tip(side: 0.0R, height: 0.4R).elevation(15deg).using(default)\n",
    )
    .expect("elevated shot DSL should build");
    let shot = scenario
        .shot
        .as_ref()
        .expect("scenario should contain a shot");
    assert_close(shot.shot.cue_elevation().as_degrees(), 15.0);

    let default_jump = parse_dsl_to_scenario(
        "ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(30deg).speed(128ips).tip(side: 0.0R, height: 0.0R).jump().using(default)\n",
    )
    .expect("jump alias DSL should build");
    assert_close(
        default_jump
            .shot
            .as_ref()
            .expect("scenario should contain a shot")
            .shot
            .cue_elevation()
            .as_degrees(),
        45.0,
    );

    let tuned_jump = parse_dsl_to_scenario(
        "ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(30deg).speed(128ips).tip(side: 0.0R, height: 0.0R).jump(32deg).using(default)\n",
    )
    .expect("parameterized jump alias DSL should build");
    assert_close(
        tuned_jump
            .shot
            .as_ref()
            .expect("scenario should contain a shot")
            .shot
            .cue_elevation()
            .as_degrees(),
        32.0,
    );

    assert_parse_error(
        "ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(30deg).speed(128ips).tip(side: 0.0R, height: 0.4R).masse(30deg).using(default)\n",
    );
}

#[test]
fn side_english_dsl_derives_rail_clearance_elevation_unless_explicitly_level() {
    let derived = parse_dsl_to_scenario(
        "ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(30deg).speed(128ips).tip(side: 0.25R, height: 0.0R).using(default)\n",
    )
    .expect("side-English shot should build");
    assert_close(
        derived
            .shot
            .as_ref()
            .expect("scenario should contain a shot")
            .shot
            .cue_elevation()
            .as_degrees(),
        1.384,
    );

    let center_ball = parse_dsl_to_scenario(
        "ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(30deg).speed(128ips).tip(side: 0.0R, height: 0.3R).using(default)\n",
    )
    .expect("center-ball shot should build");
    assert_close(
        center_ball
            .shot
            .as_ref()
            .expect("scenario should contain a shot")
            .shot
            .cue_elevation()
            .as_degrees(),
        0.0,
    );

    let explicit_level = parse_dsl_to_scenario(
        "ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(30deg).speed(128ips).tip(side: -0.25R, height: 0.0R).elevation(0deg).using(default)\n",
    )
    .expect("explicitly level side-English shot should build");
    assert_close(
        explicit_level
            .shot
            .as_ref()
            .expect("scenario should contain a shot")
            .shot
            .cue_elevation()
            .as_degrees(),
        0.0,
    );
}

#[test]
fn rejects_a_shot_that_uses_an_unknown_cue_strike() {
    let err = parse_dsl_to_scenario(
        "ball cue at center\n\
         shot(cue).heading(30deg).speed(128ips).tip(side: 0.0R, height: 0.4R).using(default)\n",
    )
    .expect_err("expected build failure");

    assert!(matches!(
        err,
        DslError::Build(DslBuildError::UnknownCueStrike(name)) if name == "default"
    ));
}

#[test]
fn rejects_a_non_cue_shot_target_in_v1() {
    let err = parse_dsl_to_scenario(
        "ball nine at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(nine).heading(30deg).speed(128ips).tip(side: 0.0R, height: 0.4R).using(default)\n",
    )
    .expect_err("expected build failure");

    assert!(matches!(
        err,
        DslError::Build(DslBuildError::ShotTargetMustBeCueBall(_))
    ));
}

#[test]
fn rejects_missing_required_shot_methods() {
    let err = parse_dsl_to_scenario(
        "ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(30deg).speed(128ips).tip(side: 0.0R, height: 0.4R)\n",
    )
    .expect_err("expected build failure");

    assert!(matches!(
        err,
        DslError::Build(DslBuildError::MissingShotMethod { method }) if method == "using"
    ));
}

#[test]
fn rejects_duplicate_shot_methods() {
    let err = parse_dsl_to_scenario(
        "ball cue at center\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(30deg).heading(45deg).speed(128ips).tip(side: 0.0R, height: 0.4R).using(default)\n",
    )
    .expect_err("expected build failure");

    assert!(matches!(
        err,
        DslError::Build(DslBuildError::DuplicateShotMethod { method }) if method == "heading"
    ));
}
