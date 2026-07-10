use billiards::{
    advance_to_next_n_ball_event_on_table, compute_next_n_ball_event_on_table,
    compute_next_n_ball_system_event_with_rails_and_pockets_on_table,
    simulate_n_ball_system_with_rails_and_pockets_on_table_until_rest, AngularVelocity3,
    BallSetPhysicsSpec, BallState, CollisionModel, Inches, Inches2, InchesPerSecondSq,
    MotionPhaseConfig, MotionTransitionConfig, NBallGeometryError, NBallSystemState,
    OnTableBallState, OnTableMotionConfig, RadiansPerSecondSq, RailModel, RollingResistanceModel,
    SlidingFrictionModel, SpinDecayModel, TableSpec, Velocity2, TYPICAL_BALL_RADIUS,
};

const RECOVERY_TOLERANCE_INCHES: f64 = 1e-6;

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
    OnTableBallState::try_from(state).expect("fixture should be on the table")
}

fn center_distance(a: &OnTableBallState, b: &OnTableBallState) -> f64 {
    let a = a.as_ball_state();
    let b = b.as_ball_state();
    (b.position.x().as_f64() - a.position.x().as_f64())
        .hypot(b.position.y().as_f64() - a.position.y().as_f64())
}

fn assert_gross_overlap(error: NBallGeometryError, first: usize, second: usize) {
    match error {
        NBallGeometryError::OverlappingOnTableBalls {
            first_ball_index,
            second_ball_index,
            center_distance,
            required_center_distance,
            penetration,
            recovery_tolerance,
        } => {
            assert_eq!((first_ball_index, second_ball_index), (first, second));
            assert!((center_distance.as_f64() - 2.0).abs() < 1e-12);
            assert!((required_center_distance.as_f64() - 2.25).abs() < 1e-12);
            assert!((penetration.as_f64() - 0.25).abs() < 1e-12);
            assert!((recovery_tolerance.as_f64() - RECOVERY_TOLERANCE_INCHES).abs() < 1e-18);
        }
    }
}

#[test]
fn stationary_and_closing_gross_overlaps_are_rejected_before_events_or_impulses() {
    let ball = BallSetPhysicsSpec::default();
    let motion = motion_config();
    let stationary = [
        on_table(BallState::resting_at(inches2(0.0, 0.0))),
        on_table(BallState::resting_at(inches2(2.0, 0.0))),
    ];
    let query =
        compute_next_n_ball_event_on_table(&[&stationary[0], &stationary[1]], &ball, &motion)
            .expect_err("a stationary material overlap must not be a terminal no-event state");
    assert_gross_overlap(query, 0, 1);

    let closing = [
        on_table(BallState::on_table(
            inches2(0.0, 0.0),
            Velocity2::new("10", "0"),
            AngularVelocity3::zero(),
        )),
        stationary[1].clone(),
    ];
    let advance =
        advance_to_next_n_ball_event_on_table(&closing, &ball, &motion, CollisionModel::Ideal)
            .expect_err("an impulse must not be applied at a penetrated rigid geometry");
    assert_gross_overlap(advance, 0, 1);
}

#[test]
fn roundoff_scale_overlap_is_projected_without_changing_kinematics_or_midpoint() {
    let ball = BallSetPhysicsSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let required = 2.0 * radius;
    let a = on_table(BallState::resting_at(inches2(0.0, 0.0)));
    let b = on_table(BallState::resting_at(inches2(
        required - 0.5 * RECOVERY_TOLERANCE_INCHES,
        0.0,
    )));
    let advanced = advance_to_next_n_ball_event_on_table(
        &[a.clone(), b.clone()],
        &ball,
        &motion_config(),
        CollisionModel::Ideal,
    )
    .expect("sub-microinch construction residue should be recovered");

    assert!(advanced.event.is_none());
    assert!(center_distance(&advanced.states[0], &advanced.states[1]) >= required);
    let midpoint = 0.5
        * (advanced.states[0].as_ball_state().position.x().as_f64()
            + advanced.states[1].as_ball_state().position.x().as_f64());
    assert!((midpoint - 0.5 * (required - 0.5 * RECOVERY_TOLERANCE_INCHES)).abs() < 1e-12);
    assert_eq!(
        advanced.states[0].as_ball_state().velocity,
        a.as_ball_state().velocity
    );
    assert_eq!(
        advanced.states[1].as_ball_state().velocity,
        b.as_ball_state().velocity
    );
    assert_eq!(
        advanced.states[0].as_ball_state().angular_velocity,
        a.as_ball_state().angular_velocity
    );
    assert_eq!(
        advanced.states[1].as_ball_state().angular_velocity,
        b.as_ball_state().angular_velocity
    );
}

#[test]
fn tolerance_boundary_is_inclusive_and_exact_frozen_contact_is_unchanged() {
    let ball = BallSetPhysicsSpec::default();
    let required = 2.0 * TYPICAL_BALL_RADIUS.as_f64();
    let at_tolerance = [
        on_table(BallState::resting_at(inches2(0.0, 0.0))),
        on_table(BallState::resting_at(inches2(
            required - RECOVERY_TOLERANCE_INCHES,
            0.0,
        ))),
    ];
    let recovered = advance_to_next_n_ball_event_on_table(
        &at_tolerance,
        &ball,
        &motion_config(),
        CollisionModel::Ideal,
    )
    .expect("the inclusive recovery boundary should normalize");
    assert!(center_distance(&recovered.states[0], &recovered.states[1]) >= required);

    let frozen = [
        on_table(BallState::resting_at(inches2(0.0, 0.0))),
        on_table(BallState::resting_at(inches2(required, 0.0))),
    ];
    let exact = advance_to_next_n_ball_event_on_table(
        &frozen,
        &ball,
        &motion_config(),
        CollisionModel::Ideal,
    )
    .expect("exact frozen contact is valid rigid geometry");
    assert!(exact.event.is_none());
    assert_eq!(exact.states, frozen);
}
#[test]
fn richer_system_boundaries_keep_original_indices_and_reject_material_overlap() {
    let ball = BallSetPhysicsSpec::default();
    let table = TableSpec::default();
    let motion = motion_config();
    let states = [
        NBallSystemState::OnTable(on_table(BallState::resting_at(inches2(-20.0, 0.0)))),
        NBallSystemState::OnTable(on_table(BallState::resting_at(inches2(0.0, 0.0)))),
        NBallSystemState::OnTable(on_table(BallState::resting_at(inches2(2.0, 0.0)))),
    ];
    let query = compute_next_n_ball_system_event_with_rails_and_pockets_on_table(
        &states, &ball, &table, &motion,
    )
    .expect_err("the pocket-aware scheduler must reject indexed material overlap");
    assert_gross_overlap(query, 1, 2);

    let simulation = simulate_n_ball_system_with_rails_and_pockets_on_table_until_rest(
        &states,
        &ball,
        &table,
        &motion,
        CollisionModel::Ideal,
        RailModel::Mirror,
    )
    .expect_err("the system simulation must not return an interpenetrating terminal state");
    assert_gross_overlap(simulation, 1, 2);
}
