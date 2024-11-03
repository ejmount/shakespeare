use std::any::Any;
use std::sync::Arc;

use shakespeare::{actor, send_stream_to, ActorOutcome, ActorSpawn};

#[actor]
pub mod CounterActor {
	#[derive(Default)]
	pub struct ActorState {
		count: usize,
	}
	#[performance(canonical)]
	impl Counter for Counting {
		fn sum(&mut self, val: usize) {
			self.count += val;
		}
	}

	fn stop(self) -> usize {
		self.count
	}
	fn catch(self, _: Box<dyn Any + Send>) {}
}

#[tokio::test]
async fn main() {
	let ActorSpawn {
		msg_handle,
		join_handle,
		..
	} = CounterActor::start(ActorState::default());

	let counter: Arc<dyn Counter> = msg_handle;

	let numbers = futures::stream::iter(0..10);
	send_stream_to(numbers, counter);

	assert_eq!(join_handle.await, ActorOutcome::Exit(45));
}
