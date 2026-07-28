use std::hint::black_box;
use std::time::Duration;

use billiards::diagram::{render_scene_to_bytes, DiagramOutputFormat};
use billiards::dsl::{
    parse_dsl_to_game_state, parse_dsl_to_scenario, DslScenario, ScenarioShotTrace,
    ScenarioTraceRenderOptions,
};
use billiards::svg_generator::serialize_prepared_svg_report;
use billiards::visualization::{BallPathRenderOptions, PathColorMode};
use billiards::{
    compute_next_ball_ball_collision_during_current_phases_on_table,
    compute_next_transition_on_table, human_tuned_preview_motion_config,
    simulate_two_on_table_balls, strike_resting_ball_on_table, trace_ball_path_with_rails_on_table,
    Angle, AngularVelocity3, BallBallCollisionConfig, BallPathStop, BallSetPhysicsSpec, BallState,
    CollisionModel, CueStrikeConfig, CueTipContact, DiagramBackground, DiagramRenderOptions,
    Diamond, GameState, Inches, Inches2, InchesPerSecond, InchesPerSecondSq, MotionPhaseConfig,
    MotionTransitionConfig, OnTableBallState, OnTableMotionConfig, Position, RadiansPerSecondSq,
    RailCollisionProfile, RailModel, RestingOnTableBallState, RollingResistanceModel, Scale,
    Seconds, SlidingFrictionModel, SpinDecayModel, TableSpec, Velocity2, TYPICAL_BALL_RADIUS,
};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use image::Rgba;

const SINGLE_BALL_SHOT_DSL: &str = "ball cue at center\ncue_strike(default).mass_ratio(1.0).energy_loss(0.1)\nshot(cue).heading(30deg).speed(16ips).tip(side: 0.0R, height: 0.4R).using(default)\n";
const TWO_BALL_LAYOUT_DSL: &str = "ball cue at center\nball nine at (2, 4.75)\ncue_strike(default).mass_ratio(1.0).energy_loss(0.1)\nshot(cue).heading(0deg).speed(16ips).tip(side: 0.0R, height: 0.0R).using(default)\n";
const THREE_BALL_PINBALL_DSL: &str = "ball cue at (1.0, 4.0)\nball one at (2.0, 4.2)\nball two at (3.0, 4.9)\ncue_strike(default).mass_ratio(1.0).energy_loss(0.1)\nshot(cue).heading(80deg).speed(120ips).tip(side: 0.0R, height: 0.0R).using(default)\n";
const NINE_BALL_BREAK_DSL: &str =
    include_str!("../examples/scenarios/nine_ball_break_head_rail.billiards");
const POCKET_CAPTURE_DSL: &str =
    include_str!("../examples/scenarios/straight_in_side_pocket.billiards");

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
        black_box(
            trace_ball_path_with_rails_on_table(
                black_box(state),
                black_box(BallPathStop::UntilRest),
                black_box(ball_set),
                black_box(table),
                black_box(motion),
                black_box(billiards::RailModel::SpinAware),
            )
            .expect("throughput single-ball trace benchmark should succeed"),
        );
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

    group.measurement_time(Duration::from_secs(8));
    group.sample_size(30);

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

struct PreparedPlaybackFixture {
    name: &'static str,
    source: &'static str,
    trace: ScenarioShotTrace,
    table_spec: TableSpec,
    scenario: DslScenario,
}

fn prepare_playback_fixture(
    name: &'static str,
    source: &'static str,
    event_limit: usize,
) -> PreparedPlaybackFixture {
    let mut scenario = parse_dsl_to_scenario(source).expect("playback fixture should parse");
    scenario.game_state.resolve_positions();
    let table_spec = scenario.game_state.table_spec.clone();
    let ball_set = scenario.ball_set_physics_spec();
    let motion = human_tuned_preview_motion_config();
    let trace = scenario
        .simulate_shot_trace_with_preferred_physics_on_table_until_event_limit(
            &ball_set,
            &motion,
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
            event_limit,
        )
        .expect("playback fixture should simulate")
        .expect("playback fixture should contain a shot");
    PreparedPlaybackFixture {
        name,
        source,
        trace,
        table_spec,
        scenario,
    }
}

