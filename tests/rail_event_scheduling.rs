use billiards::{
    collide_ball_ball_detailed_on_table, compute_next_ball_rail_impact_on_table,
    compute_next_two_ball_event_with_rails_on_table, AngularVelocity3, BallSetPhysicsSpec,
    BallState, CollisionModel, Diamond, Inches, Inches2, InchesPerSecondSq, MotionPhase,
    MotionPhaseConfig, MotionTransitionConfig, OnTableBallState, OnTableMotionConfig,
    RadiansPerSecondSq, Rail, RollingResistanceModel, SlidingFrictionModel, SpinDecayModel,
    TableSpec, TwoBallEventBall, TwoBallOnTableEvent, Velocity2, TYPICAL_BALL_RADIUS,
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
fn a_rolling_ball_predicts_a_top_rail_impact_before_it_stops() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let state = on_table(BallState::on_table(
        inches2(10.0, top_plane - 7.5),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));

    let impact = compute_next_ball_rail_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("the rolling ball should reach the rail before stopping");

    assert_eq!(impact.rail, Rail::Top);
    assert_close(impact.time_until_impact.as_f64(), 1.0);
    assert_close(
        impact.state_at_impact.as_ball_state().position.y().as_f64(),
        top_plane,
    );
    assert_eq!(
        impact
            .state_at_impact
            .as_ball_state()
            .motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Rolling
    );
}

#[test]
fn every_rail_preserves_boundary_orientation_contact_direction_and_snapping() {
    let table = TableSpec::three_cushion_carom_10ft();
    let ball = BallSetPhysicsSpec::three_cushion_carom();
    let radius = ball.radius.as_f64();
    let right_plane = table.diamond_to_inches(Diamond::four()).as_f64() - radius;
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let center_x = 0.5 * (radius + right_plane);
    let center_y = 0.5 * (radius + top_plane);
    let gap: f64 = 1.234_567;
    let expected_time = (10.0 - (100.0 - 10.0 * gap).sqrt()) / 5.0;

    for (rail, contact_x, contact_y, inside_x, inside_y, vx, vy, wx, wy) in [
        (
            Rail::Top,
            center_x,
            top_plane,
            center_x,
            top_plane - gap,
            0.0,
            10.0,
            -10.0 / radius,
            0.0,
        ),
        (
            Rail::Right,
            right_plane,
            center_y,
            right_plane - gap,
            center_y,
            10.0,
            0.0,
            0.0,
            10.0 / radius,
        ),
        (
            Rail::Bottom,
            center_x,
            radius,
            center_x,
            radius + gap,
            0.0,
            -10.0,
            10.0 / radius,
            0.0,
        ),
        (
            Rail::Left,
            radius,
            center_y,
            radius + gap,
            center_y,
            -10.0,
            0.0,
            0.0,
            -10.0 / radius,
        ),
    ] {
        let approaching = on_table(BallState::on_table(
            inches2(inside_x, inside_y),
            Velocity2::new(Inches::from_f64(vx), Inches::from_f64(vy)),
            AngularVelocity3::new(wx, wy, 0.0),
        ));
        let impact =
            compute_next_ball_rail_impact_on_table(&approaching, &ball, &table, &motion_config())
                .unwrap_or_else(|| {
                    panic!("{rail:?} should be reached while the ball is approaching")
                });

        assert_eq!(impact.rail, rail);
        assert_close(impact.time_until_impact.as_f64(), expected_time);
        let impact_state = impact.state_at_impact.as_ball_state();
        let (actual_normal, expected_normal) = match rail {
            Rail::Top | Rail::Bottom => (impact_state.position.y().as_f64(), contact_y),
            Rail::Left | Rail::Right => (impact_state.position.x().as_f64(), contact_x),
        };
        assert_eq!(
            actual_normal.to_bits(),
            expected_normal.to_bits(),
            "{rail:?} impact must be snapped exactly to its contact coordinate"
        );

        let at_contact_approaching = on_table(BallState::on_table(
            inches2(contact_x, contact_y),
            Velocity2::new(Inches::from_f64(vx), Inches::from_f64(vy)),
            AngularVelocity3::new(wx, wy, 0.0),
        ));
        let immediate = compute_next_ball_rail_impact_on_table(
            &at_contact_approaching,
            &ball,
            &table,
            &motion_config(),
        )
        .unwrap_or_else(|| panic!("{rail:?} approaching contact should schedule immediately"));
        assert_eq!(immediate.rail, rail);
        assert_eq!(immediate.time_until_impact.as_f64(), 0.0);

        let at_contact_separating = on_table(BallState::on_table(
            inches2(contact_x, contact_y),
            Velocity2::new(Inches::from_f64(-vx), Inches::from_f64(-vy)),
            AngularVelocity3::new(-wx, -wy, 0.0),
        ));
        assert!(
            compute_next_ball_rail_impact_on_table(
                &at_contact_separating,
                &ball,
                &table,
                &motion_config(),
            )
            .is_none(),
            "{rail:?} separating contact must not schedule a repeated impact"
        );
    }
}

