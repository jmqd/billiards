use billiards::{
    advance_ball_state, advance_motion_on_table, advance_spin_on_table,
    advance_within_phase_on_table, compute_next_transition_on_table,
    estimate_post_contact_cue_ball_curve_on_table, try_advance_angular_velocity_on_table,
    try_advance_ball_state, try_compute_next_transition, AngularVelocity3, BallSetPhysicsSpec,
    BallState, Inches, Inches2, InchesPerSecond, InchesPerSecondSq, MotionPhase, MotionPhaseConfig,
    MotionTransitionConfig, OnTableBallState, OnTableMotionConfig, OnTableStateError,
    RadiansPerSecondSq, RollingResistanceModel, Seconds, SlidingFrictionModel, SpinDecayModel,
    Velocity2, TYPICAL_BALL_RADIUS,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

fn assert_close_with_tolerance(actual: f64, expected: f64, tolerance: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta <= tolerance,
        "expected {expected} +/- {tolerance}, got {actual} (delta {delta})"
    );
}

const STANDARD_GRAVITY_IPS2: f64 = 386.088_582_677_165_35;
const TP_B2_ROLLING_RESISTANCE_COEFFICIENT: f64 = 0.01;
const TP_B2_SPIN_DECELERATION_RADPS2: f64 = 10.0;

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

fn tp_b2_motion_config() -> OnTableMotionConfig {
    MotionTransitionConfig {
        phase: MotionPhaseConfig::default(),
        sliding_friction: SlidingFrictionModel::ConstantAcceleration {
            acceleration_magnitude: InchesPerSecondSq::new("5"),
        },
        spin_decay: SpinDecayModel::ConstantAngularDeceleration {
            angular_deceleration: RadiansPerSecondSq::new(TP_B2_SPIN_DECELERATION_RADPS2),
        },
        rolling_resistance: RollingResistanceModel::ConstantDeceleration {
            linear_deceleration: InchesPerSecondSq::new(Inches::from_f64(
                TP_B2_ROLLING_RESISTANCE_COEFFICIENT * STANDARD_GRAVITY_IPS2,
            )),
        },
    }
}

fn on_table(state: BallState) -> OnTableBallState {
    OnTableBallState::try_from(state).expect("test states should validate as on-table")
}

fn rolling_state() -> BallState {
    let radius = TYPICAL_BALL_RADIUS.clone();
    BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius.as_f64(), 0.0, 0.0),
    )
}

fn rolling_with_vertical_spin_state() -> BallState {
    let radius = TYPICAL_BALL_RADIUS.clone();
    BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius.as_f64(), 0.0, 6.0),
    )
}

fn rolling_with_small_vertical_spin_state() -> BallState {
    let radius = TYPICAL_BALL_RADIUS.clone();
    BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / radius.as_f64(), 0.0, 2.0),
    )
}

fn tp_b2_travel_time_seconds(initial_speed_ips: f64, distance_inches: f64) -> f64 {
    let decel = TP_B2_ROLLING_RESISTANCE_COEFFICIENT * STANDARD_GRAVITY_IPS2;
    (initial_speed_ips
        - (initial_speed_ips * initial_speed_ips - 2.0 * decel * distance_inches).sqrt())
        / decel
}

fn tp_b2_rolling_side_spin_state(initial_speed_ips: f64, distance_inches: f64) -> OnTableBallState {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let travel_time = tp_b2_travel_time_seconds(initial_speed_ips, distance_inches);
    on_table(BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new(Inches::zero(), Inches::from_f64(initial_speed_ips)),
        AngularVelocity3::new(
            -initial_speed_ips / radius,
            0.0,
            TP_B2_SPIN_DECELERATION_RADPS2 * travel_time,
        ),
    ))
}

fn tp_b2_estimated_turn_degrees_at_distance(initial_speed_ips: f64, distance_inches: f64) -> f64 {
    if distance_inches == 0.0 {
        return 0.0;
    }

    estimate_post_contact_cue_ball_curve_on_table(
        &tp_b2_rolling_side_spin_state(initial_speed_ips, distance_inches),
        &BallSetPhysicsSpec::default(),
        &tp_b2_motion_config(),
    )
    .expect("TP B.2 predicts side-spin turn through this distance")
    .curve_angle_degrees
}

