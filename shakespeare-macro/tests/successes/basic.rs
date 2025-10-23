use std::any::Any;

use shakespeare::ActorHandles;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

#[shakespeare::actor]
mod Actor {

	struct ActorState {
		sender: UnboundedSender<usize>,
	}
	#[shakespeare::performance(canonical)]
	impl BasicRole for ActorState {
		fn speak(&mut self, val: usize) {
			self.sender.send(val).unwrap();
		}
	}

	fn stop(self) {
		let _ = self.sender.send(0);
	}

	fn catch(self, _thing: Box<dyn Any + Send>) -> Box<dyn Any + Send> {
		_thing
	}
}

#[tokio::test]
async fn main() {
	let (sender, mut recv) = unbounded_channel();
	let olaf = ActorState { sender };
	let ActorHandles { message_handle, .. } = Actor::start(olaf);
	let envelope = message_handle.speak(40);
	envelope.await.unwrap();
	assert_eq!(recv.recv().await.unwrap(), 40);
	drop(message_handle);
	assert_eq!(recv.recv().await.unwrap(), 0);
}
