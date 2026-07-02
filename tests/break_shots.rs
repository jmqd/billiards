use std::fs;

use billiards::dsl::{parse_dsl_to_scenario, ScenarioShotTraceEventKind};
use billiards::{
    advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table,
    human_tuned_preview_motion_config, Ball, BallBallCollisionConfig, BallSetPhysicsSpec, BallType,
    CollisionModel, NBallSystemEvent, NBallSystemState, RailCollisionProfile, RailModel, TableSpec,
};

fn position_xy(state: &NBallSystemState) -> (f64, f64) {
    let state = match state {
        NBallSystemState::OnTable(on_table) => on_table.as_ball_state(),
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
fn nine_ball_break_examples_open_the_rack_after_shared_contact() {
    for scenario_path in [
        "examples/scenarios/nine_ball_break_head_rail.billiards",
        "examples/scenarios/nine_ball_break_left_side_rail.billiards",
    ] {
        let source = fs::read_to_string(scenario_path).expect("scenario should read");
        let mut scenario = parse_dsl_to_scenario(&source).expect("scenario should parse");
        scenario.game_state.resolve_positions();
        let ball_set = BallSetPhysicsSpec::default();
        let motion = human_tuned_preview_motion_config();
        let initial_states = scenario
            .initial_shot_system_states_on_table(&ball_set)
            .expect("initial shot states should build")
            .expect("scenario should contain a shot")
            .into_iter()
            .map(NBallSystemState::from)
            .collect::<Vec<_>>();
        let mut states = initial_states.clone();
        let mut events = Vec::new();

        for _ in 0..32 {
            let advanced = advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table(
                &states,
                &ball_set,
                &scenario.game_state.table_spec,
                &motion,
                CollisionModel::ThrowAware,
                &BallBallCollisionConfig::human_tuned(),
                RailModel::SpinAware,
                &RailCollisionProfile::default(),
            );
            let Some(event) = advanced.event else {
                break;
            };
            events.push(event);
            states = advanced.states;
            if displaced_object_balls(scenario.game_state.balls(), &initial_states, &states) >= 2 {
                break;
            }
        }

        assert!(
            events
                .iter()
                .any(|event| matches!(event, NBallSystemEvent::SharedBallBallContact { .. }))
                || events
                    .iter()
                    .any(|event| matches!(event, NBallSystemEvent::BallBallCollision { .. })),
            "{scenario_path}: expected break to enter the shared rack-contact path"
        );

        let moved_object_balls =
            displaced_object_balls(scenario.game_state.balls(), &initial_states, &states);
        assert!(
            moved_object_balls >= 2,
            "{scenario_path}: expected multiple object balls to move after bounded break stepping, got {moved_object_balls}"
        );
    }
}

#[test]
fn nine_ball_break_default_traces_reach_rails_and_table_spread() {
    for scenario_path in [
        "examples/scenarios/nine_ball_break_head_rail.billiards",
        "examples/scenarios/nine_ball_break_left_side_rail.billiards",
    ] {
        let source = fs::read_to_string(scenario_path).expect("scenario should read");
        let mut scenario = parse_dsl_to_scenario(&source).expect("scenario should parse");
        scenario.game_state.resolve_positions();
        let trace_max_events = scenario
            .trace_max_events
            .expect("break scenario should declare a preview trace length");
        let ball_set = BallSetPhysicsSpec::default();
        let motion = human_tuned_preview_motion_config();
        let initial_states = scenario
            .initial_shot_system_states_on_table(&ball_set)
            .expect("initial shot states should build")
            .expect("scenario should contain a shot")
            .into_iter()
            .map(NBallSystemState::from)
            .collect::<Vec<_>>();

        let trace = scenario
            .simulate_shot_trace_with_physics_on_table_until_event_limit(
                &ball_set,
                &motion,
                CollisionModel::ThrowAware,
                &BallBallCollisionConfig::human_tuned(),
                RailModel::SpinAware,
                &RailCollisionProfile::default(),
                trace_max_events,
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
            "{scenario_path}: expected break preview to include balls reaching rails, got {rail_impacts}"
        );

        let moved_object_balls = displaced_object_balls(
            scenario.game_state.balls(),
            &initial_states,
            &trace.simulation.states,
        );
        assert!(
            moved_object_balls >= 6,
            "{scenario_path}: expected broad rack spread in default preview trace, got {moved_object_balls} moved object balls"
        );
    }
}
