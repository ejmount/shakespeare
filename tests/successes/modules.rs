use std::sync::Arc;

use crate::successes::modules::role::RoleTrait;

fn do_a_thing() {}

pub mod actor {
	#[shakespeare::actor]
	pub mod FooActor {
		pub struct Foo {}
		#[shakespeare::performance()]
		impl crate::successes::modules::role::RoleTrait for Foo {}
	}
}

pub mod role {
	#[shakespeare::role]
	pub trait RoleTrait {
		fn handler(&mut self);
	}
}

pub mod perf {
	#[shakespeare::performance]
	impl crate::successes::modules::role::RoleTrait for crate::successes::modules::actor::Foo {
		async fn handler(&mut self) {
			crate::successes::modules::do_a_thing();
		}
	}
}

#[tokio::test]
pub async fn main() {
	use crate::successes::modules::actor::FooActor;
	fn cast(a: Arc<FooActor>) -> Arc<dyn RoleTrait> {
		a
	}
	use actor::Foo;
	use shakespeare::ActorSpawn;
	let ActorSpawn { actor, .. } = crate::successes::modules::actor::Foo::start(Foo {});
	let ptr = cast(actor);
	ptr.handler().await.unwrap();
}
