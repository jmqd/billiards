use bigdecimal::ToPrimitive;
use billiards::{
    human_tuned_preview_motion_config, trace_ball_path_with_rail_profile_on_table,
    trace_ball_path_with_rails_on_table, AngularVelocity3, BallPathStop, BallSetPhysicsSpec,
    BallState, Diamond, Inches, Inches2, InchesPerSecond, InchesPerSecondSq, MotionPhaseConfig,
    MotionTransitionConfig, OnTableBallState, OnTableMotionConfig, RadiansPerSecondSq, Rail,
    RailAngleReference, RailCollisionProfile, RailModel, RailTangentDirection,
    RollingResistanceModel, SlidingFrictionModel, SpinDecayModel, TableSpec, Velocity2,
    TYPICAL_BALL_RADIUS,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

fn assert_close_with_tolerance(actual: f64, expected: f64, tolerance: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta <= tolerance,
        "expected {expected} +/- {tolerance}, got {actual} (delta {delta})"
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

#[test]
fn tracing_from_a_zero_time_rail_impact_executes_the_rebound() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let state = on_table(BallState::on_table(
        inches2(10.0, top_plane),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));

    let path = trace_ball_path_with_rails_on_table(
        &state,
        BallPathStop::Duration(billiards::Seconds::new(0.1)),
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        RailModel::Mirror,
    );

    assert_eq!(path.rail_impacts, 1);
    assert_close(path.elapsed.as_f64(), 0.1);
    assert!(
        path.final_state.as_ball_state().position.y().as_f64() < top_plane,
        "the traced ball should rebound back into the table"
    );
    assert!(
        path.final_state.as_ball_state().velocity.y().as_f64() < 0.0,
        "the traced ball should carry the resolved outbound rail velocity"
    );
    assert_eq!(path.segments.len(), 1);
}

#[test]
fn corner_five_benchmark_track_reaches_the_formula_predicted_third_rail_target() {
    let table = TableSpec::default();
    let ball = BallSetPhysicsSpec::default();
    let motion = human_tuned_preview_motion_config();
    let profile = RailCollisionProfile::human_tuned();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let width = table.diamond_to_inches(Diamond::four()).as_f64();
    let start_x = radius;
    let start_y = radius;
    let first_rail_x = width - radius;
    // In the Part XII diagram the table is rotated relative to our coordinates: the long-axis
    // labels count 7-to-1 left-to-right, so first-rail number F=2 maps to engine y=6.
    let first_rail_y = table.diamond_to_inches(Diamond::from("6")).as_f64();
    let dx = first_rail_x - start_x;
    let dy = first_rail_y - start_y;
    let length = (dx * dx + dy * dy).sqrt();
    let speed = 90.0;
    let vx = speed * dx / length;
    let vy = speed * dy / length;
    let running_english = -1.5 * speed / radius;
    let state = on_table(BallState::on_table(
        inches2(start_x, start_y),
        Velocity2::new(Inches::from_f64(vx), Inches::from_f64(vy)),
        AngularVelocity3::new(-vy / radius, vx / radius, running_english),
    ));

    let path = trace_ball_path_with_rail_profile_on_table(
        &state,
        BallPathStop::RailImpacts(3),
        &ball,
        &table,
        &motion,
        RailModel::SpinAware,
        &profile,
    );

    assert_eq!(path.rail_impacts, 3);
    let final_position = path.final_state.as_ball_state().projected_position(&table);
    let rail_center_offset = diamond_value(&table.inches_to_diamond(TYPICAL_BALL_RADIUS.clone()));
    assert_close(diamond_value(&final_position.x), rail_center_offset);

    // VEPS GEMS Part XII gives the common pool-table benchmark as D=5, F=2, so T=D-F=3.
    // The same 7-to-1 long-axis numbering maps a left-rail y coordinate to T=8-y.
    let third_rail_number = 8.0 - diamond_value(&final_position.y);
    assert_close_with_tolerance(third_rail_number, 3.0, 0.15);
}
