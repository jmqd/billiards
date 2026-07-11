use billiards::shot_simulation::{
    carom_position_from_diamonds, execute_three_cushion, execute_three_cushion_compact,
    project_three_cushion, BallBallContactResolution, BallId, CaromBallRole, ContactInstant,
    OwnedShotResult, PhysicsProfile, ResolvedEffect, ResolvedEvent, SceneBall, ShotControls,
    ShotLayout, ShotLimit, ShotSimulationError, ShotTermination, ThreeCushionAdjudication,
    ThreeCushionIndeterminate, ThreeCushionMiss, ThreeCushionRoles, ThreeCushionShooter,
    ThreeCushionShot,
};
use billiards::{Rail, Seconds};

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
    }
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
fn event_limit_is_an_explicit_indeterminate_termination() {
    let result = execute_three_cushion(
        &PhysicsProfile::three_cushion_default(),
        &fixture_layout(),
        &ThreeCushionShot::new(ThreeCushionShooter::Cue, fixture_controls()),
        ShotLimit::EventCount(0),
    )
    .unwrap();
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
fn compact_and_owned_fact_projection_are_exactly_equal() {
    let physics = PhysicsProfile::three_cushion_default();
    let layout = fixture_layout();
    let shot = ThreeCushionShot::new(ThreeCushionShooter::Cue, fixture_controls());
    let full = execute_three_cushion(&physics, &layout, &shot, ShotLimit::EventCount(24)).unwrap();
    let compact =
        execute_three_cushion_compact(&physics, &layout, &shot, ShotLimit::EventCount(24)).unwrap();

    assert_eq!(compact.completion, full.completion);
    assert_eq!(compact.final_states, full.final_states);
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
    let full = execute_three_cushion(&physics, &layout, &shot, ShotLimit::EventCount(64)).unwrap();
    let compact =
        execute_three_cushion_compact(&physics, &layout, &shot, ShotLimit::EventCount(64)).unwrap();

    let ThreeCushionAdjudication::Scored(facts) = &full.completion.summary else {
        panic!("verified direct fixture must score")
    };
    assert!(facts.object_a_first_contact.is_some());
    assert!(facts.object_b_first_contact.is_some());
    assert!(facts.cushion_contacts_before_completion >= 3);
    assert_eq!(compact.completion, full.completion);
    assert_eq!(compact.final_states, full.final_states);
    assert!(!full.events.is_empty());
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
