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
	impl Counting for Counting {
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
	let ActorSpawn {
		msg_handle,
		join_handle,
		..
	} = Counter::start(ActorState::default());

	let counting: Arc<dyn Counting> = msg_handle;

	let numbers = futures::stream::iter(0..10);
	add_stream(counting, numbers);

	assert!(join_handle.await == ActorOutcome::Exit(45));
}
