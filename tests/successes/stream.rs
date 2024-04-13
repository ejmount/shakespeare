use shakespeare::{actor, add_stream, ActorSpawn};

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
}

#[tokio::test]
async fn main() {
	let ActorSpawn {
		actor, join_handle, ..
	} = ActorState::start(ActorState::default());

	let counting: &dyn Counting = &*actor;

	let numbers = futures::stream::iter(0..10);

	add_stream(counting, numbers);
	drop(actor); // Force the actor to exit when the stream stops

	let Ok(Ok(answer)) = join_handle.await else {
		unreachable!()
	};

	assert_eq!(answer, 45);
}
