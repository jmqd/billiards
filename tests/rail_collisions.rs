use billiards::{
    cloth_contact_velocity_on_table, collide_ball_rail_on_table,
    collide_ball_rail_on_table_with_radius_and_config,
    collide_ball_rail_on_table_with_radius_and_profile,
    mathavan_rigid_cushion_contains_normal_speed, tp73_rail_vertical_spin_prediction,
    AngularVelocity3, BallState, Inches, Inches2, InchesPerSecond, MotionPhase, OnTableBallState,
    RadiansPerSecond, Rail, RailCollisionConfig, RailCollisionProfile, RailModel, Scale, Velocity2,
    MATHAVAN_RIGID_CUSHION_MAX_NORMAL_SPEED_INCHES_PER_SECOND, TYPICAL_BALL_RADIUS,
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
        "expected {expected}, got {actual} (delta {delta}, tolerance {tolerance})"
    );
}

fn on_table_state_delta(a: &OnTableBallState, b: &OnTableBallState) -> f64 {
    let a = a.as_ball_state();
    let b = b.as_ball_state();
    [
        (a.velocity.x().as_f64() - b.velocity.x().as_f64()).abs(),
        (a.velocity.y().as_f64() - b.velocity.y().as_f64()).abs(),
        (a.angular_velocity.x().as_f64() - b.angular_velocity.x().as_f64()).abs(),
        (a.angular_velocity.y().as_f64() - b.angular_velocity.y().as_f64()).abs(),
        (a.angular_velocity.z().as_f64() - b.angular_velocity.z().as_f64()).abs(),
    ]
    .into_iter()
    .fold(0.0, f64::max)
}

fn on_table(state: BallState) -> OnTableBallState {
    OnTableBallState::try_from(state).expect("test states should validate as on-table")
}

fn inches2(x: f64, y: f64) -> Inches2 {
    Inches2::new(Inches::from_f64(x), Inches::from_f64(y))
}

fn right_handed_rail_collision_basis_for_test(rail: Rail) -> (f64, f64, f64, f64) {
    match rail {
        Rail::Top => (0.0, -1.0, 1.0, 0.0),
        Rail::Bottom => (0.0, 1.0, -1.0, 0.0),
        Rail::Left => (1.0, 0.0, 0.0, 1.0),
        Rail::Right => (-1.0, 0.0, 0.0, -1.0),
    }
}

fn rail_state_from_local_frame(
    rail: Rail,
    tangent_speed: f64,
    normal_speed_toward_cushion: f64,
    angular_tangent: f64,
    angular_normal_toward_cushion: f64,
    angular_vertical: f64,
) -> OnTableBallState {
    let (normal_x, normal_y, tangent_x, tangent_y) =
        right_handed_rail_collision_basis_for_test(rail);
    let inward_normal_x = -normal_x;
    let inward_normal_y = -normal_y;
    on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new(
            Inches::from_f64(
                tangent_speed * tangent_x + normal_speed_toward_cushion * inward_normal_x,
            ),
            Inches::from_f64(
                tangent_speed * tangent_y + normal_speed_toward_cushion * inward_normal_y,
            ),
        ),
        AngularVelocity3::new(
            angular_tangent * tangent_x + angular_normal_toward_cushion * inward_normal_x,
            angular_tangent * tangent_y + angular_normal_toward_cushion * inward_normal_y,
            angular_vertical,
        ),
    ))
}

fn rail_local_frame_components(rail: Rail, state: &OnTableBallState) -> [f64; 5] {
    let state = state.as_ball_state();
    let (normal_x, normal_y, tangent_x, tangent_y) =
        right_handed_rail_collision_basis_for_test(rail);
    let inward_normal_x = -normal_x;
    let inward_normal_y = -normal_y;

    [
        state.velocity.x().as_f64() * tangent_x + state.velocity.y().as_f64() * tangent_y,
        state.velocity.x().as_f64() * inward_normal_x
            + state.velocity.y().as_f64() * inward_normal_y,
        state.angular_velocity.x().as_f64() * tangent_x
            + state.angular_velocity.y().as_f64() * tangent_y,
        state.angular_velocity.x().as_f64() * inward_normal_x
            + state.angular_velocity.y().as_f64() * inward_normal_y,
        state.angular_velocity.z().as_f64(),
    ]
}

#[test]
fn a_square_hit_on_a_horizontal_rail_reflects_straight_back() {
    let state = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("0", "10"),
        AngularVelocity3::zero(),
    ));

    let reflected = collide_ball_rail_on_table(&state, Rail::Top, RailModel::Mirror);

    assert_close(reflected.as_ball_state().velocity.x().as_f64(), 0.0);
    assert_close(reflected.as_ball_state().velocity.y().as_f64(), -10.0);
    assert_eq!(
        reflected.as_ball_state().position,
        state.as_ball_state().position
    );
}

