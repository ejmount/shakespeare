use std::any::Any;
use std::time::Duration;

use shakespeare::{actor, send_future_to, ActorOutcome, ActorSpawn, Context};
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
		async fn go(&mut self, ctx: &'_ mut Context<Self>) {
			let ActorSpawn {
				msg_handle,
				join_handle,
				..
			} = Worker::start(WorkerState {
				success: true,
				count:   0,
			});
			send_future_to::<dyn Listening, _>(join_handle, ctx.get_shell());
			msg_handle.work().await.unwrap();

			let ActorSpawn {
				msg_handle,
				join_handle,
				..
			} = Worker::start(WorkerState {
				success: false,
				count:   0,
			});
			send_future_to::<dyn Listening, _>(join_handle, ctx.get_shell());
			msg_handle.work().await.unwrap();

			let ActorSpawn {
				msg_handle,
				join_handle,
				..
			} = Worker::start(WorkerState {
				success: true,
				count:   0,
			});
			send_future_to::<dyn Listening, _>(join_handle, ctx.get_shell());

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

	fn stop(self) -> bool {
		self.success && self.failure && self.idle
	}
	fn catch(self, _val: Panic) {}
}

#[actor]
pub mod Worker {

	pub struct WorkerState {
		pub success: bool,
		pub count:   usize,
	}

	#[performance(canonical)]
	impl Work for WorkerState {
		async fn work(&mut self, ctx: &'_ mut Context<Self>) {
			self.count += 1;
			let sleep = sleep(Duration::from_millis(50));
			send_future_to::<dyn Sleeper, _>(sleep, ctx.get_shell());
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

	fn stop(self) -> bool {
		self.count > 0
	}

	fn catch(self, _val: Panic) {
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
