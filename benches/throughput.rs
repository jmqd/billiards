use std::hint::black_box;
use std::time::Duration;

use billiards::diagram::{render_scene_to_bytes, DiagramOutputFormat};
use billiards::dsl::{parse_dsl_to_game_state, parse_dsl_to_scenario, ScenarioTraceRenderOptions};
use billiards::visualization::{BallPathRenderOptions, PathColorMode};
use billiards::{
    compute_next_ball_ball_collision_during_current_phases_on_table,
    compute_next_transition_on_table, simulate_two_on_table_balls, strike_resting_ball_on_table,
    trace_ball_path_with_rails_on_table, Angle, AngularVelocity3, BallBallCollisionConfig,
    BallPathStop, BallSetPhysicsSpec, BallState, CollisionModel, CueStrikeConfig, CueTipContact,
    DiagramBackground, DiagramRenderOptions, Diamond, GameState, Inches, Inches2, InchesPerSecond,
    InchesPerSecondSq, MotionPhaseConfig, MotionTransitionConfig, OnTableBallState,
    OnTableMotionConfig, Position, RadiansPerSecondSq, RailCollisionProfile, RailModel,
    RestingOnTableBallState, RollingResistanceModel, Scale, Seconds, SlidingFrictionModel,
    SpinDecayModel, TableSpec, Velocity2, TYPICAL_BALL_RADIUS,
};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use image::Rgba;

const SINGLE_BALL_SHOT_DSL: &str = "ball cue at center\ncue_strike(default).mass_ratio(1.0).energy_loss(0.1)\nshot(cue).heading(30deg).speed(16ips).tip(side: 0.0R, height: 0.4R).using(default)\n";
const TWO_BALL_LAYOUT_DSL: &str = "ball cue at center\nball nine at (2, 4.75)\ncue_strike(default).mass_ratio(1.0).energy_loss(0.1)\nshot(cue).heading(0deg).speed(16ips).tip(side: 0.0R, height: 0.0R).using(default)\n";
const THREE_BALL_PINBALL_DSL: &str = "ball cue at (1.0, 4.0)\nball one at (2.0, 4.2)\nball two at (3.0, 4.9)\ncue_strike(default).mass_ratio(1.0).energy_loss(0.1)\nshot(cue).heading(80deg).speed(120ips).tip(side: 0.0R, height: 0.0R).using(default)\n";

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

fn inches2(x: f64, y: f64) -> Inches2 {
    Inches2::new(Inches::from_f64(x), Inches::from_f64(y))
}

fn on_table(state: BallState) -> OnTableBallState {
    OnTableBallState::try_from(state).expect("benchmark state should validate as on-table")
}

fn resting_on_table(state: BallState) -> RestingOnTableBallState {
    RestingOnTableBallState::try_from(state)
        .expect("benchmark state should validate as resting on-table")
}

fn cue_config() -> CueStrikeConfig {
    CueStrikeConfig::new(Scale::from_f64(1.0), Scale::from_f64(0.1))
        .expect("benchmark cue strike config should validate")
}

fn generate_seeded_single_ball_states(count: usize) -> Vec<OnTableBallState> {
    let ball_set = BallSetPhysicsSpec::default();
    let cue = cue_config();

    (0..count)
        .map(|i| {
            let row = (i / 8) as f64;
            let col = (i % 8) as f64;
            let x = 10.0 + col * 6.0;
            let y = 20.0 + row * 4.0;
            let heading_degrees = 10.0 + (i % 9) as f64 * 7.5;
            let speed = 12.0 + (i % 4) as f64 * 2.0;
            let tip_height = 0.15 + (i % 4) as f64 * 0.05;
            let heading = Angle::from_north(
                heading_degrees.to_radians().sin(),
                heading_degrees.to_radians().cos(),
            );
            let resting = resting_on_table(BallState::resting_at(inches2(x, y)));
            let shot = billiards::Shot::new(
                heading,
                InchesPerSecond::new(Inches::from_f64(speed)),
                CueTipContact::new(Scale::zero(), Scale::from_f64(tip_height))
                    .expect("benchmark tip contact should validate"),
            )
            .expect("benchmark shot should validate");

            strike_resting_ball_on_table(&resting, &shot, &cue, &ball_set)
                .expect("benchmark strike should succeed")
        })
        .collect()
}

