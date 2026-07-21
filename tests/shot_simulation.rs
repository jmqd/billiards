use billiards::shot_simulation::{
    carom_position_from_diamonds, execute_three_cushion, execute_three_cushion_compact,
    project_three_cushion, BallBallContactResolution, BallId, CaromBallRole, ContactInstant,
    OwnedShotResult, PhysicsProfile, ResolvedEffect, ResolvedEvent, SceneBall, ShotControls,
    ShotLayout, ShotLimit, ShotSimulationError, ShotTermination, ThreeCushionAdjudication,
    ThreeCushionIndeterminate, ThreeCushionMiss, ThreeCushionResult, ThreeCushionRoles,
    ThreeCushionShooter, ThreeCushionShot, UnsupportedPhysicsReason,
    THREE_CUSHION_MAX_CUE_BALL_HEIGHT_INCHES,
};
use billiards::{
    BallBallCollisionConfig, BallSetPhysicsSpec, Inches, Inches2, InchesPerSecond,
    InchesPerSecondSq, OnTableMotionConfig, RadiansPerSecond, RadiansPerSecondSq, Rail,
    RailCollisionProfile, RollingResistanceModel, Scale, Seconds, SlidingFrictionModel,
    SlidingToRollingModel, SpinDecayModel,
};

fn fixture_layout() -> ShotLayout {
    ShotLayout::three_cushion_from_diamonds((0.700, 1.000), (1.200, 2.100), (0.850, 6.550))
        .expect("fixture layout is valid")
}

fn fixture_controls() -> ShotControls {
    ShotControls::new(25.694, 152.0, -0.20, 0.30, 0.0).expect("fixture controls are valid")
}

fn ball_contact(first: BallId, second: BallId) -> ResolvedEffect {
    ResolvedEffect::BallBallContact {
        first,
        second,
        resolution: BallBallContactResolution::Pairwise,
    }
}

fn instant(at: f64, effects: Vec<ResolvedEffect>) -> ResolvedEvent {
    ResolvedEvent {
        at: Seconds::new(at),
        effects: effects.into_boxed_slice(),
    }
}

fn completed_point_events() -> Vec<ResolvedEvent> {
    vec![
        instant(
            1.0,
            vec![
                ResolvedEffect::BallRailContact {
                    ball: BallId::WHITE,
                    rail: Rail::Top,
                },
                ResolvedEffect::BallRailContact {
                    ball: BallId::WHITE,
                    rail: Rail::Right,
                },
                ResolvedEffect::BallRailContact {
                    ball: BallId::WHITE,
                    rail: Rail::Bottom,
                },
            ],
        ),
        instant(
            2.0,
            vec![
                ball_contact(BallId::WHITE, BallId::YELLOW),
                ball_contact(BallId::WHITE, BallId::RED),
            ],
        ),
    ]
}

fn owned(events: Vec<ResolvedEvent>, termination: ShotTermination) -> OwnedShotResult {
    OwnedShotResult {
        elapsed: events.last().map_or(Seconds::zero(), |event| event.at),
        termination,
        roles: ThreeCushionRoles {
            cue: BallId::WHITE,
            object_a: BallId::YELLOW,
            object_b: BallId::RED,
        },
        events: events.into_boxed_slice(),
        final_states: Box::new([]),
        maximum_cue_ball_height: Inches::zero(),
        estimated_closest_second_object_clearance: None,
    }
}

fn execute_with_adjudication_parity(
    physics: &PhysicsProfile,
    layout: &ShotLayout,
    shot: &ThreeCushionShot,
    limit: ShotLimit,
) -> ThreeCushionResult {
    let full = execute_three_cushion(physics, layout, shot, limit).unwrap();
    let compact = execute_three_cushion_compact(physics, layout, shot, limit).unwrap();

    assert_eq!(compact.completion, full.completion);
    assert_eq!(compact.final_states, full.final_states);

    let facts = full.completion.summary.facts();
    let retained = OwnedShotResult {
        elapsed: full.completion.elapsed,
        termination: full.completion.termination.clone(),
        roles: ThreeCushionRoles {
            cue: BallId::WHITE,
            object_a: BallId::YELLOW,
            object_b: BallId::RED,
        },
        events: full.events.clone(),
        final_states: full.final_states.clone(),
        maximum_cue_ball_height: facts.maximum_cue_ball_height.clone(),
        estimated_closest_second_object_clearance: facts
            .estimated_closest_second_object_clearance
            .clone(),
    };
    assert_eq!(project_three_cushion(&retained), full.completion.summary);

    full
}

