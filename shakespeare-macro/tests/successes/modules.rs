use std::sync::Arc;

fn do_a_thing() {}

pub mod actor {
	use shakespeare::ActorHandles;

	#[shakespeare::actor]
	pub mod FooActor {
		pub struct FooState {}
		#[shakespeare::performance]
		impl super::role::RoleTrait for FooState {}
	}

	pub(crate) fn new(state: FooState) -> ActorHandles<FooActor> {
		// Actor::start is always private for now
		FooActor::start(state)
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
	impl super::role::RoleTrait for super::actor::FooState {
		async fn handler(&mut self) {
			use super::do_a_thing;
			do_a_thing();
		}
	}
}

#[tokio::test]
pub async fn main() {
	use actor::FooState;
	use role::RoleTrait;
	use shakespeare::ActorHandles;

	let ActorHandles { message_handle, .. } = self::actor::new(FooState {});
	let ptr: Arc<dyn RoleTrait> = message_handle;
	ptr.handler().await.unwrap();
}
