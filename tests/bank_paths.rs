use bigdecimal::ToPrimitive;
use billiards::{
    trace_ball_path_with_rails_on_table, AngularVelocity3, BallPathStop, BallSetPhysicsSpec,
    BallState, Diamond, Inches, Inches2, InchesPerSecond, InchesPerSecondSq, MotionPhaseConfig,
    MotionTransitionConfig, OnTableBallState, OnTableMotionConfig, RadiansPerSecondSq, Rail,
    RailAngleReference, RailModel, RailTangentDirection, RollingResistanceModel,
    SlidingFrictionModel, SpinDecayModel, TableSpec, Velocity2, TYPICAL_BALL_RADIUS,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
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

fn diamond_value(diamond: &Diamond) -> f64 {
    diamond.magnitude.to_f64().expect("diamond magnitude")
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
fn tracing_a_thirty_degree_mirror_bank_produces_a_two_segment_path() {
    let table = TableSpec::default();
    let top_plane =
        table.diamond_to_inches(Diamond::eight()).as_f64() - TYPICAL_BALL_RADIUS.as_f64();
    let path = trace_ball_path_with_rails_on_table(
        &thirty_degree_top_rail_bank_state(&table),
        BallPathStop::Duration(billiards::Seconds::new(1.0)),
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        RailModel::Mirror,
    );

    assert_eq!(path.rail_impacts, 1);
    assert_eq!(path.segments.len(), 2);
    assert_close(path.elapsed.as_f64(), 1.0);
    assert_close(
        path.segments[0].end.as_ball_state().position.y().as_f64(),
        top_plane,
    );
    assert!(path.segments[1].end.as_ball_state().position.y().as_f64() < top_plane);
    assert!(
        path.segments[1].end.as_ball_state().position.x().as_f64()
            > path.segments[0].end.as_ball_state().position.x().as_f64()
    );

    let projected = path.projected_points(&table);
    assert_eq!(projected.len(), 3);
    assert_close(projected[0].angle_to(&projected[1]).as_degrees(), 30.0);
    let outgoing = projected[1].angle_to(&projected[2]).as_degrees();
    assert!(
        outgoing > 90.0 && outgoing < 180.0,
        "the post-bank segment should head back into the table on the reflected side"
    );
}

#[test]
fn sampling_with_a_large_time_step_matches_the_event_vertex_path() {
    let table = TableSpec::default();
    let motion = motion_config();
    let path = trace_ball_path_with_rails_on_table(
        &thirty_degree_top_rail_bank_state(&table),
        BallPathStop::Duration(billiards::Seconds::new(1.0)),
        &BallSetPhysicsSpec::default(),
        &table,
        &motion,
        RailModel::Mirror,
    );

    assert_eq!(
        path.sampled_points(
            billiards::Seconds::new(10.0),
            &BallSetPhysicsSpec::default(),
            &motion,
            &table,
        ),
        path.projected_points(&table)
    );
}

#[test]
fn sampling_with_a_small_time_step_inserts_intermediate_points() {
    let table = TableSpec::default();
    let motion = motion_config();
    let path = trace_ball_path_with_rails_on_table(
        &thirty_degree_top_rail_bank_state(&table),
        BallPathStop::Duration(billiards::Seconds::new(1.0)),
        &BallSetPhysicsSpec::default(),
        &table,
        &motion,
        RailModel::SpinAware,
    );

    let coarse = path.projected_points(&table);
    let sampled = path.sampled_points(
        billiards::Seconds::new(0.05),
        &BallSetPhysicsSpec::default(),
        &motion,
        &table,
    );

    assert!(sampled.len() > coarse.len());
    assert_close(
        table
            .diamond_to_inches(sampled.first().unwrap().x.clone())
            .as_f64(),
        table
            .diamond_to_inches(coarse.first().unwrap().x.clone())
            .as_f64(),
    );
    assert_close(
        table
            .diamond_to_inches(sampled.first().unwrap().y.clone())
            .as_f64(),
        table
            .diamond_to_inches(coarse.first().unwrap().y.clone())
            .as_f64(),
    );
    assert_close(
        table
            .diamond_to_inches(sampled.last().unwrap().x.clone())
            .as_f64(),
        table
            .diamond_to_inches(coarse.last().unwrap().x.clone())
            .as_f64(),
    );
    assert_close(
        table
            .diamond_to_inches(sampled.last().unwrap().y.clone())
            .as_f64(),
        table
            .diamond_to_inches(coarse.last().unwrap().y.clone())
            .as_f64(),
    );
}

#[test]
fn tracing_until_one_rail_impact_stops_at_the_bank_point() {
    let table = TableSpec::default();
    let top_plane =
        table.diamond_to_inches(Diamond::eight()).as_f64() - TYPICAL_BALL_RADIUS.as_f64();
    let path = trace_ball_path_with_rails_on_table(
        &thirty_degree_top_rail_bank_state(&table),
        BallPathStop::RailImpacts(1),
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        RailModel::Mirror,
    );

    assert_eq!(path.rail_impacts, 1);
    assert_eq!(path.segments.len(), 1);
    assert_close(
        path.segments[0].end.as_ball_state().position.y().as_f64(),
        top_plane,
    );
    assert_close(
        path.final_state.as_ball_state().position.y().as_f64(),
        top_plane,
    );
    assert_close(path.elapsed.as_f64(), 0.5);

    let projected = path.projected_points(&table);
    assert_eq!(projected.len(), 2);
    assert_close(projected[0].angle_to(&projected[1]).as_degrees(), 30.0);
    assert!(diamond_value(&projected[1].y) > diamond_value(&projected[0].y));
}
