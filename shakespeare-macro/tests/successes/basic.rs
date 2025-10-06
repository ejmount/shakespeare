use std::any::Any;

use shakespeare::ActorSpawn;
use tokio::sync::mpsc::UnboundedSender;

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
		println!("Exiting");
		let _ = self.sender.send(0);
	}

	fn catch(self, _thing: Box<dyn Any + Send>) -> Box<dyn Any + Send> {
		_thing
	}
}

#[tokio::test]
async fn main() {
	let (sender, mut recv) = tokio::sync::mpsc::unbounded_channel();
	let olaf = ActorState { sender };
	let ActorSpawn { actor_handle, .. } = Actor::start(olaf);
	let envelope = actor_handle.speak(40);
	envelope.await.unwrap();
	assert_eq!(recv.recv().await.unwrap(), 40);
	drop(actor_handle);
	assert_eq!(recv.recv().await.unwrap(), 0);
}
