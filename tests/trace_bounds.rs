use bigdecimal::ToPrimitive;
use billiards::dsl::parse_dsl_to_scenario;
use billiards::{
    human_tuned_preview_motion_config, BallSetPhysicsSpec, CollisionModel, RailModel, Seconds,
};
use std::fs;
use std::path::Path;

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
                Seconds::new(0.005),
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
fn known_break_scenario_trace_points_stay_within_table_diamonds() {
    for path in [
        Path::new("examples/scenarios/mini_break_cluster.billiards"),
        Path::new("examples/scenarios/nine_ball_break_head_rail.billiards"),
    ] {
        assert_trace_points_stay_within_table_diamonds(path);
    }
}
