use std::sync::Arc;
use std::time::Duration;

use shakespeare::{actor, add_future, ActorSpawn};

#[actor]
pub mod Supervisor {

	#[derive(Default)]
	pub struct SupervisorState {
		success: bool,
		failure: bool,
	}

	#[performance(canonical)]
	impl Starter for SupervisorState {
		fn go(&mut self, listener: Arc<dyn Listening>) {
			let ActorSpawn { join_handle, .. } = WorkerState::start(WorkerState { success: true });
			add_future(&*listener, join_handle);
			let ActorSpawn { join_handle, .. } = WorkerState::start(WorkerState { success: false });
			add_future(&*listener, join_handle);
		}
	}

	#[performance(canonical)]
	impl Listening for SupervisorState {
		fn leave(&mut self, val: Result<Result<(), ()>, tokio::task::JoinError>) {
			let Ok(result) = val else { unreachable!() };
			if result.is_ok() {
				self.success = true;
			} else {
				self.failure = true;
			}
		}
	}

	fn exit(state: SupervisorState) -> bool {
		state.success && state.failure
	}
}

#[actor]
pub mod Worker {
	pub struct WorkerState {
		success: bool,
	}
	#[performance(canonical)]
	impl Work for WorkerState {
		async fn work(&mut self, thing: Arc<dyn Sleeper>) {
			let sleep = tokio::time::sleep(Duration::from_millis(50));
			add_future(&*thing, sleep);
		}
	}
	#[performance(canonical)]
	impl Sleeper for WorkerState {
		fn wake(&mut self, _wake: ()) {
			if self.success {
				return;
			} else {
				panic!()
			}
		}
	}

	fn catch(_val: Box<dyn std::any::Any + std::marker::Send>) {
		// throw away the panic value
	}
}

#[tokio::test]
async fn main() {
	let ActorSpawn {
		actor: shell,
		join_handle,
		..
	} = SupervisorState::start(SupervisorState::default());

	let _ = shell.go(shell.clone()).await;
	drop(shell);

	assert!(join_handle.await.unwrap().is_ok());
}
