#[test]
fn given_game_state_internals_when_accessing_private_fields_then_the_code_does_not_compile() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/private_game_state_fields.rs");
}

#[test]
fn given_position_shift_internals_when_accessing_private_fields_then_the_code_does_not_compile() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/private_position_shift_fields.rs");
}
