use billiards::{
    advance_to_next_n_ball_event_on_table, advance_to_next_n_ball_event_with_rails_on_table,
    advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table,
    advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table,
    compute_next_airborne_ball_rail_crossing,
    compute_next_ball_ball_collision_during_current_phases_on_table,
    compute_next_ball_jaw_impact_on_table, compute_next_ball_pocket_capture_on_table,
    compute_next_ball_rail_impact_on_table, compute_next_n_ball_event_on_table,
    compute_next_n_ball_system_event_with_rails_and_pockets_on_table,
    resolve_n_ball_system_event_with_physics_and_pockets_on_table,
    simulate_n_balls_with_rails_and_pockets_on_table_until_rest,
    simulate_n_balls_with_rails_on_table_until_rest, AngularVelocity3, BallBallCollisionConfig,
    BallSetPhysicsSpec, BallState, CollisionModel, Diamond, Inches, Inches2, InchesPerSecondSq,
    MotionPhase, MotionPhaseConfig, MotionTransitionConfig, NBallGeometryError, NBallOnTableEvent,
    NBallSystemEvent, NBallSystemState, OnTableBallState, OnTableMotionConfig, Pocket, PocketJaw,
    PocketJawGeometry, PocketShapeSpec, PredictedAirborneBallBallCollision,
    PredictedAirborneBallRailCrossing, RadiansPerSecondSq, Rail, RailModel, RollingResistanceModel,
    Scale, Seconds, SlidingFrictionModel, SpinDecayModel, TableSpec, Velocity2, CENTER_SPOT,
    STANDARD_GRAVITY_INCHES_PER_SECOND_SQUARED, TYPICAL_BALL_RADIUS,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

fn assert_near(actual: f64, expected: f64, tolerance: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta <= tolerance,
        "expected {expected}, got {actual} (delta {delta}, tolerance {tolerance})"
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

#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
fn on_table_state_bits(state: &OnTableBallState) -> [u64; 9] {
    let state = state.as_ball_state();
    [
        state.position.x().as_f64().to_bits(),
        state.position.y().as_f64().to_bits(),
        state.height.as_f64().to_bits(),
        state.velocity.x().as_f64().to_bits(),
        state.velocity.y().as_f64().to_bits(),
        state.vertical_velocity.as_f64().to_bits(),
        state.angular_velocity.x().as_f64().to_bits(),
        state.angular_velocity.y().as_f64().to_bits(),
        state.angular_velocity.z().as_f64().to_bits(),
    ]
}

#[test]
fn system_zero_friction_nonideal_shared_contact_matches_coupled_normal_limit() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let states = [
        NBallSystemState::OnTable(on_table(BallState::on_table(
            inches2(0.0, -3.0_f64.sqrt() * radius),
            Velocity2::new("0", "10"),
            AngularVelocity3::zero(),
        ))),
        NBallSystemState::OnTable(on_table(BallState::resting_at(inches2(-radius, 0.0)))),
        NBallSystemState::OnTable(on_table(BallState::resting_at(inches2(radius, 0.0)))),
    ];
    let advanced = advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table(
        &states,
        &BallSetPhysicsSpec::default(),
        &TableSpec::default(),
        &motion_config(),
        CollisionModel::ThrowAware,
        &BallBallCollisionConfig::new(Scale::from_f64(1.0), Scale::zero()),
        RailModel::SpinAware,
        &billiards::RailCollisionProfile::default(),
    )
    .expect("zero-friction nonideal shared contact should use the coupled normal solver");

    match advanced.event.expect("shared contact should be reported") {
        NBallSystemEvent::SharedBallBallContact {
            time_until_contact,
            ball_ball_pairs,
            resolution,
            ..
        } => {
            assert_close(time_until_contact.as_f64(), 0.0);
            assert_eq!(ball_ball_pairs, vec![(0, 1), (0, 2)]);
            assert_eq!(resolution.as_str(), "coupled_normal");
        }
        other => panic!("expected coupled shared contact, got {other:?}"),
    }
    let cue = advanced.states[0].as_ball_state();
    let left = advanced.states[1].as_ball_state();
    let right = advanced.states[2].as_ball_state();
    assert_near(cue.velocity.y().as_f64(), -2.0, 1e-5);
    assert_near(left.velocity.x().as_f64(), -2.0 * 3.0_f64.sqrt(), 1e-5);
    assert_near(left.velocity.y().as_f64(), 6.0, 1e-5);
    assert_near(right.velocity.x().as_f64(), 2.0 * 3.0_f64.sqrt(), 1e-5);
    assert_near(right.velocity.y().as_f64(), 6.0, 1e-5);
}

#[test]
fn curved_rolling_ball_reaches_the_center_right_first_jaw() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let state = on_table(BallState::on_table(
        inches2(48.746_297_922_274_48, 45.000_002_217_026_86),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 2.0),
    ));

    let impact = compute_next_ball_jaw_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &TableSpec::default(),
        &motion_config(),
    )
    .expect("the canonical rightward curve enters the first center-right jaw before one second");
    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    {
        assert_eq!(
            impact.time_until_impact.as_f64().to_bits(),
            4_607_247_905_935_396_864
        );
        assert_eq!(
            on_table_state_bits(&impact.state_at_impact),
            [
                4_632_057_927_017_301_893,
                4_632_595_693_405_055_188,
                0,
                4_575_743_055_504_650_378,
                4_617_233_651_800_681_331,
                0,
                13_839_989_284_947_313_425,
                4_574_809_015_440_077_500,
                0,
            ]
        );
    }
    let direct_time = impact.time_until_impact.as_f64();

    assert_eq!(impact.pocket, Pocket::CenterRight);
    assert_eq!(impact.jaw, PocketJaw::First);
    assert!(
        direct_time > 0.0 && direct_time < 1.1,
        "the physical-jaw contact should remain in the current rolling phase; time={direct_time}"
    );
    let at_impact = impact.state_at_impact.as_ball_state();
    let dx = at_impact.position.x().as_f64() - 50.0;
    let dy = at_impact.position.y().as_f64() - 52.625;
    assert!(
        dx * at_impact.velocity.x().as_f64() + dy * at_impact.velocity.y().as_f64() < 0.0,
        "jaw contact must be entering"
    );

    let system_event = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &[NBallSystemState::OnTable(state)],
        &BallSetPhysicsSpec::default(),
        &TableSpec::default(),
        &motion_config(),
    )
    .expect("pocket-aware test geometry should validate")
    .expect("the pocket-aware scheduler should retain the canonical jaw event");
    match system_event {
        NBallSystemEvent::BallJawImpact { ball_index, impact } => {
            assert_eq!(ball_index, 0);
            assert_eq!(impact.pocket, Pocket::CenterRight);
            assert_eq!(impact.jaw, PocketJaw::First);
            assert_close(impact.time_until_impact.as_f64(), direct_time);
        }
        other => panic!("expected the scheduled center-right jaw impact, got {other:?}"),
    }
}

#[test]
fn opposite_spin_curve_away_does_not_create_a_center_right_jaw_impact() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let state = on_table(BallState::on_table(
        inches2(48.746_297_922_274_48, 45.000_002_217_026_86),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, -2.0),
    ));

    assert!(compute_next_ball_jaw_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &TableSpec::default(),
        &motion_config(),
    )
    .is_none());
}

fn unwrap_on_table_states(states: &[NBallSystemState]) -> Vec<OnTableBallState> {
    states
        .iter()
        .map(|state| match state {
            NBallSystemState::OnTable(on_table) => on_table.clone(),
            other => panic!("expected on-table state, got {other:?}"),
        })
        .collect()
}

fn assert_events_equivalent(
    expected: Option<&NBallOnTableEvent>,
    actual: Option<&NBallSystemEvent>,
    label: &str,
) {
    match (expected, actual) {
        (None, None) => {}
        (
            Some(NBallOnTableEvent::BallBallCollision {
                first_ball_index: expected_first,
                second_ball_index: expected_second,
                collision: expected_collision,
            }),
            Some(NBallSystemEvent::BallBallCollision {
                first_ball_index: actual_first,
                second_ball_index: actual_second,
                collision: actual_collision,
            }),
        ) => {
            assert_eq!(
                (*actual_first, *actual_second),
                (*expected_first, *expected_second)
            );
            assert_close(
                actual_collision.time_until_impact.as_f64(),
                expected_collision.time_until_impact.as_f64(),
            );
        }
        (
            Some(NBallOnTableEvent::SharedBallBallContact {
                time_until_contact: expected_time,
                ball_indices: expected_indices,
                ball_ball_pairs: expected_pairs,
                resolution: expected_resolution,
            }),
            Some(NBallSystemEvent::SharedBallBallContact {
                time_until_contact: actual_time,
                ball_indices: actual_indices,
                ball_ball_pairs: actual_pairs,
                resolution: actual_resolution,
            }),
        ) => {
            assert_close(actual_time.as_f64(), expected_time.as_f64());
            assert_eq!(actual_indices, expected_indices);
            assert_eq!(actual_pairs, expected_pairs);
            assert_eq!(actual_resolution, expected_resolution);
        }
        (
            Some(NBallOnTableEvent::BallRailImpact {
                ball_index: expected_ball,
                impact: expected_impact,
            }),
            Some(NBallSystemEvent::BallRailImpact {
                ball_index: actual_ball,
                impact: actual_impact,
            }),
        ) => {
            assert_eq!(actual_ball, expected_ball);
            assert_eq!(actual_impact, expected_impact);
        }
        (
            Some(NBallOnTableEvent::MotionTransition {
                ball_index: expected_ball,
                transition: expected_transition,
            }),
            Some(NBallSystemEvent::MotionTransition {
                ball_index: actual_ball,
                transition: actual_transition,
            }),
        ) => {
            assert_eq!(actual_ball, expected_ball);
            assert_eq!(actual_transition, expected_transition);
        }
        (expected, actual) => panic!(
            "{label}: expected pocket-aware event {actual:?} to match rail-aware event {expected:?}"
        ),
    }
}

fn assert_pocket_aware_matches_rail_aware(states: &[OnTableBallState], label: &str) {
    let ball = BallSetPhysicsSpec::default();
    let table = TableSpec::default();
    let motion = motion_config();
    let rail_aware = advance_to_next_n_ball_event_with_rails_on_table(
        states,
        &ball,
        &table,
        &motion,
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    )
    .expect("rail-aware test geometry should validate");
    let system_states = states
        .iter()
        .cloned()
        .map(NBallSystemState::from)
        .collect::<Vec<_>>();
    let pocket_aware = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &system_states,
        &ball,
        &table,
        &motion,
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    )
    .expect("pocket-aware test geometry should validate");

    assert_close(pocket_aware.elapsed.as_f64(), rail_aware.elapsed.as_f64());
    assert_events_equivalent(
        rail_aware.event.as_ref(),
        pocket_aware.event.as_ref(),
        label,
    );
    assert_eq!(
        unwrap_on_table_states(&pocket_aware.states),
        rail_aware.states,
        "{label}: pocket-aware live states should match rail-aware states when pockets are irrelevant"
    );
}

