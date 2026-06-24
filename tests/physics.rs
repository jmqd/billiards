use bigdecimal::BigDecimal;
use billiards::{
    frozen_cue_ball_jewett_cut_angle, gearing_english, predict_small_gap_combination_throw, Angle,
    BallBallCollisionConfig, BallBallFrictionModel, CutAngle, Displacement, Inches,
    InchesPerSecond, Pocket, Position, Scale, TYPICAL_BALL_RADIUS,
};

fn shot_speed_ips(ips: i64) -> InchesPerSecond {
    InchesPerSecond::new(Inches {
        magnitude: BigDecimal::from(ips),
    })
}

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

fn tp_b21_collision_config() -> BallBallCollisionConfig {
    BallBallCollisionConfig::new_with_friction_model(
        Scale::from_f64(1.0),
        BallBallFrictionModel::marlow_speed_fit(Scale::from_f64(1.0)),
    )
}

#[test]
fn gearing_english_is_zero_for_a_straight_shot() {
    let omega = gearing_english(CutAngle::new(0.0), shot_speed_ips(10));

    assert_close(omega.as_f64(), 0.0);
}

#[test]
fn gearing_english_uses_radians_for_the_cut_angle() {
    let omega = gearing_english(CutAngle::new(30.0), shot_speed_ips(10));

    assert_close(omega.as_f64(), 10.0 * 0.5 / 1.125);
}

#[test]
fn given_cue_and_object_ball_headings_when_measuring_cut_angle_then_the_acute_impact_magnitude_is_returned(
) {
    let cue_heading = Angle::from_north(0.5, 0.866_025_403_784_438_6);
    let object_ball_heading = Angle::from_north(0.0, 1.0);
    let opposite_line_heading = Angle::from_north(0.0, -1.0);

    let cut_angle = CutAngle::from_headings(cue_heading, object_ball_heading);
    let cut_angle_from_opposite_line = CutAngle::from_headings(cue_heading, opposite_line_heading);

    assert_close(cut_angle.as_degrees(), 30.0);
    assert_close(cut_angle_from_opposite_line.as_degrees(), 30.0);
}

#[test]
fn tp_a15_frozen_cue_ball_jewett_cut_angle_matches_formula_anchors() {
    for (target_angle, expected_cut_angle) in [
        (0.0, 0.0),
        (20.0, 10.314_104_815_618_194),
        (45.0, 26.565_051_177_077_99),
        (60.0, 40.893_394_649_130_91),
        (80.0, 70.574_599_859_317_19),
        (90.0, 90.0),
    ] {
        assert_close(
            frozen_cue_ball_jewett_cut_angle(target_angle).as_degrees(),
            expected_cut_angle,
        );
    }
}

#[test]
fn tp_a15_frozen_cue_ball_jewett_cut_angle_tracks_the_reported_experimental_table() {
    for (target_angle, observed_cut_angle) in [
        (0.0, 0.0),
        (13.0, 8.0),
        (34.0, 20.0),
        (50.0, 34.0),
        (61.0, 46.0),
        (70.0, 57.0),
        (78.0, 67.0),
        (87.0, 77.0),
        (90.0, 90.0),
    ] {
        let predicted = frozen_cue_ball_jewett_cut_angle(target_angle).as_degrees();
        let delta = (predicted - observed_cut_angle).abs();

        assert!(
            delta <= 8.0,
            "target angle {target_angle}: predicted {predicted}, observed {observed_cut_angle}, delta {delta}"
        );
    }
}

#[test]
fn tp_b21_small_gap_combination_throw_matches_geometry_anchors() {
    for (gap, expected_max_angle) in [
        (0.01, 84.608_077_034_296_53),
        (0.1, 73.225_255_735_904_57),
        (0.375, 58.997_280_866_126_005),
        (0.75, 48.590_377_890_729_144),
    ] {
        let prediction = predict_small_gap_combination_throw(
            InchesPerSecond::from_mph(3.0),
            0.0,
            Inches::from_f64(gap),
            TYPICAL_BALL_RADIUS.clone(),
            &tp_b21_collision_config(),
        );

        assert_close(
            prediction.max_first_object_ball_angle_degrees,
            expected_max_angle,
        );
    }

    let prediction = predict_small_gap_combination_throw(
        InchesPerSecond::from_mph(3.0),
        28.0,
        Inches::from_f64(0.375),
        TYPICAL_BALL_RADIUS.clone(),
        &tp_b21_collision_config(),
    );

    assert_close(
        prediction.line_of_centers_angle_degrees,
        5.210_518_071_084_367,
    );
    assert_close(prediction.cut_angle.as_degrees(), 33.210_518_071_084_365);
    assert_close(prediction.hit_fraction.as_f64(), 0.452_283_176_749_794);
    assert_close(prediction.throw_angle_degrees, 3.348_993_545_616_846_5);
    assert_close(
        prediction.second_object_ball_angle_degrees,
        1.861_524_525_467_520_3,
    );

    let small_angle_prediction = predict_small_gap_combination_throw(
        InchesPerSecond::from_mph(3.0),
        20.0,
        Inches::from_f64(0.375),
        TYPICAL_BALL_RADIUS.clone(),
        &tp_b21_collision_config(),
    );

    assert_close(
        small_angle_prediction.line_of_centers_angle_degrees,
        3.517_146_965_015_102,
    );
    assert_close(
        small_angle_prediction.cut_angle.as_degrees(),
        23.517_146_965_015_1,
    );
    assert_close(
        small_angle_prediction.second_object_ball_angle_degrees,
        -0.040_175_833_642_322_69,
    );
}