fn canonical_profile_with(
    alter: impl FnOnce(
        &mut BallSetPhysicsSpec,
        &mut OnTableMotionConfig,
        &mut BallBallCollisionConfig,
        &mut RailCollisionProfile,
    ),
) -> Result<PhysicsProfile, ShotSimulationError> {
    let baseline = PhysicsProfile::three_cushion_default();
    let mut ball = baseline.ball_set().clone();
    let mut motion = baseline.motion().clone();
    let mut rails = baseline.rails().clone();
    let mut collision = baseline.collision().clone();
    alter(&mut ball, &mut motion, &mut collision, &mut rails);

    PhysicsProfile::new(
        baseline.table().clone(),
        ball,
        motion,
        baseline.collision_model(),
        collision,
        baseline.rail_model(),
        rails,
    )
}

fn assert_invalid_profile(result: Result<PhysicsProfile, ShotSimulationError>) {
    assert!(matches!(
        result,
        Err(ShotSimulationError::InvalidPhysicsProfile(_))
    ));
}

#[test]
fn controls_reject_every_non_finite_scalar() {
    for controls in [
        [f64::NAN, 100.0, 0.0, 0.0, 0.0],
        [0.0, f64::INFINITY, 0.0, 0.0, 0.0],
        [0.0, 100.0, f64::NEG_INFINITY, 0.0, 0.0],
        [0.0, 100.0, 0.0, f64::NAN, 0.0],
        [0.0, 100.0, 0.0, 0.0, f64::INFINITY],
    ] {
        assert!(matches!(
            ShotControls::new(
                controls[0],
                controls[1],
                controls[2],
                controls[3],
                controls[4]
            ),
            Err(ShotSimulationError::NonFiniteInput(_))
        ));
    }
}

#[test]
fn controls_validate_radial_tip_bound_at_construction_and_preserve_offsets() {
    let inside_radius = 1.0 - 1e-9;
    let side_offset = 0.6 * inside_radius;
    let height_offset = -0.8 * inside_radius;
    let controls = ShotControls::new(25.0, 100.0, side_offset, height_offset, 0.0)
        .expect("a tip contact just inside the radial bound is valid");

    assert_eq!(controls.side_tip_offset(), side_offset);
    assert_eq!(controls.height_tip_offset(), height_offset);

    let outside_radius = 1.0 + 1e-9;
    let result = ShotControls::new(
        25.0,
        100.0,
        0.6 * outside_radius,
        -0.8 * outside_radius,
        0.0,
    );
    assert!(matches!(
        result,
        Err(ShotSimulationError::Shot(
            billiards::ShotError::CueTipContactOutsideBall { .. }
        ))
    ));
}

#[test]
fn physics_profile_rejects_out_of_domain_coefficients_before_execution() {
    let table = billiards::TableSpec::three_cushion_carom_10ft();
    let mut ball = table.default_ball_set_physics_spec();
    ball.radius = billiards::Inches::from_f64(-1.0);
    let conditions = billiards::PlayingConditions::heated_carom();
    assert!(matches!(
        PhysicsProfile::new(
            table,
            ball,
            billiards::human_tuned_preview_motion_config().applying_conditions(&conditions),
            billiards::CollisionModel::ThrowAware,
            billiards::BallBallCollisionConfig::human_tuned().applying_conditions(&conditions),
            billiards::RailModel::SpinAware,
            billiards::RailCollisionProfile::human_tuned().applying_conditions(&conditions),
        ),
        Err(ShotSimulationError::InvalidPhysicsProfile(_))
    ));
}

