use std::hint::black_box;
use std::time::Duration;

use billiards::dsl::{parse_dsl_to_game_state, parse_dsl_to_scenario, DslScenario};
use billiards::{
    collide_ball_ball_detailed_on_table,
    compute_next_ball_ball_collision_during_current_phases_on_table,
    compute_next_ball_rail_impact_on_table, compute_next_transition_on_table,
    compute_next_two_ball_event_with_rails_on_table, simulate_two_on_table_balls,
    strike_resting_ball_on_table, trace_ball_path_with_rails_on_table, Angle, AngularVelocity3,
    Ball, BallPathStop, BallSetPhysicsSpec, BallSpec, BallState, BallType, CollisionModel,
    CueStrikeConfig, CueTipContact, Diamond, GameState, Inches, Inches2, InchesPerSecond,
    InchesPerSecondSq, MotionPhaseConfig, MotionTransitionConfig, OnTableBallState,
    OnTableMotionConfig, Position, RadiansPerSecondSq, Rail, RailAngleReference, RailModel,
    RailTangentDirection, RestingOnTableBallState, RollingResistanceModel, Seconds,
    SlidingFrictionModel, SpinDecayModel, TableSpec, Velocity2, TYPICAL_BALL_RADIUS,
};
use criterion::{criterion_group, criterion_main, Criterion};

const LAYOUT_DSL: &str = "ball cue at center\nball nine at (3, 7)\nball eight frozen left (6)\n";
const SINGLE_BALL_SHOT_DSL: &str = "ball cue at center\ncue_strike(default).mass_ratio(1.0).energy_loss(0.1)\nshot(cue).heading(30deg).speed(16ips).tip(side: 0.0R, height: 0.4R).using(default)\n";
const TWO_BALL_SHOT_DSL: &str = "ball cue at center\nball nine at (2, 4.75)\ncue_strike(default).mass_ratio(1.0).energy_loss(0.1)\nshot(cue).heading(0deg).speed(16ips).tip(side: 0.0R, height: 0.0R).using(default)\n";

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
    CueStrikeConfig::new(
        billiards::Scale::from_f64(1.0),
        billiards::Scale::from_f64(0.1),
    )
    .expect("benchmark cue strike config should validate")
}

fn direct_layout_game_state() -> GameState {
    let mut state = GameState::with_balls(
        TableSpec::default(),
        [
            Ball {
                ty: BallType::Cue,
                position: Position::new(2u8, 4u8),
                spec: BallSpec::default(),
            },
            Ball {
                ty: BallType::Nine,
                position: Position::new(3u8, 7u8),
                spec: BallSpec::default(),
            },
        ],
    );
    state.freeze_to_rail(
        Rail::Left,
        Diamond::six(),
        Ball {
            ty: BallType::Eight,
            position: Position::zeroed(),
            spec: BallSpec::default(),
        },
    );
    state.resolve_positions();
    state
}

fn direct_single_ball_inputs() -> (RestingOnTableBallState, billiards::Shot, CueStrikeConfig) {
    (
        resting_on_table(BallState::resting_at(inches2(25.0, 50.0))),
        billiards::Shot::new(
            Angle::from_north(1.0, 3.0_f64.sqrt()),
            InchesPerSecond::new(Inches::from_f64(16.0)),
            CueTipContact::new(
                billiards::Scale::from_f64(0.0),
                billiards::Scale::from_f64(0.4),
            )
            .expect("benchmark tip contact should validate"),
        )
        .expect("benchmark shot should validate"),
        cue_config(),
    )
}

fn direct_two_ball_inputs() -> (
    OnTableBallState,
    OnTableBallState,
    BallSetPhysicsSpec,
    OnTableMotionConfig,
) {
    let ball_set = BallSetPhysicsSpec::default();
    let motion = motion_config();
    let (resting, shot, cue) = (
        resting_on_table(BallState::resting_at(inches2(25.0, 50.0))),
        billiards::Shot::new(
            Angle::from_north(0.0, 1.0),
            InchesPerSecond::new(Inches::from_f64(16.0)),
            CueTipContact::center(),
        )
        .expect("benchmark shot should validate"),
        cue_config(),
    );
    let cue_ball = strike_resting_ball_on_table(&resting, &shot, &cue, &ball_set)
        .expect("benchmark strike should succeed");
    let object_ball = on_table(BallState::resting_at(inches2(25.0, 59.375)));

    (cue_ball, object_ball, ball_set, motion)
}