#[test]
fn airborne_ball_table_contact_is_scheduled_before_later_on_table_events() {
    let ball = BallSetPhysicsSpec::default();
    let table = TableSpec::default();
    let motion = motion_config();
    let upward_speed_ips = 10.0;
    let states = vec![
        NBallSystemState::Airborne(BallState::airborne(
            inches2(20.0, 20.0),
            Inches::zero(),
            Velocity2::zero(),
            Inches::from_f64(upward_speed_ips),
            AngularVelocity3::zero(),
        )),
        NBallSystemState::OnTable(on_table(BallState::resting_at(inches2(40.0, 40.0)))),
    ];

    let event = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &states, &ball, &table, &motion,
    )
    .expect("pocket-aware test geometry should validate");

    let Some(NBallSystemEvent::BallTableBounce {
        ball_index,
        contact,
    }) = event
    else {
        panic!("expected airborne table-contact event, got {event:?}");
    };

    assert_eq!(ball_index, 0);
    assert_close(
        contact.time_until_contact.as_f64(),
        2.0 * upward_speed_ips / STANDARD_GRAVITY_INCHES_PER_SECOND_SQUARED,
    );
    assert_close(contact.state_at_contact.height.as_f64(), 0.0);
    assert!(contact.state_at_contact.vertical_velocity.as_f64() < 0.0);
    let NBallSystemState::Airborne(rebound_state) = &contact.state_after_contact else {
        panic!("expected table contact to rebound into a smaller airborne hop");
    };
    assert!(rebound_state.vertical_velocity.as_f64() > 0.0);
    assert!(
        rebound_state.vertical_velocity.as_f64()
            < -contact.state_at_contact.vertical_velocity.as_f64()
    );

    let first_advance = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &states,
        &ball,
        &table,
        &motion,
        CollisionModel::Ideal,
        RailModel::Mirror,
    )
    .expect("pocket-aware test geometry should validate");
    assert!(matches!(
        first_advance.states.first(),
        Some(NBallSystemState::Airborne(_))
    ));

    let second_event = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &first_advance.states,
        &ball,
        &table,
        &motion,
    )
    .expect("pocket-aware test geometry should validate");
    let Some(NBallSystemEvent::BallTableBounce {
        ball_index: second_ball_index,
        contact: second_contact,
    }) = second_event
    else {
        panic!("expected second smaller table-contact event, got {second_event:?}");
    };

    assert_eq!(second_ball_index, 0);
    assert!(second_contact.time_until_contact < contact.time_until_contact);

    let second_advance = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &first_advance.states,
        &ball,
        &table,
        &motion,
        CollisionModel::Ideal,
        RailModel::Mirror,
    )
    .expect("pocket-aware test geometry should validate");
    let Some(NBallSystemState::OnTable(settled_state)) = second_advance.states.first() else {
        panic!("expected the second table contact to settle back on the table");
    };
    assert_close(
        settled_state.as_ball_state().vertical_velocity.as_f64(),
        0.0,
    );
}

fn airborne_state_crossing_top_rail(
    table: &TableSpec,
    ball: &BallSetPhysicsSpec,
    height: Inches,
) -> BallState {
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - ball.radius.as_f64();
    BallState::airborne(
        inches2(20.0, top_plane),
        height,
        Velocity2::new(Inches::zero(), Inches::from_f64(100.0)),
        Inches::zero(),
        AngularVelocity3::zero(),
    )
}

#[test]
fn airborne_rail_crossing_uses_cushion_nose_as_the_off_table_boundary() {
    let table = TableSpec::three_cushion_carom_10ft();
    let ball = table.default_ball_set_physics_spec();
    let nose_height = table.cushion_nose_height.as_f64();

    for (height, expected_off_table) in [
        (nose_height - 1e-6, false),
        (nose_height, true),
        (nose_height + 1e-6, true),
    ] {
        let state = airborne_state_crossing_top_rail(&table, &ball, Inches::from_f64(height));
        let crossing = compute_next_airborne_ball_rail_crossing(&state, &ball, &table)
            .expect("an outward ball on the cushion plane must cross immediately");
        match crossing {
            PredictedAirborneBallRailCrossing::CushionImpact(impact) => {
                assert!(
                    !expected_off_table,
                    "height {height} should clear the cushion"
                );
                assert_eq!(impact.rail, Rail::Top);
                assert_close(impact.time_until_impact.as_f64(), 0.0);
            }
            PredictedAirborneBallRailCrossing::OffTable(exit) => {
                assert!(expected_off_table, "height {height} should hit the cushion");
                assert_eq!(exit.rail, Rail::Top);
                assert_close(exit.time_until_exit.as_f64(), 0.0);
            }
        }
    }
}

#[test]
fn airborne_ball_below_the_nose_rebounds_while_a_clear_ball_becomes_terminal() {
    let table = TableSpec::three_cushion_carom_10ft();
    let ball = table.default_ball_set_physics_spec();
    let motion = motion_config();

    let below = airborne_state_crossing_top_rail(
        &table,
        &ball,
        Inches::from_f64(table.cushion_nose_height.as_f64() - 1e-6),
    );
    let rebound = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &[NBallSystemState::Airborne(below)],
        &ball,
        &table,
        &motion,
        CollisionModel::Ideal,
        RailModel::Mirror,
    )
    .expect("a low airborne cushion crossing should resolve");
    assert!(matches!(
        rebound.event,
        Some(NBallSystemEvent::AirborneBallRailImpact { ball_index: 0, .. })
    ));
    let NBallSystemState::Airborne(rebounded) = &rebound.states[0] else {
        panic!("a low airborne cushion crossing should remain airborne")
    };
    assert!(rebounded.velocity.y().as_f64() < 0.0);

    let clear = airborne_state_crossing_top_rail(&table, &ball, table.cushion_nose_height.clone());
    let exited = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &[NBallSystemState::Airborne(clear)],
        &ball,
        &table,
        &motion,
        CollisionModel::Ideal,
        RailModel::Mirror,
    )
    .expect("a cushion-clearing crossing should resolve");
    assert!(matches!(
        exited.event,
        Some(NBallSystemEvent::BallOffTable { ball_index: 0, .. })
    ));
    assert!(matches!(
        exited.states[0],
        NBallSystemState::OffTable {
            rail: Rail::Top,
            ..
        }
    ));
}

#[test]
fn airborne_ball_over_a_rail_schedules_its_ballistic_table_contact() {
    let ball = BallSetPhysicsSpec::default();
    let table = TableSpec::default();
    let motion = motion_config();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let speed = 200.0;
    let states = vec![NBallSystemState::Airborne(BallState::airborne(
        inches2(20.0, top_plane - 10.0),
        Inches::from_f64(12.0),
        Velocity2::new(Inches::zero(), Inches::from_f64(speed)),
        Inches::zero(),
        AngularVelocity3::new(-speed / radius, 0.0, 0.0),
    ))];

    let event = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &states, &ball, &table, &motion,
    )
    .expect("pocket-aware test geometry should validate");

    let Some(NBallSystemEvent::BallTableBounce {
        ball_index,
        contact,
    }) = event
    else {
        panic!("an airborne rail projection must not become an on-table rail impact: {event:?}");
    };
    assert_eq!(ball_index, 0);
    assert_close(
        contact.time_until_contact.as_f64(),
        (24.0 / STANDARD_GRAVITY_INCHES_PER_SECOND_SQUARED).sqrt(),
    );
    assert_close(contact.state_at_contact.height.as_f64(), 0.0);
    assert!(contact.state_at_contact.vertical_velocity.as_f64() < 0.0);
}

#[test]
fn airborne_ball_over_a_pocket_schedules_its_ballistic_table_contact() {
    let ball = BallSetPhysicsSpec::default();
    let table = TableSpec::default();
    let motion = motion_config();
    let planar = fast_rolling_top_right_corner_pocket_state(0.0);
    let planar = planar.as_ball_state();
    let states = vec![NBallSystemState::Airborne(BallState::airborne(
        planar.position.clone(),
        Inches::from_f64(12.0),
        planar.velocity.clone(),
        Inches::zero(),
        planar.angular_velocity.clone(),
    ))];

    let event = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &states, &ball, &table, &motion,
    )
    .expect("pocket-aware test geometry should validate");

    let Some(NBallSystemEvent::BallTableBounce {
        ball_index,
        contact,
    }) = event
    else {
        panic!("an airborne pocket projection must not become terminal capture: {event:?}");
    };
    assert_eq!(ball_index, 0);
    assert_close(
        contact.time_until_contact.as_f64(),
        (24.0 / STANDARD_GRAVITY_INCHES_PER_SECOND_SQUARED).sqrt(),
    );
    assert_close(contact.state_at_contact.height.as_f64(), 0.0);
    assert!(contact.state_at_contact.vertical_velocity.as_f64() < 0.0);
}

#[test]
fn airborne_ball_ball_collision_is_resolved_before_later_table_contact() {
    let ball = BallSetPhysicsSpec::default();
    let table = TableSpec::default();
    let motion = motion_config();
    let cue = BallState::airborne(
        inches2(20.0, 20.0),
        Inches::from_f64(2.0),
        Velocity2::new(Inches::from_f64(40.0), Inches::zero()),
        Inches::zero(),
        AngularVelocity3::zero(),
    );
    let object = BallState::resting_at(inches2(24.0, 20.0));
    let states = vec![
        NBallSystemState::Airborne(cue),
        NBallSystemState::OnTable(on_table(object)),
    ];

    let event = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &states, &ball, &table, &motion,
    )
    .expect("pocket-aware test geometry should validate");

    let Some(NBallSystemEvent::AirborneBallBallCollision {
        first_ball_index,
        second_ball_index,
        contact,
    }) = event
    else {
        panic!("expected airborne ball-ball collision before table contact, got {event:?}");
    };

    assert_eq!((first_ball_index, second_ball_index), (0, 1));
    assert!(
        contact.time_until_contact.as_f64()
            < (4.0 / STANDARD_GRAVITY_INCHES_PER_SECOND_SQUARED).sqrt()
    );
    let dx = contact.second_at_contact.position.x().as_f64()
        - contact.first_at_contact.position.x().as_f64();
    let dy = contact.second_at_contact.position.y().as_f64()
        - contact.first_at_contact.position.y().as_f64();
    let dz = contact.second_at_contact.height.as_f64() - contact.first_at_contact.height.as_f64();
    let center_distance = (dx * dx + dy * dy + dz * dz).sqrt();
    assert_close(center_distance, 2.0 * ball.radius.as_f64());

    let advanced = advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table(
        &states,
        &ball,
        &table,
        &motion,
        CollisionModel::ThrowAware,
        &BallBallCollisionConfig::human_tuned(),
        RailModel::SpinAware,
        &billiards::RailCollisionProfile::default(),
    )
    .expect("airborne ball-ball collision should resolve");
    assert!(matches!(
        advanced.event,
        Some(NBallSystemEvent::AirborneBallBallCollision { .. })
    ));
    assert!(
        advanced.states[0].as_ball_state().velocity.x().as_f64() < 40.0,
        "the airborne cue ball should transfer forward momentum"
    );
    assert!(
        advanced.states[1].as_ball_state().velocity.x().as_f64() > 0.0,
        "the object ball should receive forward momentum"
    );
    assert_close(
        advanced.states[0].as_ball_state().velocity.x().as_f64()
            + advanced.states[1].as_ball_state().velocity.x().as_f64(),
        40.0,
    );
}