fn generate_two_ball_workload(count: usize) -> Vec<(OnTableBallState, OnTableBallState)> {
    let ball_set = BallSetPhysicsSpec::default();
    let cue = cue_config();

    (0..count)
        .map(|i| {
            let lane = (i % 10) as f64;
            let depth = (i / 10) as f64;
            let cue_x = 18.0 + lane * 2.25;
            let cue_y = 18.0 + depth * 2.5;
            let object_x = cue_x + ((i % 3) as f64 - 1.0) * 0.75;
            let object_y = cue_y + 9.0 + (i % 5) as f64 * 0.5;
            let speed = 14.0 + (i % 4) as f64;
            let resting = resting_on_table(BallState::resting_at(inches2(cue_x, cue_y)));
            let shot = billiards::Shot::new(
                Angle::from_north(0.0, 1.0),
                InchesPerSecond::new(Inches::from_f64(speed)),
                CueTipContact::center(),
            )
            .expect("benchmark shot should validate");
            let cue_ball = strike_resting_ball_on_table(&resting, &shot, &cue, &ball_set)
                .expect("benchmark strike should succeed");
            let object_ball = on_table(BallState::resting_at(inches2(object_x, object_y)));

            (cue_ball, object_ball)
        })
        .collect()
}

fn generate_transition_states(count: usize) -> Vec<OnTableBallState> {
    generate_seeded_single_ball_states(count)
}

fn generate_collision_workload(count: usize) -> Vec<(OnTableBallState, OnTableBallState)> {
    let radius = TYPICAL_BALL_RADIUS.as_f64();

    (0..count)
        .map(|i| {
            let start_gap = 6.0 + (i % 6) as f64 * 0.75;
            let speed = 8.0 + (i % 5) as f64;
            (
                on_table(BallState::on_table(
                    inches2((i % 17) as f64, -(2.0 * radius + start_gap)),
                    Velocity2::new(Inches::zero(), Inches::from_f64(speed)),
                    AngularVelocity3::new(-speed / radius, 0.0, 0.0),
                )),
                on_table(BallState::resting_at(inches2((i % 17) as f64, 0.0))),
            )
        })
        .collect()
}

fn run_parse_game_state_batch(batch_size: usize) {
    for _ in 0..batch_size {
        black_box(
            parse_dsl_to_game_state(black_box(TWO_BALL_LAYOUT_DSL))
                .expect("benchmark DSL should parse to game state"),
        );
    }
}

fn run_parse_scenario_batch(batch_size: usize) {
    for _ in 0..batch_size {
        black_box(
            parse_dsl_to_scenario(black_box(SINGLE_BALL_SHOT_DSL))
                .expect("benchmark DSL should parse to scenario"),
        );
    }
}

fn run_transition_batch(
    states: &[OnTableBallState],
    ball_set: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
) {
    for state in states {
        black_box(compute_next_transition_on_table(
            black_box(state),
            black_box(ball_set),
            black_box(motion),
        ));
    }
}

fn run_collision_prediction_batch(
    workload: &[(OnTableBallState, OnTableBallState)],
    ball_set: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
) {
    for (a, b) in workload {
        black_box(
            compute_next_ball_ball_collision_during_current_phases_on_table(
                black_box(a),
                black_box(b),
                black_box(ball_set),
                black_box(motion),
            ),
        );
    }
}

fn run_single_ball_trace_batch(
    states: &[OnTableBallState],
    ball_set: &BallSetPhysicsSpec,
    table: &TableSpec,
    motion: &OnTableMotionConfig,
) {
    for state in states {
        black_box(trace_ball_path_with_rails_on_table(
            black_box(state),
            black_box(BallPathStop::UntilRest),
            black_box(ball_set),
            black_box(table),
            black_box(motion),
            black_box(billiards::RailModel::SpinAware),
        ));
    }
}

fn run_two_ball_sim_batch(
    workload: &[(OnTableBallState, OnTableBallState)],
    ball_set: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
) {
    for (a, b) in workload {
        black_box(
            simulate_two_on_table_balls(
                black_box(a),
                black_box(b),
                black_box(Seconds::new(5.0)),
                black_box(ball_set),
                black_box(motion),
                black_box(CollisionModel::Ideal),
            )
            .expect("two-ball benchmark geometry should validate"),
        );
    }
}

