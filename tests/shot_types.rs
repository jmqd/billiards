use billiards::{
    CueStrikeConfig, CueTipContact, HumanShotSpeedBand, InchesPerSecond, Scale, Shot, ShotError,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

#[test]
fn cue_tip_contact_accepts_center_and_preserves_radius_scaled_offsets() {
    let center = CueTipContact::center();
    let contact = CueTipContact::new(Scale::from_f64(0.25), Scale::from_f64(-0.5))
        .expect("an in-ball tip contact should validate");

    assert_close(center.side_offset().as_f64(), 0.0);
    assert_close(center.height_offset().as_f64(), 0.0);
    assert_close(contact.side_offset().as_f64(), 0.25);
    assert_close(contact.height_offset().as_f64(), -0.5);
    assert_close(contact.offset_radius().as_f64(), (0.25_f64).hypot(-0.5));
}

#[test]
fn cue_tip_contact_rejects_offsets_outside_the_ball_disc() {
    let error = CueTipContact::new(Scale::from_f64(0.9), Scale::from_f64(0.9))
        .expect_err("offsets outside the unit-radius contact disc should be rejected");

    match error {
        ShotError::CueTipContactOutsideBall {
            side_offset,
            height_offset,
            offset_radius,
        } => {
            assert_close(side_offset.as_f64(), 0.9);
            assert_close(height_offset.as_f64(), 0.9);
            assert!(offset_radius.as_f64() > 1.0);
        }
        other => panic!("expected cue-tip contact error, got {other:?}"),
    }
}

#[test]
fn shot_accepts_nonnegative_cue_speed_and_preserves_inputs() {
    let tip_contact = CueTipContact::new(Scale::from_f64(-0.2), Scale::from_f64(0.4))
        .expect("tip contact should validate");
    let shot = Shot::new(
        billiards::Angle::from_north(0.0, 1.0),
        InchesPerSecond::new("18"),
        tip_contact.clone(),
    )
    .expect("nonnegative cue speed should validate");

    assert_close(shot.heading().as_degrees(), 0.0);
    assert_close(shot.cue_speed().as_f64(), 18.0);
    assert_eq!(shot.tip_contact(), &tip_contact);
}

#[test]
fn shot_rejects_negative_cue_speed() {
    let error = Shot::new(
        billiards::Angle::from_north(1.0, 0.0),
        InchesPerSecond::new("-1"),
        CueTipContact::center(),
    )
    .expect_err("negative cue speed should be rejected");

    match error {
        ShotError::NegativeCueSpeed { cue_speed } => {
            assert_close(cue_speed.as_f64(), -1.0);
        }
        other => panic!("expected negative cue speed error, got {other:?}"),
    }
}

#[test]
fn cue_strike_config_accepts_positive_mass_ratio_and_unit_interval_energy_loss() {
    let config = CueStrikeConfig::new(Scale::from_f64(3.0), Scale::from_f64(0.2))
        .expect("physically meaningful strike config should validate");
    let custom = CueStrikeConfig::new_with_miscue_offset_limit(
        Scale::from_f64(3.0),
        Scale::from_f64(0.2),
        Scale::from_f64(0.4),
    )
    .expect("a custom in-range miscue limit should validate");

    assert_close(config.cue_mass_ratio().as_f64(), 3.0);
    assert_close(config.collision_energy_loss().as_f64(), 0.2);
    assert_close(config.miscue_offset_limit().as_f64(), 0.5);
    assert_close(custom.miscue_offset_limit().as_f64(), 0.4);
}

#[test]
fn cue_strike_config_rejects_nonpositive_mass_ratio_and_out_of_range_energy_loss_or_miscue_limit() {
    let by_mass_ratio = CueStrikeConfig::new(Scale::zero(), Scale::from_f64(0.2));
    let by_energy_loss = CueStrikeConfig::new(Scale::from_f64(3.0), Scale::from_f64(1.5));
    let by_miscue_limit = CueStrikeConfig::new_with_miscue_offset_limit(
        Scale::from_f64(3.0),
        Scale::from_f64(0.2),
        Scale::from_f64(1.5),
    );

    assert!(matches!(
        by_mass_ratio,
        Err(ShotError::NonPositiveCueMassRatio { .. })
    ));
    assert!(matches!(
        by_energy_loss,
        Err(ShotError::CollisionEnergyLossOutOfRange { .. })
    ));
    assert!(matches!(
        by_miscue_limit,
        Err(ShotError::MiscueOffsetLimitOutOfRange { .. })
    ));
}

#[test]
fn human_speed_validation_reports_a_128_ips_center_ball_hit_as_an_ordinary_table_shot() {
    let shot = Shot::new(
        billiards::Angle::from_north(0.0, 1.0),
        InchesPerSecond::new("128"),
        CueTipContact::center(),
    )
    .expect("shot should validate");
    let validation = shot
        .human_speed_validation(
            &CueStrikeConfig::new(Scale::from_f64(1.0), Scale::from_f64(0.1))
                .expect("cue config should validate"),
        )
        .expect("human speed validation should succeed");

    assert_close(validation.cue_speed_at_impact.as_mph(), 128.0 / 17.6);
    assert_close(
        validation.estimated_cue_ball_speed_after_impact.as_mph(),
        128.0 * (1.0 + (0.8_f64).sqrt()) / 2.0 / 17.6,
    );
    assert_eq!(validation.cue_speed_band, HumanShotSpeedBand::MediumFast);
    assert_eq!(validation.cue_ball_speed_band, HumanShotSpeedBand::Medium);
    assert!(validation.is_typical_table_shot());
    assert!(!validation.requires_power_shot());
}

#[test]
fn human_speed_validation_flags_speeds_beyond_exceptional_human_break_range() {
    let shot = Shot::new(
        billiards::Angle::from_north(0.0, 1.0),
        InchesPerSecond::new("700"),
        CueTipContact::center(),
    )
    .expect("shot should validate");
    let validation = shot
        .human_speed_validation(
            &CueStrikeConfig::new(Scale::from_f64(1.0), Scale::from_f64(0.1))
                .expect("cue config should validate"),
        )
        .expect("human speed validation should succeed");

    assert_eq!(
        validation.cue_ball_speed_band,
        HumanShotSpeedBand::BeyondExceptionalPowerBreak
    );
    assert!(validation.requires_power_shot());
    assert!(validation.exceeds_typical_human_power_break());
    assert!(validation.exceeds_exceptional_human_shot_speed());
}
