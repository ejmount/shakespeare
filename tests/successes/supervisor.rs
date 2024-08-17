use std::any::Any;
use std::time::Duration;

use shakespeare::{actor, send_future_to, ActorOutcome, ActorSpawn};
use tokio::time::sleep;

type Panic = Box<dyn Any + Send>;

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
				msg_handle,
				join_handle,
				..
			} = Worker::start(WorkerState {
				success: true,
				count:   0,
			});
			send_future_to::<dyn Listening, _>(join_handle, self.get_shell());
			msg_handle.work().await.unwrap();

			let ActorSpawn {
				msg_handle,
				join_handle,
				..
			} = Worker::start(WorkerState {
				success: false,
				count:   0,
			});
			send_future_to::<dyn Listening, _>(join_handle, self.get_shell());
			msg_handle.work().await.unwrap();

			let ActorSpawn {
				msg_handle,
				join_handle,
				..
			} = Worker::start(WorkerState {
				success: true,
				count:   0,
			});
			send_future_to::<dyn Listening, _>(join_handle, self.get_shell());

			sleep(Duration::from_millis(500)).await;
			drop(msg_handle);
		}
	}

	#[performance(canonical)]
	impl Listening for SupervisorState {
		fn leave(&mut self, result: ActorOutcome<Worker>) {
			match result {
				ActorOutcome::Exit(true) => self.success = true,
				ActorOutcome::Exit(false) => self.idle = true,
				ActorOutcome::Panic(_) => self.failure = true,
				_ => unimplemented!(),
			}
		}
	}

	fn stop(state: SupervisorState) -> bool {
		state.success && state.failure && state.idle
	}
	fn catch(_val: Panic) {}
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
			let sleep = sleep(Duration::from_millis(50));
			send_future_to::<dyn Sleeper, _>(sleep, self.get_shell());
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

	fn catch(_val: Panic) {
		// throw away the panic value
	}
}

#[tokio::test]
async fn main() {
	let ActorSpawn {
		msg_handle,
		join_handle,
		..
	} = Supervisor::start(SupervisorState::default());

	let _ = msg_handle.go().await;
	drop(msg_handle);

	assert_eq!(join_handle.await, ActorOutcome::Exit(true));
}
