use std::hint::black_box;
use std::time::Duration;

use billiards::dsl::{parse_dsl_to_game_state, parse_dsl_to_scenario};
use billiards::{
    compute_next_ball_ball_collision_during_current_phases_on_table,
    compute_next_transition_on_table, simulate_two_on_table_balls, strike_resting_ball_on_table,
    trace_ball_path_with_rails_on_table, Angle, AngularVelocity3, BallPathStop, BallSetPhysicsSpec,
    BallState, CollisionModel, CueStrikeConfig, CueTipContact, Inches, Inches2, InchesPerSecond,
    InchesPerSecondSq, MotionPhaseConfig, MotionTransitionConfig, OnTableBallState,
    OnTableMotionConfig, RadiansPerSecondSq, RestingOnTableBallState, RollingResistanceModel,
    Scale, Seconds, SlidingFrictionModel, SpinDecayModel, TableSpec, Velocity2,
    TYPICAL_BALL_RADIUS,
};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

const SINGLE_BALL_SHOT_DSL: &str = "ball cue at center\ncue_strike(default).mass_ratio(1.0).energy_loss(0.1)\nshot(cue).heading(30deg).speed(16ips).tip(side: 0.0R, height: 0.4R).using(default)\n";
const TWO_BALL_LAYOUT_DSL: &str = "ball cue at center\nball nine at (2, 4.75)\ncue_strike(default).mass_ratio(1.0).energy_loss(0.1)\nshot(cue).heading(0deg).speed(16ips).tip(side: 0.0R, height: 0.0R).using(default)\n";

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
        black_box(simulate_two_on_table_balls(
            black_box(a),
            black_box(b),
            black_box(Seconds::new(5.0)),
            black_box(ball_set),
            black_box(motion),
            black_box(CollisionModel::Ideal),
        ));
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

criterion_group!(
    name = benches;
    config = Criterion::default().warm_up_time(Duration::from_secs(1));
    targets = bench_parse_throughput, bench_function_throughput, bench_end_to_end_throughput
);
criterion_main!(benches);