fn tp_b2_lateral_error_inches(initial_speed_ips: f64, distance_inches: f64) -> f64 {
    let steps = 1_000usize;
    let step = distance_inches / steps as f64;

    (0..steps)
        .map(|i| {
            let x0 = i as f64 * step;
            let x1 = (i + 1) as f64 * step;
            let y0 = tp_b2_estimated_turn_degrees_at_distance(initial_speed_ips, x0).to_radians();
            let y1 = tp_b2_estimated_turn_degrees_at_distance(initial_speed_ips, x1).to_radians();
            0.5 * (y0 + y1) * step
        })
        .sum()
}

fn sliding_stun_state() -> BallState {
    BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    )
}

fn sliding_with_tip_offset_state(tip_offset_over_radius: f64, speed: f64) -> OnTableBallState {
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    on_table(BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new(Inches::zero(), Inches::from_f64(speed)),
        AngularVelocity3::new(
            -(5.0 / 2.0) * tip_offset_over_radius * speed / radius,
            0.0,
            0.0,
        ),
    ))
}

fn sliding_with_vertical_spin_state() -> BallState {
    BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    )
}

fn spinning_state() -> BallState {
    BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::zero(),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    )
}

fn advance_until_rolling(state: &OnTableBallState) -> OnTableBallState {
    let ball = BallSetPhysicsSpec::default();
    let config = motion_config();
    let transition = compute_next_transition_on_table(state, &ball, &config)
        .expect("sliding state should have a rolling transition");

    assert_eq!(transition.phase_before, MotionPhase::Sliding);
    assert_eq!(transition.phase_after, MotionPhase::Rolling);

    OnTableBallState::try_from(
        advance_motion_on_table(state, transition.time_until_transition, &ball, &config).state,
    )
    .expect("advanced state should remain on-table")
}

fn travel_distance(start: &BallState, end: &BallState) -> f64 {
    let dx = end.position.x().as_f64() - start.position.x().as_f64();
    let dy = end.position.y().as_f64() - start.position.y().as_f64();
    dx.hypot(dy)
}

#[test]
fn advancing_a_resting_ball_leaves_it_unchanged() {
    let state = BallState::resting_at(Inches2::new("12.5", "37.25"));
    let advanced = advance_ball_state(
        &state,
        Seconds::new(3.0),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );

    assert_eq!(advanced, state);
}

#[test]
fn airborne_inputs_are_rejected_by_try_motion_helpers() {
    let state = BallState::airborne(
        Inches2::new("10", "20"),
        "0.5",
        Velocity2::new("0", "10"),
        "0",
        AngularVelocity3::zero(),
    );
    let config = motion_config();

    assert!(matches!(
        try_compute_next_transition(&state, &BallSetPhysicsSpec::default(), &config),
        Err(OnTableStateError::HeightAboveTablePlane { .. })
    ));
    assert!(matches!(
        try_advance_ball_state(
            &state,
            Seconds::new(1.0),
            &BallSetPhysicsSpec::default(),
            &config,
        ),
        Err(OnTableStateError::HeightAboveTablePlane { .. })
    ));
    assert!(matches!(
        try_advance_angular_velocity_on_table(
            &state,
            Seconds::new(1.0),
            &BallSetPhysicsSpec::default(),
            &config,
        ),
        Err(OnTableStateError::HeightAboveTablePlane { .. })
    ));
}

#[test]
fn advance_motion_on_table_reports_the_first_transition_crossed() {
    let state = on_table(sliding_stun_state());

    let advanced = advance_motion_on_table(
        &state,
        Seconds::new(1.0),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );

    let transition = advanced
        .transition
        .expect("the first sliding-to-rolling boundary should be reported");
    assert_eq!(transition.phase_before, MotionPhase::Sliding);
    assert_eq!(transition.phase_after, MotionPhase::Rolling);
    assert_eq!(
        advanced.state.motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Rolling
    );
    assert_close(advanced.elapsed.as_f64(), 1.0);
}

#[test]
fn advance_motion_on_table_reports_an_exact_transition_boundary() {
    let state = on_table(sliding_stun_state());
    let transition =
        compute_next_transition_on_table(&state, &BallSetPhysicsSpec::default(), &motion_config())
            .expect("sliding stun should have a sliding-to-rolling boundary");

    let advanced = advance_motion_on_table(
        &state,
        transition.time_until_transition,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );

    assert_eq!(
        advanced
            .transition
            .expect("exactly reaching the phase boundary should report it"),
        transition
    );
    assert_eq!(
        advanced.state.motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Rolling
    );
    assert_close(
        advanced.elapsed.as_f64(),
        transition.time_until_transition.as_f64(),
    );
}