fn bench_parse_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_parse");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(10);

    for batch_size in [100usize, 1_000, 10_000] {
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("dsl_parse_to_game_state", batch_size),
            &batch_size,
            |b, &batch_size| b.iter(|| run_parse_game_state_batch(batch_size)),
        );
        group.bench_with_input(
            BenchmarkId::new("dsl_parse_to_scenario", batch_size),
            &batch_size,
            |b, &batch_size| b.iter(|| run_parse_scenario_batch(batch_size)),
        );
    }

    group.finish();
}

fn bench_function_throughput(c: &mut Criterion) {
    let ball_set = BallSetPhysicsSpec::default();
    let motion = motion_config();
    let mut group = c.benchmark_group("throughput_functions");
    group.measurement_time(Duration::from_secs(6));
    group.sample_size(10);

    for batch_size in [100usize, 1_000, 10_000] {
        let states = generate_transition_states(batch_size);
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("compute_next_transition_on_table", batch_size),
            &batch_size,
            |b, _| b.iter(|| run_transition_batch(&states, &ball_set, &motion)),
        );
    }

    for batch_size in [100usize, 1_000] {
        let workload = generate_collision_workload(batch_size);
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new(
                "compute_next_ball_ball_collision_during_current_phases_on_table",
                batch_size,
            ),
            &batch_size,
            |b, _| b.iter(|| run_collision_prediction_batch(&workload, &ball_set, &motion)),
        );
    }

    group.finish();
}

fn bench_end_to_end_throughput(c: &mut Criterion) {
    let ball_set = BallSetPhysicsSpec::default();
    let motion = motion_config();
    let table = TableSpec::default();
    let mut group = c.benchmark_group("throughput_end_to_end");
    group.measurement_time(Duration::from_secs(8));
    group.sample_size(10);

    for batch_size in [10usize, 50, 100] {
        let states = generate_seeded_single_ball_states(batch_size);
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("trace_single_ball_until_rest", batch_size),
            &batch_size,
            |b, _| b.iter(|| run_single_ball_trace_batch(&states, &ball_set, &table, &motion)),
        );
    }

    for batch_size in [10usize, 25, 50] {
        let workload = generate_two_ball_workload(batch_size);
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("simulate_two_ball_to_completion", batch_size),
            &batch_size,
            |b, _| b.iter(|| run_two_ball_sim_batch(&workload, &ball_set, &motion)),
        );
    }

    group.finish();
}

