use std::str::FromStr;

use billiards::{
    format_shot_speed, strike_resting_ball_on_table, BallSetPhysicsSpec, BallState,
    CueStrikeConfig, CueTipContact, HumanShotSpeedBand, Inches2, InchesPerSecond,
    RestingOnTableBallState, Scale, Shot, ShotError, ShotSpeedPreset,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

#[test]
fn dr_dave_shot_speed_presets_round_trip_and_format_nearest_speed() {
    assert_eq!(ShotSpeedPreset::Medium.as_str(), "medium");
    assert_eq!(ShotSpeedPreset::Medium.to_string(), "medium");
    assert_eq!(ShotSpeedPreset::Medium.human_label(), "medium speed");
    assert_close(ShotSpeedPreset::Medium.mph(), 7.0);
    assert_close(
        ShotSpeedPreset::Medium.inches_per_second().as_f64(),
        7.0 * 17.6,
    );

    assert_eq!(
        ShotSpeedPreset::from_str("medium_soft").expect("underscore alias should parse"),
        ShotSpeedPreset::MediumSoft
    );
    assert_eq!(
        ShotSpeedPreset::from_str("speed2").expect("numbered speed alias should parse"),
        ShotSpeedPreset::Medium
    );
    assert_eq!(
        ShotSpeedPreset::from_str("three-quarter").expect("stroke-length alias should parse"),
        ShotSpeedPreset::Fast
    );
    assert_eq!(
        ShotSpeedPreset::nearest_to_speed(&InchesPerSecond::new("128")),
        ShotSpeedPreset::Medium
    );
    assert_eq!(
        format_shot_speed(&InchesPerSecond::new("128")),
        "128 ips (~medium speed)"
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
fn cue_tip_contact_reports_tp_2_1_no_slip_friction_requirement() {
    let center = CueTipContact::center();
    let half_radius = CueTipContact::new(Scale::from_f64(0.5), Scale::zero())
        .expect("half-radius contact should validate");
    let diagonal_half_radius = CueTipContact::new(Scale::from_f64(0.3), Scale::from_f64(0.4))
        .expect("diagonal half-radius contact should validate");
    let measured_limit = CueTipContact::new(Scale::from_f64(0.55), Scale::zero())
        .expect("measured larger-offset contact should validate");
    let rim = CueTipContact::new(Scale::from_f64(1.0), Scale::zero())
        .expect("rim contact should validate");

    assert_close(
        center
            .required_static_tip_friction_for_no_slip()
            .expect("center hit has finite no-slip friction")
            .as_f64(),
        0.0,
    );
    assert_close(
        half_radius
            .required_static_tip_friction_for_no_slip()
            .expect("half-radius hit has finite no-slip friction")
            .as_f64(),
        1.0 / 3.0_f64.sqrt(),
    );
    assert_close(
        diagonal_half_radius
            .required_static_tip_friction_for_no_slip()
            .expect("diagonal half-radius hit has finite no-slip friction")
            .as_f64(),
        1.0 / 3.0_f64.sqrt(),
    );
    assert_close(
        measured_limit
            .required_static_tip_friction_for_no_slip()
            .expect("measured limit has finite no-slip friction")
            .as_f64(),
        0.55 / (1.0_f64 - 0.55_f64.powi(2)).sqrt(),
    );
    assert!(
        rim.required_static_tip_friction_for_no_slip().is_none(),
        "TP 2.1 friction requirement diverges at the ball rim"
    );
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
fn shot_can_be_constructed_from_a_cue_ball_launch_speed() {
    let cue = CueStrikeConfig::new(Scale::from_f64(1.0), Scale::from_f64(0.1))
        .expect("cue config should validate");
    let shot = Shot::new_for_cue_ball_launch_speed(
        billiards::Angle::from_north(0.0, 1.0),
        InchesPerSecond::new("128"),
        CueTipContact::center(),
        &cue,
    )
    .expect("launch-speed shot should validate");

    assert_close(
        shot.cue_speed().as_f64(),
        128.0 / ((1.0 + 0.8_f64.sqrt()) / 2.0),
    );
    assert_close(
        shot.human_speed_validation(&cue)
            .expect("human speed validation should succeed")
            .estimated_cue_ball_speed_after_impact
            .as_f64(),
        128.0,
    );
}

#[test]
fn off_center_launch_speed_construction_preserves_post_squirt_speed_magnitude() {
    let cue = CueStrikeConfig::new(Scale::from_f64(1.0), Scale::from_f64(0.1))
        .expect("cue config should validate");
    let tip_contact = CueTipContact::new(Scale::from_f64(0.4), Scale::zero()).expect("tip contact");
    let shot = Shot::new_for_cue_ball_launch_speed(
        billiards::Angle::from_north(0.0, 1.0),
        InchesPerSecond::new("128"),
        tip_contact,
        &cue,
    )
    .expect("launch-speed shot should validate");
    let resting =
        RestingOnTableBallState::try_from(BallState::resting_at(Inches2::new("10", "20")))
            .expect("resting test state should validate");
    let struck =
        strike_resting_ball_on_table(&resting, &shot, &cue, &BallSetPhysicsSpec::default())
            .expect("off-center strike should succeed");

    assert_close(struck.as_ball_state().speed().as_f64(), 128.0);
    assert!(
        struck.as_ball_state().velocity.x().as_f64() < 0.0,
        "squirt should change launch direction, not launch-speed magnitude"
    );
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
    assert_close(config.cue_ball_to_endmass_ratio().as_f64(), 20.151);
    assert_close(config.miscue_offset_limit().as_f64(), 0.5);
    assert_close(custom.miscue_offset_limit().as_f64(), 0.4);
}

#[test]
fn cue_strike_config_can_override_the_squirt_endmass_ratio() {
    let low_squirt = CueStrikeConfig::new_with_endmass_ratio(
        Scale::from_f64(3.0),
        Scale::from_f64(0.2),
        Scale::from_f64(40.0),
    )
    .expect("positive endmass ratio should validate");
    let custom = CueStrikeConfig::new_with_miscue_offset_limit_and_endmass_ratio(
        Scale::from_f64(3.0),
        Scale::from_f64(0.2),
        Scale::from_f64(0.4),
        Scale::from_f64(12.0),
    )
    .expect("positive endmass ratio and in-range miscue limit should validate");

    assert_close(low_squirt.cue_ball_to_endmass_ratio().as_f64(), 40.0);
    assert_close(custom.miscue_offset_limit().as_f64(), 0.4);
    assert_close(custom.cue_ball_to_endmass_ratio().as_f64(), 12.0);
}

#[test]
fn cue_strike_config_rejects_nonpositive_mass_ratio_and_out_of_range_energy_loss_or_miscue_limit() {
    let by_mass_ratio = CueStrikeConfig::new(Scale::zero(), Scale::from_f64(0.2));
    let by_energy_loss = CueStrikeConfig::new(Scale::from_f64(3.0), Scale::from_f64(1.5));
    let by_endmass_ratio = CueStrikeConfig::new_with_endmass_ratio(
        Scale::from_f64(3.0),
        Scale::from_f64(0.2),
        Scale::zero(),
    );
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
        by_endmass_ratio,
        Err(ShotError::NonPositiveCueBallToEndmassRatio { .. })
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
