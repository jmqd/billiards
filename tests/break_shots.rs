use std::fs;

use billiards::dsl::{parse_dsl_to_scenario, ScenarioShotTraceEventKind};
use billiards::{
    classify_motion_phase, human_tuned_preview_motion_config, Ball, BallBallCollisionConfig,
    BallSetPhysicsSpec, BallType, CollisionModel, MotionPhase, NBallSystemState,
    RailCollisionProfile, RailModel, TableSpec,
};

fn position_xy(state: &NBallSystemState) -> (f64, f64) {
    let state = match state {
        NBallSystemState::OnTable(on_table) => on_table.as_ball_state(),
        NBallSystemState::Airborne(airborne) => airborne,
        NBallSystemState::Pocketed {
            state_at_capture, ..
        } => state_at_capture.as_ball_state(),
    };

    (state.position.x().as_f64(), state.position.y().as_f64())
}

fn ball_by_type<'a>(balls: &'a [Ball], ball_type: BallType, scenario_path: &str) -> &'a Ball {
    balls
        .iter()
        .find(|ball| ball.ty == ball_type)
        .unwrap_or_else(|| panic!("{scenario_path}: missing {ball_type:?}"))
}

fn assert_balls_touch(table: &TableSpec, a: &Ball, b: &Ball, scenario_path: &str) {
    let distance_inches = table.diamond_to_inches(a.distance(b)).as_f64();
    let diameter_inches = a.spec.radius.as_f64() + b.spec.radius.as_f64();
    let gap_inches = distance_inches - diameter_inches;

    assert!(
        gap_inches.abs() <= 1e-6,
        "{scenario_path}: {:?} and {:?} should touch in the frozen rack; gap={gap_inches:.9}in",
        a.ty,
        b.ty
    );
}

#[test]
fn nine_ball_break_examples_use_frozen_rack_geometry() {
    for scenario_path in [
        "examples/scenarios/golden_break_cut_break.billiards",
        "examples/scenarios/nine_ball_break_head_rail.billiards",
        "examples/scenarios/nine_ball_break_left_side_rail.billiards",
    ] {
        let source = fs::read_to_string(scenario_path).expect("scenario should read");
        let mut scenario = parse_dsl_to_scenario(&source).expect("scenario should parse");
        scenario.game_state.resolve_positions();
        let balls = scenario.game_state.balls();
        let table = &scenario.game_state.table_spec;

        for (first, second) in [
            (BallType::One, BallType::Two),
            (BallType::One, BallType::Three),
            (BallType::Two, BallType::Three),
            (BallType::Two, BallType::Four),
            (BallType::Two, BallType::Nine),
            (BallType::Three, BallType::Nine),
            (BallType::Three, BallType::Five),
            (BallType::Four, BallType::Nine),
            (BallType::Nine, BallType::Five),
            (BallType::Four, BallType::Six),
            (BallType::Nine, BallType::Six),
            (BallType::Nine, BallType::Seven),
            (BallType::Five, BallType::Seven),
            (BallType::Six, BallType::Seven),
            (BallType::Six, BallType::Eight),
            (BallType::Seven, BallType::Eight),
        ] {
            let first = ball_by_type(balls, first, scenario_path);
            let second = ball_by_type(balls, second, scenario_path);
            assert_balls_touch(table, first, second, scenario_path);
        }
    }
}

fn displaced_object_balls(
    balls: &[billiards::Ball],
    before: &[NBallSystemState],
    after: &[NBallSystemState],
) -> usize {
    balls
        .iter()
        .zip(before)
        .zip(after)
        .filter(|((ball, _), _)| ball.ty != BallType::Cue)
        .filter(|((_, before), after)| {
            let (before_x, before_y) = position_xy(before);
            let (after_x, after_y) = position_xy(after);

            (after_x - before_x).hypot(after_y - before_y) > 0.25
        })
        .count()
}

#[test]
fn nine_ball_break_nonideal_traces_reach_rest_and_spread_frozen_racks() {
    for scenario_path in [
        "examples/scenarios/golden_break_cut_break.billiards",
        "examples/scenarios/nine_ball_break_head_rail.billiards",
        "examples/scenarios/nine_ball_break_left_side_rail.billiards",
    ] {
        let source = fs::read_to_string(scenario_path).expect("scenario should read");
        let mut scenario = parse_dsl_to_scenario(&source).expect("scenario should parse");
        scenario.game_state.resolve_positions();
        assert!(
            scenario.trace_max_events.is_none(),
            "{scenario_path}: a complete break trace must not declare an event cap"
        );
        let ball_set = BallSetPhysicsSpec::default();
        let motion = human_tuned_preview_motion_config();
        let initial_states = scenario
            .initial_shot_system_states_on_table(&ball_set)
            .expect("initial shot states should build")
            .expect("scenario should contain a shot");

        let trace = scenario
            .simulate_shot_trace_with_physics_on_table_until_rest(
                &ball_set,
                &motion,
                CollisionModel::ThrowAware,
                &BallBallCollisionConfig::human_tuned(),
                RailModel::SpinAware,
                &RailCollisionProfile::default(),
            )
            .expect("scenario should simulate")
            .expect("scenario should contain a shot");

        let rail_impacts = trace
            .event_log
            .iter()
            .filter(|event| {
                matches!(
                    &event.kind,
                    ScenarioShotTraceEventKind::BallRailImpact { .. }
                )
            })
            .count();
        assert!(
            rail_impacts >= 3,
            "{scenario_path}: expected complete nonideal break trace to include at least three rail impacts, got {rail_impacts}"
        );

        let moved_object_balls = displaced_object_balls(
            scenario.game_state.balls(),
            &initial_states,
            &trace.simulation.states,
        );
        assert!(
            moved_object_balls >= 6,
            "{scenario_path}: expected broad rack spread in complete nonideal trace, got {moved_object_balls} moved object balls"
        );
        assert!(
            trace.simulation.states.iter().all(|state| match state {
                NBallSystemState::OnTable(state) => matches!(
                    classify_motion_phase(state.as_ball_state(), &ball_set, &motion.phase),
                    MotionPhase::Rest
                ),
                NBallSystemState::Pocketed { .. } => true,
                NBallSystemState::Airborne(_) => false,
            }),
            "{scenario_path}: uncapped break trace must finish with every live ball at rest"
        );
    }
}
