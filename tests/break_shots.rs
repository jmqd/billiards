use std::collections::HashMap;

use billiards::dsl::{parse_dsl_to_scenario, BallRef, DslScenario, ScenarioShot};
use billiards::{
    advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table, rack_9_ball, Ball,
    BallSetPhysicsSpec, BallSpec, BallType, CollisionModel, CueStrikeConfig, CueTipContact,
    GameState, Inches, InchesPerSecond, InchesPerSecondSq, MotionPhaseConfig,
    MotionTransitionConfig, NBallSystemEvent, NBallSystemState, OnTableMotionConfig, Position,
    RadiansPerSecondSq, RailModel, RollingResistanceModel, Scale, Shot, SlidingFrictionModel,
    SpinDecayModel, TableSpec,
};

fn motion_config() -> OnTableMotionConfig {
    MotionTransitionConfig {
        phase: MotionPhaseConfig::default(),
        sliding_friction: SlidingFrictionModel::ConstantAcceleration {
            acceleration_magnitude: InchesPerSecondSq::new("15"),
        },
        spin_decay: SpinDecayModel::ConstantAngularDeceleration {
            angular_deceleration: RadiansPerSecondSq::new(10.9),
        },
        rolling_resistance: RollingResistanceModel::ConstantDeceleration {
            linear_deceleration: InchesPerSecondSq::new("5"),
        },
    }
}

fn default_cue_strike() -> CueStrikeConfig {
    CueStrikeConfig::new(Scale::from_f64(1.0), Scale::from_f64(0.1))
        .expect("default cue-strike config should validate")
}

fn head_rail_break_cue_position() -> Position {
    let mut cue_position = Position::new(2u8, 8u8);
    cue_position.shift_vertically_inches(Inches::from_f64(-4.0));
    cue_position.resolve_shifts(&TableSpec::default());
    cue_position
}

fn left_side_rail_break_cue_position() -> Position {
    let mut cue_position = Position::new(0u8, 6u8);
    cue_position.shift_horizontally_inches(Inches::from_f64(4.0));
    cue_position.resolve_shifts(&TableSpec::default());
    cue_position
}

fn build_nine_ball_break_scenario(cue_position: Position) -> DslScenario {
    let table = TableSpec::default();
    let rack = rack_9_ball();
    let one_ball = rack
        .iter()
        .find(|ball| ball.ty == BallType::One)
        .expect("rack_9_ball should include the 1-ball");
    let shot = Shot::new(
        cue_position.angle_to(&one_ball.position),
        InchesPerSecond::from_mph(20.0),
        CueTipContact::new(Scale::zero(), Scale::from_f64(-0.1))
            .expect("slight draw tip contact should validate"),
    )
    .expect("break shot should validate");

    let mut balls = Vec::with_capacity(rack.len() + 1);
    balls.push(Ball {
        ty: BallType::Cue,
        position: cue_position,
        spec: BallSpec::default(),
    });
    balls.extend(rack);

    DslScenario {
        game_state: GameState::with_balls(table, balls),
        shot: Some(ScenarioShot {
            ball_ref: BallRef::Cue,
            ball: BallType::Cue,
            shot,
            cue_strike: default_cue_strike(),
        }),
        ball_ball_configs: HashMap::new(),
        rail_responses: HashMap::new(),
        rail_profiles: HashMap::new(),
        simulations: HashMap::new(),
    }
}

fn assert_position_matches_helper_geometry(label: &str, actual: &Position, expected: &Position) {
    let delta_inches = TableSpec::default()
        .diamond_to_inches(actual.displacement(expected).absolute_distance())
        .as_f64();
    assert!(
        delta_inches < 1e-9,
        "{label} drifted from helper geometry by {delta_inches} inches: actual={actual:?}, expected={expected:?}"
    );
}

fn assert_heading_tracks_one_ball(label: &str, scenario: &DslScenario) {
    let cue_ball = scenario
        .game_state
        .balls()
        .iter()
        .find(|ball| ball.ty == BallType::Cue)
        .expect("scenario should include the cue ball");
    let one_ball = scenario
        .game_state
        .balls()
        .iter()
        .find(|ball| ball.ty == BallType::One)
        .expect("scenario should include the 1-ball");
    let actual = scenario
        .shot
        .as_ref()
        .expect("scenario should include a shot")
        .shot
        .heading()
        .as_degrees();
    let expected = cue_ball.position.angle_to(&one_ball.position).as_degrees();
    let delta = (actual - expected).abs().rem_euclid(360.0);
    let delta = delta.min(360.0 - delta);
    assert!(
        delta < 1e-6,
        "{label} shot heading drifted away from the cue→1-ball line by {delta} degrees: actual={actual}, expected={expected}"
    );
}

fn assert_break_example_matches_helper_geometry(
    label: &str,
    input: &str,
    expected_cue_position: Position,
) {
    let scenario = parse_dsl_to_scenario(input).expect("break example should parse");
    assert_eq!(
        scenario.game_state.balls().len(),
        10,
        "{label} should contain a cue ball plus a full nine-ball rack"
    );

    let cue_ball = scenario
        .game_state
        .balls()
        .iter()
        .find(|ball| ball.ty == BallType::Cue)
        .expect("scenario should include the cue ball");
    assert_position_matches_helper_geometry(
        &format!("{label} cue position"),
        &cue_ball.position,
        &expected_cue_position,
    );

    for expected_ball in rack_9_ball() {
        let actual_ball = scenario
            .game_state
            .balls()
            .iter()
            .find(|ball| ball.ty == expected_ball.ty)
            .unwrap_or_else(|| panic!("{label} should include {:?}", expected_ball.ty));
        assert_position_matches_helper_geometry(
            &format!("{label} {:?} position", expected_ball.ty),
            &actual_ball.position,
            &expected_ball.position,
        );
    }

    assert_heading_tracks_one_ball(label, &scenario);
}