#[test]
fn physics_profile_accepts_ideal_ball_collision_restitution() {
    canonical_profile_with(|_, _, collision, _| {
        *collision = BallBallCollisionConfig::ideal();
    })
    .expect("ideal ball collision with normal restitution 1.0 is valid");
}

#[test]
fn physics_profile_accepts_unit_normal_restitution_for_every_rail() {
    canonical_profile_with(|_, _, _, rails| {
        for rail in [
            &mut rails.top,
            &mut rails.right,
            &mut rails.bottom,
            &mut rails.left,
        ] {
            rail.normal_restitution = Scale::from_f64(1.0);
        }
    })
    .expect("normal restitution 1.0 is valid for every rail");
}

#[test]
fn physics_profile_accepts_zero_airborne_restitution_but_rejects_one() {
    assert!(canonical_profile_with(|ball, _, _, _| {
        ball.airborne_table_contact.normal_restitution = Scale::zero();
    })
    .is_ok());

    assert_invalid_profile(canonical_profile_with(|ball, _, _, _| {
        ball.airborne_table_contact.normal_restitution = Scale::from_f64(1.0);
    }));
}

#[test]
fn physics_profile_rejects_zero_motion_rates() {
    let zero_sliding = canonical_profile_with(|_, motion, _, _| {
        motion.sliding_friction = SlidingFrictionModel::ConstantAcceleration {
            acceleration_magnitude: InchesPerSecondSq::new(Inches::zero()),
        };
    });
    let zero_spin = canonical_profile_with(|_, motion, _, _| {
        motion.spin_decay = SpinDecayModel::ConstantAngularDeceleration {
            angular_deceleration: RadiansPerSecondSq::zero(),
        };
    });
    let zero_rolling = canonical_profile_with(|_, motion, _, _| {
        motion.rolling_resistance = RollingResistanceModel::ConstantDeceleration {
            linear_deceleration: InchesPerSecondSq::new(Inches::zero()),
        };
    });

    for result in [zero_sliding, zero_spin, zero_rolling] {
        assert_invalid_profile(result);
    }
}

#[test]
fn physics_profile_rejects_a_rail_contact_height_ratio_above_one() {
    assert_invalid_profile(canonical_profile_with(|_, _, _, rails| {
        rails.top.effective_contact_height_ratio = Scale::from_f64(1.000_001);
    }));
}

#[test]
fn physics_profile_rejects_every_negative_phase_tolerance() {
    let negative_airborne_height = canonical_profile_with(|_, motion, _, _| {
        motion.phase.thresholds.airborne_height = Inches::from_f64(-1.0);
    });
    let negative_airborne_vertical_speed = canonical_profile_with(|_, motion, _, _| {
        motion.phase.thresholds.airborne_vertical_speed =
            InchesPerSecond::new(Inches::from_f64(-1.0));
    });
    let negative_rest_linear_speed = canonical_profile_with(|_, motion, _, _| {
        motion.phase.thresholds.rest_linear_speed = InchesPerSecond::new(Inches::from_f64(-1.0));
    });
    let negative_rest_angular_speed = canonical_profile_with(|_, motion, _, _| {
        motion.phase.thresholds.rest_angular_speed = RadiansPerSecond::new(-1.0);
    });
    let negative_no_slip_epsilon = canonical_profile_with(|_, motion, _, _| {
        motion.phase.sliding_to_rolling = SlidingToRollingModel::Thresholded {
            contact_speed_epsilon: InchesPerSecond::new(Inches::from_f64(-1.0)),
        };
    });

    for result in [
        negative_airborne_height,
        negative_airborne_vertical_speed,
        negative_rest_linear_speed,
        negative_rest_angular_speed,
        negative_no_slip_epsilon,
    ] {
        assert_invalid_profile(result);
    }
}

