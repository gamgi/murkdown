#[test]
fn cli_tests() {
    trycmd::TestCases::new().case("tests/cli/*.toml");
}

#[test]
fn markdown_tests() {
    trycmd::TestCases::new().case("tests/markdown/*.toml");
}
