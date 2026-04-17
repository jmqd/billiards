use bigdecimal::ToPrimitive;
use billiards::dsl::{
    parse_dsl, parse_dsl_to_game_state, parse_dsl_to_scenario, CoordinateAxis, DslBuildError,
    DslError, DslParseError, RailSide,
};
use billiards::{
    BallSetPhysicsSpec, BallType, CollisionModel, InchesPerSecondSq, MotionPhase,
    MotionPhaseConfig, MotionTransitionConfig, NBallSystemEvent, NBallSystemState,
    OnTableMotionConfig, RadiansPerSecondSq, RailModel, RollingResistanceModel,
    SlidingFrictionModel, SpinDecayModel, TYPICAL_BALL_RADIUS,
};

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
    assert_close(shot.shot.cue_speed().as_f64(), 128.0);
    assert_close(shot.shot.tip_contact().side_offset().as_f64(), 0.0);
    assert_close(shot.shot.tip_contact().height_offset().as_f64(), 0.4);
    assert_close(shot.cue_strike.cue_mass_ratio().as_f64(), 1.0);
    assert_close(shot.cue_strike.collision_energy_loss().as_f64(), 0.1);
    assert_eq!(
        seeded
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Rolling
    );
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
fn shot_scenarios_can_build_a_typed_trace_and_render_the_final_layout_with_ball_traces() {
    let scenario = parse_dsl_to_scenario(
        "ball cue at center\n\
         ball one at rack\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(90deg).speed(128ips).tip(side: 0.0R, height: 0.0R).using(default)\n",
    )
    .expect("expected shot DSL to build");

    let trace = scenario
        .simulate_shot_trace_with_rails_and_pockets_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        )
        .expect("expected full traced system simulation to succeed")
        .expect("scenario should contain a shot");
    let rendered = trace.rendered_final_layout_with_traces(
        &scenario,
        billiards::Seconds::new(0.02),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
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
    assert_eq!(trace.ball_traces.len(), 2);
    assert!(!trace.ball_traces[0].segments.is_empty());
    assert!(trace.ball_traces[1].segments.is_empty());
    assert!(
        trace.ball_traces[0]
            .sampled_points(
                billiards::Seconds::new(0.02),
                &BallSetPhysicsSpec::default(),
                &motion_config(),
                &scenario.game_state.table_spec,
            )
            .len()
            >= 2
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
