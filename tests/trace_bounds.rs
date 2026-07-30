use bigdecimal::ToPrimitive;
use billiards::dsl::{parse_dsl_to_scenario, ScenarioShotTraceEventKind};
use billiards::{
    human_tuned_preview_motion_config, BallSetPhysicsSpec, BallType, CollisionModel,
    NBallSystemState, Rail, RailModel, Seconds,
};
use std::fs;
use std::path::Path;

// This test is a broad trace-rendering regression, not a high-precision trajectory integrator.
// Sampling at 20 Hz still checks each rendered segment interior while avoiding the old 200 Hz
// oversampling cost on long break traces.
const TRACE_BOUNDS_MAX_SAMPLE_STEP_SECONDS: f64 = 0.05;

fn diamond_value(value: &billiards::Diamond) -> f64 {
    value.magnitude.to_f64().expect("diamond value")
}

fn assert_trace_points_stay_within_table_diamonds(scenario_path: &Path) {
    let source = fs::read_to_string(scenario_path).expect("scenario should read");
    let mut scenario = parse_dsl_to_scenario(&source).expect("scenario should parse");
    scenario.game_state.resolve_positions();

    let ball_set = BallSetPhysicsSpec::default();
    let motion = human_tuned_preview_motion_config();
    let Some(trace) = scenario
        .simulate_shot_trace_with_preferred_physics_on_table_until_rest(
            &ball_set,
            &motion,
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        )
        .expect("scenario trace should simulate")
    else {
        return;
    };

    for ball_trace in &trace.ball_traces {
        for (segment_index, segment) in ball_trace.segments.iter().enumerate() {
            let path = billiards::BallPath {
                initial_state: segment.start.clone(),
                final_state: segment.end.clone(),
                elapsed: segment.duration,
                rail_impacts: 0,
                segments: vec![segment.clone()],
            };
            for point in path.sampled_points(
                Seconds::new(TRACE_BOUNDS_MAX_SAMPLE_STEP_SECONDS),
                &ball_set,
                &motion,
                &scenario.game_state.table_spec,
            ) {
                let x = diamond_value(&point.x);
                let y = diamond_value(&point.y);
                assert!(
                    (-1e-9..=4.0 + 1e-9).contains(&x)
                        && (-1e-9..=8.0 + 1e-9).contains(&y),
                    "{} {:?} segment {segment_index} duration {} trace left table diamonds at ({x}, {y}); start=({},{}) end=({},{})",
                    scenario_path.display(),
                    ball_trace.ball,
                    segment.duration.as_f64(),
                    segment.start.as_ball_state().position.x().as_f64(),
                    segment.start.as_ball_state().position.y().as_f64(),
                    segment.end.as_ball_state().position.x().as_f64(),
                    segment.end.as_ball_state().position.y().as_f64(),
                );
            }
        }
    }
}

#[test]
fn mini_break_scenario_trace_points_stay_within_table_diamonds() {
    assert_trace_points_stay_within_table_diamonds(Path::new(
        "examples/scenarios/mini_break_cluster.billiards",
    ));
}

#[test]
#[ignore = "slow full-rack break trace bound check; run explicitly before trace renderer rewrites"]
fn full_rack_break_scenario_trace_points_stay_within_table_diamonds() {
    assert_trace_points_stay_within_table_diamonds(Path::new(
        "examples/scenarios/nine_ball_break_head_rail.billiards",
    ));
}