#[test]
fn canonical_layout_order_makes_execution_invariant_to_input_order() {
    let physics = PhysicsProfile::three_cushion_default();
    let white = SceneBall::resting(
        BallId::new(30),
        CaromBallRole::Cue,
        carom_position_from_diamonds(0.700, 1.000).unwrap(),
    );
    let yellow = SceneBall::resting(
        BallId::new(10),
        CaromBallRole::YellowCue,
        carom_position_from_diamonds(1.200, 2.100).unwrap(),
    );
    let red = SceneBall::resting(
        BallId::new(20),
        CaromBallRole::Red,
        carom_position_from_diamonds(0.850, 6.550).unwrap(),
    );
    let ordered = ShotLayout::new(&physics, [yellow.clone(), red.clone(), white.clone()]).unwrap();
    let reordered = ShotLayout::new(&physics, [white, yellow, red]).unwrap();
    assert_eq!(
        ordered
            .balls()
            .iter()
            .map(|ball| ball.id)
            .collect::<Vec<_>>(),
        [BallId::new(10), BallId::new(20), BallId::new(30)]
    );

    let shot = ThreeCushionShot::new(ThreeCushionShooter::Cue, fixture_controls());
    let first =
        execute_three_cushion(&physics, &ordered, &shot, ShotLimit::EventCount(24)).unwrap();
    let second =
        execute_three_cushion(&physics, &reordered, &shot, ShotLimit::EventCount(24)).unwrap();
    assert_eq!(first, second);
}

#[test]
fn simultaneous_corner_rail_contacts_are_coalesced_in_scheduler_order() {
    let physics = PhysicsProfile::three_cushion_default();
    let layout = ShotLayout::new(
        &physics,
        [
            SceneBall::resting(
                BallId::WHITE,
                CaromBallRole::Cue,
                carom_position_from_diamonds(3.5, 7.5).unwrap(),
            ),
            SceneBall::resting(
                BallId::YELLOW,
                CaromBallRole::YellowCue,
                carom_position_from_diamonds(1.0, 1.0).unwrap(),
            ),
            SceneBall::resting(
                BallId::RED,
                CaromBallRole::Red,
                carom_position_from_diamonds(2.0, 1.0).unwrap(),
            ),
        ],
    )
    .unwrap();
    let shot = ThreeCushionShot::new(
        ThreeCushionShooter::Cue,
        ShotControls::new(45.0, 100.0, 0.0, 0.0, 0.0).unwrap(),
    );

    let result = execute_three_cushion(&physics, &layout, &shot, ShotLimit::EventCount(2)).unwrap();

    assert_eq!(
        result.completion.termination,
        ShotTermination::EventLimitReached { limit: 2 }
    );
    assert!(matches!(
        result.completion.summary,
        ThreeCushionAdjudication::Indeterminate {
            reason: ThreeCushionIndeterminate::EventLimitReached { limit: 2 },
            ..
        }
    ));
    assert_eq!(result.events.len(), 1);
    let event = &result.events[0];
    assert_eq!(event.at, result.completion.elapsed);
    assert_eq!(
        event.effects.as_ref(),
        &[
            ResolvedEffect::BallRailContact {
                ball: BallId::WHITE,
                rail: Rail::Right,
            },
            ResolvedEffect::BallRailContact {
                ball: BallId::WHITE,
                rail: Rail::Top,
            },
        ]
    );
}

#[test]
fn either_cue_colored_ball_can_be_the_typed_shooter() {
    let physics = PhysicsProfile::three_cushion_default();
    let layout = fixture_layout();
    for shooter in [ThreeCushionShooter::Cue, ThreeCushionShooter::YellowCue] {
        let result = execute_three_cushion(
            &physics,
            &layout,
            &ThreeCushionShot::new(shooter, fixture_controls()),
            ShotLimit::EventCount(24),
        )
        .unwrap();
        assert_eq!(result.final_states.len(), 3);
        assert_eq!(
            result
                .final_states
                .iter()
                .map(|state| state.id)
                .collect::<Vec<_>>(),
            [BallId::WHITE, BallId::YELLOW, BallId::RED]
        );
    }
}