#[test]
fn a_forty_five_degree_bank_reflects_symmetrically_in_the_ideal_model() {
    let state = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("5", "5"),
        AngularVelocity3::zero(),
    ));

    let reflected = collide_ball_rail_on_table(&state, Rail::Top, RailModel::Mirror);

    assert_close(reflected.as_ball_state().velocity.x().as_f64(), 5.0);
    assert_close(reflected.as_ball_state().velocity.y().as_f64(), -5.0);
}

#[test]
fn an_ideal_rail_collision_leaves_spin_unchanged() {
    let state = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("-3", "7"),
        AngularVelocity3::new(1.0, 2.0, 3.0),
    ));

    let reflected = collide_ball_rail_on_table(&state, Rail::Right, RailModel::Mirror);

    assert_close(reflected.as_ball_state().velocity.x().as_f64(), 3.0);
    assert_close(reflected.as_ball_state().velocity.y().as_f64(), 7.0);
    assert_eq!(
        reflected.as_ball_state().angular_velocity,
        state.as_ball_state().angular_velocity
    );
}

#[test]
fn a_restitution_only_rail_collision_reduces_the_outgoing_normal_speed() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("5", "5"),
        AngularVelocity3::new(1.0, 2.0, 3.0),
    ));
    let config = RailCollisionConfig::new(Scale::from_f64(0.8), Scale::from_f64(1.0));

    let reflected = collide_ball_rail_on_table_with_radius_and_config(
        &state,
        Rail::Top,
        radius,
        RailModel::RestitutionOnly,
        &config,
    );

    assert_close(reflected.as_ball_state().velocity.x().as_f64(), 5.0);
    assert_close(reflected.as_ball_state().velocity.y().as_f64(), -4.0);
    assert_eq!(
        reflected.as_ball_state().angular_velocity,
        state.as_ball_state().angular_velocity
    );
}

#[test]
fn tp_6_3_restitution_only_increases_the_bank_rebound_angle() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let restitution = 0.8_f64;
    let state = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("5", "5"),
        AngularVelocity3::zero(),
    ));
    let config = RailCollisionConfig::new(Scale::from_f64(restitution), Scale::from_f64(1.0));

    let reflected = collide_ball_rail_on_table_with_radius_and_config(
        &state,
        Rail::Top,
        radius,
        RailModel::RestitutionOnly,
        &config,
    );
    let approach_angle = (5.0_f64 / 5.0).atan().to_degrees();
    let expected_rebound_angle = (approach_angle.to_radians().tan() / restitution)
        .atan()
        .to_degrees();
    let actual_rebound_angle = (reflected.as_ball_state().velocity.x().as_f64().abs()
        / reflected.as_ball_state().velocity.y().as_f64().abs())
    .atan()
    .to_degrees();

    assert_close(approach_angle, 45.0);
    assert_close(expected_rebound_angle, 51.340_191_745_909_905);
    assert_close(
        expected_rebound_angle - approach_angle,
        6.340_191_745_909_905,
    );
    assert_close(actual_rebound_angle, expected_rebound_angle);
}

#[test]
fn a_spin_aware_rail_collision_with_high_cushion_friction_trades_more_tangential_speed_for_running_english(
) {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("5", "5"),
        AngularVelocity3::zero(),
    ));
    let high_friction = RailCollisionConfig::new(Scale::from_f64(0.8), Scale::from_f64(1.0));
    let low_friction = RailCollisionConfig::new(Scale::from_f64(0.8), Scale::from_f64(0.1));

    let high = collide_ball_rail_on_table_with_radius_and_config(
        &state,
        Rail::Top,
        radius.clone(),
        RailModel::SpinAware,
        &high_friction,
    );
    let low = collide_ball_rail_on_table_with_radius_and_config(
        &state,
        Rail::Top,
        radius,
        RailModel::SpinAware,
        &low_friction,
    );

    assert!(high.as_ball_state().velocity.y().as_f64() < 0.0);
    assert!(
        high.as_ball_state().velocity.x().as_f64() < low.as_ball_state().velocity.x().as_f64(),
        "stronger cushion friction should bleed more along-rail speed during impact"
    );
    assert!(
        high.as_ball_state().angular_velocity.z().as_f64().abs()
            > low.as_ball_state().angular_velocity.z().as_f64().abs(),
        "stronger cushion friction should generate more running english"
    );
}

#[test]
fn a_spin_aware_rail_collision_exhibits_partial_slip_when_friction_is_low() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("5", "5"),
        AngularVelocity3::zero(),
    ));
    let config = RailCollisionConfig::new(Scale::from_f64(0.8), Scale::from_f64(0.1));

    let reflected = collide_ball_rail_on_table_with_radius_and_config(
        &state,
        Rail::Top,
        radius,
        RailModel::SpinAware,
        &config,
    );

    assert!(reflected.as_ball_state().velocity.x().as_f64() > 3.0);
    assert!(reflected.as_ball_state().velocity.x().as_f64() < 5.0);
    assert!(reflected.as_ball_state().velocity.y().as_f64() < 0.0);
    assert!(
        reflected
            .as_ball_state()
            .angular_velocity
            .z()
            .as_f64()
            .abs()
            > 1.0,
        "low but nonzero cushion friction should still generate some running english"
    );
}