#[test]
fn advance_within_phase_on_table_clamps_at_the_phase_boundary() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(sliding_stun_state());

    let advanced = advance_within_phase_on_table(
        &state,
        MotionPhase::Sliding,
        Seconds::new(1.0),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );

    assert_close(advanced.as_ball_state().velocity.y().as_f64(), 50.0 / 7.0);
    assert_eq!(
        advanced.as_ball_state().motion_phase(radius),
        MotionPhase::Rolling
    );
}

#[test]
fn advancing_spin_on_table_depends_on_ball_state_and_total_spin() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(0.0, 0.0, 6.0),
    ));

    let angular = advance_spin_on_table(
        &state,
        Seconds::new(2.0 / 7.0),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );
    let base_x = -25.0 / (7.0 * radius.as_f64());

    assert_close(angular.x().as_f64(), base_x);
    assert_close(angular.y().as_f64(), 0.0);
    assert_close(angular.z().as_f64(), 38.0 / 7.0);
}

#[test]
fn advancing_a_sliding_stun_ball_halfway_matches_the_section_7_3_linear_velocity_and_spin_equations(
) {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = sliding_stun_state();
    let dt = Seconds::new(2.0 / 7.0);

    let advanced = advance_ball_state(&state, dt, &BallSetPhysicsSpec::default(), &motion_config());

    assert_close(advanced.position.x().as_f64(), 10.0);
    assert_close(advanced.position.y().as_f64(), 20.0 + 130.0 / 49.0);
    assert_close(advanced.velocity.x().as_f64(), 0.0);
    assert_close(advanced.velocity.y().as_f64(), 60.0 / 7.0);
    assert_close(
        advanced.angular_velocity.x().as_f64(),
        -25.0 / (7.0 * radius.as_f64()),
    );
    assert_close(advanced.angular_velocity.y().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.z().as_f64(), 0.0);
    assert_eq!(advanced.motion_phase(radius), MotionPhase::Sliding);
}

#[test]
fn advancing_a_sliding_ball_with_vertical_spin_no_longer_curves_in_the_horizontal_on_table_model() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = sliding_with_vertical_spin_state();
    let dt = Seconds::new(2.0 / 7.0);

    let advanced = advance_ball_state(&state, dt, &BallSetPhysicsSpec::default(), &motion_config());

    assert_close(advanced.position.x().as_f64(), 10.0);
    assert_close(advanced.velocity.x().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.z().as_f64(), 38.0 / 7.0);
    assert_eq!(advanced.motion_phase(radius), MotionPhase::Sliding);
}

#[test]
fn the_curve_estimate_is_none_for_a_sliding_state_with_only_vertical_spin_in_the_horizontal_model()
{
    let state = on_table(sliding_with_vertical_spin_state());

    assert!(estimate_post_contact_cue_ball_curve_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .is_none());
}

#[test]
fn advancing_a_sliding_stun_ball_to_the_transition_time_reaches_pure_rolling() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(sliding_stun_state());
    let config = motion_config();
    let transition_time =
        compute_next_transition_on_table(&state, &BallSetPhysicsSpec::default(), &config)
            .expect("sliding balls should predict a rolling transition")
            .time_until_transition;

    let advanced = advance_ball_state(
        state.as_ball_state(),
        transition_time,
        &BallSetPhysicsSpec::default(),
        &config,
    );

    assert_close(advanced.position.x().as_f64(), 10.0);
    assert_close(advanced.position.y().as_f64(), 20.0 + 240.0 / 49.0);
    assert_close(advanced.velocity.x().as_f64(), 0.0);
    assert_close(advanced.velocity.y().as_f64(), 50.0 / 7.0);
    assert_close(
        advanced.angular_velocity.x().as_f64(),
        -50.0 / (7.0 * radius.as_f64()),
    );
    assert_close(advanced.angular_velocity.y().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.z().as_f64(), 0.0);
    assert_eq!(advanced.motion_phase(radius.clone()), MotionPhase::Rolling);
    assert_close(advanced.cloth_contact_speed(radius).as_f64(), 0.0);
}

