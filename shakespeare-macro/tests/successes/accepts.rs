//! Tests that sending messages still works for actors that do *not* implement `Accepts`
use std::sync::Arc;

#[shakespeare::actor]
mod Actor {

	struct ActorState {
		state: usize,
	}

	#[shakespeare::performance(canonical)]
	impl BasicRole for ActorState {
		fn increase(&mut self, n: usize) {
			self.state += n;
		}

		fn decrease(&mut self, n: usize) {
			self.state -= n
		}

		fn read(&self) -> usize {
			self.state
		}
	}
}

#[tokio::test]
async fn main() {
	let state = ActorState { state: 0 };
	let message_handle: Arc<dyn BasicRole> = Actor::start(state).message_handle;

	message_handle.increase(5).await.unwrap();
	message_handle.decrease(3).await.unwrap();

	assert_eq!(message_handle.read().await.unwrap(), 2);
}