#[test]
fn simultaneous_airborne_pair_collision_applies_configured_table_contact_response() {
    let contact_time_seconds = 1.0;
    let approach_speed = 10.0;
    let gravity = STANDARD_GRAVITY_INCHES_PER_SECOND_SQUARED;
    let initial_vertical_speed = gravity * contact_time_seconds / 2.0;
    let table_restitution = 0.5;
    let table_sliding_friction = 0.005;
    let mut ball = BallSetPhysicsSpec::default();
    ball.airborne_table_contact.normal_restitution = Scale::from_f64(table_restitution);
    ball.airborne_table_contact.sliding_friction_coefficient =
        Scale::from_f64(table_sliding_friction);
    ball.airborne_table_contact.minimum_rebound_vertical_speed = billiards::InchesPerSecond::zero();
    let radius = ball.radius.as_f64();

    // At t = 1, h(t) = (g / 2)t - (g / 2)t^2 = 0 and the initial
    // horizontal separation 2R + 10t closes to exactly 2R.
    let states = vec![
        NBallSystemState::Airborne(BallState::airborne(
            inches2(20.0, 20.0),
            Inches::zero(),
            Velocity2::new(Inches::from_f64(approach_speed), Inches::zero()),
            Inches::from_f64(initial_vertical_speed),
            AngularVelocity3::zero(),
        )),
        NBallSystemState::Airborne(BallState::airborne(
            inches2(
                20.0 + 2.0 * radius + approach_speed * contact_time_seconds,
                20.0,
            ),
            Inches::zero(),
            Velocity2::zero(),
            Inches::from_f64(initial_vertical_speed),
            AngularVelocity3::zero(),
        )),
    ];

    let advanced = advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table(
        &states,
        &ball,
        &TableSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
        &BallBallCollisionConfig::new(Scale::from_f64(1.0), Scale::zero()),
        RailModel::Mirror,
        &billiards::RailCollisionProfile::default(),
    )
    .expect("the exact simultaneous airborne contacts should resolve");

    let Some(NBallSystemEvent::AirborneBallBallCollision {
        first_ball_index,
        second_ball_index,
        contact,
    }) = &advanced.event
    else {
        panic!(
            "expected the scheduler-derived airborne pair event, got {:?}",
            advanced.event
        );
    };
    assert_eq!((*first_ball_index, *second_ball_index), (0, 1));
    assert_close(advanced.elapsed.as_f64(), contact_time_seconds);
    assert_close(contact.time_until_contact.as_f64(), contact_time_seconds);
    assert_close(contact.first_at_contact.height.as_f64(), 0.0);
    assert_close(contact.second_at_contact.height.as_f64(), 0.0);
    assert_close(
        contact.second_at_contact.position.x().as_f64()
            - contact.first_at_contact.position.x().as_f64(),
        2.0 * radius,
    );

    let first_after = advanced.states[0].as_ball_state();
    let second_after = advanced.states[1].as_ball_state();
    assert_close(first_after.velocity.x().as_f64(), 0.0);
    assert!(
        second_after.velocity.x().as_f64() > 0.0,
        "the pair collision must transfer forward momentum to the object ball"
    );

    let expected_rebound_speed = table_restitution * initial_vertical_speed;
    assert_close(
        first_after.vertical_velocity.as_f64(),
        expected_rebound_speed,
    );
    assert_close(
        second_after.vertical_velocity.as_f64(),
        expected_rebound_speed,
    );
    assert!(matches!(advanced.states[0], NBallSystemState::Airborne(_)));
    assert!(matches!(advanced.states[1], NBallSystemState::Airborne(_)));

    // The ideal pair impulse gives the object ball 10 ips. This deliberately
    // small cloth coefficient stays below the sticking cap, so the configured
    // Coulomb impulse must slow the object and convert that impulse into +y spin.
    let table_tangential_impulse =
        table_sliding_friction * (1.0 + table_restitution) * initial_vertical_speed;
    let expected_object_speed = approach_speed - table_tangential_impulse;
    let expected_object_angular_speed = 2.5 * table_tangential_impulse / radius;
    assert_close(second_after.velocity.x().as_f64(), expected_object_speed);
    assert_close(
        second_after.angular_velocity.y().as_f64(),
        expected_object_angular_speed,
    );
    assert_close(first_after.angular_velocity.y().as_f64(), 0.0);
}

fn resolve_table_height_airborne_pair_with_base_vz(
    vertical_velocity: f64,
) -> Vec<NBallSystemState> {
    let ball = BallSetPhysicsSpec::default();
    let table = TableSpec::default();
    let motion = motion_config();
    let radius = ball.radius.as_f64();
    let airborne = BallState::airborne(
        inches2(20.0, 20.0),
        Inches::zero(),
        Velocity2::new("10", "0"),
        Inches::from_f64(vertical_velocity),
        AngularVelocity3::zero(),
    );
    let object = BallState::resting_at(inches2(20.0 + 2.0 * radius, 20.0));
    let states = vec![
        NBallSystemState::Airborne(airborne.clone()),
        NBallSystemState::OnTable(on_table(object.clone())),
    ];
    let event = NBallSystemEvent::AirborneBallBallCollision {
        first_ball_index: 0,
        second_ball_index: 1,
        contact: PredictedAirborneBallBallCollision {
            time_until_contact: Seconds::zero(),
            first_at_contact: airborne,
            second_at_contact: object,
        },
    };
    resolve_n_ball_system_event_with_physics_and_pockets_on_table(
        &states,
        &event,
        &ball,
        &table,
        &motion,
        CollisionModel::Ideal,
        &BallBallCollisionConfig::ideal(),
        RailModel::Mirror,
        &billiards::RailCollisionProfile::default(),
    )
    .expect("the recorded table-height airborne collision should replay")
}

#[test]
fn subresolution_vertical_launch_is_supported_but_force_follow_hop_remains_airborne() {
    let tiny_launch = resolve_table_height_airborne_pair_with_base_vz(1e-6);
    assert!(
        matches!(tiny_launch[0], NBallSystemState::OnTable(_)),
        "a launch whose ballistic apex is below airborne_height must be supported"
    );
    assert_close(
        tiny_launch[0].as_ball_state().vertical_velocity.as_f64(),
        0.0,
    );

    let downward = resolve_table_height_airborne_pair_with_base_vz(-1.0);
    assert!(
        matches!(downward[0], NBallSystemState::OnTable(_)),
        "downward table-height motion must clamp to unilateral support"
    );

    let force_follow_vertical_speed = 4.2536;
    let force_follow = resolve_table_height_airborne_pair_with_base_vz(force_follow_vertical_speed);
    assert!(matches!(force_follow[0], NBallSystemState::Airborne(_)));
    assert_close(
        force_follow[0].as_ball_state().vertical_velocity.as_f64(),
        force_follow_vertical_speed,
    );
    let next = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &force_follow,
        &BallSetPhysicsSpec::default(),
        &TableSpec::default(),
        &motion_config(),
    )
    .expect("force-follow post-collision state should schedule");
    match next {
        Some(NBallSystemEvent::BallTableBounce {
            ball_index,
            contact,
        }) => {
            assert_eq!(ball_index, 0);
            assert!(
                contact.time_until_contact.as_f64() > 1e-3,
                "the physical force-follow hop must not collapse into a nanosecond bounce"
            );
        }
        other => panic!("expected a real force-follow table contact, got {other:?}"),
    }
}

#[test]
fn recorded_airborne_preliminary_event_replays_exactly_from_the_same_prestate() {
    let ball = BallSetPhysicsSpec::default();
    let table = TableSpec::default();
    let motion = motion_config();
    let radius = ball.radius.as_f64();
    let states = vec![
        NBallSystemState::Airborne(BallState::airborne(
            inches2(20.0, 20.0),
            Inches::zero(),
            Velocity2::new("10", "0"),
            Inches::from_f64(4.2536),
            AngularVelocity3::zero(),
        )),
        NBallSystemState::OnTable(on_table(BallState::resting_at(inches2(
            20.0 + 2.0 * radius,
            20.0,
        )))),
    ];
    let event = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &states, &ball, &table, &motion,
    )
    .expect("preliminary event should compute")
    .expect("the pair should collide immediately");
    assert!(matches!(
        event,
        NBallSystemEvent::AirborneBallBallCollision { .. }
    ));

    let replayed = resolve_n_ball_system_event_with_physics_and_pockets_on_table(
        &states,
        &event,
        &ball,
        &table,
        &motion,
        CollisionModel::Ideal,
        &BallBallCollisionConfig::ideal(),
        RailModel::Mirror,
        &billiards::RailCollisionProfile::default(),
    )
    .expect("recorded event should replay");
    let advanced = advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table(
        &states,
        &ball,
        &table,
        &motion,
        CollisionModel::Ideal,
        &BallBallCollisionConfig::ideal(),
        RailModel::Mirror,
        &billiards::RailCollisionProfile::default(),
    )
    .expect("scheduler should resolve the same event");
    assert_eq!(advanced.event, Some(event));
    assert_eq!(replayed, advanced.states);
}

#[test]
fn geometrically_separated_pair_can_reenter_and_receive_restitution_again() {
    let ball = BallSetPhysicsSpec::default();
    let table = TableSpec::default();
    let motion = motion_config();
    let radius = ball.radius.as_f64();
    let mut states = vec![
        NBallSystemState::OnTable(on_table(BallState::on_table(
            inches2(35.0, 30.0),
            Velocity2::new("40", "0"),
            AngularVelocity3::zero(),
        ))),
        NBallSystemState::OnTable(on_table(BallState::resting_at(inches2(
            35.0 + 2.0 * radius,
            30.0,
        )))),
    ];
    let mut elapsed_since_first_collision = 0.0;
    let mut collision_count = 0usize;
    for _ in 0..32 {
        let advanced = advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table(
            &states,
            &ball,
            &table,
            &motion,
            CollisionModel::Ideal,
            &BallBallCollisionConfig::ideal(),
            RailModel::Mirror,
            &billiards::RailCollisionProfile::default(),
        )
        .expect("the separated pair should continue through its rail return");
        let Some(event) = advanced.event else {
            break;
        };
        if collision_count > 0 {
            elapsed_since_first_collision += advanced.elapsed.as_f64();
        }
        if matches!(event, NBallSystemEvent::BallBallCollision { .. }) {
            collision_count += 1;
            if collision_count == 2 {
                assert!(
                    elapsed_since_first_collision > 0.2,
                    "the second response must follow actual geometric separation: elapsed={}, states={:?}",
                    elapsed_since_first_collision,
                    advanced.states
                );
                assert!(
                    advanced.states[0].as_ball_state().velocity.x().as_f64() < -1.0,
                    "the returning ball should transfer a new restitutive impulse"
                );
                assert!(
                    advanced.states[1]
                        .as_ball_state()
                        .velocity
                        .x()
                        .as_f64()
                        .abs()
                        < 1.0,
                    "the isolated equal-mass re-collision should again transfer most normal speed"
                );
                return;
            }
        }
        states = advanced.states;
    }
    panic!("the pair did not separate and re-collide within the focused event window");
}

#[test]
fn airborne_collision_prediction_is_independent_of_state_order() {
    let ball = BallSetPhysicsSpec::default();
    let table = TableSpec::default();
    let motion = motion_config();
    let airborne = BallState::airborne(
        inches2(20.0, 20.0),
        Inches::from_f64(2.0),
        Velocity2::new(Inches::from_f64(40.0), Inches::zero()),
        Inches::zero(),
        AngularVelocity3::zero(),
    );
    let states = vec![
        NBallSystemState::OnTable(on_table(BallState::resting_at(inches2(24.0, 20.0)))),
        NBallSystemState::Airborne(airborne),
    ];

    let event = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &states, &ball, &table, &motion,
    )
    .expect("reversed pocket-aware test geometry should validate");

    let Some(NBallSystemEvent::AirborneBallBallCollision {
        first_ball_index,
        second_ball_index,
        ..
    }) = event
    else {
        panic!("expected reversed airborne pair to collide before table contact, got {event:?}");
    };
    assert_eq!((first_ball_index, second_ball_index), (0, 1));
}

#[test]
fn numerical_velocity_residue_does_not_schedule_a_zero_time_collision() {
    let ball = BallSetPhysicsSpec::default();
    let motion = motion_config();
    let radius = ball.radius.as_f64();
    let first = on_table(BallState::on_table(
        inches2(20.0, 20.0),
        Velocity2::new(Inches::from_f64(10.0 + 1e-10), Inches::zero()),
        AngularVelocity3::zero(),
    ));
    let second = on_table(BallState::on_table(
        inches2(20.0 + 2.0 * radius, 20.0),
        Velocity2::new(Inches::from_f64(10.0), Inches::zero()),
        AngularVelocity3::zero(),
    ));

    let collision = compute_next_ball_ball_collision_during_current_phases_on_table(
        &first, &second, &ball, &motion,
    );

    assert!(
        collision.is_none(),
        "sub-nanoinch velocity residue must not create a no-progress collision"
    );
}

