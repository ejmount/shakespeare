use std::any::Any;
use std::mem::drop;
use std::time::Duration;

use shakespeare::{ActorOutcome, ActorSpawn, Context, Message, actor};
use tokio::time::sleep;

type Panic = Box<dyn Any + Send>;

#[actor]
pub mod Supervisor {
	use std::sync::Arc;

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
				actor_handle,
				join_handle,
				..
			} = Worker::start(WorkerState {
				success: true,
				count:   0,
			});
			join_handle.send_to(ctx.get_shell() as Arc<dyn Listening>);
			actor_handle.work().await.unwrap();

			let ActorSpawn {
				actor_handle,
				join_handle,
				..
			} = Worker::start(WorkerState {
				success: false,
				count:   0,
			});
			join_handle.send_to(ctx.get_shell() as Arc<dyn Listening>);
			actor_handle.work().await.unwrap();

			let ActorSpawn {
				actor_handle,
				join_handle,
				..
			} = Worker::start(WorkerState {
				success: true,
				count:   0,
			});
			join_handle.send_to(ctx.get_shell() as Arc<dyn Listening>);

			sleep(Duration::from_millis(500)).await;
			drop(actor_handle);
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
			sleep(Duration::from_millis(50)).send_to(ctx.get_shell() as Arc<dyn Sleeper>);
		}
	}
	#[performance(canonical)]
	impl Sleeper for WorkerState {
		fn wake(&mut self, _wake: ()) {
			if !self.success {
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
		actor_handle,
		join_handle,
		..
	} = Supervisor::start(SupervisorState::default());

	let _ = actor_handle.go().await;
	drop(actor_handle);

	assert_eq!(join_handle.await, ActorOutcome::Exit(true));
}
