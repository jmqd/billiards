use billiards::{
    cloth_contact_velocity_on_table, collide_ball_rail_on_table,
    collide_ball_rail_on_table_with_radius_and_config,
    collide_ball_rail_on_table_with_radius_and_profile, AngularVelocity3, BallState, Inches,
    Inches2, MotionPhase, OnTableBallState, Rail, RailCollisionConfig, RailCollisionProfile,
    RailModel, Scale, Velocity2, TYPICAL_BALL_RADIUS,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

fn on_table(state: BallState) -> OnTableBallState {
    OnTableBallState::try_from(state).expect("test states should validate as on-table")
}

fn inches2(x: f64, y: f64) -> Inches2 {
    Inches2::new(Inches::from_f64(x), Inches::from_f64(y))
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
    let config = RailCollisionConfig {
        normal_restitution: Scale::from_f64(0.8),
        tangential_friction_coefficient: Scale::from_f64(1.0),
    };

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
fn a_spin_aware_rail_collision_with_high_cushion_friction_trades_more_tangential_speed_for_running_english(
) {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("5", "5"),
        AngularVelocity3::zero(),
    ));
    let high_friction = RailCollisionConfig {
        normal_restitution: Scale::from_f64(0.8),
        tangential_friction_coefficient: Scale::from_f64(1.0),
    };
    let low_friction = RailCollisionConfig {
        normal_restitution: Scale::from_f64(0.8),
        tangential_friction_coefficient: Scale::from_f64(0.1),
    };

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
    let config = RailCollisionConfig {
        normal_restitution: Scale::from_f64(0.8),
        tangential_friction_coefficient: Scale::from_f64(0.1),
    };

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
fn a_spin_aware_rail_collision_with_topspin_reduces_vertical_plane_spin() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("0", "5"),
        AngularVelocity3::new(4.0, 0.0, 0.0),
    ));
    let config = RailCollisionConfig {
        normal_restitution: Scale::from_f64(0.8),
        tangential_friction_coefficient: Scale::from_f64(1.0),
    };

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
    let config = RailCollisionConfig {
        normal_restitution: Scale::from_f64(0.8),
        tangential_friction_coefficient: Scale::from_f64(1.0),
    };
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
    let config = RailCollisionConfig {
        normal_restitution: Scale::from_f64(0.8),
        tangential_friction_coefficient: Scale::from_f64(1.0),
    };

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
fn a_rolling_low_english_entry_leaves_the_rail_with_bounded_horizontal_cloth_slip() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let radius_value = radius.as_f64();
    let rolling = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("5", "5"),
        AngularVelocity3::new(-5.0 / radius_value, 5.0 / radius_value, 0.0),
    ));
    let reflected = collide_ball_rail_on_table(&rolling, Rail::Top, RailModel::SpinAware);
    let slip = cloth_contact_velocity_on_table(reflected.as_ball_state(), radius.clone());
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
        reflected.as_ball_state().motion_phase(radius.clone()),
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
    let config = RailCollisionConfig {
        normal_restitution: Scale::from_f64(0.8),
        tangential_friction_coefficient: Scale::from_f64(0.1),
    };

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
fn a_rail_profile_can_make_different_rails_play_differently() {
    let radius = TYPICAL_BALL_RADIUS.clone();
    let state = on_table(BallState::on_table(
        inches2(10.0, 20.0),
        Velocity2::new("5", "5"),
        AngularVelocity3::zero(),
    ));
    let profile = RailCollisionProfile::human_tuned()
        .with_top(RailCollisionConfig {
            normal_restitution: Scale::from_f64(0.6),
            tangential_friction_coefficient: Scale::from_f64(1.0),
        })
        .with_right(RailCollisionConfig {
            normal_restitution: Scale::from_f64(0.9),
            tangential_friction_coefficient: Scale::from_f64(1.0),
        });

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
