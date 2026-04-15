use billiards::{
    collide_ball_rail_on_table, AngularVelocity3, BallState, Inches, Inches2, OnTableBallState,
    Rail, RailModel, Velocity2,
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
