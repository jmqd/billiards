use std::hint::black_box;
use std::time::Duration;

use billiards::dsl::{parse_dsl_to_game_state, parse_dsl_to_scenario, DslScenario};
use billiards::{
    advance_to_next_n_ball_event_on_table,
    advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table, classify_motion_phase,
    collide_ball_ball_detailed_on_table, collide_ball_rail_on_table_with_radius_and_profile,
    compute_next_ball_ball_collision_during_current_phases_on_table,
    compute_next_ball_jaw_impact_on_table, compute_next_ball_pocket_capture_on_table,
    compute_next_ball_rail_impact_on_table,
    compute_next_n_ball_system_event_with_rails_and_pockets_on_table,
    compute_next_transition_on_table, compute_next_two_ball_event_with_rails_on_table,
    simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit,
    simulate_n_balls_with_rails_and_pockets_on_table_until_rest, simulate_two_on_table_balls,
    strike_resting_ball_on_table, trace_ball_path_with_rails_on_table, Angle, AngularVelocity3,
    Ball, BallBallCollisionConfig, BallPathStop, BallSetPhysicsSpec, BallSpec, BallState, BallType,
    CollisionModel, CueStrikeConfig, CueTipContact, Diamond, GameState, Inches, Inches2,
    InchesPerSecond, InchesPerSecondSq, MotionPhase, MotionPhaseConfig, MotionTransitionConfig,
    NBallSystemState, OnTableBallState, OnTableMotionConfig, Pocket, Position, RadiansPerSecondSq,
    Rail, RailAngleReference, RailCollisionProfile, RailModel, RailTangentDirection,
    RestingOnTableBallState, RollingResistanceModel, Seconds, SlidingFrictionModel, SpinDecayModel,
    TableSpec, Velocity2, CENTER_SPOT, TYPICAL_BALL_RADIUS,
};
use criterion::{criterion_group, criterion_main, Criterion, Throughput};

const LAYOUT_DSL: &str = "ball cue at center\nball nine at (3, 7)\nball eight frozen left (6)\n";
const SINGLE_BALL_SHOT_DSL: &str = "ball cue at center\ncue_strike(default).mass_ratio(1.0).energy_loss(0.1)\nshot(cue).heading(30deg).speed(16ips).tip(side: 0.0R, height: 0.4R).using(default)\n";
const TWO_BALL_SHOT_DSL: &str = "ball cue at center\nball nine at (2, 4.75)\ncue_strike(default).mass_ratio(1.0).energy_loss(0.1)\nshot(cue).heading(0deg).speed(16ips).tip(side: 0.0R, height: 0.0R).using(default)\n";
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

