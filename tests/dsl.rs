use bigdecimal::ToPrimitive;
use billiards::dsl::{
    CoordinateAxis, DslBuildError, DslError, DslParseError, RailSide, parse_dsl,
    parse_dsl_to_game_state,
};
use billiards::BallType;

fn assert_parse_error(input: &str) {
    let err = parse_dsl_to_game_state(input).expect_err("expected parse failure");
    assert!(matches!(err, DslError::Parse(_)), "unexpected error: {err}");
}

fn assert_close(actual: f64, expected: f64) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-9,
        "expected {expected}, got {actual} (delta {delta})"
    );
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
fn given_comments_blank_lines_aliases_and_frozen_balls_when_building_then_positions_match_table_space() {
    let state = parse_dsl_to_game_state(
        "# comment\n\n\
         pos hanger = (3.93, 7.93)\n\
         ball cue at center\n\
         ball nine at hanger\n\
         ball eight frozen left (6.0)\n",
    )
    .expect("expected DSL to build");

    let cue = state.select_ball(BallType::Cue).expect("cue ball");
    let nine = state.select_ball(BallType::Nine).expect("nine ball");
    let eight = state.select_ball(BallType::Eight).expect("eight ball");

    assert_close(cue.position.x.magnitude.to_f64().expect("cue x"), 2.0);
    assert_close(cue.position.y.magnitude.to_f64().expect("cue y"), 4.0);
    assert_close(nine.position.x.magnitude.to_f64().expect("nine x"), 3.93);
    assert_close(nine.position.y.magnitude.to_f64().expect("nine y"), 7.93);
    assert_close(eight.position.x.magnitude.to_f64().expect("eight x"), 0.09);
    assert_close(eight.position.y.magnitude.to_f64().expect("eight y"), 6.0);
}

#[test]
fn given_an_invalid_second_statement_when_parsing_then_the_error_offset_points_at_the_bad_token() {
    let err = parse_dsl("ball cue at center\nball nine nope").expect_err("expected parse failure");

    assert_eq!(
        err,
        DslParseError {
            message: "invalid DSL".to_string(),
            offset: 29,
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
