use billiards::*;
use image::Rgba;
use std::path::Path;

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

fn cue_strike_config() -> CueStrikeConfig {
    CueStrikeConfig::new(Scale::from_f64(1.0), Scale::from_f64(0.1))
        .expect("demo cue-strike config should validate")
}

fn thirty_degree_top_rail_bank_shot() -> Shot {
    Shot::new(
        Rail::Top.bank_heading_toward(
            30.0,
            RailAngleReference::FromNormal,
            RailTangentDirection::Positive,
        ),
        InchesPerSecond::new("128"),
        CueTipContact::new(Scale::zero(), Scale::from_f64(0.4))
            .expect("demo cue-tip contact should validate"),
    )
    .expect("demo shot should validate")
}

fn thirty_degree_top_rail_bank_resting_ball(
    table: &TableSpec,
    shot: &Shot,
    cue: &CueStrikeConfig,
    ball_set: &BallSetPhysicsSpec,
) -> RestingOnTableBallState {
    let preview_ball = RestingOnTableBallState::try_from(BallState::resting_at(Inches2::zero()))
        .expect("preview resting ball should validate");
    let preview_state = strike_resting_ball_on_table(&preview_ball, shot, cue, ball_set)
        .expect("demo preview strike should succeed");
    let speed_f64 = preview_state.as_ball_state().speed().as_f64();
    let impact_time = 0.5;
    let along_path_distance_to_impact =
        speed_f64 * impact_time - 0.5 * 5.0 * impact_time * impact_time;
    let radians = shot.heading().as_degrees().to_radians();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - ball_set.radius.as_f64();

    RestingOnTableBallState::try_from(BallState::resting_at(Inches2::new(
        Inches::from_f64(10.0),
        Inches::from_f64(top_plane - along_path_distance_to_impact * radians.cos()),
    )))
    .expect("demo resting bank state should validate")
}

fn main() {
    let table = TableSpec::brunswick_gc4_9ft();
    let motion = motion_config();
    let ball_set = BallSetPhysicsSpec::default();
    let cue = cue_strike_config();
    let shot = thirty_degree_top_rail_bank_shot();
    let resting_ball = thirty_degree_top_rail_bank_resting_ball(&table, &shot, &cue, &ball_set);
    let state = strike_resting_ball_on_table(&resting_ball, &shot, &cue, &ball_set)
        .expect("demo strike should succeed");
    let path = trace_ball_path_with_rails_on_table(
        &state,
        BallPathStop::Duration(Seconds::new(1.0)),
        &ball_set,
        &table,
        &motion,
        RailModel::SpinAware,
    );

    let sampled_points = path.sampled_points(Seconds::new(0.02), &ball_set, &motion, &table);

    let mut game_state = GameState::with_balls(
        table.clone(),
        [Ball {
            ty: BallType::Cue,
            position: state.as_ball_state().projected_position(&table),
            spec: BallSpec::default(),
        }],
    );
    game_state.add_smooth_polyline(&sampled_points, Rgba([0, 0, 0, 255]));

    let output_path = Path::new("bank_path_demo.png");
    let image = game_state.draw_2d_diagram();
    write_png_to_file(&image, Some(output_path));

    println!("Wrote {:?}", output_path);
    println!(
        "Shot heading: {:.1}° (30° from the top-rail normal, positive/right branch)",
        shot.heading().as_degrees()
    );
    println!(
        "Cue speed: {:.1} in/s; tip contact: side {:.2}R, height {:.2}R",
        shot.cue_speed().as_f64(),
        shot.tip_contact().side_offset().as_f64(),
        shot.tip_contact().height_offset().as_f64(),
    );
    println!(
        "Seeded ball speed: {:.1} in/s; phase: {:?}",
        state.as_ball_state().speed().as_f64(),
        state.as_ball_state().motion_phase(ball_set.radius.clone())
    );
    println!("Rail model: {:?}", RailModel::SpinAware);
    println!("Traced {} visible segment(s)", path.segments.len());
    println!("Drew {} sampled point(s)", sampled_points.len());
}