#[test]
fn zero_cushion_contact_slip_does_not_choose_an_arbitrary_cushion_friction_direction() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let radius_value = radius.as_f64();
    let sin_theta = 2.0 / 5.0;
    let cos_theta = (1.0_f64 - sin_theta * sin_theta).sqrt();
    let tangent_speed = 4.0;
    let normal_speed_toward_top = 5.0;
    let exact_wz = tangent_speed / (radius_value * cos_theta);
    let state_with_wz = |wz_delta: f64| {
        on_table(BallState::on_table(
            inches2(10.0, 20.0),
            Velocity2::new(
                Inches::from_f64(tangent_speed),
                Inches::from_f64(normal_speed_toward_top),
            ),
            AngularVelocity3::new(
                normal_speed_toward_top * sin_theta / radius_value,
                0.0,
                exact_wz + wz_delta,
            ),
        ))
    };
    let grabby_cushion = RailCollisionConfig::new(Scale::from_f64(0.8), Scale::from_f64(1.0))
        .with_impact_cloth_friction_coefficient(Scale::from_f64(0.0))
        .with_effective_contact_height_ratio(Scale::from_f64(0.0));

    let exact_rebound = collide_ball_rail_on_table_with_radius_and_config(
        &state_with_wz(0.0),
        Rail::Top,
        radius.clone(),
        RailModel::SpinAware,
        &grabby_cushion,
    );
    let positive_slip_rebound = collide_ball_rail_on_table_with_radius_and_config(
        &state_with_wz(1e-10),
        Rail::Top,
        radius.clone(),
        RailModel::SpinAware,
        &grabby_cushion,
    );
    let negative_slip_rebound = collide_ball_rail_on_table_with_radius_and_config(
        &state_with_wz(-1e-10),
        Rail::Top,
        radius,
        RailModel::SpinAware,
        &grabby_cushion,
    );

    let positive_delta = on_table_state_delta(&exact_rebound, &positive_slip_rebound);
    let negative_delta = on_table_state_delta(&exact_rebound, &negative_slip_rebound);
    assert!(
        positive_delta < 1e-2,
        "exact cushion no-slip should be continuous from positive slip; delta={positive_delta}"
    );
    assert!(
        negative_delta < 1e-2,
        "exact cushion no-slip should be continuous from negative slip; delta={negative_delta}"
    );
}

#[test]
fn zero_cloth_contact_slip_does_not_choose_an_arbitrary_table_friction_direction() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let radius_value = radius.as_f64();
    let tangent_speed = 4.0;
    let normal_speed_toward_top = 5.0;
    let state_with_wy = |wy_delta: f64| {
        on_table(BallState::on_table(
            inches2(10.0, 20.0),
            Velocity2::new(
                Inches::from_f64(tangent_speed),
                Inches::from_f64(normal_speed_toward_top),
            ),
            AngularVelocity3::new(
                -normal_speed_toward_top / radius_value,
                tangent_speed / radius_value + wy_delta,
                0.0,
            ),
        ))
    };
    let grabby_table = RailCollisionConfig::new(Scale::from_f64(0.8), Scale::from_f64(0.0))
        .with_impact_cloth_friction_coefficient(Scale::from_f64(1.0))
        .with_effective_contact_height_ratio(Scale::from_f64(0.0));

    let exact_rebound = collide_ball_rail_on_table_with_radius_and_config(
        &state_with_wy(0.0),
        Rail::Top,
        radius.clone(),
        RailModel::SpinAware,
        &grabby_table,
    );
    let positive_slip_rebound = collide_ball_rail_on_table_with_radius_and_config(
        &state_with_wy(1e-10),
        Rail::Top,
        radius.clone(),
        RailModel::SpinAware,
        &grabby_table,
    );
    let negative_slip_rebound = collide_ball_rail_on_table_with_radius_and_config(
        &state_with_wy(-1e-10),
        Rail::Top,
        radius,
        RailModel::SpinAware,
        &grabby_table,
    );

    let positive_delta = on_table_state_delta(&exact_rebound, &positive_slip_rebound);
    let negative_delta = on_table_state_delta(&exact_rebound, &negative_slip_rebound);
    assert!(
        positive_delta < 1e-2,
        "exact cloth no-slip should be continuous from positive slip; delta={positive_delta}"
    );
    assert!(
        negative_delta < 1e-2,
        "exact cloth no-slip should be continuous from negative slip; delta={negative_delta}"
    );
}