fn mix_playback_checksum(checksum: &mut u64, value: u64) {
    *checksum = checksum.rotate_left(9) ^ value.wrapping_mul(0x9e37_79b9_7f4a_7c15);
}

fn ball_type_checksum(ball: &billiards::BallType) -> u64 {
    match ball {
        billiards::BallType::Cue => 0,
        billiards::BallType::One => 1,
        billiards::BallType::Two => 2,
        billiards::BallType::Three => 3,
        billiards::BallType::Four => 4,
        billiards::BallType::Five => 5,
        billiards::BallType::Six => 6,
        billiards::BallType::Seven => 7,
        billiards::BallType::Eight => 8,
        billiards::BallType::Nine => 9,
        billiards::BallType::YellowCue => 10,
        billiards::BallType::Red => 11,
    }
}

fn playback_stream_checksum(trace: &ScenarioShotTrace, step: Seconds) -> (u64, u64, u64) {
    let mut frame_count = 0_u64;
    let mut ball_count = 0_u64;
    let mut checksum = 0xcbf2_9ce4_8422_2325_u64;
    for frame in trace.playback_frames_iter(step) {
        frame_count += 1;
        mix_playback_checksum(&mut checksum, frame.time.as_f64().to_bits());
        for ball in frame.balls {
            ball_count += 1;
            mix_playback_checksum(&mut checksum, ball_type_checksum(&ball.ball));
            let state = ball.state;
            for scalar in [
                state.position.x().as_f64(),
                state.position.y().as_f64(),
                state.height.as_f64(),
                state.velocity.x().as_f64(),
                state.velocity.y().as_f64(),
                state.vertical_velocity.as_f64(),
                state.angular_velocity.x().as_f64(),
                state.angular_velocity.y().as_f64(),
                state.angular_velocity.z().as_f64(),
            ] {
                mix_playback_checksum(&mut checksum, scalar.to_bits());
            }
        }
    }
    mix_playback_checksum(&mut checksum, frame_count);
    mix_playback_checksum(&mut checksum, ball_count);
    (frame_count, ball_count, checksum)
}

fn playback_benchmark_fixtures() -> Vec<PreparedPlaybackFixture> {
    vec![
        prepare_playback_fixture("two_ball_event_limit_64", TWO_BALL_LAYOUT_DSL, 64),
        prepare_playback_fixture("three_ball_event_limit_1", THREE_BALL_PINBALL_DSL, 1),
        prepare_playback_fixture("three_ball_event_limit_4", THREE_BALL_PINBALL_DSL, 4),
        prepare_playback_fixture("three_ball_event_limit_8", THREE_BALL_PINBALL_DSL, 8),
        prepare_playback_fixture("ten_ball_event_limit_1", NINE_BALL_BREAK_DSL, 1),
        prepare_playback_fixture("ten_ball_event_limit_8", NINE_BALL_BREAK_DSL, 8),
        prepare_playback_fixture("ten_ball_event_limit_32", NINE_BALL_BREAK_DSL, 32),
        prepare_playback_fixture("pocket_capture_event_limit_32", POCKET_CAPTURE_DSL, 32),
    ]
}