fn rail_resolution_matrix() -> Vec<(OnTableBallState, Rail)> {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let mut fixtures = Vec::with_capacity(216);

    for rail in [Rail::Left, Rail::Right, Rail::Bottom, Rail::Top] {
        for speed in [10.0_f64, 60.0, 120.0] {
            for tangent_ratio in [0.0_f64, 0.5, 1.0] {
                let normal_speed = speed / (1.0 + tangent_ratio * tangent_ratio).sqrt();
                let tangent_speed = tangent_ratio * normal_speed;
                let (vx, vy) = match rail {
                    Rail::Left => (-normal_speed, tangent_speed),
                    Rail::Right => (normal_speed, -tangent_speed),
                    Rail::Bottom => (tangent_speed, -normal_speed),
                    Rail::Top => (-tangent_speed, normal_speed),
                };

                for side_spin_factor in [-1.0_f64, 0.0, 1.0] {
                    for rolling_entry in [false, true] {
                        let (wx, wy) = if rolling_entry {
                            (-vy / radius, vx / radius)
                        } else {
                            (0.0, 0.0)
                        };
                        fixtures.push((
                            on_table(BallState::on_table(
                                inches2(20.0, 40.0),
                                Velocity2::new(Inches::from_f64(vx), Inches::from_f64(vy)),
                                AngularVelocity3::new(wx, wy, side_spin_factor * speed / radius),
                            )),
                            rail,
                        ));
                    }
                }
            }
        }
    }

    assert_eq!(fixtures.len(), 216);
    fixtures
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

fn curved_collision_predictor_states() -> (OnTableBallState, OnTableBallState) {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    (
        on_table(BallState::on_table(
            inches2(10.0, 20.0),
            Velocity2::new("0", "10"),
            AngularVelocity3::new(-10.0 / radius, 0.0, 2.0),
        )),
        on_table(BallState::resting_at(inches2(
            12.253_702_077_725_524,
            27.499_997_782_973_136,
        ))),
    )
}

fn zero_time_shared_contact_states() -> [OnTableBallState; 3] {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let contact_y = -3.0_f64.sqrt() * radius;
    [
        on_table(BallState::on_table(
            inches2(0.0, contact_y),
            Velocity2::new("0", "10"),
            AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
        )),
        on_table(BallState::resting_at(inches2(-radius, 0.0))),
        on_table(BallState::resting_at(inches2(radius, 0.0))),
    ]
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

fn direct_pocket_aware_inputs() -> (
    Vec<OnTableBallState>,
    BallSetPhysicsSpec,
    TableSpec,
    OnTableMotionConfig,
) {
    let table = TableSpec::default();
    let ball_set = BallSetPhysicsSpec::default();
    let motion = motion_config();
    let cue = on_table(BallState::on_table(
        inches2(
            40.0,
            table.diamond_to_inches(CENTER_SPOT.y.clone()).as_f64(),
        ),
        Velocity2::new("10", "0"),
        AngularVelocity3::new(0.0, 10.0 / TYPICAL_BALL_RADIUS.as_f64(), 0.0),
    ));
    let spinner = on_table(BallState::on_table(
        inches2(20.0, 20.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    ));

    (vec![cue, spinner], ball_set, table, motion)
}

fn rolling_side_pocket_state_at_angle(
    speed: f64,
    angle_degrees: f64,
    perpendicular_offset: f64,
    table: &TableSpec,
) -> OnTableBallState {
    let pocket_center = Pocket::CenterRight.aiming_center();
    let pocket_x = table.diamond_to_inches(pocket_center.x).as_f64();
    let pocket_y = table.diamond_to_inches(pocket_center.y).as_f64();
    let angle = angle_degrees.to_radians();
    let direction_x = angle.cos();
    let direction_y = angle.sin();
    let tangent_x = -direction_y;
    let tangent_y = direction_x;
    let radius = TYPICAL_BALL_RADIUS.as_f64();

    on_table(BallState::on_table(
        inches2(
            pocket_x - 10.0 * direction_x + perpendicular_offset * tangent_x,
            pocket_y - 10.0 * direction_y + perpendicular_offset * tangent_y,
        ),
        Velocity2::new(
            Inches::from_f64(speed * direction_x),
            Inches::from_f64(speed * direction_y),
        ),
        AngularVelocity3::new(
            -speed * direction_y / radius,
            speed * direction_x / radius,
            0.0,
        ),
    ))
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

fn run_preparsed_dsl_three_ball_pinball_event_limit(
    scenario: &DslScenario,
    ball_set: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
    collision_config: &BallBallCollisionConfig,
    rail_profile: &RailCollisionProfile,
) {
    black_box(
        scenario
            .simulate_shot_trace_with_physics_on_table_until_event_limit(
                ball_set,
                motion,
                CollisionModel::ThrowAware,
                collision_config,
                RailModel::SpinAware,
                rail_profile,
                8,
            )
            .expect("benchmark three-ball scenario trace should build")
            .expect("benchmark three-ball scenario should contain a shot"),
    );
}

fn run_preparsed_dsl_three_ball_pinball_system_only_event_limit(
    scenario: &DslScenario,
    ball_set: &BallSetPhysicsSpec,
    motion: &OnTableMotionConfig,
    collision_config: &BallBallCollisionConfig,
    rail_profile: &RailCollisionProfile,
) {
    let states = scenario
        .initial_shot_system_states_on_table(ball_set)
        .expect("benchmark three-ball initial state should build")
        .expect("benchmark three-ball scenario should contain a shot");
    black_box(
        simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit(
            &states,
            ball_set,
            &scenario.game_state.table_spec,
            motion,
            CollisionModel::ThrowAware,
            collision_config,
            RailModel::SpinAware,
            rail_profile,
            Some(8),
        )
        .expect("benchmark three-ball system should simulate"),
    );
}

fn run_direct_two_ball_shot_to_completion() {
    let (cue_ball, object_ball, ball_set, motion) = direct_two_ball_inputs();
    black_box(
        simulate_two_on_table_balls(
            &cue_ball,
            &object_ball,
            Seconds::new(5.0),
            &ball_set,
            &motion,
            CollisionModel::Ideal,
        )
        .expect("two-ball benchmark geometry should validate"),
    );
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

    black_box(
        simulate_two_on_table_balls(
            &cue_ball,
            &object_ball,
            Seconds::new(5.0),
            &ball_set,
            &motion,
            CollisionModel::Ideal,
        )
        .expect("two-ball benchmark geometry should validate"),
    );
}

fn run_cached_pocket_aware_until_rest() {
    let (states, ball_set, table, motion) = direct_pocket_aware_inputs();
    black_box(
        simulate_n_balls_with_rails_and_pockets_on_table_until_rest(
            &states,
            &ball_set,
            &table,
            &motion,
            CollisionModel::Ideal,
            RailModel::Mirror,
        )
        .expect("pocket-aware benchmark geometry should validate"),
    );
}

fn run_manual_pocket_aware_until_rest() {
    let (states, ball_set, table, motion) = direct_pocket_aware_inputs();
    let mut system_states = states
        .into_iter()
        .map(NBallSystemState::from)
        .collect::<Vec<_>>();

    loop {
        let advanced = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
            &system_states,
            &ball_set,
            &table,
            &motion,
            CollisionModel::Ideal,
            RailModel::Mirror,
        )
        .expect("pocket-aware benchmark geometry should validate");
        if advanced.event.is_none() {
            break;
        }
        system_states = advanced.states;
    }

    black_box(system_states);
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
            black_box(
                compute_next_two_ball_event_with_rails_on_table(
                    black_box(&collision_a),
                    black_box(&collision_b),
                    black_box(&collision_ball_set),
                    black_box(&table),
                    black_box(&collision_motion),
                )
                .expect("two-ball benchmark geometry should validate"),
            )
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

fn bench_pocket_predictors(c: &mut Criterion) {
    let table = TableSpec::default();
    let ball_set = BallSetPhysicsSpec::default();
    let motion = motion_config();
    let slow_capture = rolling_side_pocket_state_at_angle(10.0, 30.0, 0.0, &table);
    let slow_target_miss = rolling_side_pocket_state_at_angle(10.0, 30.0, 1.8, &table);
    let fast_capture = rolling_side_pocket_state_at_angle(200.0, 0.0, 0.0, &table);
    let mouth_width = table
        .diamond_to_inches(table.pocket_spec(Pocket::CenterRight).width.clone())
        .as_f64();
    let fast_jaw_hit = rolling_side_pocket_state_at_angle(200.0, 0.0, 0.5 * mouth_width, &table);
    let query_states = [NBallSystemState::from(slow_capture.clone())];

    let slow_capture_result =
        compute_next_ball_pocket_capture_on_table(&slow_capture, &ball_set, &table, &motion)
            .expect("slow side-pocket control should be captured");
    let slow_capture_state = slow_capture_result.state_at_capture.as_ball_state();
    assert_eq!(slow_capture_result.pocket, Pocket::CenterRight);
    assert_eq!(
        [
            slow_capture_result.time_until_capture.as_f64().to_bits(),
            slow_capture_state.position.x().as_f64().to_bits(),
            slow_capture_state.position.y().as_f64().to_bits(),
            slow_capture_state.velocity.x().as_f64().to_bits(),
            slow_capture_state.velocity.y().as_f64().to_bits(),
            slow_capture_state.angular_velocity.x().as_f64().to_bits(),
            slow_capture_state.angular_velocity.y().as_f64().to_bits(),
            slow_capture_state.angular_velocity.z().as_f64().to_bits(),
        ],
        [
            0x3ff4_776c_e2b5_9c84,
            0x4048_7000_0000_0000,
            0x4048_acdc_8f46_f71a,
            0x4008_f882_fca6_8d38,
            0x3ffc_d56f_c939_f8b4,
            0xbff9_a146_ebc1_c0a0,
            0x4006_323b_8b3e_b66b,
            0x0000_0000_0000_0000,
        ]
    );
    assert!(compute_next_ball_pocket_capture_on_table(
        &slow_target_miss,
        &ball_set,
        &table,
        &motion,
    )
    .is_none());
    assert!(
        compute_next_ball_pocket_capture_on_table(&fast_capture, &ball_set, &table, &motion,)
            .is_some()
    );
    assert!(
        compute_next_ball_jaw_impact_on_table(&fast_jaw_hit, &ball_set, &table, &motion).is_some()
    );
    assert!(
        compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
            &query_states,
            &ball_set,
            &table,
            &motion,
        )
        .expect("one-ball pocket benchmark geometry should validate")
        .is_some()
    );

    let mut group = c.benchmark_group("pocket_predictors");
    group.measurement_time(Duration::from_secs(8));
    group.sample_size(20);

    group.bench_function("capture/slow_side_30deg_hit", |b| {
        b.iter(|| {
            black_box(compute_next_ball_pocket_capture_on_table(
                black_box(&slow_capture),
                black_box(&ball_set),
                black_box(&table),
                black_box(&motion),
            ))
        })
    });
    group.bench_function("capture/slow_side_30deg_target_miss", |b| {
        b.iter(|| {
            black_box(compute_next_ball_pocket_capture_on_table(
                black_box(&slow_target_miss),
                black_box(&ball_set),
                black_box(&table),
                black_box(&motion),
            ))
        })
    });
    group.bench_function("capture/fast_side_analytic_hit", |b| {
        b.iter(|| {
            black_box(compute_next_ball_pocket_capture_on_table(
                black_box(&fast_capture),
                black_box(&ball_set),
                black_box(&table),
                black_box(&motion),
            ))
        })
    });
    group.bench_function("jaw/fast_side_hit", |b| {
        b.iter(|| {
            black_box(compute_next_ball_jaw_impact_on_table(
                black_box(&fast_jaw_hit),
                black_box(&ball_set),
                black_box(&table),
                black_box(&motion),
            ))
        })
    });
    group.bench_function("scheduler/one_ball_slow_side_capture", |b| {
        b.iter(|| {
            black_box(
                compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
                    black_box(&query_states),
                    black_box(&ball_set),
                    black_box(&table),
                    black_box(&motion),
                )
                .expect("one-ball pocket benchmark geometry should validate"),
            )
        })
    });

    group.finish();
}

fn bench_motion_phase_classification(c: &mut Criterion) {
    let ball_set = BallSetPhysicsSpec::default();
    let motion = motion_config();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let fixtures = [
        (
            "rest",
            on_table(BallState::resting_at(inches2(20.0, 20.0))),
            MotionPhase::Rest,
        ),
        (
            "spinning",
            on_table(BallState::on_table(
                inches2(20.0, 20.0),
                Velocity2::zero(),
                AngularVelocity3::new(0.0, 0.0, 6.0),
            )),
            MotionPhase::Spinning,
        ),
        (
            "rolling",
            on_table(BallState::on_table(
                inches2(20.0, 20.0),
                Velocity2::new("10", "0"),
                AngularVelocity3::new(0.0, 10.0 / radius, 3.0),
            )),
            MotionPhase::Rolling,
        ),
        (
            "sliding",
            on_table(BallState::on_table(
                inches2(20.0, 20.0),
                Velocity2::new("10", "0"),
                AngularVelocity3::zero(),
            )),
            MotionPhase::Sliding,
        ),
    ];

    for (_, state, expected) in &fixtures {
        assert_eq!(
            classify_motion_phase(state.as_ball_state(), &ball_set, &motion.phase),
            *expected
        );
    }

    let mut group = c.benchmark_group("motion_phase_classification");
    group.measurement_time(Duration::from_secs(8));
    group.sample_size(30);

    for (name, state, _) in &fixtures {
        group.bench_function(*name, |b| {
            b.iter(|| {
                black_box(classify_motion_phase(
                    black_box(state.as_ball_state()),
                    black_box(&ball_set),
                    black_box(&motion.phase),
                ))
            })
        });
    }

    group.finish();
}

fn bench_collision_predictor_paths(c: &mut Criterion) {
    let (linear_a, linear_b, ball_set, motion) = collision_predictor_states();
    let (curved_a, curved_b) = curved_collision_predictor_states();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let parallel_a = on_table(BallState::on_table(
        inches2(0.0, 0.0),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let parallel_b = on_table(BallState::on_table(
        inches2(10.0, 0.0),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let grazing_a = on_table(BallState::on_table(
        inches2(2.0 * radius + 1e-4, -7.5),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let grazing_b = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let fixtures = [
        ("linear_hit", linear_a, linear_b, true),
        ("curved_rolling_hit", curved_a, curved_b, true),
        ("parallel_miss", parallel_a, parallel_b, false),
        ("grazing_miss", grazing_a, grazing_b, false),
    ];

    for (name, first, second, expected_hit) in &fixtures {
        let prediction = compute_next_ball_ball_collision_during_current_phases_on_table(
            first, second, &ball_set, &motion,
        );
        assert_eq!(
            prediction.is_some(),
            *expected_hit,
            "collision fixture {name} changed branch"
        );
    }

    let mut group = c.benchmark_group("collision_predictor_paths");
    group.measurement_time(Duration::from_secs(8));
    group.sample_size(30);

    for (name, first, second, _) in &fixtures {
        group.bench_function(*name, |b| {
            b.iter(|| {
                black_box(
                    compute_next_ball_ball_collision_during_current_phases_on_table(
                        black_box(first),
                        black_box(second),
                        black_box(&ball_set),
                        black_box(&motion),
                    ),
                )
            })
        });
    }

    group.finish();
}

fn bench_shared_contact_resolution(c: &mut Criterion) {
    let states = zero_time_shared_contact_states();
    let ball_set = BallSetPhysicsSpec::default();
    let motion = motion_config();
    let control =
        advance_to_next_n_ball_event_on_table(&states, &ball_set, &motion, CollisionModel::Ideal)
            .expect("shared-contact benchmark geometry should validate");
    assert!(matches!(
        control.event,
        Some(billiards::NBallOnTableEvent::SharedBallBallContact {
            ref ball_ball_pairs,
            ..
        }) if ball_ball_pairs == &[(0, 1), (0, 2)]
    ));

    let mut group = c.benchmark_group("shared_contact_resolution");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(40);
    group.bench_function("symmetric_3_ball_2_contact", |b| {
        b.iter(|| {
            black_box(
                advance_to_next_n_ball_event_on_table(
                    black_box(&states),
                    black_box(&ball_set),
                    black_box(&motion),
                    black_box(CollisionModel::Ideal),
                )
                .expect("shared-contact benchmark geometry should validate"),
            )
        })
    });
    group.finish();
}

fn bench_rail_resolution(c: &mut Criterion) {
    let fixtures = rail_resolution_matrix();
    let profile = RailCollisionProfile::default();
    let ball_radius = BallSetPhysicsSpec::default().radius;

    let mut group = c.benchmark_group("rail_resolution");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(30);
    group.throughput(Throughput::Elements(fixtures.len() as u64));

    for (name, model) in [
        ("spin_aware_216_impacts", RailModel::SpinAware),
        (
            "restitution_only_control_216_impacts",
            RailModel::RestitutionOnly,
        ),
        ("mirror_control_216_impacts", RailModel::Mirror),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| {
                for (state, rail) in &fixtures {
                    black_box(collide_ball_rail_on_table_with_radius_and_profile(
                        black_box(state),
                        black_box(*rail),
                        black_box(ball_radius.clone()),
                        black_box(model),
                        black_box(&profile),
                    ));
                }
            })
        });
    }

    group.finish();
}

fn bench_pocket_cache_rebuild(c: &mut Criterion) {
    let (pocket_states, ball_set, table, motion) = direct_pocket_aware_inputs();
    let pocket_system_states = pocket_states
        .into_iter()
        .map(NBallSystemState::from)
        .collect::<Vec<_>>();
    let bank_system_states = vec![NBallSystemState::from(bank_state_near_top_rail(&table))];
    let collision_config = BallBallCollisionConfig::ideal();
    let rail_profile = RailCollisionProfile::default();

    for (name, states) in [
        ("one_ball_bank", bank_system_states.as_slice()),
        ("two_ball_pocket", pocket_system_states.as_slice()),
    ] {
        for max_events in [1usize, 2, 4] {
            let control =
                simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit(
                    states,
                    &ball_set,
                    &table,
                    &motion,
                    CollisionModel::Ideal,
                    &collision_config,
                    RailModel::Mirror,
                    &rail_profile,
                    Some(max_events),
                )
                .expect("cache benchmark geometry should validate");
            assert!(
                !control.events.is_empty() && control.events.len() <= max_events,
                "{name} should produce events up to the requested cap"
            );
        }
    }

    let mut group = c.benchmark_group("pocket_cache_rebuild");
    group.measurement_time(Duration::from_secs(8));
    group.sample_size(10);

    for (name, states) in [
        ("one_ball_bank", bank_system_states.as_slice()),
        ("two_ball_pocket", pocket_system_states.as_slice()),
    ] {
        for max_events in [1usize, 2, 4] {
            group.bench_function(format!("{name}/event_limit_{max_events}"), |b| {
                b.iter(|| {
                    black_box(
                        simulate_n_ball_system_with_physics_and_pockets_on_table_until_event_limit(
                            black_box(states),
                            black_box(&ball_set),
                            black_box(&table),
                            black_box(&motion),
                            black_box(CollisionModel::Ideal),
                            black_box(&collision_config),
                            black_box(RailModel::Mirror),
                            black_box(&rail_profile),
                            black_box(Some(max_events)),
                        )
                        .expect("cache benchmark geometry should validate"),
                    )
                })
            });
        }
    }

    group.finish();
}

fn bench_end_to_end(c: &mut Criterion) {
    let scenario = parse_dsl_to_scenario(SINGLE_BALL_SHOT_DSL)
        .expect("benchmark single-ball shot DSL should parse");
    let three_ball_scenario = parse_dsl_to_scenario(THREE_BALL_PINBALL_DSL)
        .expect("benchmark three-ball pinball DSL should parse");
    let ball_set = BallSetPhysicsSpec::default();
    let motion = motion_config();
    let collision_config = BallBallCollisionConfig::default();
    let rail_profile = RailCollisionProfile::default();

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
    group.bench_function(
        "dsl/preparsed_three_ball_pinball_system_only_event_limit_8",
        |b| {
            b.iter(|| {
                run_preparsed_dsl_three_ball_pinball_system_only_event_limit(
                    &three_ball_scenario,
                    &ball_set,
                    &motion,
                    &collision_config,
                    &rail_profile,
                )
            })
        },
    );
    group.bench_function("dsl/preparsed_three_ball_pinball_event_limit_8", |b| {
        b.iter(|| {
            run_preparsed_dsl_three_ball_pinball_event_limit(
                &three_ball_scenario,
                &ball_set,
                &motion,
                &collision_config,
                &rail_profile,
            )
        })
    });
    group.bench_function("direct/two_ball_simulate_to_completion", |b| {
        b.iter(run_direct_two_ball_shot_to_completion)
    });
    group.bench_function("dsl/parse_and_simulate_two_ball_to_completion", |b| {
        b.iter(run_dsl_two_ball_shot_to_completion_from_parse)
    });
    group.bench_function("direct/pocket_aware_until_rest_cached", |b| {
        b.iter(run_cached_pocket_aware_until_rest)
    });
    group.bench_function("direct/pocket_aware_until_rest_manual", |b| {
        b.iter(run_manual_pocket_aware_until_rest)
    });

    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default().warm_up_time(Duration::from_secs(1));
    targets = bench_setup, bench_core_functions, bench_pocket_predictors, bench_motion_phase_classification, bench_collision_predictor_paths, bench_shared_contact_resolution, bench_rail_resolution, bench_pocket_cache_rebuild, bench_end_to_end
);
criterion_main!(benches);
