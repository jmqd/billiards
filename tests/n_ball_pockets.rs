use billiards::{
    advance_to_next_n_ball_event_on_table, advance_to_next_n_ball_event_with_rails_on_table,
    advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table,
    compute_next_ball_jaw_impact_on_table, compute_next_ball_pocket_capture_on_table,
    compute_next_n_ball_system_event_with_rails_and_pockets_on_table,
    simulate_n_balls_with_rails_and_pockets_on_table_until_rest,
    simulate_n_balls_with_rails_on_table_until_rest, AngularVelocity3, BallSetPhysicsSpec,
    BallState, CollisionModel, Diamond, Inches, Inches2, InchesPerSecondSq, MotionPhase,
    MotionPhaseConfig, MotionTransitionConfig, NBallOnTableEvent, NBallSystemEvent,
    NBallSystemState, OnTableBallState, OnTableMotionConfig, Pocket, PocketJawGeometry,
    PocketShapeSpec, RadiansPerSecondSq, Rail, RollingResistanceModel, SlidingFrictionModel,
    SpinDecayModel, TableSpec, Velocity2, CENTER_SPOT, TYPICAL_BALL_RADIUS,
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
    );
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
    );

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

fn shared_three_ball_contact_fixture() -> Vec<OnTableBallState> {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let shared_contact_y = -3.0_f64.sqrt() * radius;

    vec![
        on_table(BallState::on_table(
            inches2(20.0, 20.0 + shared_contact_y - 7.5),
            Velocity2::new("0", "10"),
            AngularVelocity3::new(-10.0 / radius, 0.0, 0.0),
        )),
        on_table(BallState::resting_at(inches2(20.0 - radius, 20.0))),
        on_table(BallState::resting_at(inches2(20.0 + radius, 20.0))),
    ]
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
fn a_ball_touching_a_side_pocket_jaw_and_moving_inward_predicts_immediate_impact() {
    let table = TableSpec::default();
    let pocket = Pocket::CenterRight;
    let pocket_center = pocket.aiming_center();
    let jaw_x = table.diamond_to_inches(pocket_center.x).as_f64();
    let jaw_y = table.diamond_to_inches(pocket_center.y).as_f64()
        + 0.5
            * table
                .diamond_to_inches(table.pocket_spec(pocket).width.clone())
                .as_f64();
    let nose_radius = match &table.pocket_spec(pocket).shape.jaw_geometry {
        PocketJawGeometry::PointNoses => 0.0,
        PocketJawGeometry::RoundedNoses { nose_radius } => nose_radius.as_f64(),
    };
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
    );

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
    );

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
    );

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
        inches2(40.5, 56.0),
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
    );

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
    );

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
    );

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
    );
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
    );

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
    );

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
    );

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
    );
    let pocket_aware = simulate_n_balls_with_rails_and_pockets_on_table_until_rest(
        &states,
        &ball_set,
        &table,
        &motion,
        CollisionModel::Ideal,
        billiards::RailModel::Mirror,
    );

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
        );
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
    );

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
        );
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
    );

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