#[test]
fn airborne_ball_over_a_jaw_schedules_its_ballistic_table_contact() {
    let ball = BallSetPhysicsSpec::default();
    let table = TableSpec::default();
    let motion = motion_config();
    let mouth_width = table
        .diamond_to_inches(table.pocket_spec(Pocket::CenterRight).width.clone())
        .as_f64();
    let planar = fast_rolling_side_pocket_state(0.5 * mouth_width);
    let planar = planar.as_ball_state();
    let states = vec![NBallSystemState::Airborne(BallState::airborne(
        planar.position.clone(),
        Inches::from_f64(12.0),
        planar.velocity.clone(),
        Inches::zero(),
        planar.angular_velocity.clone(),
    ))];

    let event = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &states, &ball, &table, &motion,
    )
    .expect("pocket-aware test geometry should validate");

    let Some(NBallSystemEvent::BallTableBounce {
        ball_index,
        contact,
    }) = event
    else {
        panic!("an airborne jaw projection must not become an on-table jaw impact: {event:?}");
    };
    assert_eq!(ball_index, 0);
    assert_close(
        contact.time_until_contact.as_f64(),
        (24.0 / STANDARD_GRAVITY_INCHES_PER_SECOND_SQUARED).sqrt(),
    );
    assert_close(contact.state_at_contact.height.as_f64(), 0.0);
    assert!(contact.state_at_contact.vertical_velocity.as_f64() < 0.0);
}

fn shared_three_ball_contact_fixture() -> Vec<OnTableBallState> {
    shared_three_ball_contact_fixture_with_time_offset(0.0)
}

fn shared_three_ball_contact_fixture_with_time_offset(
    second_contact_time_offset: f64,
) -> Vec<OnTableBallState> {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let shared_contact_y = -3.0_f64.sqrt() * radius;
    let second_object_y_offset =
        5.0 * second_contact_time_offset - 2.5 * second_contact_time_offset.powi(2);

    vec![
        on_table(BallState::on_table(
            inches2(20.0, 20.0 + shared_contact_y - 7.5),
            Velocity2::new("0", "10"),
            AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
        )),
        on_table(BallState::resting_at(inches2(20.0 - radius, 20.0))),
        on_table(BallState::resting_at(inches2(
            20.0 + radius,
            20.0 + second_object_y_offset,
        ))),
    ]
}

fn rolling_transition_just_before_shared_contact() -> OnTableBallState {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let speed = 5.0 * (1.0 - 5e-13);
    on_table(BallState::on_table(
        inches2(40.0, 40.0),
        Velocity2::new(Inches::from_f64(speed), Inches::zero()),
        AngularVelocity3::new(0.0, speed / radius, 0.0),
    ))
}

fn rolling_toward_center_right_side_pocket() -> OnTableBallState {
    let table = TableSpec::default();
    on_table(BallState::on_table(
        inches2(
            40.0,
            table.diamond_to_inches(CENTER_SPOT.y.clone()).as_f64(),
        ),
        Velocity2::new("10", "0"),
        AngularVelocity3::new(0.0, 10.0 / TYPICAL_BALL_RADIUS.as_f64(), 0.0),
    ))
}

fn fast_rolling_side_pocket_state(y_offset: f64) -> OnTableBallState {
    let table = TableSpec::default();
    let pocket_center = Pocket::CenterRight.aiming_center();
    let pocket_x = table.diamond_to_inches(pocket_center.x).as_f64();
    let pocket_y = table.diamond_to_inches(pocket_center.y).as_f64();
    let speed = 200.0;

    on_table(BallState::on_table(
        inches2(pocket_x - 10.0, pocket_y + y_offset),
        Velocity2::new(Inches::from_f64(speed), Inches::from_f64(0.0)),
        AngularVelocity3::new(0.0, speed / TYPICAL_BALL_RADIUS.as_f64(), 0.0),
    ))
}

