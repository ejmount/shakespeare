#[test]
fn ux() {
	let t = trybuild::TestCases::new();
	//t.pass("tests/successes/basic.rs");
	//t.pass("tests/successes/modules.rs");
	t.compile_fail("tests/fails/*.rs");
}

mod successes;