#[test]
fn event_limited_completion_matches_compact_and_retained_projection() {
    let physics = PhysicsProfile::three_cushion_default();
    let result = execute_with_adjudication_parity(
        &physics,
        &fixture_layout(),
        &ThreeCushionShot::new(ThreeCushionShooter::Cue, fixture_controls()),
        ShotLimit::EventCount(0),
    );
    assert_eq!(
        result.completion.termination,
        ShotTermination::EventLimitReached { limit: 0 }
    );
    assert!(matches!(
        result.completion.summary,
        ThreeCushionAdjudication::Indeterminate {
            reason: ThreeCushionIndeterminate::EventLimitReached { limit: 0 },
            ..
        }
    ));
}

#[test]
fn settled_completion_matches_compact_and_retained_projection() {
    let physics = PhysicsProfile::three_cushion_default();
    let shot = ThreeCushionShot::new(
        ThreeCushionShooter::Cue,
        ShotControls::new(0.0, 0.0, 0.0, 0.0, 0.0).unwrap(),
    );
    let result = execute_with_adjudication_parity(
        &physics,
        &fixture_layout(),
        &shot,
        ShotLimit::UntilSettled,
    );

    assert_eq!(result.completion.termination, ShotTermination::Settled);
    assert!(matches!(
        result.completion.summary,
        ThreeCushionAdjudication::Miss {
            reason: ThreeCushionMiss::MissingObjectContact,
            ..
        }
    ));
}

#[test]
fn unsupported_contact_completion_matches_compact_and_retained_projection() {
    let physics = canonical_profile_with(|_, _, collision, _| {
        collision.object_table_static_friction_coefficient = Scale::from_f64(0.1);
    })
    .unwrap();
    let radius = physics.ball_set().radius.as_f64();
    let object_y = 60.0;
    let contact_y = object_y - 3.0_f64.sqrt() * radius;
    let position = |x, y| Inches2::new(Inches::from_f64(x), Inches::from_f64(y));
    let layout = ShotLayout::new(
        &physics,
        [
            SceneBall::resting(
                BallId::WHITE,
                CaromBallRole::Cue,
                position(50.0, contact_y - 7.5),
            ),
            SceneBall::resting(
                BallId::YELLOW,
                CaromBallRole::YellowCue,
                position(50.0 - radius, object_y),
            ),
            SceneBall::resting(
                BallId::RED,
                CaromBallRole::Red,
                position(50.0 + radius, object_y),
            ),
        ],
    )
    .unwrap();
    let shot = ThreeCushionShot::new(
        ThreeCushionShooter::Cue,
        ShotControls::new(0.0, 100.0, 0.0, 0.0, 0.0).unwrap(),
    );

    let result =
        execute_with_adjudication_parity(&physics, &layout, &shot, ShotLimit::UntilSettled);

    assert!(
        matches!(
            result.completion.termination,
            ShotTermination::UnsupportedPhysics {
                reason: UnsupportedPhysicsReason::NonIdealSharedBallBallContact { .. }
            }
        ),
        "expected unsupported shared contact, got {:?}: {:#?}",
        result.completion.termination,
        result.events
    );
    assert!(matches!(
        result.completion.summary,
        ThreeCushionAdjudication::Indeterminate {
            reason: ThreeCushionIndeterminate::UnsupportedPhysics {
                reason: UnsupportedPhysicsReason::NonIdealSharedBallBallContact { .. }
            },
            ..
        }
    ));
}

#[test]
fn no_dsl_direct_execution_produces_a_verified_three_cushion_point() {
    let physics = PhysicsProfile::three_cushion_default();
    let layout = fixture_layout();
    let controls = ShotControls::new(
        196.391_792_039,
        237.947_968_822,
        -0.230_661_681,
        0.365_060_077,
        0.0,
    )
    .unwrap();
    let shot = ThreeCushionShot::new(ThreeCushionShooter::Cue, controls);
    let full =
        execute_with_adjudication_parity(&physics, &layout, &shot, ShotLimit::EventCount(64));

    let ThreeCushionAdjudication::Scored(facts) = &full.completion.summary else {
        panic!("verified direct fixture must score")
    };
    assert!(facts.object_a_touched());
    assert!(facts.object_b_touched());
    assert!(facts.three_cushions_touched());
    assert!(facts
        .estimated_closest_second_object_clearance
        .as_ref()
        .is_some_and(|clearance| clearance.as_f64() <= 1e-9));
}

