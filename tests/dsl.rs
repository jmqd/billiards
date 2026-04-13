use billiards::dsl::{parse_dsl_to_game_state, DslError};

fn assert_parse_error(input: &str) {
    let err = parse_dsl_to_game_state(input).expect_err("expected parse failure");
    assert!(matches!(err, DslError::Parse(_)), "unexpected error: {err}");
}

#[test]
fn rejects_alias_values_on_the_next_line() {
    assert_parse_error("pos spot =\ncenter");
}

#[test]
fn rejects_multiline_coordinates() {
    assert_parse_error("ball cue at (2,\n4)");
}