#[test]
fn tp_b21_optimal_three_eighths_gap_cancels_throw_for_small_angle_combos() {
    let config = tp_b21_collision_config();
    let slow_speed = InchesPerSecond::from_mph(1.0);
    let small_gap = predict_small_gap_combination_throw(
        slow_speed.clone(),
        20.0,
        Inches::from_f64(0.25),
        TYPICAL_BALL_RADIUS.clone(),
        &config,
    );
    let optimal_gap = predict_small_gap_combination_throw(
        slow_speed.clone(),
        20.0,
        Inches::from_f64(0.375),
        TYPICAL_BALL_RADIUS.clone(),
        &config,
    );
    let large_gap = predict_small_gap_combination_throw(
        slow_speed,
        20.0,
        Inches::from_f64(0.5),
        TYPICAL_BALL_RADIUS.clone(),
        &config,
    );

    assert!(
        small_gap.second_object_ball_angle_degrees < -0.9,
        "smaller gaps should over-throw the second object ball; got {}",
        small_gap.second_object_ball_angle_degrees
    );
    assert!(
        optimal_gap.second_object_ball_angle_degrees.abs() < 0.05,
        "the TP B.21 3/8-inch gap should nearly cancel throw at small angles; got {}",
        optimal_gap.second_object_ball_angle_degrees
    );
    assert!(
        large_gap.second_object_ball_angle_degrees > 0.9,
        "larger gaps should leave the second object ball in the cut direction; got {}",
        large_gap.second_object_ball_angle_degrees
    );
}

#[test]
fn tp_b21_small_gap_combination_throw_respects_zero_and_scaled_friction() {
    let zero_friction = BallBallCollisionConfig::new(Scale::from_f64(1.0), Scale::zero());
    let zero = predict_small_gap_combination_throw(
        InchesPerSecond::from_mph(3.0),
        28.0,
        Inches::from_f64(0.375),
        TYPICAL_BALL_RADIUS.clone(),
        &zero_friction,
    );

    assert_close(zero.throw_angle_degrees, 0.0);
    assert_close(
        zero.second_object_ball_angle_degrees,
        zero.line_of_centers_angle_degrees,
    );

    let throw_for_scale = |scale| {
        let config = BallBallCollisionConfig::new_with_friction_model(
            Scale::from_f64(1.0),
            BallBallFrictionModel::marlow_speed_fit(Scale::from_f64(scale)),
        );
        predict_small_gap_combination_throw(
            InchesPerSecond::from_mph(3.0),
            28.0,
            Inches::from_f64(0.375),
            TYPICAL_BALL_RADIUS.clone(),
            &config,
        )
        .throw_angle_degrees
        .abs()
    };

    let half = throw_for_scale(0.5);
    let average = throw_for_scale(1.0);
    let double = throw_for_scale(2.0);

    assert!(half < average);
    assert!(average < double);
}

#[test]
fn tp_b21_small_gap_combination_throw_mirrors_signed_angles() {
    let positive = predict_small_gap_combination_throw(
        InchesPerSecond::from_mph(3.0),
        28.0,
        Inches::from_f64(0.375),
        TYPICAL_BALL_RADIUS.clone(),
        &tp_b21_collision_config(),
    );
    let negative = predict_small_gap_combination_throw(
        InchesPerSecond::from_mph(3.0),
        -28.0,
        Inches::from_f64(0.375),
        TYPICAL_BALL_RADIUS.clone(),
        &tp_b21_collision_config(),
    );

    assert_close(
        negative.line_of_centers_angle_degrees,
        -positive.line_of_centers_angle_degrees,
    );
    assert_close(
        negative.cut_angle.as_degrees(),
        positive.cut_angle.as_degrees(),
    );
    assert_close(negative.throw_angle_degrees, -positive.throw_angle_degrees);
    assert_close(
        negative.second_object_ball_angle_degrees,
        -positive.second_object_ball_angle_degrees,
    );
}

#[test]
fn given_a_three_four_displacement_when_measuring_its_angle_from_north_then_the_expected_heading_is_returned(
) {
    let displacement = Displacement::new("3", "4");

    let angle = displacement.angle_from_north();

    assert_close(
        angle.to_string().parse().expect("angle degrees"),
        36.869_897_645_844_02,
    );
}

#[test]
fn given_a_position_and_a_pocket_when_measuring_to_and_from_the_pocket_then_the_directions_are_opposites(
) {
    let position = Position::new(2u8, 4u8);

    let to_pocket = position.angle_to_pocket(Pocket::TopRight);
    let from_pocket = position.angle_from_pocket(Pocket::TopRight);
    let to_pocket_degrees = to_pocket
        .to_string()
        .parse::<f64>()
        .expect("to-pocket angle");
    let from_pocket_degrees = from_pocket
        .to_string()
        .parse::<f64>()
        .expect("from-pocket angle");

    assert!(to_pocket_degrees > 0.0);
    assert!(to_pocket_degrees < 90.0);
    assert_close(
        from_pocket_degrees,
        (to_pocket_degrees + 180.0).rem_euclid(360.0),
    );
}