fn bench_playback_streaming(c: &mut Criterion) {
    let fixtures = playback_benchmark_fixtures();
    let steps = [
        ("20ms", 0.020_f64),
        ("5ms", 0.005_f64),
        ("2_5ms", 0.0025_f64),
    ];

    let mut owned_group = c.benchmark_group("playback_owned");
    owned_group.warm_up_time(Duration::from_secs(2));
    owned_group.measurement_time(Duration::from_secs(15));
    owned_group.sample_size(30);
    for fixture in &fixtures {
        for (step_name, step_seconds) in steps {
            let step = Seconds::new(step_seconds);
            let control = fixture.trace.playback_frames(step);
            let emitted_ball_states = control
                .iter()
                .map(|frame| frame.balls.len() as u64)
                .sum::<u64>();
            let timeline_segments = fixture
                .trace
                .ball_traces
                .iter()
                .map(|ball_trace| ball_trace.timeline_segments.len())
                .sum::<usize>();
            eprintln!(
                "playback fixture={} source_bytes={} balls={} events={} segments={} step={} frames={} emitted_ball_states={}",
                fixture.name,
                fixture.source.len(),
                fixture.trace.ball_traces.len(),
                fixture.trace.event_log.len(),
                timeline_segments,
                step_name,
                control.len(),
                emitted_ball_states,
            );
            owned_group.throughput(Throughput::Elements(emitted_ball_states));
            owned_group.bench_function(format!("{}/{}", fixture.name, step_name), |b| {
                b.iter(|| {
                    black_box(
                        fixture
                            .trace
                            .playback_frames(black_box(Seconds::new(step_seconds))),
                    )
                })
            });
        }
    }
    owned_group.finish();

    let mut stream_group = c.benchmark_group("playback_stream_fold");
    stream_group.warm_up_time(Duration::from_secs(2));
    stream_group.measurement_time(Duration::from_secs(15));
    stream_group.sample_size(30);
    for fixture in &fixtures {
        for (step_name, step_seconds) in steps {
            let (_, emitted_ball_states, checksum) =
                playback_stream_checksum(&fixture.trace, Seconds::new(step_seconds));
            assert_ne!(checksum, 0);
            stream_group.throughput(Throughput::Elements(emitted_ball_states));
            stream_group.bench_function(format!("{}/{}", fixture.name, step_name), |b| {
                b.iter(|| {
                    black_box(playback_stream_checksum(
                        black_box(&fixture.trace),
                        black_box(Seconds::new(step_seconds)),
                    ))
                })
            });
        }
    }
    stream_group.finish();

    let mut report_group = c.benchmark_group("playback_svg_report");
    report_group.warm_up_time(Duration::from_secs(2));
    report_group.measurement_time(Duration::from_secs(15));
    report_group.sample_size(30);
    for fixture in &fixtures {
        for (step_name, step_seconds) in steps {
            let trace_options = ScenarioTraceRenderOptions {
                path_render: BallPathRenderOptions {
                    max_time_step: Seconds::new(step_seconds),
                    ..ScenarioTraceRenderOptions::default().path_render
                },
                start_ghost_balls: true,
                event_markers: true,
                labels: false,
                spin_glyphs: true,
                path_color_mode: PathColorMode::Solid,
            };
            let render_options = DiagramRenderOptions::default();
            let rendered = fixture
                .trace
                .rendered_final_layout_with_trace_options(&fixture.scenario, &trace_options);
            let svg = rendered.draw_2d_svg_with_options(&render_options);
            let mut control = String::new();
            serialize_prepared_svg_report(
                &mut control,
                &svg,
                Some(&fixture.trace),
                &fixture.table_spec,
                Seconds::new(step_seconds),
            );
            eprintln!(
                "playback report fixture={} events={} step={} bytes={}",
                fixture.name,
                fixture.trace.event_log.len(),
                step_name,
                control.len(),
            );
            report_group.throughput(Throughput::Bytes(control.len() as u64));
            report_group.bench_function(format!("{}/{}", fixture.name, step_name), |b| {
                b.iter(|| {
                    let mut report = String::new();
                    serialize_prepared_svg_report(
                        &mut report,
                        black_box(&svg),
                        Some(black_box(&fixture.trace)),
                        black_box(&fixture.table_spec),
                        black_box(Seconds::new(step_seconds)),
                    );
                    black_box(report)
                })
            });
        }
    }
    report_group.finish();
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
    let transparent_scene = rendered.to_diagram_scene(&transparent_options);
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
    let transparent_png_control = render_scene_to_bytes(
        &transparent_scene,
        DiagramOutputFormat::Png,
        &transparent_options,
    );
    assert!(
        !svg_control.is_empty() && !png_control.is_empty() && !transparent_png_control.is_empty()
    );
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
                black_box(&transparent_scene),
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
    targets = bench_parse_throughput, bench_function_throughput, bench_end_to_end_throughput, bench_rendering_throughput, bench_playback_streaming
);
criterion_main!(benches);