#[test]
fn near_zero_cushion_contact_slip_changes_the_rail_response_continuously() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let radius_value = radius.as_f64();
    let sin_theta = 2.0 / 5.0;
    let cos_theta = (1.0_f64 - sin_theta * sin_theta).sqrt();
    let tangent_speed = 4.0;
    let normal_speed_toward_top = 5.0;
    let exact_wz = tangent_speed / (radius_value * cos_theta);
    let exact = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new(
            Inches::from_f64(tangent_speed),
            Inches::from_f64(normal_speed_toward_top),
        ),
        AngularVelocity3::new(
            normal_speed_toward_top * sin_theta / radius_value,
            0.0,
            exact_wz,
        ),
    ));
    let almost_exact = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new(
            Inches::from_f64(tangent_speed),
            Inches::from_f64(normal_speed_toward_top),
        ),
        AngularVelocity3::new(
            normal_speed_toward_top * sin_theta / radius_value,
            0.0,
            exact_wz + 1e-10,
        ),
    ));
    let grabby_cushion = RailCollisionConfig::new(Scale::from_f64(0.8), Scale::from_f64(1.0))
        .with_impact_cloth_friction_coefficient(Scale::from_f64(0.0))
        .with_effective_contact_height_ratio(Scale::from_f64(0.0));

    let exact_rebound = collide_ball_rail_on_table_with_radius_and_config(
        &exact,
        Rail::Top,
        radius.clone(),
        RailModel::SpinAware,
        &grabby_cushion,
    );
    let almost_exact_rebound = collide_ball_rail_on_table_with_radius_and_config(
        &almost_exact,
        Rail::Top,
        radius,
        RailModel::SpinAware,
        &grabby_cushion,
    );

    let rebound_delta = on_table_state_delta(&almost_exact_rebound, &exact_rebound);
    assert!(
        rebound_delta < 1e-3,
        "an infinitesimal geared-slip perturbation should not cause a finite kinetic-friction jump; delta={rebound_delta}"
    );
}

#[test]
fn a_spin_aware_rail_collision_with_topspin_reduces_vertical_plane_spin() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("0", "5"),
        AngularVelocity3::new(4.0, 0.0, 0.0),
    ));
    let config = RailCollisionConfig::new(Scale::from_f64(0.8), Scale::from_f64(1.0));

    let reflected = collide_ball_rail_on_table_with_radius_and_config(
        &state,
        Rail::Top,
        radius,
        RailModel::SpinAware,
        &config,
    );

    assert!(reflected.as_ball_state().velocity.x().as_f64().abs() < 0.01);
    assert!(reflected.as_ball_state().velocity.y().as_f64() < -3.0);
    assert!(
        reflected
            .as_ball_state()
            .angular_velocity
            .y()
            .as_f64()
            .abs()
            < 0.01
    );
    assert!(
        reflected
            .as_ball_state()
            .angular_velocity
            .z()
            .as_f64()
            .abs()
            < 0.01
    );
    assert!(
        reflected
            .as_ball_state()
            .angular_velocity
            .x()
            .as_f64()
            .abs()
            < state.as_ball_state().angular_velocity.x().as_f64().abs(),
        "the stronger impact solve should reduce the carried vertical-plane spin magnitude"
    );
}

#[test]
fn an_ordinary_no_english_rolling_entry_seeds_less_running_english_than_a_sliding_entry() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let radius_value = radius.as_f64();
    let config = RailCollisionConfig::new(Scale::from_f64(0.8), Scale::from_f64(1.0));
    let sliding = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("5", "5"),
        AngularVelocity3::zero(),
    ));
    let rolling = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("5", "5"),
        AngularVelocity3::new(-5.0 / radius_value, 5.0 / radius_value, 0.0),
    ));

    let sliding_reflected = collide_ball_rail_on_table_with_radius_and_config(
        &sliding,
        Rail::Top,
        radius.clone(),
        RailModel::SpinAware,
        &config,
    );
    let rolling_reflected = collide_ball_rail_on_table_with_radius_and_config(
        &rolling,
        Rail::Top,
        radius,
        RailModel::SpinAware,
        &config,
    );

    assert!(
        rolling_reflected.as_ball_state().angular_velocity.z().as_f64().abs()
            < sliding_reflected.as_ball_state().angular_velocity.z().as_f64().abs(),
        "ordinary rolling entries without explicit side spin should seed less fresh running english than fully sliding entries"
    );
}

#[test]
fn a_rolling_entry_with_carried_side_spin_scrubs_some_of_that_spin_at_the_rail() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let radius_value = radius.as_f64();
    let state = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("5", "5"),
        AngularVelocity3::new(-5.0 / radius_value, 5.0 / radius_value, -8.0),
    ));
    let config = RailCollisionConfig::new(Scale::from_f64(0.8), Scale::from_f64(1.0));

    let reflected = collide_ball_rail_on_table_with_radius_and_config(
        &state,
        Rail::Top,
        radius,
        RailModel::SpinAware,
        &config,
    );

    assert!(
        reflected.as_ball_state().angular_velocity.z().as_f64().abs()
            < state.as_ball_state().angular_velocity.z().as_f64().abs(),
        "a rolling rail entry with carried side spin should leave the rail with less side spin than it brought in"
    );
}