fn simulate_break_trace(scenario: &DslScenario) -> billiards::dsl::ScenarioShotTrace {
    scenario
        .simulate_shot_trace_with_rails_and_pockets_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &motion_config(),
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        )
        .expect("break scenario should simulate")
        .expect("break scenario should include a shot")
}

fn opening_break_summary(scenario: &DslScenario) -> ((usize, usize), usize, usize) {
    let ball_set = BallSetPhysicsSpec::default();
    let mut states = scenario
        .initial_shot_system_states_on_table(&ball_set)
        .expect("break scenario should seed initial states")
        .expect("break scenario should include a shot")
        .into_iter()
        .map(NBallSystemState::from)
        .collect::<Vec<_>>();
    let mut events = Vec::new();
    let mut elapsed = 0.0;
    let mut positive_time_steps = 0;
    let mut total_steps = 0;
    while positive_time_steps < 10 && elapsed < 0.5 && total_steps < 200 {
        let advance = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
            &states,
            &ball_set,
            &scenario.game_state.table_spec,
            &motion_config(),
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        );
        let Some(event) = advance.event else {
            break;
        };
        let step_elapsed = advance.elapsed.as_f64();
        elapsed += step_elapsed;
        if step_elapsed > f64::EPSILON {
            positive_time_steps += 1;
        }
        total_steps += 1;
        events.push(event);
        states = advance.states;
    }

    let first_ball_collision = events
        .iter()
        .find_map(|event| match event {
            NBallSystemEvent::BallBallCollision {
                first_ball_index,
                second_ball_index,
                ..
            } => Some((*first_ball_index, *second_ball_index)),
            _ => None,
        })
        .expect("break should include at least one ball-ball collision");
    let ball_ball_collision_count = events
        .iter()
        .filter(|event| matches!(event, NBallSystemEvent::BallBallCollision { .. }))
        .count();
    let moved_or_pocketed_ball_count = scenario
        .game_state
        .balls()
        .iter()
        .zip(&states)
        .filter(|(ball, final_state)| match final_state {
            NBallSystemState::Pocketed { .. } => true,
            NBallSystemState::OnTable(state) => {
                let final_position = state
                    .as_ball_state()
                    .projected_position(&scenario.game_state.table_spec);
                let moved = ball
                    .position
                    .displacement(&final_position)
                    .absolute_distance();
                scenario
                    .game_state
                    .table_spec
                    .diamond_to_inches(moved)
                    .as_f64()
                    > 0.01
            }
        })
        .count();

    (
        first_ball_collision,
        ball_ball_collision_count,
        moved_or_pocketed_ball_count,
    )
}

#[test]
fn a_second_diamond_head_rail_nine_ball_break_with_slight_draw_opens_the_rack() {
    let scenario = build_nine_ball_break_scenario(head_rail_break_cue_position());
    let (first_ball_collision, ball_ball_collision_count, moved_or_pocketed_ball_count) =
        opening_break_summary(&scenario);

    assert_eq!(first_ball_collision, (0, 1));
    assert!(
        moved_or_pocketed_ball_count >= 6,
        "expected the opening break to set much of the rack in motion, got only {moved_or_pocketed_ball_count} moving or pocketed balls"
    );
    assert!(
        ball_ball_collision_count >= 6,
        "expected a busy opening break spread, got only {ball_ball_collision_count} collisions"
    );
}

#[test]
fn a_second_diamond_left_side_rail_nine_ball_break_with_slight_draw_opens_the_rack() {
    let scenario = build_nine_ball_break_scenario(left_side_rail_break_cue_position());
    let (first_ball_collision, ball_ball_collision_count, moved_or_pocketed_ball_count) =
        opening_break_summary(&scenario);

    assert_eq!(first_ball_collision, (0, 1));
    assert!(
        moved_or_pocketed_ball_count >= 4,
        "expected the side-rail break to open part of the rack, got only {moved_or_pocketed_ball_count} moving or pocketed balls"
    );
    assert!(
        ball_ball_collision_count >= 4,
        "expected multiple opening collisions from the side-rail break, got only {ball_ball_collision_count} collisions"
    );
}

#[test]
fn the_head_rail_break_example_uses_exact_helper_owned_rack_geometry() {
    assert_break_example_matches_helper_geometry(
        "examples/scenarios/nine_ball_break_head_rail.billiards",
        include_str!("../examples/scenarios/nine_ball_break_head_rail.billiards"),
        head_rail_break_cue_position(),
    );
}

#[test]
fn the_left_side_rail_break_example_uses_exact_helper_owned_rack_geometry() {
    assert_break_example_matches_helper_geometry(
        "examples/scenarios/nine_ball_break_left_side_rail.billiards",
        include_str!("../examples/scenarios/nine_ball_break_left_side_rail.billiards"),
        left_side_rail_break_cue_position(),
    );
}

#[test]
fn head_rail_break_trace_groups_effectively_simultaneous_collisions() {
    let scenario = build_nine_ball_break_scenario(head_rail_break_cue_position());
    let trace = simulate_break_trace(&scenario);
    let lines = trace.event_lines();

    assert!(
        lines.len() < trace.event_log.len(),
        "expected at least one effectively simultaneous event bucket in the head-rail break trace"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains(" | ") && line.matches("collision").count() >= 2),
        "expected the head-rail break trace to group at least one effectively simultaneous collision bucket"
    );
}
