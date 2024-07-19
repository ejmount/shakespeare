use std::sync::Arc;

fn do_a_thing() {}

pub mod actor {
	#[shakespeare::actor]
	pub mod FooActor {
		pub struct FooState {}
		#[shakespeare::performance()]
		impl crate::successes::modules::role::RoleTrait for FooState {}
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
	impl crate::successes::modules::role::RoleTrait for crate::successes::modules::actor::FooState {
		async fn handler(&mut self) {
			use crate::successes::modules::do_a_thing;
			do_a_thing();
		}
	}
}

#[tokio::test]
pub async fn main() {
	use shakespeare::ActorSpawn;

	use crate::successes::modules::actor::FooState;
	use crate::successes::modules::role::RoleTrait;

	let ActorSpawn { msg_handle, .. } =
		crate::successes::modules::actor::FooActor::start(FooState {});
	let ptr: Arc<dyn RoleTrait> = msg_handle;
	ptr.handler().await.unwrap();
}