fn direct_seeded_single_ball() -> (
    OnTableBallState,
    BallSetPhysicsSpec,
    TableSpec,
    OnTableMotionConfig,
) {
    let ball_set = BallSetPhysicsSpec::default();
    let table = TableSpec::default();
    let motion = motion_config();
    let (resting, shot, cue) = direct_single_ball_inputs();
    let seeded = strike_resting_ball_on_table(&resting, &shot, &cue, &ball_set)
        .expect("benchmark strike should succeed");

    (seeded, ball_set, table, motion)
}

fn bank_state_near_top_rail(table: &TableSpec) -> OnTableBallState {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let heading = Rail::Top.bank_heading_toward(
        30.0,
        RailAngleReference::FromNormal,
        RailTangentDirection::Positive,
    );
    let speed = InchesPerSecond::new(Inches::from_f64(10.0));
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

fn collision_predictor_states() -> (
    OnTableBallState,
    OnTableBallState,
    BallSetPhysicsSpec,
    OnTableMotionConfig,
) {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    (
        on_table(BallState::on_table(
            inches2(0.0, -(2.0 * radius + 7.5)),
            Velocity2::new("0", "10"),
            AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
        )),
        on_table(BallState::resting_at(inches2(0.0, 0.0))),
        BallSetPhysicsSpec::default(),
        motion_config(),
    )
}

fn throw_aware_collision_states() -> (OnTableBallState, OnTableBallState) {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    (
        on_table(BallState::on_table(
            inches2(
                7.2 - radius * 2.0_f64.sqrt(),
                40.0 - radius * 2.0_f64.sqrt(),
            ),
            Velocity2::new("0", "10"),
            AngularVelocity3::new(-6.0, 0.0, -6.0),
        )),
        on_table(BallState::resting_at(inches2(7.2, 40.0))),
    )
}

fn run_direct_single_ball_shot_to_completion() {
    let (seeded, ball_set, table, motion) = direct_seeded_single_ball();
    black_box(trace_ball_path_with_rails_on_table(
        &seeded,
        BallPathStop::UntilRest,
        &ball_set,
        &table,
        &motion,
        RailModel::SpinAware,
    ));
}

fn run_dsl_single_ball_shot_to_completion_from_parse() {
    let scenario = parse_dsl_to_scenario(SINGLE_BALL_SHOT_DSL)
        .expect("benchmark shot scenario DSL should parse");
    let ball_set = BallSetPhysicsSpec::default();
    let motion = motion_config();

    black_box(
        scenario
            .trace_shot_path_with_rails_on_table(
                BallPathStop::UntilRest,
                &ball_set,
                &motion,
                RailModel::SpinAware,
            )
            .expect("benchmark shot trace should build")
            .expect("benchmark scenario should contain a shot"),
    );
}

fn run_preparsed_dsl_single_ball_shot_to_completion(
    scenario: &DslScenario,
    ball_set: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
) {
    black_box(
        scenario
            .trace_shot_path_with_rails_on_table(
                BallPathStop::UntilRest,
                ball_set,
                motion,
                RailModel::SpinAware,
            )
            .expect("benchmark shot trace should build")
            .expect("benchmark scenario should contain a shot"),
    );
}

fn run_direct_two_ball_shot_to_completion() {
    let (cue_ball, object_ball, ball_set, motion) = direct_two_ball_inputs();
    black_box(simulate_two_on_table_balls(
        &cue_ball,
        &object_ball,
        Seconds::new(5.0),
        &ball_set,
        &motion,
        CollisionModel::Ideal,
    ));
}

fn run_dsl_two_ball_shot_to_completion_from_parse() {
    let scenario =
        parse_dsl_to_scenario(TWO_BALL_SHOT_DSL).expect("benchmark two-ball shot DSL should parse");
    let ball_set = BallSetPhysicsSpec::default();
    let motion = motion_config();
    let cue_ball = scenario
        .strike_shot_on_table(&ball_set)
        .expect("benchmark strike should build")
        .expect("benchmark scenario should contain a shot");
    let object_ball = scenario
        .game_state
        .select_ball(BallType::Nine)
        .map(|ball| BallState::from_position(&ball.position, &scenario.game_state.table_spec))
        .map(on_table)
        .expect("benchmark scenario should place the nine ball");

    black_box(simulate_two_on_table_balls(
        &cue_ball,
        &object_ball,
        Seconds::new(5.0),
        &ball_set,
        &motion,
        CollisionModel::Ideal,
    ));
}

fn bench_setup(c: &mut Criterion) {
    let mut group = c.benchmark_group("setup");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(20);

    group.bench_function("direct/build_game_state", |b| {
        b.iter(|| black_box(direct_layout_game_state()))
    });
    group.bench_function("dsl/parse_to_game_state", |b| {
        b.iter(|| black_box(parse_dsl_to_game_state(black_box(LAYOUT_DSL)).unwrap()))
    });
    group.bench_function("direct/build_single_ball_inputs", |b| {
        b.iter(|| black_box(direct_single_ball_inputs()))
    });
    group.bench_function("dsl/parse_to_scenario", |b| {
        b.iter(|| black_box(parse_dsl_to_scenario(black_box(SINGLE_BALL_SHOT_DSL)).unwrap()))
    });

    group.finish();
}

fn bench_core_functions(c: &mut Criterion) {
    let motion = motion_config();
    let ball_set = BallSetPhysicsSpec::default();
    let table = TableSpec::default();
    let bank_state = bank_state_near_top_rail(&table);
    let (seeded, _, _, _) = direct_seeded_single_ball();
    let (collision_a, collision_b, collision_ball_set, collision_motion) =
        collision_predictor_states();
    let (throw_a, throw_b) = throw_aware_collision_states();

    let mut group = c.benchmark_group("core_functions");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(20);

    group.bench_function("compute_next_transition_on_table/sliding", |b| {
        b.iter(|| {
            black_box(compute_next_transition_on_table(
                black_box(&seeded),
                black_box(&ball_set),
                black_box(&motion),
            ))
        })
    });
    group.bench_function(
        "compute_next_ball_ball_collision_during_current_phases_on_table",
        |b| {
            b.iter(|| {
                black_box(
                    compute_next_ball_ball_collision_during_current_phases_on_table(
                        black_box(&collision_a),
                        black_box(&collision_b),
                        black_box(&collision_ball_set),
                        black_box(&collision_motion),
                    ),
                )
            })
        },
    );
    group.bench_function("compute_next_ball_rail_impact_on_table", |b| {
        b.iter(|| {
            black_box(compute_next_ball_rail_impact_on_table(
                black_box(&bank_state),
                black_box(&ball_set),
                black_box(&table),
                black_box(&motion),
            ))
        })
    });
    group.bench_function("compute_next_two_ball_event_with_rails_on_table", |b| {
        b.iter(|| {
            black_box(compute_next_two_ball_event_with_rails_on_table(
                black_box(&collision_a),
                black_box(&collision_b),
                black_box(&collision_ball_set),
                black_box(&table),
                black_box(&collision_motion),
            ))
        })
    });
    group.bench_function("collide_ball_ball_detailed_on_table/throw_aware", |b| {
        b.iter(|| {
            black_box(collide_ball_ball_detailed_on_table(
                black_box(&throw_a),
                black_box(&throw_b),
                black_box(CollisionModel::ThrowAware),
            ))
        })
    });
    group.bench_function(
        "trace_ball_path_with_rails_on_table/bank_duration_1s",
        |b| {
            b.iter(|| {
                black_box(trace_ball_path_with_rails_on_table(
                    black_box(&bank_state),
                    black_box(BallPathStop::Duration(Seconds::new(1.0))),
                    black_box(&ball_set),
                    black_box(&table),
                    black_box(&motion),
                    black_box(RailModel::Mirror),
                ))
            })
        },
    );

    group.finish();
}

fn bench_end_to_end(c: &mut Criterion) {
    let scenario = parse_dsl_to_scenario(SINGLE_BALL_SHOT_DSL)
        .expect("benchmark single-ball shot DSL should parse");
    let ball_set = BallSetPhysicsSpec::default();
    let motion = motion_config();

    let mut group = c.benchmark_group("end_to_end");
    group.measurement_time(Duration::from_secs(8));
    group.sample_size(10);

    group.bench_function("direct/strike_and_trace_until_rest", |b| {
        b.iter(run_direct_single_ball_shot_to_completion)
    });
    group.bench_function("dsl/parse_and_trace_until_rest", |b| {
        b.iter(run_dsl_single_ball_shot_to_completion_from_parse)
    });
    group.bench_function("dsl/preparsed_trace_until_rest", |b| {
        b.iter(|| run_preparsed_dsl_single_ball_shot_to_completion(&scenario, &ball_set, &motion))
    });
    group.bench_function("direct/two_ball_simulate_to_completion", |b| {
        b.iter(run_direct_two_ball_shot_to_completion)
    });
    group.bench_function("dsl/parse_and_simulate_two_ball_to_completion", |b| {
        b.iter(run_dsl_two_ball_shot_to_completion_from_parse)
    });

    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default().warm_up_time(Duration::from_secs(1));
    targets = bench_setup, bench_core_functions, bench_end_to_end
);
criterion_main!(benches);