#[test]
fn elevated_execution_tracks_jump_height_in_full_compact_and_projected_results() {
    let physics = PhysicsProfile::three_cushion_default();
    let layout = fixture_layout();
    let controls = ShotControls::new(
        196.391_792_039,
        237.947_968_822,
        -0.230_661_681,
        0.365_060_077,
        30.0,
    )
    .expect("elevated fixture controls are valid");
    let shot = ThreeCushionShot::new(ThreeCushionShooter::Cue, controls);
    let full =
        execute_with_adjudication_parity(&physics, &layout, &shot, ShotLimit::EventCount(64));
    let facts = full.completion.summary.facts();

    assert!(facts.maximum_cue_ball_height.as_f64() > THREE_CUSHION_MAX_CUE_BALL_HEIGHT_INCHES);
    assert!(!full.completion.summary.is_scored());
}

#[test]
fn repeated_cushions_count_and_completion_instant_cushion_does_not() {
    let result = owned(
        vec![
            instant(
                1.0,
                vec![ResolvedEffect::BallRailContact {
                    ball: BallId::WHITE,
                    rail: Rail::Top,
                }],
            ),
            instant(
                2.0,
                vec![
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Top,
                    },
                    ball_contact(BallId::WHITE, BallId::YELLOW),
                ],
            ),
            instant(
                3.0,
                vec![ResolvedEffect::BallRailContact {
                    ball: BallId::WHITE,
                    rail: Rail::Left,
                }],
            ),
            instant(
                4.0,
                vec![
                    ball_contact(BallId::WHITE, BallId::RED),
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Bottom,
                    },
                ],
            ),
        ],
        ShotTermination::Settled,
    );
    let ThreeCushionAdjudication::Scored(facts) = project_three_cushion(&result) else {
        panic!("three strictly earlier cushion onsets must score")
    };
    assert_eq!(facts.cushion_contacts_before_completion, 3);
    assert_eq!(
        facts.first_three_qualifying_cushions,
        [Some(Rail::Top), Some(Rail::Top), Some(Rail::Left)]
    );
    assert_eq!(
        facts.completion,
        Some(ContactInstant {
            event_index: 3,
            at: Seconds::new(4.0),
        })
    );
}

#[test]
fn cue_ball_height_limit_is_strict_for_the_entire_shot() {
    let mut boundary = owned(completed_point_events(), ShotTermination::Settled);
    boundary.maximum_cue_ball_height = Inches::from_f64(THREE_CUSHION_MAX_CUE_BALL_HEIGHT_INCHES);
    assert!(matches!(
        project_three_cushion(&boundary),
        ThreeCushionAdjudication::Scored(_)
    ));

    let mut jump = owned(completed_point_events(), ShotTermination::Settled);
    jump.maximum_cue_ball_height = Inches::from_f64(1.25);
    let ThreeCushionAdjudication::Miss { facts, reason } = project_three_cushion(&jump) else {
        panic!("any cue-ball height over the limit must invalidate the point")
    };
    assert_eq!(facts.maximum_cue_ball_height, Inches::from_f64(1.25));
    assert_eq!(
        reason,
        ThreeCushionMiss::CueBallHeightExceeded {
            limit: Inches::from_f64(THREE_CUSHION_MAX_CUE_BALL_HEIGHT_INCHES),
            observed: Inches::from_f64(1.25),
        }
    );
}

