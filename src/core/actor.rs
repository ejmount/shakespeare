use std::fmt::Debug;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::Future;
use tokio::task::JoinHandle;

pub enum Outcome<A: Shell> {
	Aborted(tokio::task::JoinError),
	Exit(A::ExitType),
	Panic(A::PanicType),
}

impl<A: Shell> Debug for Outcome<A> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Outcome::Aborted(_) => f.write_str("ActorOutcome::Aborted"),
			Outcome::Exit(_) => f.write_str("ActorOutcome::Exit"),
			Outcome::Panic(_) => f.write_str("ActorOutcome::Panic"),
		}
	}
}

impl<A: Shell> PartialEq for Outcome<A>
where
	A::ExitType: PartialEq,
	A::PanicType: PartialEq,
{
	fn eq(&self, other: &Self) -> bool {
		use Outcome::{Exit, Panic};
		match (self, other) {
			(Exit(a), Exit(b)) => a == b,
			(Panic(a), Panic(b)) => a == b,
			_ => false,
		}
	}
}

impl<A: Shell> Eq for Outcome<A>
where
	A::ExitType: Eq,
	A::PanicType: Eq,
{
}

pub struct Handle<A: Shell>(JoinHandle<Result<A::ExitType, A::PanicType>>);

impl<A: Shell> Handle<A> {
	fn new(val: JoinHandle<Result<A::ExitType, A::PanicType>>) -> Handle<A> {
		Handle(val)
	}
}

impl<A: Shell> Debug for Handle<A> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str("ActorHandle")
	}
}

impl<A: Shell> Future for Handle<A> {
	type Output = Outcome<A>;

	fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let handle = &mut self.get_mut().0;
		tokio::pin!(handle);
		match handle.poll(cx) {
			Poll::Pending => Poll::Pending,
			Poll::Ready(result) => match result {
				Ok(Ok(e)) => Outcome::Exit(e),
				Ok(Err(f)) => Outcome::Panic(f),
				Err(e) => Outcome::Aborted(e),
			}
			.into(),
		}
	}
}

pub trait Shell {
	type ExitType;
	type PanicType;
}

#[non_exhaustive]
#[derive(Debug)]
pub struct Spawn<A>
where
	A: Shell,
{
	/// A handle for sending messages to the actor
	pub msg_handle:  Arc<A>,
	/// A future for awaiting the actor's completion
	pub join_handle: Handle<A>,
}

impl<A: Shell> Spawn<A> {
	#[doc(hidden)]
	pub fn new(actor: Arc<A>, handle: JoinHandle<Result<A::ExitType, A::PanicType>>) -> Spawn<A> {
		Spawn {
			msg_handle:  actor,
			join_handle: Handle::new(handle),
		}
	}
}
