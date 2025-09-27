#[shakespeare::actor]
mod Actor {
	#[derive(Default)]
	struct ActorState {
		active: bool,
	}
	#[shakespeare::performance(canonical)]
	impl BasicRole for ActorState {
		fn begin(&mut self) {
			self.active = true;
		}

		fn speak(&self, val: usize) -> usize {
			if self.active {
				2 * val
			} else {
				0
			}
		}
	}
}

#[tokio::test]
async fn main() {
	let shakespeare::ActorSpawn { actor_handle, .. } = Actor::start(ActorState { active: false });

	let actor: std::sync::Arc<dyn BasicRole> = actor_handle;

	let inactive = actor.speak(40).await.unwrap();
	assert_eq!(inactive, 0);
	let non_response = actor.begin();
	std::mem::drop(non_response);
	tokio::time::sleep(std::time::Duration::from_millis(10)).await;
	let envelope = actor.speak(40);
	let result: usize = envelope.await.unwrap();
	assert_eq!(result, 80);
}