#[test]
fn mathavan_high_left_sidespin_near_normal_can_rebound_same_side_and_faster_than_incident() {
    let radius = Inches::from_f64(26.25 / 25.4);
    let radius_value = radius.as_f64();
    let incident_speed = 39.370_078_740_157_48; // 1 m/s.
    let alpha = 88.0_f64.to_radians();
    let tangent_speed = incident_speed * alpha.cos();
    let normal_speed = incident_speed * alpha.sin();
    let side_spin_scale = -5.0;
    let state = rail_state_from_local_frame(
        Rail::Top,
        tangent_speed,
        normal_speed,
        -normal_speed / radius_value,
        tangent_speed / radius_value,
        side_spin_scale * incident_speed / radius_value,
    );
    let mathavan_config = RailCollisionConfig::new(Scale::from_f64(0.98), Scale::from_f64(0.14))
        .with_impact_cloth_friction_coefficient(Scale::from_f64(0.212))
        .with_effective_contact_height_ratio(Scale::from_f64(0.0));

    // Mathavan/Jackson/Parkin Fig. 10 uses omega_0S = k V0/R, V0 = 1 m/s, and a
    // rolling ball. They report that high left spin near 90 degrees can rebound
    // faster than the incident speed and back toward the same side.
    let reflected = collide_ball_rail_on_table_with_radius_and_config(
        &state,
        Rail::Top,
        radius,
        RailModel::SpinAware,
        &mathavan_config,
    );
    let actual = rail_local_frame_components(Rail::Top, &reflected);
    let rebound_speed = actual[0].hypot(actual[1]);

    assert!(
        actual[0] * tangent_speed < 0.0,
        "high left spin near normal incidence should rebound toward the same side; incoming tangent={tangent_speed}, outgoing tangent={}",
        actual[0]
    );
    assert!(
        rebound_speed > incident_speed,
        "high side spin can transfer rotational energy into rebound speed; incident={incident_speed}, rebound={rebound_speed}"
    );
}

#[test]
fn mathavan_rigid_cushion_calibration_range_flags_high_normal_speeds() {
    let one_meter_per_second = InchesPerSecond::new(Inches::from_f64(39.370_078_740_157_48));
    let three_meters_per_second =
        InchesPerSecond::new(Inches::from_f64(3.0 * 39.370_078_740_157_48));

    assert_close_with_tolerance(
        MATHAVAN_RIGID_CUSHION_MAX_NORMAL_SPEED_INCHES_PER_SECOND,
        98.425_196_850_393_69,
        1e-12,
    );
    assert!(mathavan_rigid_cushion_contains_normal_speed(
        &one_meter_per_second
    ));
    assert!(!mathavan_rigid_cushion_contains_normal_speed(
        &three_meters_per_second
    ));
}

#[test]
fn a_rolling_low_english_entry_leaves_the_rail_with_bounded_horizontal_cloth_slip() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let radius_value = radius.as_f64();
    let rolling = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("5", "5"),
        AngularVelocity3::new(-5.0 / radius_value, 5.0 / radius_value, 0.0),
    ));
    let reflected = collide_ball_rail_on_table(&rolling, Rail::Top, RailModel::SpinAware);
    let slip = cloth_contact_velocity_on_table(reflected.as_ball_state(), radius);
    let slip_ratio =
        slip.x().as_f64().hypot(slip.y().as_f64()) / reflected.as_ball_state().speed().as_f64();

    assert!(
        slip_ratio <= 0.8 + 1e-9,
        "rolling-style low-english rail entries should not leave the reduced on-table model with excessive post-rail cloth slip"
    );
}

#[test]
fn a_spin_aware_rolling_entry_exits_much_closer_to_tp73_near_stun_than_a_mirror_rebound() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let radius_value = radius.as_f64();
    let rolling = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("0", "5"),
        AngularVelocity3::new(-5.0 / radius_value, 0.0, 0.0),
    ));

    let mirror = collide_ball_rail_on_table(&rolling, Rail::Top, RailModel::Mirror);
    let spin_aware = collide_ball_rail_on_table(&rolling, Rail::Top, RailModel::SpinAware);
    let outgoing_rolling_wx = -spin_aware.as_ball_state().velocity.y().as_f64() / radius_value;

    assert!(
        spin_aware.as_ball_state().angular_velocity.x().as_f64().abs()
            < mirror.as_ball_state().angular_velocity.x().as_f64().abs() * 0.5,
        "TP 7.3 rolling rail entries should rebound much closer to stun than the mirror-limit draw-like carryover"
    );
    assert!(
        spin_aware.as_ball_state().angular_velocity.x().as_f64() > 0.0,
        "the rebound should still carry a small amount of forward vertical-plane roll in the new travel direction"
    );
    assert!(
        spin_aware.as_ball_state().angular_velocity.x().as_f64() < outgoing_rolling_wx,
        "the rebound should remain clearly below immediate pure rolling after impact"
    );
}

