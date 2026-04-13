#[test]
fn given_two_lengths_when_multiplying_them_then_the_code_does_not_compile() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/inches_times_inches.rs");
}
