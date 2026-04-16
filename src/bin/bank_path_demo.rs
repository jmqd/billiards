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

fn thirty_degree_top_rail_bank_state(table: &TableSpec) -> OnTableBallState {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let heading = Rail::Top.bank_heading_toward(
        30.0,
        RailAngleReference::FromNormal,
        RailTangentDirection::Positive,
    );
    let speed = InchesPerSecond::new("128");
    let speed_f64 = speed.as_f64();
    let velocity = Velocity2::from_polar(speed, heading);
    let impact_time = 0.5;
    let along_path_distance_to_impact =
        speed_f64 * impact_time - 0.5 * 5.0 * impact_time * impact_time;
    let radians = heading.as_degrees().to_radians();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;

    OnTableBallState::try_from(BallState::on_table(
        Inches2::new(
            Inches::from_f64(10.0),
            Inches::from_f64(top_plane - along_path_distance_to_impact * radians.cos()),
        ),
        velocity,
        AngularVelocity3::new(
            -speed_f64 * radians.cos() / radius,
            speed_f64 * radians.sin() / radius,
            0.0,
        ),
    ))
    .expect("demo bank state should validate as on-table")
}

fn main() {
    let table = TableSpec::brunswick_gc4_9ft();
    let motion = motion_config();
    let state = thirty_degree_top_rail_bank_state(&table);
    let path = trace_ball_path_with_rails_on_table(
        &state,
        BallPathStop::Duration(Seconds::new(1.0)),
        &BallSetPhysicsSpec::default(),
        &table,
        &motion,
        RailModel::SpinAware,
    );

    let sampled_points = path.sampled_points(
        Seconds::new(0.02),
        &BallSetPhysicsSpec::default(),
        &motion,
        &table,
    );

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
        "Heading: {:.1}° (30° from the top-rail normal, positive/right branch)",
        Rail::Top
            .bank_heading_toward(
                30.0,
                RailAngleReference::FromNormal,
                RailTangentDirection::Positive,
            )
            .as_degrees()
    );
    println!("Rail model: {:?}", RailModel::SpinAware);
    println!("Traced {} visible segment(s)", path.segments.len());
    println!("Drew {} sampled point(s)", sampled_points.len());
}