#[test]
fn a_spin_aware_stun_entry_picks_up_some_forward_vertical_plane_roll_from_tp73_geometry() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let radius_value = radius.as_f64();
    let stun = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("0", "5"),
        AngularVelocity3::zero(),
    ));

    let reflected = collide_ball_rail_on_table(&stun, Rail::Top, RailModel::SpinAware);
    let outgoing_rolling_wx = -reflected.as_ball_state().velocity.y().as_f64() / radius_value;

    assert!(
        reflected.as_ball_state().angular_velocity.x().as_f64() > 0.0,
        "TP 7.3 predicts that even a stun rail impact can pick up some forward roll from cushion geometry"
    );
    assert!(
        reflected.as_ball_state().angular_velocity.x().as_f64() < outgoing_rolling_wx,
        "the stun rebound should still leave the ball sliding, not immediately rolling"
    );
}

#[test]
fn a_larger_effective_contact_height_ratio_seeds_more_forward_roll_for_a_stun_entry() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let stun = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("0", "5"),
        AngularVelocity3::zero(),
    ));
    let no_geometry = RailCollisionConfig::new(Scale::from_f64(0.8), Scale::from_f64(0.0))
        .with_impact_cloth_friction_coefficient(Scale::from_f64(0.0))
        .with_effective_contact_height_ratio(Scale::from_f64(0.0));
    let stronger_geometry = RailCollisionConfig::new(Scale::from_f64(0.8), Scale::from_f64(0.0))
        .with_impact_cloth_friction_coefficient(Scale::from_f64(0.0))
        .with_effective_contact_height_ratio(Scale::from_f64(0.08));

    let without_geometry = collide_ball_rail_on_table_with_radius_and_config(
        &stun,
        Rail::Top,
        radius.clone(),
        RailModel::SpinAware,
        &no_geometry,
    );
    let with_geometry = collide_ball_rail_on_table_with_radius_and_config(
        &stun,
        Rail::Top,
        radius,
        RailModel::SpinAware,
        &stronger_geometry,
    );

    assert!(
        with_geometry.as_ball_state().angular_velocity.x().as_f64()
            > without_geometry.as_ball_state().angular_velocity.x().as_f64() + 1e-9,
        "a larger TP 7.3-style effective contact height should seed more forward vertical-plane roll"
    );
}

#[test]
fn tp73_vertical_spin_prediction_matches_published_examples() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let radius_value = radius.as_f64();
    let impact_speed = 5.0 * 12.0;

    for (_name, incoming_angular_speed, expected_outgoing_angular_speed) in [
        ("rolling", impact_speed / radius_value, 3.333),
        ("overspin", 1.5 * impact_speed / radius_value, -23.333),
        ("stun", 0.0, 18.133),
        ("draw", -0.5 * impact_speed / radius_value, 6.267),
    ] {
        let prediction = tp73_rail_vertical_spin_prediction(
            InchesPerSecond::new(Inches::from_f64(impact_speed)),
            RadiansPerSecond::new(incoming_angular_speed),
            radius.clone(),
            Scale::from_f64(0.7),
            Scale::from_f64(0.17),
            Scale::from_f64(0.08),
        );

        assert_close_with_tolerance(prediction.outgoing_normal_speed.as_f64(), 42.0, 0.001);
        assert_close_with_tolerance(
            prediction.outgoing_pure_roll_angular_speed.as_f64(),
            37.333,
            0.001,
        );
        assert_close_with_tolerance(
            prediction.outgoing_angular_speed.as_f64(),
            expected_outgoing_angular_speed,
            0.001,
        );
    }
}

#[test]
fn mathavan_perpendicular_rolling_rebound_matches_low_speed_rigid_slope() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let radius_value = radius.as_f64();
    // Mathavan/Jackson/Parkin report ee = 0.98, mu_w = 0.14, table-felt sliding friction
    // mu_s = 0.212. Their broad 0.28-3.5 m/s experimental fit gives an effective speed ratio of
    // 0.818, but they attribute the higher-speed drop to cushion deformation and report a
    // low-speed rigid-cushion gradient around 0.910.
    let incident_speed = 39.370_078_740_157_48; // 1 m/s, inside Mathavan's rigid-cushion range.
    let rolling = rail_state_from_local_frame(
        Rail::Top,
        0.0,
        incident_speed,
        -incident_speed / radius_value,
        0.0,
        0.0,
    );
    let mathavan_config = RailCollisionConfig::new(Scale::from_f64(0.98), Scale::from_f64(0.14))
        .with_impact_cloth_friction_coefficient(Scale::from_f64(0.212))
        .with_effective_contact_height_ratio(Scale::from_f64(0.0));

    let reflected = collide_ball_rail_on_table_with_radius_and_config(
        &rolling,
        Rail::Top,
        radius,
        RailModel::SpinAware,
        &mathavan_config,
    );
    let actual = rail_local_frame_components(Rail::Top, &reflected);
    let rebound_ratio = -actual[1] / incident_speed;

    assert_close_with_tolerance(actual[0], 0.0, 1e-9);
    assert_close_with_tolerance(rebound_ratio, 0.910, 0.02);
}

