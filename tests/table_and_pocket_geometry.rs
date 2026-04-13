use bigdecimal::ToPrimitive;
use billiards::{Angle, Pocket, PocketType, Rail, TableSpec, CENTER_SPOT};

fn angle_degrees(angle: Angle) -> f64 {
    angle.to_string().parse().expect("angle degrees")
}

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
}

#[test]
fn given_a_brunswick_gc4_table_when_constructed_then_the_standard_lengths_and_pocket_openings_are_expressed_in_diamonds() {
    let table = TableSpec::brunswick_gc4_9ft();

    assert_close(
        table.diamond_length.magnitude.to_f64().expect("diamond length"),
        12.5,
    );
    assert_close(
        table.cushion_diamond_buffer
            .magnitude
            .to_f64()
            .expect("cushion buffer"),
        0.295,
    );

    match table.pockets[0].ty {
        PocketType::Corner => {}
        PocketType::Side => panic!("expected a corner pocket"),
    }
    assert_close(
        table.pockets[0].depth.magnitude.to_f64().expect("corner depth"),
        0.112,
    );
    assert_close(
        table.pockets[0].width.magnitude.to_f64().expect("corner width"),
        0.36,
    );

    match table.pockets[1].ty {
        PocketType::Side => {}
        PocketType::Corner => panic!("expected a side pocket"),
    }
    assert_close(
        table.pockets[1].width.magnitude.to_f64().expect("side width"),
        0.4,
    );
}

#[test]
fn given_the_center_spot_when_aiming_at_the_side_pockets_then_the_angles_follow_the_table_compass() {
    assert_close(angle_degrees(CENTER_SPOT.angle_to_pocket(Pocket::CenterRight)), 90.0);
    assert_close(angle_degrees(CENTER_SPOT.angle_to_pocket(Pocket::CenterLeft)), 270.0);
    assert_close(angle_degrees(CENTER_SPOT.angle_from_pocket(Pocket::CenterRight)), 270.0);
}

#[test]
fn given_each_rail_when_querying_its_origin_and_orientation_then_the_values_match_the_table_axes() {
    let top = Rail::Top;
    let right = Rail::Right;
    let bottom = Rail::Bottom;
    let left = Rail::Left;

    assert_eq!(top.rail_origin().x.magnitude.to_f64().expect("top x"), 0.0);
    assert_eq!(top.rail_origin().y.magnitude.to_f64().expect("top y"), 8.0);
    assert!(top.is_horizontal());
    assert!(!top.is_vertical());

    assert_eq!(right.rail_origin().x.magnitude.to_f64().expect("right x"), 4.0);
    assert_eq!(right.rail_origin().y.magnitude.to_f64().expect("right y"), 0.0);
    assert!(right.is_vertical());
    assert!(!right.is_horizontal());

    assert_eq!(bottom.rail_origin().x.magnitude.to_f64().expect("bottom x"), 0.0);
    assert_eq!(bottom.rail_origin().y.magnitude.to_f64().expect("bottom y"), 0.0);
    assert!(bottom.is_horizontal());
    assert!(!bottom.is_vertical());

    assert_eq!(left.rail_origin().x.magnitude.to_f64().expect("left x"), 0.0);
    assert_eq!(left.rail_origin().y.magnitude.to_f64().expect("left y"), 0.0);
    assert!(left.is_vertical());
    assert!(!left.is_horizontal());
}