#[test]
fn cue_ball_height_violation_precedes_insufficient_cushions() {
    let mut jump = owned(
        vec![
            instant(
                1.0,
                vec![
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Top,
                    },
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Right,
                    },
                ],
            ),
            instant(
                2.0,
                vec![
                    ball_contact(BallId::WHITE, BallId::YELLOW),
                    ball_contact(BallId::WHITE, BallId::RED),
                ],
            ),
        ],
        ShotTermination::Settled,
    );
    jump.maximum_cue_ball_height = Inches::from_f64(1.25);

    assert!(matches!(
        project_three_cushion(&jump),
        ThreeCushionAdjudication::Miss {
            reason: ThreeCushionMiss::CueBallHeightExceeded { .. },
            ..
        }
    ));
}

#[test]
fn same_instant_third_cushion_is_not_before_completion() {
    let result = owned(
        vec![
            instant(
                1.0,
                vec![ResolvedEffect::BallRailContact {
                    ball: BallId::WHITE,
                    rail: Rail::Top,
                }],
            ),
            instant(
                2.0,
                vec![
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Right,
                    },
                    ball_contact(BallId::WHITE, BallId::YELLOW),
                ],
            ),
            instant(
                3.0,
                vec![
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Bottom,
                    },
                    ball_contact(BallId::WHITE, BallId::RED),
                ],
            ),
        ],
        ShotTermination::Settled,
    );
    assert!(matches!(
        project_three_cushion(&result),
        ThreeCushionAdjudication::Miss {
            reason: ThreeCushionMiss::InsufficientCushions {
                required: 3,
                observed: 2
            },
            ..
        }
    ));
}

#[test]
fn simultaneous_first_contacts_share_completion_without_invented_order() {
    let result = owned(
        vec![
            instant(
                1.0,
                vec![
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Top,
                    },
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Right,
                    },
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Bottom,
                    },
                ],
            ),
            instant(
                2.0,
                vec![
                    ball_contact(BallId::WHITE, BallId::YELLOW),
                    ball_contact(BallId::WHITE, BallId::RED),
                ],
            ),
        ],
        ShotTermination::Settled,
    );
    let ThreeCushionAdjudication::Scored(facts) = project_three_cushion(&result) else {
        panic!("simultaneous distinct object contacts complete the point")
    };
    assert_eq!(facts.object_a_first_contact, facts.object_b_first_contact);
    assert_eq!(facts.completion, facts.object_a_first_contact);
}

#[test]
fn incomplete_abnormal_termination_is_indeterminate_but_completed_point_is_final() {
    let incomplete = owned(Vec::new(), ShotTermination::EventLimitReached { limit: 0 });
    assert!(matches!(
        project_three_cushion(&incomplete),
        ThreeCushionAdjudication::Indeterminate {
            reason: ThreeCushionIndeterminate::EventLimitReached { limit: 0 },
            ..
        }
    ));

    let completed = owned(
        vec![
            instant(
                1.0,
                vec![
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Top,
                    },
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Right,
                    },
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Bottom,
                    },
                ],
            ),
            instant(
                2.0,
                vec![
                    ball_contact(BallId::WHITE, BallId::YELLOW),
                    ball_contact(BallId::WHITE, BallId::RED),
                ],
            ),
        ],
        ShotTermination::EventLimitReached { limit: 2 },
    );
    assert!(matches!(
        project_three_cushion(&completed),
        ThreeCushionAdjudication::Scored(_)
    ));
}

#[test]
fn unresolved_shared_object_contacts_cannot_score() {
    let reason = UnsupportedPhysicsReason::NonIdealSharedBallBallContact {
        balls: vec![BallId::WHITE, BallId::YELLOW, BallId::RED].into_boxed_slice(),
    };
    let result = owned(
        vec![
            instant(
                1.0,
                vec![
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Top,
                    },
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Right,
                    },
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Bottom,
                    },
                ],
            ),
            instant(
                2.0,
                vec![
                    ResolvedEffect::BallBallContact {
                        first: BallId::WHITE,
                        second: BallId::YELLOW,
                        resolution: BallBallContactResolution::SharedCoupledCausalityUnresolved,
                    },
                    ResolvedEffect::BallBallContact {
                        first: BallId::WHITE,
                        second: BallId::RED,
                        resolution: BallBallContactResolution::SharedCoupledCausalityUnresolved,
                    },
                    ResolvedEffect::UnsupportedContact {
                        reason: reason.clone(),
                    },
                ],
            ),
        ],
        ShotTermination::UnsupportedPhysics { reason },
    );

    let adjudication = project_three_cushion(&result);
    assert!(matches!(
        adjudication,
        ThreeCushionAdjudication::Indeterminate {
            reason: ThreeCushionIndeterminate::UnsupportedPhysics {
                reason: UnsupportedPhysicsReason::NonIdealSharedBallBallContact { .. }
            },
            ..
        }
    ));
    assert_eq!(adjudication.facts().object_a_first_contact, None);
    assert_eq!(adjudication.facts().object_b_first_contact, None);
    assert_eq!(adjudication.facts().completion, None);
}