const OFF_TABLE_AIRBORNE_TRACE_CASES: [(&str, &str); 2] = [
    (
        "first reported carom shot",
        r#"
table three_cushion_carom_10ft
game three_cushion
ball cue at (2.354, 6.309)
ball yellow at (3.491, 5.838)
ball red at (2.762, 3.888)
cue_strike(default).mass_ratio(1.0).energy_loss(0.08)
ball_ball(carom).normal_restitution(0.98).tangential_friction(0.05)
rail_response(lively).normal_restitution(0.82).tangential_friction(0.82)
rails(carom).default(lively)
simulation(default)
  .collision_model(throw_aware)
  .ball_ball(carom)
  .rail_model(spin_aware)
  .rails(carom)
  .conditions(heated_carom)
  .max_events(188)
trace(max_events: 188)
shot(cue).heading(104.9752768179163deg).speed(154.54320838415867ips).tip(side: -0.241670018266813R, height: 0.10147386005033804R).elevation(11.048607052617005deg).using(default)
"#,
    ),
    (
        "second reported carom shot",
        r#"
table three_cushion_carom_10ft
game three_cushion
ball cue at (2.354, 6.309)
ball yellow at (3.491, 5.838)
ball red at (2.762, 3.888)
cue_strike(default).mass_ratio(1.0).energy_loss(0.08)
ball_ball(carom).normal_restitution(0.98).tangential_friction(0.05)
rail_response(lively).normal_restitution(0.82).tangential_friction(0.82)
rails(carom).default(lively)
simulation(default)
  .collision_model(throw_aware)
  .ball_ball(carom)
  .rail_model(spin_aware)
  .rails(carom)
  .conditions(heated_carom)
  .max_events(188)
trace(max_events: 188)
shot(cue).heading(165.80576430992357deg).speed(246.20839387485356ips).tip(side: -0.251703236609268R, height: 0.12645339511916992R).elevation(9.830566144742164deg).using(default)
"#,
    ),
];

const CUSHION_CLEARING_FOUL_CASE: &str = r#"
table three_cushion_carom_10ft
game three_cushion
ball cue at (2.0, 7.5)
ball yellow at (1.0, 2.0)
ball red at (3.0, 2.0)
cue_strike(default).mass_ratio(1.0).energy_loss(0.08)
ball_ball(carom).normal_restitution(0.98).tangential_friction(0.05)
rail_response(lively).normal_restitution(0.82).tangential_friction(0.82)
rails(carom).default(lively)
simulation(default)
  .collision_model(throw_aware)
  .ball_ball(carom)
  .rail_model(spin_aware)
  .rails(carom)
  .conditions(heated_carom)
  .max_events(64)
trace(max_events: 64)
shot(cue).heading(0deg).speed(200ips).tip(side: 0R, height: 0R).elevation(30deg).using(default)
"#;

#[test]
fn cushion_clearing_cue_ball_emits_a_terminal_foul_at_the_rail_plane() {
    let mut scenario =
        parse_dsl_to_scenario(CUSHION_CLEARING_FOUL_CASE).expect("foul scenario should parse");
    scenario.game_state.resolve_positions();
    let table = &scenario.game_state.table_spec;
    let ball_set = table.default_ball_set_physics_spec();
    let trace = scenario
        .simulate_shot_trace_with_preferred_physics_on_table_until_rest(
            &ball_set,
            &human_tuned_preview_motion_config(),
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        )
        .expect("foul scenario should simulate")
        .expect("foul scenario should contain a shot");

    assert!(matches!(
        trace.event_log.as_slice(),
        [event]
            if matches!(
                event.kind,
                ScenarioShotTraceEventKind::BallOffTable {
                    ball: BallType::Cue,
                    rail: Rail::Top,
                }
            )
    ));
    let event_lines = trace.event_lines();
    assert_eq!(event_lines.len(), 1);
    assert!(event_lines[0].ends_with("cue off table over top rail"));

    let cue_trace = trace
        .ball_traces
        .iter()
        .find(|ball_trace| ball_trace.ball == BallType::Cue)
        .expect("cue trace should exist");
    let NBallSystemState::OffTable {
        rail,
        state_at_exit,
    } = &cue_trace.final_state
    else {
        panic!("cue ball should finish off table")
    };
    assert_eq!(*rail, Rail::Top);
    let top_plane = table
        .diamond_to_inches(billiards::Diamond::eight())
        .as_f64()
        - ball_set.radius.as_f64();
    assert!((state_at_exit.position.y().as_f64() - top_plane).abs() <= 1e-9);
    assert!(
        state_at_exit.height.as_f64() >= table.cushion_nose_height.as_f64(),
        "terminal exit must clear the cushion nose"
    );

    let last_frame = trace
        .playback_frames(Seconds::new(0.01))
        .pop()
        .expect("playback should contain a terminal frame");
    assert!(
        last_frame
            .balls
            .iter()
            .all(|ball| ball.ball != BallType::Cue),
        "off-table cue ball must disappear from terminal playback"
    );
    let rendered = trace.rendered_final_layout_with_traces(&scenario, Seconds::new(0.01));
    assert!(
        rendered.balls().iter().all(|ball| ball.ty != BallType::Cue),
        "off-table cue ball must not remain in the final layout"
    );
}

