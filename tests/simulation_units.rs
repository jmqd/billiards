use billiards::{
    Angle, AngularVelocity3, Inches2, InchesPerSecond, InchesPerSecondSq, RadiansPerSecond,
    RadiansPerSecondSq, Seconds, Velocity2, TYPICAL_BALL_RADIUS,
};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

#[test]
fn linear_units_compose_into_distance_and_speed() {
    let distance = InchesPerSecond::new("12") * Seconds::new(0.25);
    let speed = InchesPerSecondSq::new("8") * Seconds::new(0.5);

    assert_close(distance.as_f64(), 3.0);
    assert_close(speed.as_f64(), 4.0);
}

#[test]
fn angular_units_convert_between_surface_speed_and_spin_rate() {
    let surface_speed = RadiansPerSecond::new(4.0) * TYPICAL_BALL_RADIUS.clone();
    let spin_rate = RadiansPerSecondSq::new(6.0) * Seconds::new(0.5);

    assert_close(surface_speed.as_f64(), 4.5);
    assert_close(spin_rate.as_f64(), 3.0);
}

#[test]
fn velocity2_round_trips_between_cartesian_and_polar_forms() {
    let angle = Angle::from_north(3.0, 4.0);
    let velocity = Velocity2::from_polar(InchesPerSecond::new("10"), angle);

    assert_close(velocity.x().as_f64(), 6.0);
    assert_close(velocity.y().as_f64(), 8.0);
    assert_close(velocity.speed().as_f64(), 10.0);
    assert_close(
        velocity
            .angle_from_north()
            .expect("velocity heading")
            .as_degrees(),
        angle.as_degrees(),
    );
}

#[test]
fn velocity2_times_time_yields_an_inches_vector() {
    let displacement = Velocity2::new("6", "8").displacement_over(Seconds::new(0.5));

    assert_close(displacement.x().as_f64(), 3.0);
    assert_close(displacement.y().as_f64(), 4.0);
    assert_close(displacement.magnitude().as_f64(), 5.0);
    assert_close(
        displacement
            .angle_from_north()
            .expect("displacement heading")
            .as_degrees(),
        Angle::from_north(3.0, 4.0).as_degrees(),
    );
}

#[test]
fn zero_vectors_have_no_heading() {
    assert!(Velocity2::zero().angle_from_north().is_none());
    assert!(Inches2::zero().angle_from_north().is_none());
}

#[test]
fn angular_velocity3_exposes_named_components() {
    let spin = AngularVelocity3::new(1.0, -2.0, 3.0);

    assert_close(spin.x().as_f64(), 1.0);
    assert_close(spin.y().as_f64(), -2.0);
    assert_close(spin.z().as_f64(), 3.0);
}

#[test]
fn inches_per_second_round_trips_with_miles_per_hour() {
    let speed = InchesPerSecond::from_mph(12.5);

    assert_close(speed.as_f64(), 220.0);
    assert_close(speed.as_mph(), 12.5);
}