fn fast_rolling_side_pocket_state_at_angle(
    angle_degrees: f64,
    perpendicular_offset: f64,
) -> OnTableBallState {
    let table = TableSpec::default();
    let pocket_center = Pocket::CenterRight.aiming_center();
    let pocket_x = table.diamond_to_inches(pocket_center.x).as_f64();
    let pocket_y = table.diamond_to_inches(pocket_center.y).as_f64();
    let angle = angle_degrees.to_radians();
    let distance = 10.0;
    let speed = 120.0;
    let direction_x = angle.cos();
    let direction_y = angle.sin();
    let tangent_x = -direction_y;
    let tangent_y = direction_x;
    let radius = TYPICAL_BALL_RADIUS.as_f64();

    on_table(BallState::on_table(
        inches2(
            pocket_x - distance * direction_x + perpendicular_offset * tangent_x,
            pocket_y - distance * direction_y + perpendicular_offset * tangent_y,
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

fn slow_rolling_side_pocket_state_at_angle(
    angle_degrees: f64,
    perpendicular_offset: f64,
) -> OnTableBallState {
    let table = TableSpec::default();
    let pocket_center = Pocket::CenterRight.aiming_center();
    let pocket_x = table.diamond_to_inches(pocket_center.x).as_f64();
    let pocket_y = table.diamond_to_inches(pocket_center.y).as_f64();
    let angle = angle_degrees.to_radians();
    let distance = 10.0;
    let speed = 10.0;
    let direction_x = angle.cos();
    let direction_y = angle.sin();
    let tangent_x = -direction_y;
    let tangent_y = direction_x;
    let radius = TYPICAL_BALL_RADIUS.as_f64();

    on_table(BallState::on_table(
        inches2(
            pocket_x - distance * direction_x + perpendicular_offset * tangent_x,
            pocket_y - distance * direction_y + perpendicular_offset * tangent_y,
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

fn fast_rolling_top_right_corner_pocket_state(perpendicular_offset: f64) -> OnTableBallState {
    let table = TableSpec::default();
    let pocket_center = Pocket::TopRight.aiming_center();
    let pocket_x = table.diamond_to_inches(pocket_center.x).as_f64();
    let pocket_y = table.diamond_to_inches(pocket_center.y).as_f64();
    let diagonal = 0.5_f64.sqrt();
    let distance = 10.0;
    let speed = 120.0;
    let vx = speed * diagonal;
    let vy = speed * diagonal;
    let radius = TYPICAL_BALL_RADIUS.as_f64();

    on_table(BallState::on_table(
        inches2(
            pocket_x - distance * diagonal - perpendicular_offset * diagonal,
            pocket_y - distance * diagonal + perpendicular_offset * diagonal,
        ),
        Velocity2::new(Inches::from_f64(vx), Inches::from_f64(vy)),
        AngularVelocity3::new(-vy / radius, vx / radius, 0.0),
    ))
}

fn fast_rolling_top_right_corner_pocket_state_at_angle(
    angle_degrees: f64,
    perpendicular_offset: f64,
) -> OnTableBallState {
    let table = TableSpec::default();
    let pocket_center = Pocket::TopRight.aiming_center();
    let pocket_x = table.diamond_to_inches(pocket_center.x).as_f64();
    let pocket_y = table.diamond_to_inches(pocket_center.y).as_f64();
    let diagonal = 0.5_f64.sqrt();
    let entry_x = diagonal;
    let entry_y = diagonal;
    let tangent_x = -entry_y;
    let tangent_y = entry_x;
    let angle = angle_degrees.to_radians();
    let speed = 120.0;
    let direction_x = angle.cos() * entry_x + angle.sin() * tangent_x;
    let direction_y = angle.cos() * entry_y + angle.sin() * tangent_y;
    let vx = speed * direction_x;
    let vy = speed * direction_y;
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let mouth_width = table
        .diamond_to_inches(table.pocket_spec(Pocket::TopRight).width.clone())
        .as_f64();
    let corner_offset = mouth_width / 2.0_f64.sqrt();
    let jaw_x = table.diamond_to_inches(Diamond::eight()).as_f64() - corner_offset;
    let jaw_y = table.diamond_to_inches(Diamond::four()).as_f64();
    let mouth_projection = entry_x * jaw_x + entry_y * jaw_y;
    let pocket_projection = entry_x * pocket_x + entry_y * pocket_y;
    let along_offset = mouth_projection - radius - pocket_projection;

    on_table(BallState::on_table(
        inches2(
            pocket_x + along_offset * entry_x + perpendicular_offset * tangent_x,
            pocket_y + along_offset * entry_y + perpendicular_offset * tangent_y,
        ),
        Velocity2::new(Inches::from_f64(vx), Inches::from_f64(vy)),
        AngularVelocity3::new(-vy / radius, vx / radius, 0.0),
    ))
}

fn slow_rolling_top_right_corner_pocket_state(perpendicular_offset: f64) -> OnTableBallState {
    slow_rolling_top_right_corner_pocket_state_at_angle(0.0, perpendicular_offset)
}

fn slow_rolling_top_right_corner_pocket_state_at_angle(
    angle_degrees: f64,
    perpendicular_offset: f64,
) -> OnTableBallState {
    let table = TableSpec::default();
    let pocket_center = Pocket::TopRight.aiming_center();
    let pocket_x = table.diamond_to_inches(pocket_center.x).as_f64();
    let pocket_y = table.diamond_to_inches(pocket_center.y).as_f64();
    let diagonal = 0.5_f64.sqrt();
    let entry_x = diagonal;
    let entry_y = diagonal;
    let tangent_x = -entry_y;
    let tangent_y = entry_x;
    let angle = angle_degrees.to_radians();
    let speed = 10.0;
    let direction_x = angle.cos() * entry_x + angle.sin() * tangent_x;
    let direction_y = angle.cos() * entry_y + angle.sin() * tangent_y;
    let vx = speed * direction_x;
    let vy = speed * direction_y;
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let mouth_width = table
        .diamond_to_inches(table.pocket_spec(Pocket::TopRight).width.clone())
        .as_f64();
    let corner_offset = mouth_width / 2.0_f64.sqrt();
    let jaw_x = table.diamond_to_inches(Diamond::eight()).as_f64() - corner_offset;
    let jaw_y = table.diamond_to_inches(Diamond::four()).as_f64();
    let mouth_projection = entry_x * jaw_x + entry_y * jaw_y;
    let pocket_projection = entry_x * pocket_x + entry_y * pocket_y;
    let along_offset = mouth_projection - radius - pocket_projection;

    on_table(BallState::on_table(
        inches2(
            pocket_x + along_offset * entry_x + perpendicular_offset * tangent_x,
            pocket_y + along_offset * entry_y + perpendicular_offset * tangent_y,
        ),
        Velocity2::new(Inches::from_f64(vx), Inches::from_f64(vy)),
        AngularVelocity3::new(-vy / radius, vx / radius, 0.0),
    ))
}

fn old_pocket_scan_timestep_seconds() -> f64 {
    200.0 / 5.0 / 512.0
}

#[test]
fn a_slow_angled_side_pocket_entry_outside_the_tp35_target_curve_is_rejected() {
    let table = TableSpec::default();
    let centered = slow_rolling_side_pocket_state_at_angle(30.0, 0.0);
    let outside_target = slow_rolling_side_pocket_state_at_angle(30.0, 1.8);

    assert!(
        compute_next_ball_pocket_capture_on_table(
            &centered,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .is_some(),
        "a centered 30-degree slow side-pocket entry should still be accepted"
    );
    assert!(
        compute_next_ball_pocket_capture_on_table(
            &outside_target,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .is_none(),
        "TP 3.5's slow 30-degree side-pocket target is narrower than the old capture circle"
    );
}

#[test]
fn prepared_geometry_preserves_the_exact_slow_side_capture_signature() {
    let table = TableSpec::default();
    let ball = BallSetPhysicsSpec::default();
    let motion = motion_config();
    let centered = slow_rolling_side_pocket_state_at_angle(30.0, 0.0);
    let outside_target = slow_rolling_side_pocket_state_at_angle(30.0, 1.8);
    let capture = compute_next_ball_pocket_capture_on_table(&centered, &ball, &table, &motion)
        .expect("the centered slow side-pocket shot should be captured");
    let state = capture.state_at_capture.as_ball_state();

    assert_eq!(capture.pocket, Pocket::CenterRight);
    assert_eq!(
        [
            capture.time_until_capture.as_f64().to_bits(),
            state.position.x().as_f64().to_bits(),
            state.position.y().as_f64().to_bits(),
            state.velocity.x().as_f64().to_bits(),
            state.velocity.y().as_f64().to_bits(),
            state.angular_velocity.x().as_f64().to_bits(),
            state.angular_velocity.y().as_f64().to_bits(),
            state.angular_velocity.z().as_f64().to_bits(),
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
    assert!(
        compute_next_ball_pocket_capture_on_table(&outside_target, &ball, &table, &motion,)
            .is_none()
    );
}

#[test]
fn prepared_geometry_preserves_exact_corner_and_fast_capture_signatures() {
    let table = TableSpec::default();
    let ball = BallSetPhysicsSpec::default();
    let motion = motion_config();

    for (state, expected_pocket, expected_signature) in [
        (
            slow_rolling_top_right_corner_pocket_state(0.0),
            Pocket::TopRight,
            [
                0x0000_0000_0000_0000,
                0x4047_ce87_a598_4a57,
                0x4058_6743_d2cc_252b,
                0x401c_48c6_001f_0ac0,
                0x401c_48c6_001f_0ac0,
                0xc019_243e_38ff_2600,
                0x4019_243e_38ff_2600,
                0x0000_0000_0000_0000,
            ],
        ),
        (
            fast_rolling_side_pocket_state(0.0),
            Pocket::CenterRight,
            [
                0x3fa6_bb8c_c145_5e2e,
                0x4048_7000_0000_0000,
                0x4049_0000_0000_0000,
                0x4068_f8e5_6403_9a53,
                0x0000_0000_0000_0000,
                0x8000_0000_0000_0000,
                0x4066_3293_0391_6cbc,
                0x0000_0000_0000_0000,
            ],
        ),
    ] {
        let capture = compute_next_ball_pocket_capture_on_table(&state, &ball, &table, &motion)
            .expect("exact-signature fixture should be captured");
        let state = capture.state_at_capture.as_ball_state();

        assert_eq!(capture.pocket, expected_pocket);
        assert_eq!(
            [
                capture.time_until_capture.as_f64().to_bits(),
                state.position.x().as_f64().to_bits(),
                state.position.y().as_f64().to_bits(),
                state.velocity.x().as_f64().to_bits(),
                state.velocity.y().as_f64().to_bits(),
                state.angular_velocity.x().as_f64().to_bits(),
                state.angular_velocity.y().as_f64().to_bits(),
                state.angular_velocity.z().as_f64().to_bits(),
            ],
            expected_signature
        );
    }
}

#[test]
fn a_pocketless_table_never_predicts_a_capture() {
    assert!(compute_next_ball_pocket_capture_on_table(
        &rolling_toward_center_right_side_pocket(),
        &BallSetPhysicsSpec::default(),
        &TableSpec::three_cushion_carom_10ft(),
        &motion_config(),
    )
    .is_none());
}

#[test]
fn a_single_ball_heading_into_the_side_pocket_predicts_capture_before_the_rail() {
    let table = TableSpec::default();
    let state = rolling_toward_center_right_side_pocket();

    let capture = compute_next_ball_pocket_capture_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("the rolling ball should be captured by the side pocket");
    let event = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &[NBallSystemState::from(state)],
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("pocket-aware test geometry should validate")
    .expect("an event should be predicted");

    assert_eq!(capture.pocket, Pocket::CenterRight);
    assert_close(capture.time_until_capture.as_f64(), 1.329179606750062);
    match event {
        NBallSystemEvent::BallPocketCapture {
            ball_index,
            capture,
        } => {
            assert_eq!(ball_index, 0);
            assert_eq!(capture.pocket, Pocket::CenterRight);
            assert_close(capture.time_until_capture.as_f64(), 1.329179606750062);
        }
        other => panic!("expected pocket capture, got {other:?}"),
    }
}

#[test]
fn a_fast_ball_entering_a_side_pocket_between_old_scan_samples_predicts_capture() {
    let table = TableSpec::default();
    let state = fast_rolling_side_pocket_state(0.0);

    let capture = compute_next_ball_pocket_capture_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("the fast straight side-pocket entry should not tunnel through the capture region");

    assert_eq!(capture.pocket, Pocket::CenterRight);
    assert!(
        capture.time_until_capture.as_f64() < old_pocket_scan_timestep_seconds(),
        "the reproducer should land before the first old fixed-step sample"
    );
}

#[test]
fn side_pocket_capture_waits_until_the_ball_reaches_the_mouth_plane() {
    let table = TableSpec::default();
    let state = rolling_toward_center_right_side_pocket();

    let capture = compute_next_ball_pocket_capture_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("straight side-pocket entry should eventually capture");

    let mouth_x = table
        .diamond_to_inches(Pocket::CenterRight.aiming_center().x)
        .as_f64();
    let captured_x = capture
        .state_at_capture
        .as_ball_state()
        .position
        .x()
        .as_f64();

    assert!(
        captured_x >= mouth_x - TYPICAL_BALL_RADIUS.as_f64() - 1e-9,
        "captured_x={captured_x}, mouth threshold={}",
        mouth_x - TYPICAL_BALL_RADIUS.as_f64()
    );
}

#[test]
fn a_fast_straight_side_pocket_entry_outside_the_tp37_target_width_is_rejected() {
    let table = TableSpec::default();
    let state = fast_rolling_side_pocket_state(1.8);

    assert!(
        compute_next_ball_pocket_capture_on_table(
            &state,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .is_none(),
        "TP 3.7's fast straight-in side-pocket target is much narrower than the full mouth"
    );
}

#[test]
fn a_fast_angled_side_pocket_entry_outside_the_tp37_target_curve_is_rejected() {
    let table = TableSpec::default();
    let centered = fast_rolling_side_pocket_state_at_angle(30.0, 0.0);
    let outside_target = fast_rolling_side_pocket_state_at_angle(30.0, 1.8);

    assert!(
        compute_next_ball_pocket_capture_on_table(
            &centered,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .is_some(),
        "a centered 30-degree fast side-pocket entry should still be accepted"
    );
    assert!(
        compute_next_ball_pocket_capture_on_table(
            &outside_target,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .is_none(),
        "TP 3.7's 30-degree fast side-pocket target is narrower than the straight-in target"
    );
}

#[test]
fn a_fast_side_pocket_entry_beyond_the_effective_target_angle_is_rejected() {
    let table = TableSpec::default();
    let pocket_center = Pocket::CenterRight.aiming_center();
    let pocket_x = table.diamond_to_inches(pocket_center.x).as_f64();
    let pocket_y = table.diamond_to_inches(pocket_center.y).as_f64();
    let angle_radians = 60.0_f64.to_radians();
    let distance = 10.0;
    let speed = 80.0;
    let state = on_table(BallState::on_table(
        inches2(
            pocket_x - distance * angle_radians.cos(),
            pocket_y - distance * angle_radians.sin(),
        ),
        Velocity2::new(
            Inches::from_f64(speed * angle_radians.cos()),
            Inches::from_f64(speed * angle_radians.sin()),
        ),
        AngularVelocity3::zero(),
    ));

    assert!(
        compute_next_ball_pocket_capture_on_table(
            &state,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .is_none(),
        "a steep fast side-pocket approach should now be rejected by the jaw-aware capture gate"
    );
}

#[test]
fn a_rejected_side_pocket_entry_does_not_rebound_from_the_open_rail_mouth() {
    let table = TableSpec::default();
    let pocket_center = Pocket::CenterRight.aiming_center();
    let pocket_x = table.diamond_to_inches(pocket_center.x).as_f64();
    let pocket_y = table.diamond_to_inches(pocket_center.y).as_f64();
    let angle_radians = 60.0_f64.to_radians();
    let distance = 10.0;
    let speed = 80.0;
    let state = on_table(BallState::on_table(
        inches2(
            pocket_x - distance * angle_radians.cos(),
            pocket_y - distance * angle_radians.sin(),
        ),
        Velocity2::new(
            Inches::from_f64(speed * angle_radians.cos()),
            Inches::from_f64(speed * angle_radians.sin()),
        ),
        AngularVelocity3::zero(),
    ));

    assert!(
        compute_next_ball_pocket_capture_on_table(
            &state,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .is_none(),
        "fixture must remain outside the TP 3.7 capture target"
    );
    assert!(
        compute_next_ball_rail_impact_on_table(
            &state,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .is_none(),
        "a rejected entry through an open pocket mouth must not hit a phantom full-width rail"
    );
}

#[test]
fn a_ball_aimed_at_a_side_pocket_jaw_predicts_a_jaw_impact() {
    let table = TableSpec::default();
    let state = on_table(BallState::on_table(
        inches2(44.0, 58.0),
        Velocity2::new("12", "-11"),
        AngularVelocity3::zero(),
    ));

    let impact = compute_next_ball_jaw_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("a jaw impact should be predicted");

    assert_eq!(impact.pocket, Pocket::CenterRight);
}

#[test]
fn a_fast_ball_entering_a_side_pocket_jaw_between_old_scan_samples_predicts_impact() {
    let table = TableSpec::default();
    let mouth_width = table
        .diamond_to_inches(table.pocket_spec(Pocket::CenterRight).width.clone())
        .as_f64();
    let state = fast_rolling_side_pocket_state(0.5 * mouth_width);

    let impact = compute_next_ball_jaw_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("the fast side-pocket jaw entry should not tunnel through the jaw circle");

    assert_eq!(impact.pocket, Pocket::CenterRight);
    assert!(
        impact.time_until_impact.as_f64() < old_pocket_scan_timestep_seconds(),
        "the reproducer should land before the first old fixed-step sample"
    );
}

#[test]
fn rounded_side_jaws_preserve_the_configured_physical_mouth_clearance() {
    let table = TableSpec::default();
    let ball_radius = TYPICAL_BALL_RADIUS.as_f64();
    let mouth_width = table
        .diamond_to_inches(table.pocket_spec(Pocket::CenterRight).width.clone())
        .as_f64();
    let lateral_offset = 1.3;
    assert!(
        lateral_offset < 0.5 * mouth_width - ball_radius,
        "fixture must fit through the configured physical mouth"
    );
    let speed = 200.0;
    let state = on_table(BallState::on_table(
        inches2(40.0, 50.0 + lateral_offset),
        Velocity2::new(Inches::from_f64(speed), Inches::zero()),
        AngularVelocity3::new(0.0, speed / ball_radius, 0.0),
    ));

    assert!(
        compute_next_ball_jaw_impact_on_table(
            &state,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .is_none(),
        "rounded jaw curvature must not consume the configured point-to-point mouth width"
    );
}

#[test]
fn a_ball_touching_a_side_pocket_jaw_and_moving_inward_predicts_immediate_impact() {
    let table = TableSpec::default();
    let pocket = Pocket::CenterRight;
    let pocket_center = pocket.aiming_center();
    let jaw_x = table.diamond_to_inches(pocket_center.x).as_f64();
    let mouth_tip_y = table.diamond_to_inches(pocket_center.y).as_f64()
        + 0.5
            * table
                .diamond_to_inches(table.pocket_spec(pocket).width.clone())
                .as_f64();
    let nose_radius = match &table.pocket_spec(pocket).shape.jaw_geometry {
        PocketJawGeometry::PointNoses => 0.0,
        PocketJawGeometry::RoundedNoses { nose_radius } => nose_radius.as_f64(),
    };
    // A rounded upper jaw's virtual center sits one nose radius beyond its physical mouth tip.
    let jaw_y = mouth_tip_y + nose_radius;
    let ball_radius = TYPICAL_BALL_RADIUS.as_f64();
    let speed = 10.0;
    let state = on_table(BallState::on_table(
        inches2(jaw_x - ball_radius - nose_radius, jaw_y),
        Velocity2::new(Inches::from_f64(speed), Inches::zero()),
        AngularVelocity3::new(0.0, speed / ball_radius, 0.0),
    ));

    let impact = compute_next_ball_jaw_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("a frozen-to-jaw incoming ball should schedule an immediate jaw impact");

    assert_eq!(impact.pocket, pocket);
    assert_close(impact.time_until_impact.as_f64(), 0.0);

    let advanced = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &[NBallSystemState::from(state)],
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    )
    .expect("pocket-aware test geometry should validate");

    match advanced.event.expect("an event should be reported") {
        NBallSystemEvent::BallJawImpact { ball_index, impact } => {
            assert_eq!(ball_index, 0);
            assert_eq!(impact.pocket, pocket);
            assert_close(impact.time_until_impact.as_f64(), 0.0);
        }
        other => panic!("expected immediate jaw impact, got {other:?}"),
    }
}

#[test]
fn advancing_a_near_jaw_side_pocket_entry_resolves_the_explicit_jaw() {
    let table = TableSpec::default();
    let state = on_table(BallState::on_table(
        inches2(43.0, 57.0),
        Velocity2::new("12", "-10"),
        AngularVelocity3::zero(),
    ));

    let _jaw = compute_next_ball_jaw_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("the same shot should also have an explicit jaw impact");

    let advanced = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &[NBallSystemState::from(state)],
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    )
    .expect("pocket-aware test geometry should validate");

    match advanced.event.expect("a first event should be predicted") {
        NBallSystemEvent::BallJawImpact { ball_index, impact } => {
            assert_eq!(ball_index, 0);
            assert_eq!(impact.pocket, Pocket::CenterRight);
        }
        other => {
            panic!("expected the pocket-aware scheduler to resolve the explicit jaw, got {other:?}")
        }
    }
}

#[test]
fn a_near_jaw_entry_can_late_drop_on_the_same_jaw_impact_step() {
    let table = TableSpec::default();
    let state = on_table(BallState::on_table(
        inches2(43.0, 57.0),
        Velocity2::new("12", "-10"),
        AngularVelocity3::zero(),
    ));

    let advanced = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &[NBallSystemState::from(state)],
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    )
    .expect("pocket-aware test geometry should validate");

    match advanced.event.expect("a first event should be predicted") {
        NBallSystemEvent::BallJawImpact { ball_index, impact } => {
            assert_eq!(ball_index, 0);
            assert_eq!(impact.pocket, Pocket::CenterRight);
        }
        other => panic!("expected jaw impact, got {other:?}"),
    }
    match &advanced.states[0] {
        NBallSystemState::Pocketed {
            pocket,
            state_at_capture,
        } => {
            assert_eq!(*pocket, Pocket::CenterRight);
            assert!(
                state_at_capture.as_ball_state().speed().as_f64() > 0.0,
                "the late-drop path should preserve a meaningful post-jaw capture state"
            );
        }
        other => panic!("expected the jaw impact to resolve into a late drop, got {other:?}"),
    }
}

#[test]
fn a_shallow_side_jaw_glance_is_rejected_instead_of_late_dropping() {
    let table = TableSpec::default();
    let state = on_table(BallState::on_table(
        inches2(40.5, 56.125),
        Velocity2::new("13", "-6"),
        AngularVelocity3::zero(),
    ));

    let advanced = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &[NBallSystemState::from(state.clone())],
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    )
    .expect("pocket-aware test geometry should validate");

    match advanced.event.expect("a first event should be predicted") {
        NBallSystemEvent::BallJawImpact { ball_index, impact } => {
            assert_eq!(ball_index, 0);
            assert_eq!(impact.pocket, Pocket::CenterRight);
        }
        other => panic!("expected jaw impact, got {other:?}"),
    }
    match &advanced.states[0] {
        NBallSystemState::OnTable(state_after_jaw) => {
            assert!(
                state_after_jaw.as_ball_state().speed().as_f64() > 0.0,
                "the rejected jaw-glance should remain a live on-table state"
            );
        }
        other => panic!("expected the shallow jaw glance to stay on-table, got {other:?}"),
    }

    let simulated = simulate_n_balls_with_rails_and_pockets_on_table_until_rest(
        &[state],
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    )
    .expect("pocket-aware test geometry should validate");

    assert!(matches!(
        simulated.events.first(),
        Some(NBallSystemEvent::BallJawImpact { ball_index: 0, impact })
            if impact.pocket == Pocket::CenterRight
    ));
    assert!(!simulated.events.iter().any(|event| matches!(
        event,
        NBallSystemEvent::BallPocketCapture {
            ball_index: 0,
            capture,
        } if capture.pocket == Pocket::CenterRight
    )));
    match &simulated.states[0] {
        NBallSystemState::OnTable(state) => assert_eq!(
            state
                .as_ball_state()
                .motion_phase(TYPICAL_BALL_RADIUS.clone()),
            MotionPhase::Rest
        ),
        other => panic!(
            "expected the rejected jaw-glance to roll back out and stop on table, got {other:?}"
        ),
    }
}

#[test]
fn injected_pocket_shape_changes_the_predicted_jaw_impact_time() {
    let state = on_table(BallState::on_table(
        inches2(44.0, 58.0),
        Velocity2::new("12", "-11"),
        AngularVelocity3::zero(),
    ));
    let point_table =
        TableSpec::default().with_pocket_shape(Pocket::CenterRight, PocketShapeSpec::point_noses());
    let rounded_table = TableSpec::default().with_pocket_shape(
        Pocket::CenterRight,
        PocketShapeSpec::rounded_noses(Inches::from_f64(0.75)),
    );

    let point_impact = compute_next_ball_jaw_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &point_table,
        &motion_config(),
    )
    .expect("point jaws should still predict an impact");
    let rounded_impact = compute_next_ball_jaw_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &rounded_table,
        &motion_config(),
    )
    .expect("rounded jaws should still predict an impact");

    assert_eq!(point_impact.pocket, Pocket::CenterRight);
    assert_eq!(rounded_impact.pocket, Pocket::CenterRight);
    assert!(
        rounded_impact.time_until_impact.as_f64() < point_impact.time_until_impact.as_f64(),
        "larger rounded jaws should be struck earlier than point jaws"
    );
}

#[test]
fn a_slow_angled_corner_pocket_entry_outside_the_tp36_target_curve_is_rejected() {
    let table = TableSpec::default();
    let centered = slow_rolling_top_right_corner_pocket_state_at_angle(30.0, 0.0);
    let outside_target = slow_rolling_top_right_corner_pocket_state_at_angle(30.0, 1.6);

    assert!(
        compute_next_ball_pocket_capture_on_table(
            &centered,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .is_some(),
        "a centered 30-degree slow corner-pocket entry should still be accepted"
    );
    assert!(
        compute_next_ball_pocket_capture_on_table(
            &outside_target,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .is_none(),
        "TP 3.6's slow 30-degree corner-pocket target is narrower than the old capture circle"
    );
}

#[test]
fn a_slow_straight_corner_pocket_entry_outside_the_tp36_target_width_is_rejected() {
    let table = TableSpec::default();
    let centered = slow_rolling_top_right_corner_pocket_state(0.0);
    let outside_target = slow_rolling_top_right_corner_pocket_state(1.6);

    assert!(
        compute_next_ball_pocket_capture_on_table(
            &centered,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .is_some(),
        "a centered slow straight-in corner-pocket entry should still be accepted"
    );
    assert!(
        compute_next_ball_pocket_capture_on_table(
            &outside_target,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .is_none(),
        "TP 3.6's slow straight-in corner-pocket target is narrower than the old capture circle"
    );
}

#[test]
fn a_ball_heading_cleanly_into_a_corner_pocket_still_predicts_capture() {
    let table = TableSpec::default();
    let pocket_center = Pocket::TopRight.aiming_center();
    let pocket_x = table.diamond_to_inches(pocket_center.x).as_f64();
    let pocket_y = table.diamond_to_inches(pocket_center.y).as_f64();
    let diagonal = 0.5_f64.sqrt();
    let distance = 10.0;
    let speed = 40.0;
    let state = on_table(BallState::on_table(
        inches2(
            pocket_x - distance * diagonal,
            pocket_y - distance * diagonal,
        ),
        Velocity2::new(
            Inches::from_f64(speed * diagonal),
            Inches::from_f64(speed * diagonal),
        ),
        AngularVelocity3::zero(),
    ));

    let capture = compute_next_ball_pocket_capture_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("a straight-in corner-pocket entry should still be accepted");
    assert_eq!(capture.pocket, Pocket::TopRight);
}

#[test]
fn a_fast_straight_corner_pocket_entry_outside_the_tp38_target_width_is_rejected() {
    let table = TableSpec::default();
    let state = fast_rolling_top_right_corner_pocket_state(1.6);

    assert!(
        compute_next_ball_pocket_capture_on_table(
            &state,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .is_none(),
        "TP 3.8's fast straight-in corner-pocket target is much narrower than the old capture circle"
    );
}

#[test]
fn a_fast_corner_pocket_entry_uses_tp38_signed_target_asymmetry() {
    let table = TableSpec::default();
    let near_point_side = fast_rolling_top_right_corner_pocket_state_at_angle(45.0, 0.70);
    let far_wall_side = fast_rolling_top_right_corner_pocket_state_at_angle(45.0, -0.70);

    assert!(
        compute_next_ball_pocket_capture_on_table(
            &near_point_side,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .is_none(),
        "TP 3.8 rejects the near-point side of the signed fast-corner target"
    );
    assert!(
        compute_next_ball_pocket_capture_on_table(
            &far_wall_side,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .is_some(),
        "TP 3.8 accepts the wider far-wall side of the signed fast-corner target"
    );
}

#[test]
fn a_fast_corner_pocket_entry_beyond_the_tp38_effective_target_angle_is_rejected() {
    let table = TableSpec::default();
    let pocket_center = Pocket::TopRight.aiming_center();
    let pocket_x = table.diamond_to_inches(pocket_center.x).as_f64();
    let pocket_y = table.diamond_to_inches(pocket_center.y).as_f64();
    let tp38_over_limit_angle_degrees = 60.5;
    let absolute_angle_radians = (45.0_f64 + tp38_over_limit_angle_degrees).to_radians();
    let distance = 10.0;
    let speed = 80.0;
    let state = on_table(BallState::on_table(
        inches2(
            pocket_x - distance * absolute_angle_radians.cos(),
            pocket_y - distance * absolute_angle_radians.sin(),
        ),
        Velocity2::new(
            Inches::from_f64(speed * absolute_angle_radians.cos()),
            Inches::from_f64(speed * absolute_angle_radians.sin()),
        ),
        AngularVelocity3::zero(),
    ));

    assert!(
        compute_next_ball_pocket_capture_on_table(
            &state,
            &BallSetPhysicsSpec::default(),
            &table,
            &motion_config(),
        )
        .is_none(),
        "TP 3.8's fast-corner 59.841° cap should reject a synthetic 60.5° entry"
    );
}

#[test]
fn pocket_aware_advancing_matches_rail_aware_advancing_when_pockets_are_irrelevant() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let sliding_transition = vec![on_table(BallState::on_table(
        inches2(20.0, 20.0),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    ))];
    let opening_collision = vec![
        on_table(BallState::on_table(
            inches2(20.0, 20.0 - (2.0 * radius + 7.5)),
            Velocity2::new("0", "10"),
            AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
        )),
        on_table(BallState::resting_at(inches2(20.0, 20.0))),
    ];
    let shared_contact = shared_three_ball_contact_fixture();
    let disjoint_same_time_collisions = vec![
        on_table(BallState::on_table(
            inches2(10.0, 20.0 - (2.0 * radius + 7.5)),
            Velocity2::new("0", "10"),
            AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
        )),
        on_table(BallState::resting_at(inches2(10.0, 20.0))),
        on_table(BallState::on_table(
            inches2(30.0, 20.0 - (2.0 * radius + 7.5)),
            Velocity2::new("0", "10"),
            AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
        )),
        on_table(BallState::resting_at(inches2(30.0, 20.0))),
    ];
    let rail_impact = vec![on_table(BallState::on_table(
        inches2(20.0, top_plane - 7.5),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ))];

    for (label, states) in [
        ("motion transition", sliding_transition),
        ("opening collision", opening_collision),
        ("shared contact", shared_contact),
        (
            "disjoint same-time collisions",
            disjoint_same_time_collisions,
        ),
        ("rail impact", rail_impact),
    ] {
        assert_pocket_aware_matches_rail_aware(&states, label);
    }
}

#[test]
fn ordinary_and_pocket_aware_schedulers_share_the_contact_tolerance_boundary() {
    let ball = BallSetPhysicsSpec::default();
    let table = TableSpec::default();
    let motion = motion_config();

    for (label, contact_time_offset, expect_shared_contact) in [
        ("inside tolerance", 0.5e-12, true),
        ("outside tolerance", 2.0e-12, false),
    ] {
        let states = shared_three_ball_contact_fixture_with_time_offset(contact_time_offset);
        let first_collision = compute_next_ball_ball_collision_during_current_phases_on_table(
            &states[0], &states[1], &ball, &motion,
        )
        .expect("the first object ball should be reached");
        let second_collision = compute_next_ball_ball_collision_during_current_phases_on_table(
            &states[0], &states[2], &ball, &motion,
        )
        .expect("the second object ball should be reached");
        let actual_time_offset = (second_collision.time_until_impact.as_f64()
            - first_collision.time_until_impact.as_f64())
        .abs();
        if expect_shared_contact {
            assert!(
                actual_time_offset <= 1e-12,
                "{label}: fixture contacts were {actual_time_offset:e} seconds apart"
            );
        } else {
            assert!(
                actual_time_offset > 1e-12,
                "{label}: fixture contacts were {actual_time_offset:e} seconds apart"
            );
        }

        let state_refs = states.iter().collect::<Vec<_>>();
        let ordinary = compute_next_n_ball_event_on_table(&state_refs, &ball, &motion)
            .expect("ordinary scheduler geometry should validate")
            .expect("ordinary scheduler should predict a collision");
        let system_states = states
            .into_iter()
            .map(NBallSystemState::from)
            .collect::<Vec<_>>();
        let pocket_aware = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
            &system_states,
            &ball,
            &table,
            &motion,
        )
        .expect("pocket-aware scheduler geometry should validate")
        .expect("pocket-aware scheduler should predict a collision");

        match (expect_shared_contact, &ordinary) {
            (
                true,
                NBallOnTableEvent::SharedBallBallContact {
                    ball_indices,
                    ball_ball_pairs,
                    ..
                },
            ) => {
                assert_eq!(ball_indices, &[0, 1, 2], "{label}");
                assert_eq!(ball_ball_pairs, &[(0, 1), (0, 2)], "{label}");
            }
            (
                false,
                NBallOnTableEvent::BallBallCollision {
                    first_ball_index: 0,
                    second_ball_index: 1,
                    ..
                },
            ) => {}
            (_, event) => panic!("{label}: unexpected ordinary scheduler event {event:?}"),
        }
        assert_events_equivalent(Some(&ordinary), Some(&pocket_aware), label);
    }
}

#[test]
fn pocket_aware_shared_contact_uses_the_ball_ball_time_when_an_unrelated_transition_is_tied() {
    let table = TableSpec::default();
    let mut states = shared_three_ball_contact_fixture()
        .into_iter()
        .map(NBallSystemState::from)
        .collect::<Vec<_>>();
    states.push(NBallSystemState::from(
        rolling_transition_just_before_shared_contact(),
    ));

    let event = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &states,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("pocket-aware test geometry should validate")
    .expect("an event should be predicted");

    match event {
        NBallSystemEvent::SharedBallBallContact {
            time_until_contact,
            ball_indices,
            ball_ball_pairs,
            ..
        } => {
            assert!(
                (time_until_contact.as_f64() - 1.0).abs() <= 1e-13,
                "shared contact should be reported at the ball-ball contact time, not an unrelated transition time: {time_until_contact:?}"
            );
            assert_eq!(ball_indices, vec![0, 1, 2]);
            assert_eq!(ball_ball_pairs, vec![(0, 1), (0, 2)]);
        }
        other => panic!("expected shared contact, got {other:?}"),
    }
}

#[test]
fn pocket_aware_advancing_also_batches_disjoint_same_time_ball_ball_collisions() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let a = NBallSystemState::from(on_table(BallState::on_table(
        inches2(0.0, -(2.0 * radius + 7.5)),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    )));
    let b = NBallSystemState::from(on_table(BallState::resting_at(inches2(0.0, 0.0))));
    let c = NBallSystemState::from(on_table(BallState::on_table(
        inches2(20.0, -(2.0 * radius + 7.5)),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    )));
    let d = NBallSystemState::from(on_table(BallState::resting_at(inches2(20.0, 0.0))));

    let advanced = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &[a, b, c, d],
        &BallSetPhysicsSpec::default(),
        &TableSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    )
    .expect("pocket-aware test geometry should validate");

    match advanced.event.expect("an event should be reported") {
        NBallSystemEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } => {
            assert_eq!((first_ball_index, second_ball_index), (0, 1));
            assert_close(collision.time_until_impact.as_f64(), 1.0);
        }
        other => panic!("expected primary ball-ball collision, got {other:?}"),
    }
    match &advanced.states[0] {
        NBallSystemState::OnTable(state) => {
            assert_close(state.as_ball_state().speed().as_f64(), 0.0)
        }
        other => panic!("expected first ball to remain on table, got {other:?}"),
    }
    match &advanced.states[1] {
        NBallSystemState::OnTable(state) => {
            assert_close(state.as_ball_state().velocity.y().as_f64(), 5.0)
        }
        other => panic!("expected second ball to remain on table, got {other:?}"),
    }
    match &advanced.states[2] {
        NBallSystemState::OnTable(state) => {
            assert_close(state.as_ball_state().speed().as_f64(), 0.0)
        }
        other => panic!("expected third ball to remain on table, got {other:?}"),
    }
    match &advanced.states[3] {
        NBallSystemState::OnTable(state) => {
            assert_close(state.as_ball_state().velocity.y().as_f64(), 5.0)
        }
        other => panic!("expected fourth ball to remain on table, got {other:?}"),
    }
}

fn throw_aware_frozen_line_fixture(downstream_gap: f64) -> Vec<NBallSystemState> {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let first_object_x = 30.0;
    let y = 30.0;

    vec![
        NBallSystemState::from(on_table(BallState::on_table(
            inches2(first_object_x - (2.0 * radius + 7.5), y),
            Velocity2::new("10", "0"),
            AngularVelocity3::new(0.0, 10.0 / radius, 4.0),
        ))),
        NBallSystemState::from(on_table(BallState::resting_at(inches2(first_object_x, y)))),
        NBallSystemState::from(on_table(BallState::resting_at(inches2(
            first_object_x + 2.0 * radius,
            y,
        )))),
        NBallSystemState::from(on_table(BallState::resting_at(inches2(
            first_object_x + 4.0 * radius + downstream_gap,
            y,
        )))),
    ]
}

fn advance_throw_aware_system_once(states: &[NBallSystemState]) -> Vec<NBallSystemState> {
    advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table(
        states,
        &BallSetPhysicsSpec::default(),
        &TableSpec::default(),
        &motion_config(),
        CollisionModel::ThrowAware,
        &BallBallCollisionConfig::human_tuned(),
        RailModel::SpinAware,
        &billiards::RailCollisionProfile::default(),
    )
    .expect("throw-aware system contact should resolve")
    .states
}

fn assert_system_states_near_by_position(
    expected: &[NBallSystemState],
    actual: &[NBallSystemState],
) {
    const TOLERANCE: f64 = 1e-8;

    assert_eq!(actual.len(), expected.len());
    for expected_system_state in expected {
        let expected_ball = expected_system_state.as_ball_state();
        let expected_x = expected_ball.position.x().as_f64();
        let expected_y = expected_ball.position.y().as_f64();
        let actual_system_state = actual
            .iter()
            .find(|state| {
                let ball = state.as_ball_state();
                (ball.position.x().as_f64() - expected_x).abs() <= TOLERANCE
                    && (ball.position.y().as_f64() - expected_y).abs() <= TOLERANCE
            })
            .unwrap_or_else(|| {
                panic!(
                    "no permuted result remained at physical position ({expected_x}, {expected_y})"
                )
            });
        match (expected_system_state, actual_system_state) {
            (NBallSystemState::OnTable(_), NBallSystemState::OnTable(_))
            | (NBallSystemState::Airborne(_), NBallSystemState::Airborne(_)) => {}
            (expected_kind, actual_kind) => panic!(
                "state classification changed under index permutation at ({expected_x}, {expected_y}): expected {expected_kind:?}, got {actual_kind:?}"
            ),
        }

        let actual_ball = actual_system_state.as_ball_state();
        for (label, expected_value, actual_value) in [
            (
                "height",
                expected_ball.height.as_f64(),
                actual_ball.height.as_f64(),
            ),
            (
                "velocity x",
                expected_ball.velocity.x().as_f64(),
                actual_ball.velocity.x().as_f64(),
            ),
            (
                "velocity y",
                expected_ball.velocity.y().as_f64(),
                actual_ball.velocity.y().as_f64(),
            ),
            (
                "vertical velocity",
                expected_ball.vertical_velocity.as_f64(),
                actual_ball.vertical_velocity.as_f64(),
            ),
            (
                "angular velocity x",
                expected_ball.angular_velocity.x().as_f64(),
                actual_ball.angular_velocity.x().as_f64(),
            ),
            (
                "angular velocity y",
                expected_ball.angular_velocity.y().as_f64(),
                actual_ball.angular_velocity.y().as_f64(),
            ),
            (
                "angular velocity z",
                expected_ball.angular_velocity.z().as_f64(),
                actual_ball.angular_velocity.z().as_f64(),
            ),
        ] {
            assert!(
                (actual_value - expected_value).abs() <= TOLERANCE,
                "{label} changed under index permutation at ({expected_x}, {expected_y}): expected {expected_value}, got {actual_value}"
            );
        }
    }
}

#[test]
fn throw_aware_rolling_spin_propagates_through_a_four_ball_frozen_line_in_one_advance() {
    let states = throw_aware_frozen_line_fixture(0.0);
    let advanced_states = advance_throw_aware_system_once(&states);
    let downstream_speed = advanced_states[3].as_ball_state().speed().as_f64();

    assert!(
        downstream_speed > 1e-6,
        "the downstream ball in the touching component should move in the impact advance, got speed {downstream_speed}"
    );
}

#[test]
fn throw_aware_contact_component_does_not_cross_a_clear_downstream_gap() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let gap = 1e-4;
    let states = throw_aware_frozen_line_fixture(gap);
    let measured_gap = states[3].as_ball_state().position.x().as_f64()
        - states[2].as_ball_state().position.x().as_f64()
        - 2.0 * radius;
    assert!(measured_gap > 1e-7);

    let advanced_states = advance_throw_aware_system_once(&states);
    let downstream = advanced_states[3].as_ball_state();
    assert!(
        downstream.speed().as_f64() <= 1e-10,
        "a ball beyond the touching tolerance should not receive same-step linear motion"
    );
    assert!(
        downstream.angular_velocity.x().as_f64().abs() <= 1e-10
            && downstream.angular_velocity.y().as_f64().abs() <= 1e-10
            && downstream.angular_velocity.z().as_f64().abs() <= 1e-10,
        "a ball beyond the touching tolerance should not receive same-step spin"
    );
    assert!(matches!(advanced_states[3], NBallSystemState::OnTable(_)));
}

#[test]
fn throw_aware_frozen_component_resolution_is_invariant_to_ball_index_permutation() {
    let states = throw_aware_frozen_line_fixture(0.0);
    let permuted_states = vec![
        states[3].clone(),
        states[1].clone(),
        states[0].clone(),
        states[2].clone(),
    ];

    let advanced = advance_throw_aware_system_once(&states);
    let permuted_advanced = advance_throw_aware_system_once(&permuted_states);

    assert_system_states_near_by_position(&advanced, &permuted_advanced);
}

#[test]
fn throw_aware_spin_contact_applies_only_upward_unilateral_table_support() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let states = vec![
        NBallSystemState::from(on_table(BallState::on_table(
            inches2(30.0 - (2.0 * radius + 7.5), 30.0),
            Velocity2::new("10", "0"),
            AngularVelocity3::new(0.0, 10.0 / radius, 0.0),
        ))),
        NBallSystemState::from(on_table(BallState::resting_at(inches2(30.0, 30.0)))),
    ];
    let collision_config = BallBallCollisionConfig::human_tuned();
    let advanced = advance_to_next_n_ball_system_event_with_physics_and_pockets_on_table(
        &states,
        &BallSetPhysicsSpec::default(),
        &TableSpec::default(),
        &motion_config(),
        CollisionModel::ThrowAware,
        &collision_config,
        RailModel::SpinAware,
        &billiards::RailCollisionProfile::default(),
    )
    .expect("throw-aware spinning pair contact should resolve");
    let NBallSystemEvent::BallBallCollision {
        first_ball_index,
        second_ball_index,
        collision,
    } = advanced
        .event
        .as_ref()
        .expect("the spinning incoming ball should produce a pair impact")
    else {
        panic!(
            "expected a ball-ball collision event, got {:?}",
            advanced.event
        );
    };
    let free = billiards::collide_ball_ball_detailed_on_table_with_config(
        &collision.a_at_impact,
        &collision.b_at_impact,
        CollisionModel::ThrowAware,
        &collision_config,
    );
    assert!(
        free.a_after.vertical_velocity.as_f64() > 1e-6,
        "the free pair response should lift the rolling incoming ball"
    );
    assert!(
        free.b_after.vertical_velocity.as_f64() < -1e-6,
        "the free pair response should drive the object ball downward"
    );

    let NBallSystemState::Airborne(lifted) = &advanced.states[*first_ball_index] else {
        panic!(
            "the upward free response should remain airborne, got {:?}",
            advanced.states[*first_ball_index]
        );
    };
    assert!(lifted.vertical_velocity.as_f64() > 1e-6);
    let NBallSystemState::OnTable(supported) = &advanced.states[*second_ball_index] else {
        panic!(
            "the downward free response should receive table support, got {:?}",
            advanced.states[*second_ball_index]
        );
    };
    assert!(supported.as_ball_state().vertical_velocity.as_f64().abs() <= 1e-10);
}

#[test]
fn pocket_aware_frozen_three_ball_line_contact_matches_on_table_resolution() {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let cue_ball = on_table(BallState::on_table(
        inches2(-(2.0 * radius + 7.5), 0.0),
        Velocity2::new("10", "0"),
        AngularVelocity3::new(0.0, 10.0 / radius, 0.0),
    ));
    let first_object = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let second_object = on_table(BallState::resting_at(inches2(2.0 * radius, 0.0)));
    let states = vec![cue_ball, first_object, second_object];

    let plain = advance_to_next_n_ball_event_on_table(
        &states,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
    )
    .expect("on-table test geometry should validate");
    let system_states = states
        .iter()
        .cloned()
        .map(NBallSystemState::from)
        .collect::<Vec<_>>();
    let pocket_aware = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &system_states,
        &BallSetPhysicsSpec::default(),
        &TableSpec::default(),
        &motion_config(),
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    )
    .expect("pocket-aware test geometry should validate");

    match plain.event.expect("plain event should be reported") {
        billiards::NBallOnTableEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } => {
            assert_eq!((first_ball_index, second_ball_index), (0, 1));
            assert_close(collision.time_until_impact.as_f64(), 1.0);
        }
        other => panic!("expected plain ball-ball collision, got {other:?}"),
    }
    match pocket_aware
        .event
        .expect("pocket-aware event should be reported")
    {
        NBallSystemEvent::BallBallCollision {
            first_ball_index,
            second_ball_index,
            collision,
        } => {
            assert_eq!((first_ball_index, second_ball_index), (0, 1));
            assert_close(collision.time_until_impact.as_f64(), 1.0);
        }
        other => panic!("expected pocket-aware ball-ball collision, got {other:?}"),
    }

    assert_close(pocket_aware.elapsed.as_f64(), plain.elapsed.as_f64());
    assert_eq!(unwrap_on_table_states(&pocket_aware.states), plain.states);
}

#[test]
fn advancing_to_a_pocket_capture_marks_that_ball_pocketed_and_advances_other_balls() {
    let table = TableSpec::default();
    let a = NBallSystemState::from(rolling_toward_center_right_side_pocket());
    let b = NBallSystemState::from(on_table(BallState::on_table(
        inches2(20.0, 20.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    )));

    let advanced = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &[a, b],
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    )
    .expect("pocket-aware test geometry should validate");

    let event = advanced.event.expect("an event should be reported");
    match event {
        NBallSystemEvent::BallPocketCapture {
            ball_index,
            capture,
        } => {
            assert_eq!(ball_index, 0);
            assert_eq!(capture.pocket, Pocket::CenterRight);
        }
        other => panic!("expected pocket capture, got {other:?}"),
    }
    match &advanced.states[0] {
        NBallSystemState::Pocketed {
            pocket,
            state_at_capture,
        } => {
            assert_eq!(*pocket, Pocket::CenterRight);
            assert_eq!(
                state_at_capture
                    .as_ball_state()
                    .motion_phase(TYPICAL_BALL_RADIUS.clone()),
                MotionPhase::Rolling
            );
        }
        other => panic!("expected pocketed state, got {other:?}"),
    }
    match &advanced.states[1] {
        NBallSystemState::OnTable(state) => {
            assert_eq!(
                state
                    .as_ball_state()
                    .motion_phase(TYPICAL_BALL_RADIUS.clone()),
                MotionPhase::Spinning
            );
            assert_close(
                state.as_ball_state().angular_velocity.z().as_f64(),
                6.0 - 2.0 * advanced.elapsed.as_f64(),
            );
        }
        other => panic!("expected on-table passive state, got {other:?}"),
    }
}

#[test]
fn simulating_with_pockets_until_rest_keeps_pocketed_balls_out_of_play_and_stops_the_rest() {
    let table = TableSpec::default();
    let a = rolling_toward_center_right_side_pocket();
    let b = on_table(BallState::on_table(
        inches2(20.0, 20.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    ));

    let simulated = simulate_n_balls_with_rails_and_pockets_on_table_until_rest(
        &[a, b],
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    )
    .expect("pocket-aware test geometry should validate");

    assert!(simulated.events.iter().any(|event| matches!(
        event,
        NBallSystemEvent::BallPocketCapture {
            ball_index: 0,
            capture,
        } if capture.pocket == Pocket::CenterRight
    )));
    match &simulated.states[0] {
        NBallSystemState::Pocketed { pocket, .. } => assert_eq!(*pocket, Pocket::CenterRight),
        other => panic!("expected first ball to be pocketed, got {other:?}"),
    }
    match &simulated.states[1] {
        NBallSystemState::OnTable(state) => assert_eq!(
            state
                .as_ball_state()
                .motion_phase(TYPICAL_BALL_RADIUS.clone()),
            MotionPhase::Rest
        ),
        other => panic!("expected second ball to remain on the table, got {other:?}"),
    }
    assert!(!simulated.events.iter().any(|event| matches!(
        event,
        NBallSystemEvent::BallRailImpact {
            ball_index: 0,
            impact,
        } if impact.rail == Rail::Right
    )));
}

#[test]
fn pocket_aware_until_rest_reports_frozen_rail_contact_no_progress() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let frozen = on_table(BallState::on_table(
        inches2(10.0, top_plane),
        Velocity2::zero(),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let passive = on_table(BallState::resting_at(inches2(30.0, 20.0)));

    let error = simulate_n_balls_with_rails_and_pockets_on_table_until_rest(
        &[frozen, passive],
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
        CollisionModel::Ideal,
        RailModel::Mirror,
    )
    .expect_err("an unchanged zero-time rail response must not look like successful rest");

    assert_eq!(error, NBallGeometryError::ZeroTimeNoProgress);
}

#[test]
fn pocket_aware_until_rest_continues_after_shared_contact_like_rail_aware_when_pockets_are_irrelevant(
) {
    let table = TableSpec::default();
    let ball_set = BallSetPhysicsSpec::default();
    let motion = motion_config();
    let states = shared_three_ball_contact_fixture();

    let rail_aware = simulate_n_balls_with_rails_on_table_until_rest(
        &states,
        &ball_set,
        &table,
        &motion,
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    )
    .expect("rail-aware test geometry should validate");
    let pocket_aware = simulate_n_balls_with_rails_and_pockets_on_table_until_rest(
        &states,
        &ball_set,
        &table,
        &motion,
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    )
    .expect("pocket-aware test geometry should validate");

    assert_eq!(
        rail_aware.events.len(),
        pocket_aware.events.len(),
        "pocket-aware simulation should record the same event count when pockets are irrelevant"
    );

    assert!(
        rail_aware.events.len() > 1,
        "until-rest simulation should continue after the resolved shared contact"
    );

    for (index, (rail_event, pocket_event)) in rail_aware
        .events
        .iter()
        .zip(&pocket_aware.events)
        .enumerate()
    {
        assert_events_equivalent(
            Some(rail_event),
            Some(pocket_event),
            &format!("until-rest event {index}"),
        );
    }

    assert_close(pocket_aware.elapsed.as_f64(), rail_aware.elapsed.as_f64());
    assert_eq!(
        unwrap_on_table_states(&pocket_aware.states),
        rail_aware.states
    );
}

#[test]
fn cached_pocket_aware_until_rest_simulation_matches_manual_event_stepping() {
    let table = TableSpec::default();
    let ball_set = BallSetPhysicsSpec::default();
    let motion = motion_config();
    let seed_a = rolling_toward_center_right_side_pocket();
    let seed_b = on_table(BallState::on_table(
        inches2(20.0, 20.0),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    ));
    let mut manual_states = vec![
        NBallSystemState::from(seed_a.clone()),
        NBallSystemState::from(seed_b.clone()),
    ];
    let mut manual_elapsed = 0.0;
    let mut manual_events = Vec::new();

    loop {
        let advanced = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
            &manual_states,
            &ball_set,
            &table,
            &motion,
            CollisionModel::Ideal,
            billiards::RailModel::Mirror,
        )
        .expect("pocket-aware test geometry should validate");
        let Some(event) = advanced.event else {
            break;
        };
        manual_elapsed += advanced.elapsed.as_f64();
        manual_states = advanced.states;
        manual_events.push(event);
    }

    let cached = simulate_n_balls_with_rails_and_pockets_on_table_until_rest(
        &[seed_a, seed_b],
        &ball_set,
        &table,
        &motion,
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    )
    .expect("pocket-aware test geometry should validate");

    assert_close(cached.elapsed.as_f64(), manual_elapsed);
    assert_eq!(cached.events, manual_events);
    assert_eq!(cached.states, manual_states);
}

#[test]
fn cached_pocket_aware_shared_contact_matches_manual_event_stepping() {
    let table = TableSpec::default();
    let ball_set = BallSetPhysicsSpec::default();
    let motion = motion_config();
    let seeds = shared_three_ball_contact_fixture();
    let mut manual_states = seeds
        .iter()
        .cloned()
        .map(NBallSystemState::from)
        .collect::<Vec<_>>();
    let mut manual_elapsed = 0.0;
    let mut manual_events = Vec::new();

    loop {
        let previous_states = manual_states.clone();
        let advanced = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
            &manual_states,
            &ball_set,
            &table,
            &motion,
            CollisionModel::Ideal,
            billiards::RailModel::Mirror,
        )
        .expect("pocket-aware test geometry should validate");
        let Some(event) = advanced.event else {
            break;
        };
        let step_elapsed = advanced.elapsed.as_f64();
        manual_elapsed += step_elapsed;
        manual_states = advanced.states;
        manual_events.push(event);
        if step_elapsed <= 1e-12 && manual_states == previous_states {
            break;
        }
    }

    let cached = simulate_n_balls_with_rails_and_pockets_on_table_until_rest(
        &seeds,
        &ball_set,
        &table,
        &motion,
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    )
    .expect("pocket-aware test geometry should validate");

    match cached.events.first() {
        Some(NBallSystemEvent::SharedBallBallContact {
            ball_ball_pairs, ..
        }) => assert_eq!(ball_ball_pairs, &vec![(0, 1), (0, 2)]),
        other => panic!("expected shared contact event, got {other:?}"),
    }
    assert_close(cached.elapsed.as_f64(), manual_elapsed);
    assert_eq!(cached.events, manual_events);
    assert_eq!(cached.states, manual_states);
}