#[test]
fn airborne_carom_trace_timelines_stay_inside_cushion_contact_planes() {
    for (case_name, source) in OFF_TABLE_AIRBORNE_TRACE_CASES {
        let mut scenario = parse_dsl_to_scenario(source).expect("scenario should parse");
        scenario.game_state.resolve_positions();
        let table = &scenario.game_state.table_spec;
        let ball_set = table.default_ball_set_physics_spec();
        let motion = human_tuned_preview_motion_config();
        let trace = scenario
            .simulate_shot_trace_with_preferred_physics_on_table_until_rest(
                &ball_set,
                &motion,
                CollisionModel::ThrowAware,
                RailModel::SpinAware,
            )
            .expect("scenario trace should simulate")
            .expect("reported scenario should contain a shot");
        assert!(
            trace
                .shot_executions
                .iter()
                .flat_map(|execution| &execution.simulation.events)
                .any(|event| matches!(
                    event,
                    billiards::NBallSystemEvent::AirborneBallRailImpact { impact, .. }
                        if impact.state_at_impact.height.as_f64() > 0.0
                )),
            "{case_name} should exercise an above-table airborne cushion impact",
        );

        let radius = ball_set.radius.as_f64();
        let min_x = radius;
        let max_x = table.diamond_to_inches(billiards::Diamond::four()).as_f64() - radius;
        let min_y = radius;
        let max_y = table
            .diamond_to_inches(billiards::Diamond::eight())
            .as_f64()
            - radius;
        let tolerance = 1e-8;

        for ball_trace in &trace.ball_traces {
            for (segment_index, pair) in ball_trace.timeline_segments.windows(2).enumerate() {
                let previous = &pair[0].end.position;
                let next = &pair[1].start.position;
                assert!(
                    (previous.x().as_f64() - next.x().as_f64()).abs() <= tolerance
                        && (previous.y().as_f64() - next.y().as_f64()).abs() <= tolerance,
                    "{case_name} {:?} timeline jumped after segment {segment_index}: ({}, {}) -> ({}, {})",
                    ball_trace.ball,
                    previous.x().as_f64(),
                    previous.y().as_f64(),
                    next.x().as_f64(),
                    next.y().as_f64(),
                );
            }
            for (segment_index, segment) in ball_trace.timeline_segments.iter().enumerate() {
                for (endpoint, state) in [("start", &segment.start), ("end", &segment.end)] {
                    let x = state.position.x().as_f64();
                    let y = state.position.y().as_f64();
                    assert!(
                        (min_x - tolerance..=max_x + tolerance).contains(&x)
                            && (min_y - tolerance..=max_y + tolerance).contains(&y),
                        "{case_name} {:?} timeline segment {segment_index} {endpoint} left the cushion contact planes at ({x}, {y}); bounds=({min_x}..={max_x}, {min_y}..={max_y})",
                        ball_trace.ball,
                    );
                }
            }
        }
    }
}