fn rolling_ball_with_side_spin(x: f64, y: f64, vertical_spin: f64) -> OnTableBallState {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    on_table(BallState::on_table(
        inches2(x, y),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, vertical_spin),
    ))
}

#[test]
fn curved_rolling_ball_reaches_the_right_rail_before_its_transition() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let right_plane = table.diamond_to_inches(Diamond::four()).as_f64() - radius;
    let state = rolling_ball_with_side_spin(48.871, 20.0, 2.0);

    let impact = compute_next_ball_rail_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("the canonical rightward curve reaches the rail before one second");
    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    {
        assert_eq!(
            impact.time_until_impact.as_f64().to_bits(),
            4_606_382_454_841_163_776
        );
        assert_eq!(
            on_table_state_bits(&impact.state_at_impact),
            [
                4_632_075_362_052_866_048,
                4_628_303_234_083_889_926,
                0,
                4_575_546_620_839_147_161,
                4_617_815_489_278_519_670,
                0,
                13_840_506_473_816_503_060,
                4_574_558_109_874_095_581,
                4_595_567_731_961_364_480,
            ]
        );
    }

    assert_eq!(impact.rail, Rail::Right);
    assert!(impact.time_until_impact.as_f64() > 0.0 && impact.time_until_impact.as_f64() < 1.0);
    assert_close(
        impact.state_at_impact.as_ball_state().position.x().as_f64(),
        right_plane,
    );
    assert!(
        impact.state_at_impact.as_ball_state().velocity.x().as_f64() > 0.0,
        "right-rail contact must be entering"
    );
}

#[test]
fn opposite_spin_curve_away_does_not_create_a_right_rail_impact() {
    let state = rolling_ball_with_side_spin(48.871, 20.0, -2.0);

    assert!(compute_next_ball_rail_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &TableSpec::default(),
        &motion_config(),
    )
    .is_none());
}

#[test]
fn a_ball_already_touching_a_rail_and_moving_into_it_predicts_an_immediate_impact() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let state = on_table(BallState::on_table(
        inches2(10.0, top_plane),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));

    let impact = compute_next_ball_rail_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect(
        "a frozen-to-rail incoming ball should rebound immediately instead of tunneling through the cushion",
    );

    assert_eq!(impact.rail, Rail::Top);
    assert_close(impact.time_until_impact.as_f64(), 0.0);
    assert_close(
        impact.state_at_impact.as_ball_state().position.y().as_f64(),
        top_plane,
    );
}

#[test]
fn a_ball_touching_a_rail_and_accelerating_into_it_predicts_an_immediate_impact() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let state = on_table(BallState::on_table(
        inches2(10.0, top_plane),
        Velocity2::new("0", "0"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));

    let impact = compute_next_ball_rail_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect(
        "a frozen-to-rail ball whose sliding friction accelerates it into the cushion should rebound immediately",
    );

    assert_eq!(impact.rail, Rail::Top);
    assert_close(impact.time_until_impact.as_f64(), 0.0);
    assert_close(
        impact.state_at_impact.as_ball_state().position.y().as_f64(),
        top_plane,
    );
}

#[test]
fn a_rolling_ball_returns_none_when_it_stops_before_the_rail() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let state = on_table(BallState::on_table(
        inches2(10.0, top_plane - 11.0),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));

    assert!(compute_next_ball_rail_impact_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .is_none());
}

