#[test]
fn ux() {
	if option_env!("TOOLCHAIN").map_or(false, |s| s.contains("stable")) {
		let t = trybuild::TestCases::new();
		//t.pass("tests/successes/basic.rs");
		//t.pass("tests/successes/modules.rs");
		t.compile_fail("tests/fails/empty.rs");
		t.compile_fail("tests/fails/missing_data.rs");
		t.compile_fail("tests/fails/multiple_data.rs");
	}
}

mod successes;