#[test]
fn tp_a18_tip_offset_distances_match_non_overspin_sliding_formula() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let speed = 10.0;
    let sliding_acceleration = 5.0;

    for tip_offset_over_radius in [-0.5_f64, -0.25, 0.0, 0.25] {
        let start = sliding_with_tip_offset_state(tip_offset_over_radius, speed);
        let end = advance_until_rolling(&start);
        let start = start.as_ball_state();
        let end = end.as_ball_state();
        let expected_distance = speed.powi(2) / (98.0 * sliding_acceleration)
            * (24.0 - 50.0 * tip_offset_over_radius - 25.0 * tip_offset_over_radius.powi(2));
        let expected_final_speed = (5.0 / 7.0) * speed * (1.0 + tip_offset_over_radius);

        assert_close(travel_distance(start, end), expected_distance);
        assert_close(end.speed().as_f64(), expected_final_speed);
        assert_close(end.cloth_contact_speed(radius.clone()).as_f64(), 0.0);
        assert_eq!(end.motion_phase(radius.clone()), MotionPhase::Rolling);
    }
}

#[test]
fn tp_a18_overspin_tip_offset_accelerates_until_natural_roll_develops() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let speed = 10.0;
    let sliding_acceleration = 5.0;

    for tip_offset_over_radius in [0.45_f64, 0.5] {
        let start = sliding_with_tip_offset_state(tip_offset_over_radius, speed);
        let end = advance_until_rolling(&start);
        let start = start.as_ball_state();
        let end = end.as_ball_state();
        let signed_slip = 1.0 - 2.5 * tip_offset_over_radius;
        let expected_distance = signed_slip.signum() * speed.powi(2)
            / (98.0 * sliding_acceleration)
            * (24.0 - 50.0 * tip_offset_over_radius - 25.0 * tip_offset_over_radius.powi(2));
        let expected_final_speed = (5.0 / 7.0) * speed * (1.0 + tip_offset_over_radius);

        assert_close(travel_distance(start, end), expected_distance);
        assert!(
            end.speed().as_f64() > start.speed().as_f64(),
            "overspin should accelerate the ball before natural roll develops"
        );
        assert_close(end.speed().as_f64(), expected_final_speed);
        assert_close(end.cloth_contact_speed(radius.clone()).as_f64(), 0.0);
        assert_eq!(end.motion_phase(radius.clone()), MotionPhase::Rolling);
    }
}

#[test]
fn advancing_past_the_sliding_transition_continues_into_rolling_for_the_remaining_time() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(sliding_stun_state());
    let config = motion_config();
    let transition_time =
        compute_next_transition_on_table(&state, &BallSetPhysicsSpec::default(), &config)
            .expect("sliding balls should predict a rolling transition")
            .time_until_transition;

    let advanced = advance_ball_state(
        state.as_ball_state(),
        Seconds::new(transition_time.as_f64() + 1.0),
        &BallSetPhysicsSpec::default(),
        &config,
    );

    assert_close(advanced.position.x().as_f64(), 10.0);
    assert_close(advanced.position.y().as_f64(), 20.0 + 935.0 / 98.0);
    assert_close(advanced.velocity.x().as_f64(), 0.0);
    assert_close(advanced.velocity.y().as_f64(), 15.0 / 7.0);
    assert_close(
        advanced.angular_velocity.x().as_f64(),
        -15.0 / (7.0 * radius.as_f64()),
    );
    assert_close(advanced.angular_velocity.y().as_f64(), 0.0);
    assert_eq!(advanced.motion_phase(radius), MotionPhase::Rolling);
}

#[test]
fn advancing_a_pure_spinning_ball_leaves_position_fixed_and_decays_z_spin_linearly() {
    let state = spinning_state();

    let advanced = advance_ball_state(
        &state,
        Seconds::new(1.0),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );

    assert_close(advanced.position.x().as_f64(), 10.0);
    assert_close(advanced.position.y().as_f64(), 20.0);
    assert_close(advanced.speed().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.x().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.y().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.z().as_f64(), 4.0);
    assert_eq!(
        advanced.motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Spinning
    );
}

#[test]
fn advancing_a_rolling_ball_with_vertical_spin_no_longer_curls_once_it_is_in_pure_rolling_motion() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = rolling_with_vertical_spin_state();
    let advanced = advance_ball_state(
        &state,
        Seconds::new(1.0),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );

    assert_close(advanced.position.x().as_f64(), 10.0);
    assert_close(advanced.position.y().as_f64(), 27.5);
    assert_close(advanced.speed().as_f64(), 5.0);
    assert_close(advanced.velocity.x().as_f64(), 0.0);
    assert_close(advanced.velocity.y().as_f64(), 5.0);
    assert_close(
        advanced.angular_velocity.x().as_f64(),
        -5.0 / radius.as_f64(),
    );
    assert_close(advanced.angular_velocity.y().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.z().as_f64(), 4.0);
    assert_eq!(advanced.motion_phase(radius), MotionPhase::Rolling);
}