#[test]
fn a_higher_impact_cloth_friction_coefficient_reduces_post_rail_cloth_slip_for_rolling_entries() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let radius_value = radius.as_f64();
    let rolling = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("0", "5"),
        AngularVelocity3::new(-5.0 / radius_value, 0.0, 0.0),
    ));
    let low_impact_cloth_friction =
        RailCollisionConfig::new(Scale::from_f64(0.8), Scale::from_f64(0.0))
            .with_impact_cloth_friction_coefficient(Scale::from_f64(0.0))
            .with_effective_contact_height_ratio(Scale::from_f64(0.0));
    let high_impact_cloth_friction =
        RailCollisionConfig::new(Scale::from_f64(0.8), Scale::from_f64(0.0))
            .with_impact_cloth_friction_coefficient(Scale::from_f64(0.4))
            .with_effective_contact_height_ratio(Scale::from_f64(0.0));

    let low_friction_rebound = collide_ball_rail_on_table_with_radius_and_config(
        &rolling,
        Rail::Top,
        radius.clone(),
        RailModel::SpinAware,
        &low_impact_cloth_friction,
    );
    let high_friction_rebound = collide_ball_rail_on_table_with_radius_and_config(
        &rolling,
        Rail::Top,
        radius.clone(),
        RailModel::SpinAware,
        &high_impact_cloth_friction,
    );
    let low_slip =
        cloth_contact_velocity_on_table(low_friction_rebound.as_ball_state(), radius.clone())
            .x()
            .as_f64()
            .hypot(
                cloth_contact_velocity_on_table(
                    low_friction_rebound.as_ball_state(),
                    radius.clone(),
                )
                .y()
                .as_f64(),
            );
    let high_slip =
        cloth_contact_velocity_on_table(high_friction_rebound.as_ball_state(), radius.clone())
            .x()
            .as_f64()
            .hypot(
                cloth_contact_velocity_on_table(high_friction_rebound.as_ball_state(), radius)
                    .y()
                    .as_f64(),
            );

    assert!(
        high_slip < low_slip,
        "stronger impact-time cloth friction should leave a rolling-style rebound closer to stun / lower cloth slip"
    );
}

#[test]
fn a_spin_aware_overspin_entry_can_leave_with_reverse_vertical_plane_spin() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let radius_value = radius.as_f64();
    let overspin = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("0", "5"),
        AngularVelocity3::new(-1.5 * 5.0 / radius_value, 0.0, 0.0),
    ));

    let reflected = collide_ball_rail_on_table(&overspin, Rail::Top, RailModel::SpinAware);
    let outgoing_rolling_wx = -reflected.as_ball_state().velocity.y().as_f64() / radius_value;

    assert!(
        reflected.as_ball_state().angular_velocity.x().as_f64().signum()
            != outgoing_rolling_wx.signum(),
        "strong overspin / follow-style entries should be able to leave the rail with reverse vertical-plane spin"
    );
}

#[test]
fn a_slightly_overspinning_entry_still_can_reverse_spin_relative_to_the_new_direction() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let radius_value = radius.as_f64();
    let slight_overspin = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("0", "5"),
        AngularVelocity3::new(-1.08 * 5.0 / radius_value, 0.0, 0.0),
    ));

    let reflected = collide_ball_rail_on_table(&slight_overspin, Rail::Top, RailModel::SpinAware);
    let outgoing_wx = reflected.as_ball_state().angular_velocity.x().as_f64();
    let outgoing_rolling_wx = -reflected.as_ball_state().velocity.y().as_f64() / radius_value;

    assert!(
        outgoing_wx < 0.0,
        "TP 7.3 flips to reverse spin for slight overspin beyond the near-rolling crossover; got outgoing_wx={outgoing_wx}, outgoing_rolling_wx={outgoing_rolling_wx}"
    );
}

