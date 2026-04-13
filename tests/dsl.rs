use billiards::dsl::{
    CoordinateAxis, DslBuildError, DslError, DslParseError, RailSide, parse_dsl,
    parse_dsl_to_game_state,
};

fn assert_parse_error(input: &str) {
    let err = parse_dsl_to_game_state(input).expect_err("expected parse failure");
    assert!(matches!(err, DslError::Parse(_)), "unexpected error: {err}");
}

#[test]
fn parse_dsl_returns_a_crate_owned_error_with_a_byte_offset() {
    let err = parse_dsl("ball cue nope").expect_err("expected parse failure");

    assert_eq!(
        err,
        DslParseError {
            message: "invalid DSL".to_string(),
            offset: 9,
        }
    );
}

#[test]
fn rejects_alias_values_on_the_next_line() {
    assert_parse_error("pos spot =\ncenter");
}

#[test]
fn rejects_multiline_coordinates() {
    assert_parse_error("ball cue at (2,\n4)");
}

#[test]
fn rejects_coordinates_outside_the_table_bounds() {
    let err = parse_dsl_to_game_state("ball cue at (4.01, 2)").expect_err("expected build failure");

    assert!(matches!(
        err,
        DslError::Build(DslBuildError::CoordinateOutOfRange {
            axis: CoordinateAxis::X,
            value,
            min: 0.0,
            max: 4.0,
        }) if (value - 4.01).abs() < f64::EPSILON
    ));
}

#[test]
fn rejects_frozen_coordinates_past_the_end_of_a_rail() {
    let err = parse_dsl_to_game_state("ball cue frozen top (4.01)")
        .expect_err("expected build failure");

    assert!(matches!(
        err,
        DslError::Build(DslBuildError::FrozenCoordinateOutOfRange {
            rail: RailSide::Top,
            value,
            min: 0.0,
            max: 4.0,
        }) if (value - 4.01).abs() < f64::EPSILON
    ));
}