#[test]
fn cross_near_vertical_axis_relation_holds_for_rolling_translation_with_residual_z_spin() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(rolling_with_vertical_spin_state());
    let config = motion_config();
    let dt = Seconds::new(1.0);
    let transition =
        compute_next_transition_on_table(&state, &BallSetPhysicsSpec::default(), &config)
            .expect("rolling ball with residual z spin should predict a later transition");

    assert_eq!(
        state.as_ball_state().motion_phase(radius.clone()),
        MotionPhase::Rolling
    );
    assert_close(
        state
            .as_ball_state()
            .cloth_contact_speed(radius.clone())
            .as_f64(),
        0.0,
    );
    assert!(
        dt.as_f64() < transition.time_until_transition.as_f64(),
        "test advances within the rolling phase, before the model's rest/spinning boundary"
    );

    let advanced = advance_ball_state(
        state.as_ball_state(),
        dt,
        &BallSetPhysicsSpec::default(),
        &config,
    );

    // Cross, `whitepapers/rolling_motion_of_a_ball_spinning_about_a_near_vertical_axis.pdf`:
    // corpus lines 42593-42597 define rolling by v = Rw and a resting cloth contact point;
    // lines 42631-42638 and 42776-42778 give cos(theta) = r/R = v/(wR).
    assert_eq!(advanced.motion_phase(radius.clone()), MotionPhase::Rolling);
    assert_close(advanced.cloth_contact_speed(radius.clone()).as_f64(), 0.0);

    let radius = radius.as_f64();
    let speed = advanced.speed().as_f64();
    let wx = advanced.angular_velocity.x().as_f64();
    let wy = advanced.angular_velocity.y().as_f64();
    let wz = advanced.angular_velocity.z().as_f64();
    let horizontal_spin = wx.hypot(wy);
    let total_spin = horizontal_spin.hypot(wz);
    let implied_theta = (wz.abs() / total_spin).asin();

    assert_close_with_tolerance(horizontal_spin, speed / radius, 1e-12);
    assert_close_with_tolerance(implied_theta.cos(), speed / (total_spin * radius), 1e-12);
}

#[test]
fn the_curve_estimate_reports_tp_b2_rolling_side_spin_turn() {
    let state = on_table(rolling_with_small_vertical_spin_state());
    let curve = estimate_post_contact_cue_ball_curve_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .expect("TP B.2 predicts a small rolling ball turn while side spin decays");

    assert_close(curve.time_until_curve_starts.as_f64(), 0.0);
    assert_close(curve.time_until_curve_completes.as_f64(), 1.0);
    assert_close(curve.curve_angle_degrees, 0.09257711527464842);
    assert_close(curve.heading_after_curve.as_degrees(), 0.09257711527464842);
}

#[test]
fn tp_b2_rolling_side_spin_curve_estimate_matches_published_examples() {
    for (mph, distance_inches, expected_time, expected_turn, expected_error) in [
        (2.0, 96.0, 3.339, 0.305, 0.217),
        (5.0, 36.0, 0.413, 0.012, 0.003_811),
    ] {
        let speed = InchesPerSecond::from_mph(mph).as_f64();
        let curve = estimate_post_contact_cue_ball_curve_on_table(
            &tp_b2_rolling_side_spin_state(speed, distance_inches),
            &BallSetPhysicsSpec::default(),
            &tp_b2_motion_config(),
        )
        .expect("TP B.2 predicts side-spin turn through this distance");

        assert_close_with_tolerance(curve.time_until_curve_starts.as_f64(), 0.0, 1e-12);
        assert_close_with_tolerance(
            curve.time_until_curve_completes.as_f64(),
            expected_time,
            0.001,
        );
        assert_close_with_tolerance(
            curve.curve_angle_degrees.abs(),
            expected_turn,
            if mph == 2.0 { 0.005 } else { 0.001 },
        );
        assert_close_with_tolerance(
            tp_b2_lateral_error_inches(speed, distance_inches).abs(),
            expected_error,
            if mph == 2.0 { 0.005 } else { 0.0005 },
        );
    }
}

