use std::collections::HashMap;

use billiards::dsl::{BallRef, DslScenario, ScenarioShot};
use billiards::{
    advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table, rack_9_ball, Ball,
    BallSetPhysicsSpec, BallSpec, BallType, CollisionModel, CueStrikeConfig, CueTipContact,
    GameState, Inches, InchesPerSecond, InchesPerSecondSq, MotionPhaseConfig,
    MotionTransitionConfig, NBallSystemEvent, NBallSystemState, OnTableMotionConfig, Position,
    RadiansPerSecondSq, RailModel, RollingResistanceModel, Scale, Shot, SlidingFrictionModel,
    SpinDecayModel, TableSpec,
};

fn motion_config() -> OnTableMotionConfig {
    MotionTransitionConfig {
        phase: MotionPhaseConfig::default(),
        sliding_friction: SlidingFrictionModel::ConstantAcceleration {
            acceleration_magnitude: InchesPerSecondSq::new("15"),
        },
        spin_decay: SpinDecayModel::ConstantAngularDeceleration {
            angular_deceleration: RadiansPerSecondSq::new(10.9),
        },
        rolling_resistance: RollingResistanceModel::ConstantDeceleration {
            linear_deceleration: InchesPerSecondSq::new("5"),
        },
    }
}

fn default_cue_strike() -> CueStrikeConfig {
    CueStrikeConfig::new(Scale::from_f64(1.0), Scale::from_f64(0.1))
        .expect("default cue-strike config should validate")
}

#[test]
fn a_second_diamond_head_rail_nine_ball_break_with_slight_draw_opens_the_rack() {
    let table = TableSpec::default();
    let rack = rack_9_ball();
    let one_ball = rack
        .iter()
        .find(|ball| ball.ty == BallType::One)
        .expect("rack_9_ball should include the 1-ball");

    let mut cue_position = Position::new(2u8, 8u8);
    cue_position.shift_vertically_inches(Inches::from_f64(-4.0));
    cue_position.resolve_shifts(&table);

    let shot = Shot::new(
        cue_position.angle_to(&one_ball.position),
        InchesPerSecond::from_mph(20.0),
        CueTipContact::new(Scale::zero(), Scale::from_f64(-0.1))
            .expect("slight draw tip contact should validate"),
    )
    .expect("break shot should validate");

    let mut balls = Vec::with_capacity(rack.len() + 1);
    balls.push(Ball {
        ty: BallType::Cue,
        position: cue_position,
        spec: BallSpec::default(),
    });
    balls.extend(rack);

    let scenario = DslScenario {
        game_state: GameState::with_balls(table, balls),
        shot: Some(ScenarioShot {
            ball_ref: BallRef::Cue,
            ball: BallType::Cue,
            shot,
            cue_strike: default_cue_strike(),
        }),
        ball_ball_configs: HashMap::new(),
        rail_responses: HashMap::new(),
        rail_profiles: HashMap::new(),
        simulations: HashMap::new(),
    };

    let ball_set = BallSetPhysicsSpec::default();
    let mut states = scenario
        .initial_shot_system_states_on_table(&ball_set)
        .expect("break scenario should seed initial states")
        .expect("break scenario should include a shot")
        .into_iter()
        .map(NBallSystemState::from)
        .collect::<Vec<_>>();
    let mut events = Vec::new();
    let mut elapsed = 0.0;
    while events.len() < 10 && elapsed < 0.5 {
        let advance = advance_to_next_n_ball_system_event_with_rails_and_pockets_on_table(
            &states,
            &ball_set,
            &scenario.game_state.table_spec,
            &motion_config(),
            CollisionModel::ThrowAware,
            RailModel::SpinAware,
        );
        let Some(event) = advance.event else {
            break;
        };
        elapsed += advance.elapsed.as_f64();
        events.push(event);
        states = advance.states;
    }

    let first_ball_collision = events
        .iter()
        .find_map(|event| match event {
            NBallSystemEvent::BallBallCollision {
                first_ball_index,
                second_ball_index,
                ..
            } => Some((*first_ball_index, *second_ball_index)),
            _ => None,
        })
        .expect("break should include at least one ball-ball collision");
    let ball_ball_collision_count = events
        .iter()
        .filter(|event| matches!(event, NBallSystemEvent::BallBallCollision { .. }))
        .count();
    let moved_or_pocketed_ball_count = scenario
        .game_state
        .balls()
        .iter()
        .zip(&states)
        .filter(|(ball, final_state)| match final_state {
            NBallSystemState::Pocketed { .. } => true,
            NBallSystemState::OnTable(state) => {
                let final_position = state
                    .as_ball_state()
                    .projected_position(&scenario.game_state.table_spec);
                let moved = ball
                    .position
                    .displacement(&final_position)
                    .absolute_distance();
                scenario
                    .game_state
                    .table_spec
                    .diamond_to_inches(moved)
                    .as_f64()
                    > 0.01
            }
        })
        .count();

    assert_eq!(first_ball_collision, (0, 1));
    assert!(
        moved_or_pocketed_ball_count >= 6,
        "expected the opening break to set much of the rack in motion, got only {moved_or_pocketed_ball_count} moving or pocketed balls"
    );
    assert!(
        ball_ball_collision_count >= 6,
        "expected a busy opening break spread, got only {ball_ball_collision_count} collisions"
    );
}
