use std::any::Any;

use tokio::sync::mpsc::UnboundedSender;

struct Dropper<T>(T);

impl<T> Drop for Dropper<T> {
	fn drop(&mut self) {
		println!("Goodbye")
	}
}

#[shakespeare::actor]
mod Actor {

	struct ActorState {
		sender: Dropper<UnboundedSender<usize>>,
	}
	#[shakespeare::performance(canonical)]
	impl BasicRole for ActorState {
		fn speak(&mut self, val: usize) {
			self.sender.0.send(val).unwrap();
		}
	}

	fn stop(self) {
		println!("Exiting");
	}

	fn catch(self, _thing: Box<dyn Any + Send>) -> Box<dyn Any + Send> {
		_thing
	}
}

#[tokio::test]
async fn main() {
	let (sender, mut recv) = tokio::sync::mpsc::unbounded_channel();
	let olaf = ActorState {
		sender: Dropper(sender),
	};
	let shakespeare::ActorSpawn { actor_handle, .. } = Actor::start(olaf);
	let f = actor_handle.speak(40);
	f.await.unwrap();
	assert_eq!(recv.recv().await.unwrap(), 40);
	//std::mem::drop(actor);
}
