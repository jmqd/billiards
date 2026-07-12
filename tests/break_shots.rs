use std::fs;

use billiards::dsl::{parse_dsl_to_scenario, ScenarioShotTraceEventKind};
use billiards::{
    advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table, classify_motion_phase,
    human_tuned_preview_motion_config,
    simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit, Ball,
    BallBallCollisionConfig, BallSetPhysicsSpec, BallType, CollisionModel, MotionPhase,
    NBallSystemEvent, NBallSystemState, RailCollisionProfile, RailModel, TableSpec,
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

fn state_by_type<'a>(
    balls: &[Ball],
    states: &'a [NBallSystemState],
    ball_type: BallType,
    scenario_path: &str,
) -> &'a billiards::BallState {
    let index = balls
        .iter()
        .position(|ball| ball.ty == ball_type)
        .unwrap_or_else(|| panic!("{scenario_path}: missing {ball_type:?}"));
    states[index].as_ball_state()
}

fn velocity_xy(state: &billiards::BallState) -> (f64, f64) {
    (state.velocity.x().as_f64(), state.velocity.y().as_f64())
}

fn kinetic_energy_units(state: &billiards::BallState, radius: f64) -> f64 {
    let linear = state.velocity.x().as_f64().powi(2)
        + state.velocity.y().as_f64().powi(2)
        + state.vertical_velocity.as_f64().powi(2);
    let angular = state.angular_velocity.x().as_f64().powi(2)
        + state.angular_velocity.y().as_f64().powi(2)
        + state.angular_velocity.z().as_f64().powi(2);
    0.5 * linear + radius * radius * angular / 5.0
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
fn opening_nonideal_nine_ball_break_contact_preserves_full_rack_geometry() {
    for scenario_path in [
        "examples/scenarios/golden_break_cut_break.billiards",
        "examples/scenarios/nine_ball_break_head_rail.billiards",
        "examples/scenarios/nine_ball_break_left_side_rail.billiards",
    ] {
        let source = fs::read_to_string(scenario_path).expect("scenario should read");
        let mut scenario = parse_dsl_to_scenario(&source).expect("scenario should parse");
        scenario.game_state.resolve_positions();
        let ball_set = BallSetPhysicsSpec::default();
        let initial_states = scenario
            .initial_shot_system_states_on_table(&ball_set)
            .expect("initial shot states should build")
            .expect("scenario should contain a shot");
        let simulation =
            simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit(
                &initial_states,
                &ball_set,
                &scenario.game_state.table_spec,
                &human_tuned_preview_motion_config(),
                CollisionModel::ThrowAware,
                &BallBallCollisionConfig::human_tuned(),
                RailModel::SpinAware,
                &RailCollisionProfile::default(),
                Some(1),
            )
            .expect("the opening full-rack contact should resolve without overlap");

        assert_eq!(simulation.events.len(), 1);
        let diameter = 2.0 * ball_set.radius.as_f64();
        for first_index in 0..simulation.states.len() {
            let Some(first) = simulation.states[first_index].as_on_table() else {
                continue;
            };
            for second_index in first_index + 1..simulation.states.len() {
                let Some(second) = simulation.states[second_index].as_on_table() else {
                    continue;
                };
                let dx = second.as_ball_state().position.x().as_f64()
                    - first.as_ball_state().position.x().as_f64();
                let dy = second.as_ball_state().position.y().as_f64()
                    - first.as_ball_state().position.y().as_f64();
                assert!(
                    dx.hypot(dy) >= diameter,
                    "{scenario_path}: balls {first_index}/{second_index} overlap after the full-rack solve"
                );
            }
        }

        if scenario_path.ends_with("nine_ball_break_head_rail.billiards") {
            let collision = match &simulation.events[0] {
                NBallSystemEvent::BallBallCollision { collision, .. } => collision,
                other => {
                    panic!("centered rack should open with a ball-ball collision, got {other:?}")
                }
            };
            let incoming_momentum = {
                let (a_x, a_y) = velocity_xy(collision.a_at_impact.as_ball_state());
                let (b_x, b_y) = velocity_xy(collision.b_at_impact.as_ball_state());
                (a_x + b_x, a_y + b_y)
            };
            let outgoing_momentum = simulation.states.iter().fold((0.0, 0.0), |sum, state| {
                let (vx, vy) = velocity_xy(state.as_ball_state());
                (sum.0 + vx, sum.1 + vy)
            });
            let momentum_tolerance = 2e-5 * incoming_momentum.1.abs().max(1.0);
            assert!(
                (outgoing_momentum.0 - incoming_momentum.0).abs() <= momentum_tolerance
                    && (outgoing_momentum.1 - incoming_momentum.1).abs()
                        <= momentum_tolerance,
                "centered rack momentum changed: before={incoming_momentum:?}, after={outgoing_momentum:?}"
            );

            let incoming_energy =
                kinetic_energy_units(collision.a_at_impact.as_ball_state(), diameter / 2.0)
                    + kinetic_energy_units(collision.b_at_impact.as_ball_state(), diameter / 2.0);
            let outgoing_energy = simulation
                .states
                .iter()
                .map(|state| kinetic_energy_units(state.as_ball_state(), diameter / 2.0))
                .sum::<f64>();
            assert!(
                outgoing_energy <= incoming_energy + 2e-5 * incoming_energy.max(1.0),
                "centered rack gained kinetic energy: before={incoming_energy}, after={outgoing_energy}"
            );

            let balls = scenario.game_state.balls();
            let cue = state_by_type(balls, &simulation.states, BallType::Cue, scenario_path);
            let one = state_by_type(balls, &simulation.states, BallType::One, scenario_path);
            let eight = state_by_type(balls, &simulation.states, BallType::Eight, scenario_path);
            assert!(
                cue.velocity.y().as_f64() > 1.0,
                "compliant rack wave should rebound the cue toward the head rail: vy={}",
                cue.velocity.y().as_f64()
            );
            assert!(
                eight.velocity.y().as_f64() < -10.0,
                "back ball should carry substantial footward speed: vy={}",
                eight.velocity.y().as_f64()
            );
            assert!(
                (cue.velocity.y().as_f64() + 61.196).abs() > 1.0
                    || (one.velocity.y().as_f64() + 61.196).abs() > 1.0
                    || (eight.velocity.y().as_f64() + 15.299).abs() > 1.0,
                "opening response retained the deleted rigid-LCP signature"
            );

            for (left_type, right_type) in [
                (BallType::Two, BallType::Three),
                (BallType::Four, BallType::Five),
                (BallType::Six, BallType::Seven),
            ] {
                let left =
                    state_by_type(balls, &simulation.states, left_type.clone(), scenario_path);
                let right =
                    state_by_type(balls, &simulation.states, right_type.clone(), scenario_path);
                assert!(
                    (left.velocity.x().as_f64() + right.velocity.x().as_f64()).abs() <= 1e-4
                        && (left.velocity.y().as_f64() - right.velocity.y().as_f64()).abs() <= 1e-4,
                    "centered rack lost mirror symmetry for {left_type:?}/{right_type:?}"
                );
            }
            for center_type in [BallType::One, BallType::Nine, BallType::Eight] {
                let center = state_by_type(
                    balls,
                    &simulation.states,
                    center_type.clone(),
                    scenario_path,
                );
                assert!(
                    center.velocity.x().as_f64().abs() <= 1e-4,
                    "center-line {center_type:?} acquired asymmetric vx={}",
                    center.velocity.x().as_f64()
                );
            }
        }
    }
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

        assert!(
            trace.simulation.events.len() < 2_048,
            "{scenario_path}: complete break exceeded the bounded event budget with {} events",
            trace.simulation.events.len()
        );
        let mut zero_time_run = 0usize;
        let mut longest_zero_time_run = 0usize;
        for events in trace.event_log.windows(2) {
            if events[1].time.as_f64() - events[0].time.as_f64() <= 1e-12 {
                zero_time_run += 1;
                longest_zero_time_run = longest_zero_time_run.max(zero_time_run);
            } else {
                zero_time_run = 0;
            }
        }
        assert!(
            longest_zero_time_run < 8,
            "{scenario_path}: unresolved zero-time event cycle reached {longest_zero_time_run} consecutive steps"
        );
        for (event_index, event) in trace.event_log.iter().enumerate() {
            let ScenarioShotTraceEventKind::BallTableBounce { ball } = &event.kind else {
                continue;
            };
            if let Some(previous) = trace.event_log[..event_index]
                .iter()
                .rev()
                .find(|previous| {
                    matches!(
                        &previous.kind,
                        ScenarioShotTraceEventKind::BallTableBounce { ball: previous_ball }
                            if previous_ball == ball
                    )
                })
            {
                assert!(
                    event.time.as_f64() - previous.time.as_f64() > 1e-6,
                    "{scenario_path}: {ball:?} repeated a sub-microsecond table bounce"
                );
            }
        }
        let balls = scenario.game_state.balls();
        for first in 0..trace.simulation.states.len() {
            if matches!(
                trace.simulation.states[first],
                NBallSystemState::Pocketed { .. }
            ) {
                continue;
            }
            for second in first + 1..trace.simulation.states.len() {
                if matches!(
                    trace.simulation.states[second],
                    NBallSystemState::Pocketed { .. }
                ) {
                    continue;
                }
                let (first_x, first_y) = position_xy(&trace.simulation.states[first]);
                let (second_x, second_y) = position_xy(&trace.simulation.states[second]);
                let minimum_distance =
                    balls[first].spec.radius.as_f64() + balls[second].spec.radius.as_f64();
                assert!(
                    (second_x - first_x).hypot(second_y - first_y) >= minimum_distance - 1e-6,
                    "{scenario_path}: final live balls {:?}/{:?} overlap",
                    balls[first].ty,
                    balls[second].ty
                );
            }
        }
        println!(
            "{scenario_path}: events={}, elapsed={:.9}s, longest_zero_time_run={longest_zero_time_run}",
            trace.simulation.events.len(),
            trace.simulation.elapsed.as_f64()
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

#[test]
fn force_follow_first_hop_remains_airborne_and_trace_reaches_rest() {
    let scenario_path = "examples/scenarios/seven_ball_force_follow_breakout.billiards";
    let source = fs::read_to_string(scenario_path).expect("scenario should read");
    let mut scenario = parse_dsl_to_scenario(&source).expect("scenario should parse");
    scenario.game_state.resolve_positions();
    let ball_set = BallSetPhysicsSpec::default();
    let motion = human_tuned_preview_motion_config();
    let collision = BallBallCollisionConfig::human_tuned();
    let rail_profile = RailCollisionProfile::default();
    let balls = scenario.game_state.balls();
    let cue_index = balls
        .iter()
        .position(|ball| ball.ty == BallType::Cue)
        .expect("force-follow fixture should contain the cue ball");
    let mut states = scenario
        .initial_shot_system_states_on_table(&ball_set)
        .expect("initial force-follow states should build")
        .expect("force-follow scenario should contain a shot");
    let mut first_hop_vertical_speed = None;
    for _ in 0..64 {
        let advanced = advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table(
            &states,
            &ball_set,
            &scenario.game_state.table_spec,
            &motion,
            CollisionModel::ThrowAware,
            &collision,
            RailModel::SpinAware,
            &rail_profile,
        )
        .expect("force-follow event should resolve");
        let Some(event) = advanced.event else {
            break;
        };
        let cue_contact = match event {
            NBallSystemEvent::BallBallCollision {
                first_ball_index,
                second_ball_index,
                ..
            }
            | NBallSystemEvent::AirborneBallBallCollision {
                first_ball_index,
                second_ball_index,
                ..
            } => first_ball_index == cue_index || second_ball_index == cue_index,
            NBallSystemEvent::SharedBallBallContact {
                ref ball_indices, ..
            } => ball_indices.contains(&cue_index),
            _ => false,
        };
        states = advanced.states;
        if cue_contact {
            assert!(
                matches!(states[cue_index], NBallSystemState::Airborne(_)),
                "the first force-follow cue contact must launch a physically resolved hop"
            );
            first_hop_vertical_speed =
                Some(states[cue_index].as_ball_state().vertical_velocity.as_f64());
            break;
        }
    }
    let first_hop_vertical_speed =
        first_hop_vertical_speed.expect("force-follow fixture should reach its first cue contact");
    assert!(
        first_hop_vertical_speed > 4.0,
        "force-follow hop was unexpectedly suppressed: vz={first_hop_vertical_speed}"
    );

    let trace = scenario
        .simulate_shot_trace_with_physics_on_table_until_rest(
            &ball_set,
            &motion,
            CollisionModel::ThrowAware,
            &collision,
            RailModel::SpinAware,
            &rail_profile,
        )
        .expect("force-follow scenario should simulate")
        .expect("force-follow scenario should contain a shot");
    assert!(
        trace.simulation.states.iter().all(|state| match state {
            NBallSystemState::OnTable(state) => matches!(
                classify_motion_phase(state.as_ball_state(), &ball_set, &motion.phase),
                MotionPhase::Rest
            ),
            NBallSystemState::Pocketed { .. } => true,
            NBallSystemState::Airborne(_) => false,
        }),
        "force-follow trace must reach natural rest"
    );
    println!(
        "{scenario_path}: events={}, elapsed={:.9}s, first_hop_vz={first_hop_vertical_speed:.9}",
        trace.simulation.events.len(),
        trace.simulation.elapsed.as_f64()
    );
}
