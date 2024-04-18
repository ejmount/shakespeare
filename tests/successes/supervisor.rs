use std::time::Duration;

use shakespeare::{actor, add_future, ActorSpawn};

#[actor]
pub mod Supervisor {

	#[derive(Default)]
	pub struct SupervisorState {
		success: bool,
		failure: bool,
		idle:    bool,
	}

	#[performance(canonical)]
	impl Starter for SupervisorState {
		fn go(&mut self) {
			let ActorSpawn {
				actor, join_handle, ..
			} = WorkerState::start(WorkerState {
				success: true,
				count:   0,
			});
			add_future::<dyn Listening, _>(Self::get_shell(), join_handle);
			actor.work().await.unwrap();

			let ActorSpawn {
				actor, join_handle, ..
			} = WorkerState::start(WorkerState {
				success: false,
				count:   0,
			});
			add_future::<dyn Listening, _>(Self::get_shell(), join_handle);
			actor.work().await.unwrap();

			let ActorSpawn {
				actor, join_handle, ..
			} = WorkerState::start(WorkerState {
				success: true,
				count:   0,
			});
			add_future::<dyn Listening, _>(Self::get_shell(), join_handle);
			tokio::time::sleep(Duration::from_millis(500)).await;
			drop(actor);
		}
	}

	#[performance(canonical)]
	impl Listening for SupervisorState {
		fn leave(&mut self, val: Result<Result<bool, ()>, tokio::task::JoinError>) {
			let Ok(result) = val else { unreachable!() };
			match result {
				Ok(true) => self.success = true,
				Ok(false) => self.idle = true,
				Err(_) => {
					self.failure = true;
				}
			}
		}
	}

	fn stop(state: SupervisorState) -> bool {
		state.success && state.failure && state.idle
	}
}

#[actor]
pub mod Worker {

	pub struct WorkerState {
		pub success: bool,
		pub count:   usize,
	}
	#[performance(canonical)]
	impl Work for WorkerState {
		async fn work(&mut self) {
			self.count += 1;
			let sleep = tokio::time::sleep(Duration::from_millis(50));
			add_future::<dyn Sleeper, _>(Self::get_shell(), sleep);
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

	fn stop(ws: WorkerState) -> bool {
		ws.count > 0
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

	let _ = shell.go().await;
	drop(shell);

	assert!(join_handle.await.unwrap().unwrap());
}