#[test]
fn post_collision_follow_requires_richer_rail_scheduling() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let object_ball = on_table(BallState::resting_at(inches2(4.2, 40.0)));
    let outside_continuation = collide_ball_ball_detailed_on_table(
        &on_table(BallState::on_table(
            inches2(
                4.2 - radius * 2.0_f64.sqrt(),
                40.0 - radius * 2.0_f64.sqrt(),
            ),
            Velocity2::new("0", "10"),
            AngularVelocity3::new(-10.0 / radius, 0.0, -6.0),
        )),
        &object_ball,
        CollisionModel::ThrowAware,
    )
    .into_cue_ball_continuation();
    let inside_continuation = collide_ball_ball_detailed_on_table(
        &on_table(BallState::on_table(
            inches2(
                4.2 - radius * 2.0_f64.sqrt(),
                40.0 - radius * 2.0_f64.sqrt(),
            ),
            Velocity2::new("0", "10"),
            AngularVelocity3::new(-10.0 / radius, 0.0, 6.0),
        )),
        &object_ball,
        CollisionModel::ThrowAware,
    )
    .into_cue_ball_continuation();

    for continuation in [outside_continuation, inside_continuation] {
        assert!(matches!(
            continuation
                .next_rail_impact(&BallSetPhysicsSpec::default(), &table, &motion_config(),),
            Err(billiards::OnTableStateError::VerticalVelocityPresent { .. })
        ));
    }
}

#[test]
fn follow_and_english_require_richer_rail_aware_scheduling_after_contact() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let object_ball_1 = on_table(BallState::resting_at(inches2(4.2, 40.0)));
    let passive_ball = on_table(BallState::resting_at(inches2(30.0, 30.0)));
    let follow_outside = on_table(BallState::on_table(
        inches2(
            4.2 - radius * 2.0_f64.sqrt(),
            40.0 - radius * 2.0_f64.sqrt(),
        ),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-6.0, 0.0, -6.0),
    ));
    let follow_inside = on_table(BallState::on_table(
        inches2(
            4.2 - radius * 2.0_f64.sqrt(),
            40.0 - radius * 2.0_f64.sqrt(),
        ),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-6.0, 0.0, 6.0),
    ));

    for continuation in [
        collide_ball_ball_detailed_on_table(
            &follow_outside,
            &object_ball_1,
            CollisionModel::ThrowAware,
        )
        .into_cue_ball_continuation(),
        collide_ball_ball_detailed_on_table(
            &follow_inside,
            &object_ball_1,
            CollisionModel::ThrowAware,
        )
        .into_cue_ball_continuation(),
    ] {
        assert!(matches!(
            continuation.next_event_against_ball_with_rails(
                &passive_ball,
                &BallSetPhysicsSpec::default(),
                &table,
                &motion_config(),
            ),
            Err(billiards::PostContactContinuationError::NotOnTable(
                billiards::OnTableStateError::VerticalVelocityPresent { .. }
            ))
        ));
    }
}

#[test]
fn the_rail_aware_scheduler_picks_a_rail_impact_before_a_later_motion_transition() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let a = on_table(BallState::on_table(
        inches2(10.0, top_plane - 7.5),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let b = on_table(BallState::resting_at(inches2(30.0, 30.0)));

    let event = compute_next_two_ball_event_with_rails_on_table(
        &a,
        &b,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("test geometry should validate")
    .expect("an event should be predicted");

    match event {
        TwoBallOnTableEvent::BallRailImpact { ball, impact } => {
            assert_eq!(ball, TwoBallEventBall::A);
            assert_eq!(impact.rail, Rail::Top);
            assert_close(impact.time_until_impact.as_f64(), 1.0);
        }
        other => panic!("expected ball-rail impact, got {other:?}"),
    }
}

#[test]
fn the_rail_aware_scheduler_still_prefers_motion_transition_when_the_rail_is_not_reachable() {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let top_plane = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let a = on_table(BallState::on_table(
        inches2(10.0, top_plane - 11.0),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
    ));
    let b = on_table(BallState::resting_at(inches2(30.0, 30.0)));

    let event = compute_next_two_ball_event_with_rails_on_table(
        &a,
        &b,
        &BallSetPhysicsSpec::default(),
        &table,
        &motion_config(),
    )
    .expect("test geometry should validate")
    .expect("an event should be predicted");

    match event {
        TwoBallOnTableEvent::MotionTransition { ball, transition } => {
            assert_eq!(ball, TwoBallEventBall::A);
            assert_eq!(transition.phase_before, MotionPhase::Rolling);
            assert_eq!(transition.phase_after, MotionPhase::Rest);
            assert_close(transition.time_until_transition.as_f64(), 2.0);
        }
        other => panic!("expected motion transition, got {other:?}"),
    }
}