#[test]
fn completion_at_the_same_instant_as_unsupported_physics_is_indeterminate() {
    let reason = UnsupportedPhysicsReason::NonIdealSharedBallBallContact {
        balls: vec![BallId::WHITE, BallId::YELLOW, BallId::RED].into_boxed_slice(),
    };
    let result = owned(
        vec![
            instant(
                1.0,
                vec![
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Top,
                    },
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Right,
                    },
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Bottom,
                    },
                ],
            ),
            instant(
                2.0,
                vec![
                    ball_contact(BallId::WHITE, BallId::YELLOW),
                    ball_contact(BallId::WHITE, BallId::RED),
                    ResolvedEffect::BallBallContact {
                        first: BallId::YELLOW,
                        second: BallId::RED,
                        resolution: BallBallContactResolution::SharedCoupledCausalityUnresolved,
                    },
                    ResolvedEffect::UnsupportedContact {
                        reason: reason.clone(),
                    },
                ],
            ),
        ],
        ShotTermination::UnsupportedPhysics { reason },
    );

    let adjudication = project_three_cushion(&result);
    assert!(matches!(
        adjudication,
        ThreeCushionAdjudication::Indeterminate {
            reason: ThreeCushionIndeterminate::UnsupportedPhysics {
                reason: UnsupportedPhysicsReason::NonIdealSharedBallBallContact { .. }
            },
            ..
        }
    ));
    assert_eq!(
        adjudication.facts().completion,
        Some(ContactInstant {
            event_index: 1,
            at: Seconds::new(2.0),
        })
    );
}

#[test]
fn a_valid_point_remains_scored_when_later_contact_is_unsupported() {
    let reason = UnsupportedPhysicsReason::NonIdealSharedBallBallContact {
        balls: vec![BallId::WHITE, BallId::YELLOW, BallId::RED].into_boxed_slice(),
    };
    let result = owned(
        vec![
            instant(
                1.0,
                vec![
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Top,
                    },
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Right,
                    },
                    ResolvedEffect::BallRailContact {
                        ball: BallId::WHITE,
                        rail: Rail::Bottom,
                    },
                ],
            ),
            instant(
                2.0,
                vec![
                    ball_contact(BallId::WHITE, BallId::YELLOW),
                    ball_contact(BallId::WHITE, BallId::RED),
                ],
            ),
            instant(
                3.0,
                vec![
                    ResolvedEffect::BallBallContact {
                        first: BallId::WHITE,
                        second: BallId::YELLOW,
                        resolution: BallBallContactResolution::SharedCoupledCausalityUnresolved,
                    },
                    ResolvedEffect::UnsupportedContact { reason },
                ],
            ),
        ],
        ShotTermination::UnsupportedPhysics {
            reason: UnsupportedPhysicsReason::NonIdealSharedBallBallContact {
                balls: vec![BallId::WHITE, BallId::YELLOW, BallId::RED].into_boxed_slice(),
            },
        },
    );

    let ThreeCushionAdjudication::Scored(facts) = project_three_cushion(&result) else {
        panic!("a completed point must remain final after a later unsupported contact")
    };
    assert_eq!(
        facts.completion,
        Some(ContactInstant {
            event_index: 1,
            at: Seconds::new(2.0),
        })
    );
}
