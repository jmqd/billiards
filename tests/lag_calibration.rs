use billiards::dsl::parse_dsl_to_scenario;
use billiards::{
    human_tuned_preview_motion_config, BallBallCollisionConfig, BallSetPhysicsSpec, CollisionModel,
    Diamond, NBallSystemEvent, Rail, RailCollisionProfile, RailModel, TableSpec,
    TYPICAL_BALL_RADIUS,
};

const DR_DAVE_TP_B6_RAIL_REBOUND_SPEED_RATIO: f64 = 0.7;
const DR_DAVE_TP_B6_ROLLING_RESISTANCE_COEFFICIENT: f64 = 0.01;
const DR_DAVE_TP_B6_SLIDING_FRICTION_COEFFICIENT: f64 = 0.2;
const GRAVITY_IPS2: f64 = 386.088_582_677_165_35;

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta <= tolerance,
        "expected {expected} +/- {tolerance}, got {actual} (delta {delta})"
    );
}

fn dr_dave_tp_b6_travel_until_rest(
    launch_speed_ips: f64,
    first_rail_distance_inches: f64,
    rail_to_rail_distance_inches: f64,
) -> f64 {
    let rolling_deceleration = DR_DAVE_TP_B6_ROLLING_RESISTANCE_COEFFICIENT * GRAVITY_IPS2;
    let sliding_deceleration = DR_DAVE_TP_B6_SLIDING_FRICTION_COEFFICIENT * GRAVITY_IPS2;
    let rolling_stop_distance = |speed: f64| speed * speed / (2.0 * rolling_deceleration);
    let rolling_speed_after_distance = |speed: f64, distance: f64| {
        (speed * speed - 2.0 * rolling_deceleration * distance)
            .max(0.0)
            .sqrt()
    };
    // TP B.6 assumes a rolling ball leaves the cushion approximately stunned; the post-rebound
    // stun/skid distance and 5/7 speed ratio are from TP 4.1 / TP B.6.
    let post_rail_skid_distance = |speed: f64| 12.0 * speed * speed / (49.0 * sliding_deceleration);
    let sliding_speed_after_distance = |speed: f64, distance: f64| {
        (speed * speed - 2.0 * sliding_deceleration * distance)
            .max(0.0)
            .sqrt()
    };

    let mut distance = 0.0;
    let mut next_rail_distance = first_rail_distance_inches;
    let mut speed = launch_speed_ips;
    let mut rolling = true;

    while speed > f64::EPSILON {
        let distance_to_next_rail = next_rail_distance - distance;
        if rolling {
            let stop_distance = rolling_stop_distance(speed);
            if stop_distance < distance_to_next_rail {
                return distance + stop_distance;
            }

            speed = rolling_speed_after_distance(speed, distance_to_next_rail)
                * DR_DAVE_TP_B6_RAIL_REBOUND_SPEED_RATIO;
            distance = next_rail_distance;
            next_rail_distance += rail_to_rail_distance_inches;
            rolling = false;
        } else {
            let skid_distance = post_rail_skid_distance(speed);
            if skid_distance < distance_to_next_rail {
                distance += skid_distance;
                speed *= 5.0 / 7.0;
                rolling = true;
            } else {
                speed = sliding_speed_after_distance(speed, distance_to_next_rail)
                    * DR_DAVE_TP_B6_RAIL_REBOUND_SPEED_RATIO;
                distance = next_rail_distance;
                next_rail_distance += rail_to_rail_distance_inches;
                rolling = false;
            }
        }
    }

    distance
}

fn solve_launch_speed_for_dr_dave_tp_b6_travel(
    target_distance_inches: f64,
    first_rail_distance_inches: f64,
    rail_to_rail_distance_inches: f64,
) -> f64 {
    let mut low = 0.0;
    let mut high = 200.0;

    for _ in 0..100 {
        let mid = (low + high) / 2.0;
        let distance = dr_dave_tp_b6_travel_until_rest(
            mid,
            first_rail_distance_inches,
            rail_to_rail_distance_inches,
        );
        if distance < target_distance_inches {
            low = mid;
        } else {
            high = mid;
        }
    }

    (low + high) / 2.0
}

fn second_diamond_lag_geometry() -> (f64, f64, f64) {
    let table = TableSpec::default();
    let radius = TYPICAL_BALL_RADIUS.as_f64();
    let start_y = table.diamond_to_inches(Diamond::from("2")).as_f64();
    let top_rail_center_y = table.diamond_to_inches(Diamond::eight()).as_f64() - radius;
    let bottom_rail_center_y = radius;
    let first_rail_distance = top_rail_center_y - start_y;
    let rail_to_rail_distance = top_rail_center_y - bottom_rail_center_y;

    (
        first_rail_distance,
        rail_to_rail_distance,
        first_rail_distance + rail_to_rail_distance,
    )
}

fn adjusted_dr_dave_tp_b6_second_diamond_lag_speed_ips() -> f64 {
    let (first_rail_distance, rail_to_rail_distance, target_distance) =
        second_diamond_lag_geometry();
    solve_launch_speed_for_dr_dave_tp_b6_travel(
        target_distance,
        first_rail_distance,
        rail_to_rail_distance,
    )
}

fn rolling_lag_reaches_second_cushion(launch_speed_ips: f64) -> bool {
    let dsl = format!(
        "table brunswick_gc4_9ft\n\
         ball cue at (2.0, 2.0)\n\
         cue_strike(default).mass_ratio(1.0).energy_loss(0.1)\n\
         shot(cue).heading(0deg).speed({launch_speed_ips}ips).tip(side: 0.0R, height: 0.4R).using(default)\n"
    );
    let scenario = parse_dsl_to_scenario(&dsl).expect("lag calibration scenario should build");
    let simulation = scenario
        .simulate_shot_system_with_physics_on_table_until_rest(
            &BallSetPhysicsSpec::default(),
            &human_tuned_preview_motion_config(),
            CollisionModel::ThrowAware,
            &BallBallCollisionConfig::human_tuned(),
            RailModel::SpinAware,
            &RailCollisionProfile::human_tuned(),
        )
        .expect("lag calibration simulation should run")
        .expect("lag calibration scenario should contain a shot");

    simulation.events.iter().any(|event| {
        matches!(
            event,
            NBallSystemEvent::BallRailImpact { ball_index: 0, impact }
                if impact.rail == Rail::Bottom
        )
    })
}

#[test]
fn dr_dave_tp_b6_lag_speed_is_adjusted_for_starting_at_the_second_diamond() {
    let speed = adjusted_dr_dave_tp_b6_second_diamond_lag_speed_ips();

    // TP B.6 gives 3.465 mph ≈ 61 ips for a full two-table-length rolling lag. Starting on the
    // second diamond shortens the first leg, so the adjusted second-cushion benchmark is lower but
    // still close.
    assert_close(speed, 58.75, 0.05);
}

#[test]
fn human_tuned_spin_aware_rails_match_the_adjusted_rolling_lag_benchmark() {
    let target_speed = adjusted_dr_dave_tp_b6_second_diamond_lag_speed_ips();

    assert!(
        !rolling_lag_reaches_second_cushion(target_speed - 0.25),
        "a rolling lag 0.25 ips below the adjusted TP B.6 target should die before the second cushion"
    );
    assert!(
        rolling_lag_reaches_second_cushion(target_speed + 0.25),
        "a rolling lag 0.25 ips above the adjusted TP B.6 target should reach the second cushion"
    );
}