fn bench_rendering_throughput(c: &mut Criterion) {
    let ball_set = BallSetPhysicsSpec::default();
    let motion = motion_config();
    let scenario =
        parse_dsl_to_scenario(TWO_BALL_LAYOUT_DSL).expect("benchmark scenario should parse");
    let trace = scenario
        .simulate_shot_trace_with_preferred_physics_on_table_until_rest(
            &ball_set,
            &motion,
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        )
        .expect("benchmark scenario should simulate")
        .expect("benchmark scenario should contain a shot");
    let pinball_scenario = parse_dsl_to_scenario(THREE_BALL_PINBALL_DSL)
        .expect("benchmark pinball scenario should parse");
    let pinball_trace = pinball_scenario
        .simulate_shot_trace_with_physics_on_table_until_event_limit(
            &ball_set,
            &motion,
            CollisionModel::ThrowAware,
            &BallBallCollisionConfig::default(),
            RailModel::SpinAware,
            &RailCollisionProfile::default(),
            8,
        )
        .expect("benchmark pinball scenario should simulate")
        .expect("benchmark pinball scenario should contain a shot");
    let trace_options = ScenarioTraceRenderOptions {
        path_render: BallPathRenderOptions {
            max_time_step: Seconds::new(0.005),
            ..ScenarioTraceRenderOptions::default().path_render
        },
        start_ghost_balls: true,
        event_markers: true,
        labels: false,
        spin_glyphs: true,
        path_color_mode: PathColorMode::MotionPhase,
    };
    let render_options = DiagramRenderOptions {
        scale_factor: 1,
        background: DiagramBackground::Table,
    };
    let rendered = trace.rendered_final_layout_with_trace_options(&scenario, &trace_options);
    let scene = rendered.to_diagram_scene(&render_options);
    let transparent_options = DiagramRenderOptions {
        scale_factor: 1,
        background: DiagramBackground::Transparent,
    };
    let long_polyline_points = (0..1_000)
        .map(|index| {
            let t = index as f64 / 999.0;
            let x = 0.1 + 3.8 * t;
            let y = 4.0 + 3.0 * (t * std::f64::consts::TAU * 8.0).sin();
            Position::new(
                Diamond::from(x.to_string().as_str()),
                Diamond::from(y.to_string().as_str()),
            )
        })
        .collect::<Vec<_>>();
    let mut long_polyline_state = GameState::new(TableSpec::default());
    long_polyline_state.add_smooth_polyline(&long_polyline_points, Rgba([0x09, 0x6b, 0xd8, 0xff]));
    let long_polyline_scene = long_polyline_state.to_diagram_scene(&transparent_options);
    let svg_control = render_scene_to_bytes(&scene, DiagramOutputFormat::Svg, &render_options);
    let png_control = render_scene_to_bytes(&scene, DiagramOutputFormat::Png, &render_options);
    assert!(!svg_control.is_empty() && !png_control.is_empty());
    let long_polyline_svg_control = render_scene_to_bytes(
        &long_polyline_scene,
        DiagramOutputFormat::Svg,
        &transparent_options,
    );
    assert!(!long_polyline_svg_control.is_empty());

    let mut stage_group = c.benchmark_group("render_stages");
    stage_group.measurement_time(Duration::from_secs(8));
    stage_group.sample_size(10);
    stage_group.bench_function("scene_build/rich_trace", |b| {
        b.iter(|| {
            black_box(rendered.to_diagram_scene(black_box(&render_options)));
        })
    });
    stage_group.bench_function("backend/svg_rich_trace", |b| {
        b.iter(|| {
            black_box(render_scene_to_bytes(
                black_box(&scene),
                black_box(DiagramOutputFormat::Svg),
                black_box(&render_options),
            ));
        })
    });
    stage_group.bench_function("backend/svg_long_polyline_1000", |b| {
        b.iter(|| {
            black_box(render_scene_to_bytes(
                black_box(&long_polyline_scene),
                black_box(DiagramOutputFormat::Svg),
                black_box(&transparent_options),
            ));
        })
    });
    stage_group.bench_function("backend/png_table_rich_trace", |b| {
        b.iter(|| {
            black_box(render_scene_to_bytes(
                black_box(&scene),
                black_box(DiagramOutputFormat::Png),
                black_box(&render_options),
            ));
        })
    });
    stage_group.bench_function("backend/png_transparent_rich_trace", |b| {
        b.iter(|| {
            black_box(render_scene_to_bytes(
                black_box(&scene),
                black_box(DiagramOutputFormat::Png),
                black_box(&transparent_options),
            ));
        })
    });
    stage_group.finish();

    let mut playback_group = c.benchmark_group("playback_scaling");
    playback_group.measurement_time(Duration::from_secs(8));
    playback_group.sample_size(10);
    for (fixture_name, fixture_trace) in [
        ("two_ball", &trace),
        ("three_ball_event_limit_8", &pinball_trace),
    ] {
        for (step_name, step_seconds) in [("20ms", 0.020_f64), ("5ms", 0.005), ("2_5ms", 0.0025)] {
            let control = fixture_trace.playback_frames(Seconds::new(step_seconds));
            let emitted_ball_states = control
                .iter()
                .map(|frame| frame.balls.len() as u64)
                .sum::<u64>();
            assert!(!control.is_empty() && emitted_ball_states > 0);
            playback_group.throughput(Throughput::Elements(emitted_ball_states));
            playback_group.bench_function(format!("{fixture_name}/{step_name}"), |b| {
                b.iter(|| {
                    black_box(fixture_trace.playback_frames(black_box(Seconds::new(step_seconds))))
                })
            });
        }
    }
    playback_group.finish();

    c.bench_function("throughput_rendering/trace_final_layout_svg", |b| {
        b.iter(|| {
            let rendered = trace.rendered_final_layout_with_trace_options(
                black_box(&scenario),
                black_box(&trace_options),
            );
            black_box(rendered.render_2d_diagram_with_options(
                DiagramOutputFormat::Svg,
                black_box(&render_options),
            ));
        });
    });

    c.bench_function("throughput_rendering/trace_playback_frames_2_5ms", |b| {
        b.iter(|| black_box(trace.playback_frames(black_box(Seconds::new(0.0025)))));
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default().warm_up_time(Duration::from_secs(1));
    targets = bench_parse_throughput, bench_function_throughput, bench_end_to_end_throughput, bench_rendering_throughput
);
criterion_main!(benches);
