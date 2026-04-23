use billiards::{
    human_tuned_preview_motion_config, BallBallCollisionConfig, PlayingConditions,
    RailCollisionConfig, RailCollisionProfile, RollingResistanceModel, SlidingFrictionModel,
    SpinDecayModel,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

fn sliding_accel(config: &billiards::MotionTransitionConfig) -> f64 {
    match &config.sliding_friction {
        SlidingFrictionModel::ConstantAcceleration {
            acceleration_magnitude,
        } => acceleration_magnitude.as_f64(),
    }
}

fn spin_decay(config: &billiards::MotionTransitionConfig) -> f64 {
    match &config.spin_decay {
        SpinDecayModel::ConstantAngularDeceleration {
            angular_deceleration,
        } => angular_deceleration.as_f64(),
    }
}

fn rolling_decel(config: &billiards::MotionTransitionConfig) -> f64 {
    match &config.rolling_resistance {
        RollingResistanceModel::ConstantDeceleration { linear_deceleration } => {
            linear_deceleration.as_f64()
        }
    }
}

#[test]
fn neutral_conditions_leave_existing_configs_unchanged() {
    let conditions = PlayingConditions::neutral();
    let motion = human_tuned_preview_motion_config();
    let ball_ball = BallBallCollisionConfig::human_tuned();
    let rail = RailCollisionConfig::human_tuned();
    let profile = RailCollisionProfile::human_tuned();

    assert_eq!(motion.applying_conditions(&conditions), motion);
    assert_eq!(ball_ball.applying_conditions(&conditions), ball_ball);
    assert_eq!(rail.applying_conditions(&conditions), rail);
    assert_eq!(profile.applying_conditions(&conditions), profile);
}

#[test]
fn humid_dirty_conditions_increase_motion_damping_and_deaden_contacts() {
    let conditions = PlayingConditions::humid_dirty();
    let base_motion = human_tuned_preview_motion_config();
    let scaled_motion = base_motion.applying_conditions(&conditions);
    let base_ball_ball = BallBallCollisionConfig::human_tuned();
    let scaled_ball_ball = base_ball_ball.applying_conditions(&conditions);
    let base_rail = RailCollisionConfig::human_tuned();
    let scaled_rail = base_rail.applying_conditions(&conditions);

    assert!(sliding_accel(&scaled_motion) > sliding_accel(&base_motion));
    assert!(spin_decay(&scaled_motion) > spin_decay(&base_motion));
    assert!(rolling_decel(&scaled_motion) > rolling_decel(&base_motion));

    assert!(
        scaled_ball_ball.normal_restitution.as_f64() < base_ball_ball.normal_restitution.as_f64()
    );
    assert!(
        scaled_ball_ball.tangential_friction_coefficient.as_f64()
            > base_ball_ball.tangential_friction_coefficient.as_f64()
    );

    assert!(scaled_rail.normal_restitution.as_f64() < base_rail.normal_restitution.as_f64());
    assert!(
        scaled_rail.tangential_friction_coefficient.as_f64()
            > base_rail.tangential_friction_coefficient.as_f64()
    );
    assert!(
        scaled_rail.impact_cloth_friction_coefficient.as_f64()
            > base_rail.impact_cloth_friction_coefficient.as_f64()
    );
    assert_close(
        scaled_rail.effective_contact_height_ratio.as_f64(),
        base_rail.effective_contact_height_ratio.as_f64(),
    );
}

#[test]
fn conditions_apply_across_each_rail_in_a_profile() {
    let conditions = PlayingConditions::humid_dirty();
    let profile = RailCollisionProfile::human_tuned()
        .with_top(RailCollisionConfig::human_tuned().with_effective_contact_height_ratio(
            billiards::Scale::from_f64(0.08),
        ))
        .with_right(
            RailCollisionConfig::human_tuned()
                .with_impact_cloth_friction_coefficient(billiards::Scale::from_f64(0.3)),
        );

    let scaled = profile.applying_conditions(&conditions);

    assert!(scaled.top.normal_restitution.as_f64() < profile.top.normal_restitution.as_f64());
    assert!(
        scaled.right.impact_cloth_friction_coefficient.as_f64()
            > profile.right.impact_cloth_friction_coefficient.as_f64()
    );
    assert_close(
        scaled.top.effective_contact_height_ratio.as_f64(),
        profile.top.effective_contact_height_ratio.as_f64(),
    );
    assert_close(
        scaled.left.effective_contact_height_ratio.as_f64(),
        profile.left.effective_contact_height_ratio.as_f64(),
    );
}