#[test]
fn opposite_rolling_side_spin_turns_the_other_way() {
    let state = on_table(BallState::on_table(
        Inches2::new("10", "20"),
        Velocity2::new("0", "10"),
        AngularVelocity3::new(-10.0 / TYPICAL_BALL_RADIUS.as_f64(), 0.0, -2.0),
    ));
    let curve = estimate_post_contact_cue_ball_curve_on_table(
        &state,
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    )
    .expect("opposite side spin should still produce a small TP B.2 turn");

    assert_close(curve.curve_angle_degrees, -0.09257711527464842);
    assert_close(
        curve.heading_after_curve.as_degrees(),
        360.0 - 0.09257711527464842,
    );
}

#[test]
fn rolling_side_spin_curve_estimate_is_none_when_translation_stops_before_spin() {
    let state = on_table(rolling_with_vertical_spin_state());

    assert!(
        estimate_post_contact_cue_ball_curve_on_table(
            &state,
            &BallSetPhysicsSpec::default(),
            &motion_config(),
        )
        .is_none(),
        "TP B.2's turn-rate integral is not used once translation stops before side spin decays"
    );
}

#[test]
fn advancing_a_rolling_ball_with_vertical_spin_can_enter_the_spinning_phase() {
    let state = rolling_with_vertical_spin_state();

    let advanced = advance_ball_state(
        &state,
        Seconds::new(2.5),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );

    assert_close(advanced.position.x().as_f64(), 10.0);
    assert_close(advanced.position.y().as_f64(), 30.0);
    assert_close(advanced.speed().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.x().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.y().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.z().as_f64(), 1.0);
    assert_eq!(
        advanced.motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Spinning
    );
}

#[test]
fn advancing_a_rolling_ball_updates_position_speed_and_spin_consistently() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = rolling_state();
    let advanced = advance_ball_state(
        &state,
        Seconds::new(1.0),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );

    assert_close(advanced.position.x().as_f64(), 10.0);
    assert_close(advanced.position.y().as_f64(), 27.5);
    assert_close(advanced.speed().as_f64(), 5.0);
    assert_close(advanced.velocity.x().as_f64(), 0.0);
    assert_close(advanced.velocity.y().as_f64(), 5.0);
    assert_close(
        advanced.angular_velocity.x().as_f64(),
        -5.0 / radius.as_f64(),
    );
    assert_close(advanced.angular_velocity.y().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.z().as_f64(), 0.0);
    assert_eq!(advanced.motion_phase(radius), MotionPhase::Rolling);
}

#[test]
fn advancing_by_the_predicted_stop_time_reaches_rest() {
    let state = on_table(rolling_state());
    let config = motion_config();
    let stop_time =
        compute_next_transition_on_table(&state, &BallSetPhysicsSpec::default(), &config)
            .expect("rolling balls should predict a rest transition")
            .time_until_transition;

    let advanced = advance_ball_state(
        state.as_ball_state(),
        stop_time,
        &BallSetPhysicsSpec::default(),
        &config,
    );

    assert_close(advanced.position.x().as_f64(), 10.0);
    assert_close(advanced.position.y().as_f64(), 30.0);
    assert_close(advanced.speed().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.x().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.y().as_f64(), 0.0);
    assert_close(advanced.angular_velocity.z().as_f64(), 0.0);
    assert_eq!(
        advanced.motion_phase(TYPICAL_BALL_RADIUS.clone()),
        MotionPhase::Rest
    );
}

#[test]
fn advancing_past_the_predicted_stop_time_clamps_at_the_same_rest_state() {
    let state = on_table(rolling_state());
    let config = motion_config();
    let stop_time =
        compute_next_transition_on_table(&state, &BallSetPhysicsSpec::default(), &config)
            .expect("rolling balls should predict a rest transition")
            .time_until_transition;

    let at_stop = advance_ball_state(
        state.as_ball_state(),
        stop_time,
        &BallSetPhysicsSpec::default(),
        &config,
    );
    let after_stop = advance_ball_state(
        state.as_ball_state(),
        Seconds::new(stop_time.as_f64() + 1.0),
        &BallSetPhysicsSpec::default(),
        &config,
    );

    assert_eq!(after_stop, at_stop);
}

#[test]
fn advancing_by_zero_seconds_is_an_identity_operation() {
    let state = rolling_state();
    let advanced = advance_ball_state(
        &state,
        Seconds::zero(),
        &BallSetPhysicsSpec::default(),
        &motion_config(),
    );

    assert_eq!(advanced, state);
}