#[test]
fn a_rolling_ball_rebounding_from_a_rail_carries_draw_like_spin_relative_to_its_new_direction() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let radius_value = radius.as_f64();
    let rolling = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("0", "5"),
        AngularVelocity3::new(-5.0 / radius_value, 0.0, 0.0),
    ));

    assert_eq!(
        rolling.as_ball_state().motion_phase(radius.clone()),
        MotionPhase::Rolling
    );

    let reflected = collide_ball_rail_on_table(&rolling, Rail::Top, RailModel::Mirror);
    let slip = cloth_contact_velocity_on_table(reflected.as_ball_state(), radius.clone());

    assert_eq!(
        reflected.as_ball_state().motion_phase(radius),
        MotionPhase::Sliding,
        "a rail rebound should generally break the no-slip rolling condition"
    );
    assert_close(reflected.as_ball_state().velocity.y().as_f64(), -5.0);
    assert_close(
        reflected.as_ball_state().angular_velocity.x().as_f64(),
        -5.0 / radius_value,
    );
    assert!(
        slip.y().as_f64().signum() == reflected.as_ball_state().velocity.y().as_f64().signum(),
        "the carried pre-impact rolling spin should act like draw against the new travel direction"
    );
}

#[test]
fn gearing_english_preserves_side_spin_better_than_tangential_speed_under_table_contact() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let geared_spin = 5.0 / (radius.as_f64() * (21.0_f64).sqrt() / 5.0);
    let state = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("5", "5"),
        AngularVelocity3::new(0.0, 0.0, geared_spin),
    ));
    let config = RailCollisionConfig::new(Scale::from_f64(0.8), Scale::from_f64(0.1));

    let reflected = collide_ball_rail_on_table_with_radius_and_config(
        &state,
        Rail::Top,
        radius,
        RailModel::SpinAware,
        &config,
    );

    assert!(reflected.as_ball_state().velocity.x().as_f64() > 4.0);
    assert!(reflected.as_ball_state().velocity.x().as_f64() < 5.0);
    assert!(reflected.as_ball_state().velocity.y().as_f64() < 0.0);
    assert!(reflected.as_ball_state().angular_velocity.x().as_f64() > 0.0);
    assert_close(
        reflected.as_ball_state().angular_velocity.z().as_f64(),
        geared_spin,
    );
}

#[test]
fn spin_aware_rail_collision_is_local_frame_invariant_across_rails() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let config = RailCollisionConfig::new(Scale::from_f64(0.7), Scale::from_f64(0.17))
        .with_impact_cloth_friction_coefficient(Scale::from_f64(0.2))
        .with_effective_contact_height_ratio(Scale::from_f64(0.08));
    let expected = {
        let state = rail_state_from_local_frame(Rail::Top, 4.0, 5.0, -1.5, 2.0, 3.0);
        let reflected = collide_ball_rail_on_table_with_radius_and_config(
            &state,
            Rail::Top,
            radius.clone(),
            RailModel::SpinAware,
            &config,
        );
        rail_local_frame_components(Rail::Top, &reflected)
    };

    for rail in [Rail::Bottom, Rail::Left, Rail::Right] {
        let state = rail_state_from_local_frame(rail, 4.0, 5.0, -1.5, 2.0, 3.0);
        let reflected = collide_ball_rail_on_table_with_radius_and_config(
            &state,
            rail,
            radius.clone(),
            RailModel::SpinAware,
            &config,
        );
        let actual = rail_local_frame_components(rail, &reflected);

        for ((component, actual), expected) in [
            "tangent velocity",
            "normal velocity",
            "angular tangent",
            "angular normal",
            "angular vertical",
        ]
        .into_iter()
        .zip(actual)
        .zip(expected)
        {
            let delta = (actual - expected).abs();
            assert!(
                delta < 1e-9,
                "{rail:?} {component}: expected {expected}, got {actual} (delta {delta})"
            );
        }
    }
}

#[test]
fn a_rail_profile_can_make_different_rails_play_differently() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("5", "5"),
        AngularVelocity3::zero(),
    ));
    let profile = RailCollisionProfile::human_tuned()
        .with_top(RailCollisionConfig::new(
            Scale::from_f64(0.6),
            Scale::from_f64(1.0),
        ))
        .with_right(RailCollisionConfig::new(
            Scale::from_f64(0.9),
            Scale::from_f64(1.0),
        ));

    let top_reflected = collide_ball_rail_on_table_with_radius_and_profile(
        &state,
        Rail::Top,
        radius.clone(),
        RailModel::RestitutionOnly,
        &profile,
    );
    let right_reflected = collide_ball_rail_on_table_with_radius_and_profile(
        &state,
        Rail::Right,
        radius,
        RailModel::RestitutionOnly,
        &profile,
    );

    assert_close(top_reflected.as_ball_state().velocity.x().as_f64(), 5.0);
    assert_close(top_reflected.as_ball_state().velocity.y().as_f64(), -3.0);
    assert_close(right_reflected.as_ball_state().velocity.x().as_f64(), -4.5);
    assert_close(right_reflected.as_ball_state().velocity.y().as_f64(), 5.0);
}
