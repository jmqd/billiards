use bigdecimal::ToPrimitive;
use billiards::{Inches, OPTIMAL_PACKING_RADIUS_SHIFT, TYPICAL_BALL_RADIUS};

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

#[test]
fn given_the_optimal_packing_radius_shift_when_applied_to_a_ball_radius_then_it_scales_by_sqrt_three(
) {
    let shifted_radius: Inches = TYPICAL_BALL_RADIUS.clone() * OPTIMAL_PACKING_RADIUS_SHIFT.clone();

    assert_close(
        OPTIMAL_PACKING_RADIUS_SHIFT
            .magnitude
            .to_f64()
            .expect("shift magnitude"),
        3.0_f64.sqrt(),
    );
    assert_close(
        shifted_radius.magnitude.to_f64().expect("shifted radius"),
        1.125 * 3.0_f64.sqrt(),
    );
}
