use std::fs;

use billiards::dsl::{parse_dsl_to_scenario, ScenarioShotTraceEventKind};
use billiards::{
    advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table,
    human_tuned_preview_motion_config, BallBallCollisionConfig, BallSetPhysicsSpec, BallType,
    CollisionModel, NBallSystemEvent, NBallSystemState, Pocket, RailCollisionProfile, RailModel,
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
fn nine_ball_break_default_traces_reach_wing_ball_pockets_and_table_spread() {
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

        let wing_ball_pocketed = trace.event_log.iter().any(|event| {
            if let ScenarioShotTraceEventKind::BallPocketCapture { ball, pocket } = &event.kind {
                matches!(ball, &BallType::Four | &BallType::Five)
                    && matches!(pocket, &Pocket::BottomLeft | &Pocket::BottomRight)
            } else {
                false
            }
        });
        assert!(
            wing_ball_pocketed,
            "{scenario_path}: default preview trace should run long enough to show a wing ball pocket"
        );

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
