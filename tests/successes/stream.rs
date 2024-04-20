use std::any::Any;
use std::sync::Arc;

use shakespeare::{actor, add_stream, ActorOutcome, ActorSpawn};

#[actor]
pub mod Counter {
	#[derive(Default)]
	pub struct ActorState {
		count: usize,
	}
	#[performance(canonical)]
	impl Counting for ActorState {
		fn sum(&mut self, val: usize) {
			self.count += val;
		}
	}

	fn stop(state: ActorState) -> usize {
		state.count
	}
	fn catch(_: Box<dyn Any + Send>) {}
}

#[tokio::test]
async fn main() {
	let ActorSpawn { actor, handle, .. } = ActorState::start(ActorState::default());

	let counting: Arc<dyn Counting> = actor;

	let numbers = futures::stream::iter(0..10);
	add_stream(counting, numbers);

	assert!(handle.await == ActorOutcome::Exit(45));
}
