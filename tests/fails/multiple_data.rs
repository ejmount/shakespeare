use shakespeare::actor;

#[actor]
mod actor {
	struct A {}
	enum B {
		Foo,
	}
	enum C {
		Bar,
	}
}

fn main() {}
