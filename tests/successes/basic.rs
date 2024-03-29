use tokio::sync::mpsc::*;

#[shakespeare::actor]
mod Actor {
	struct ActorState {
		sender: UnboundedSender<usize>,
	}
	#[shakespeare::performance(canonical)]
	impl Role for ActorState {
		fn speak(&mut self, val: usize) {
			self.sender.send(val).unwrap();
		}
	}
}

#[tokio::test]
async fn main() {
	let (sender, mut recv) = tokio::sync::mpsc::unbounded_channel();
	let olaf = ActorState { sender };
	let shakespeare::ActorSpawn { actor, .. } = ActorState::start(olaf);
	actor.speak(40).await.unwrap();
	assert_eq!(recv.recv().await.unwrap(), 40);
}
