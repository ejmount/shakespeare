use std::sync::Arc;

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
	use shakespeare::ActorSpawn;
	let ActorSpawn { msg_handle, .. } =
	let ptr: Arc<dyn RoleTrait> = msg_handle;
	ptr.handler().await.unwrap();
}
