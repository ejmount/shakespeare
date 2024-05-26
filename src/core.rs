use std::fmt::Debug;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::Future;
use tokio::task::JoinHandle;

use crate::returnval::{ReturnEnvelope, ReturnPath};
use crate::Role2SendError;

pub enum ActorOutcome<A: ActorShell> {
	Aborted(tokio::task::JoinError),
	Exit(A::ExitType),
	Panic(A::PanicType),
}

impl<A: ActorShell> Debug for ActorOutcome<A> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ActorOutcome::Aborted(_) => f.write_str("ActorOutcome::Aborted"),
			ActorOutcome::Exit(_) => f.write_str("ActorOutcome::Exit"),
			ActorOutcome::Panic(_) => f.write_str("ActorOutcome::Panic"),
		}
	}
}

impl<A: ActorShell> PartialEq for ActorOutcome<A>
where
	A::ExitType: PartialEq,
	A::PanicType: PartialEq,
{
	fn eq(&self, other: &Self) -> bool {
		use ActorOutcome::{Exit, Panic};
		match (self, other) {
			(Exit(a), Exit(b)) => a == b,
			(Panic(a), Panic(b)) => a == b,
			_ => false,
		}
	}
}

impl<A: ActorShell> Eq for ActorOutcome<A>
where
	A::ExitType: Eq,
	A::PanicType: Eq,
{
}

pub struct ActorHandle<A: ActorShell>(JoinHandle<Result<A::ExitType, A::PanicType>>);

impl<A: ActorShell> ActorHandle<A> {
	fn new(val: JoinHandle<Result<A::ExitType, A::PanicType>>) -> ActorHandle<A> {
		ActorHandle(val)
	}
}

impl<A: ActorShell> Debug for ActorHandle<A> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str("ActorHandle")
	}
}

impl<A: ActorShell> Future for ActorHandle<A> {
	type Output = ActorOutcome<A>;

	fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let handle = &mut self.get_mut().0;
		tokio::pin!(handle);
		match handle.poll(cx) {
			Poll::Pending => Poll::Pending,
			Poll::Ready(result) => match result {
				Ok(Ok(e)) => ActorOutcome::Exit(e),
				Ok(Err(f)) => ActorOutcome::Panic(f),
				Err(e) => ActorOutcome::Aborted(e),
			}
			.into(),
		}
	}
}

pub trait ActorShell {
	type ExitType;
	type PanicType;
}

#[non_exhaustive]
#[derive(Debug)]
pub struct ActorSpawn<A>
where
	A: ActorShell,
{
	pub actor:  Arc<A>,
	pub handle: ActorHandle<A>,
}

impl<A: ActorShell> ActorSpawn<A> {
	#[doc(hidden)]
	pub fn new(
		actor: Arc<A>,
		handle: JoinHandle<Result<A::ExitType, A::PanicType>>,
	) -> ActorSpawn<A> {
		ActorSpawn {
			actor,
			handle: ActorHandle::new(handle),
		}
	}
}

#[doc(hidden)]
#[trait_variant::make(Send)]
pub trait RoleSender<T: Send>: Sync + Send + Clone {
	type Error: Send;
	async fn send(&self, msg: T) -> Result<(), Self::Error>;
}

#[doc(hidden)]
#[trait_variant::make(Send)]
pub trait RoleReceiver<T: Send> {
	async fn recv(&mut self) -> Option<T>;
	fn is_empty(&self) -> bool;
}

#[doc(hidden)]
pub trait Channel {
	type Input;
	type Item: Send;
	type Sender: RoleSender<Self::Item>;
	type Receiver: RoleReceiver<Self::Item>;
	fn new(init: Self::Input) -> (Self::Sender, Self::Receiver);

	#[must_use]
	fn new_default() -> (Self::Sender, Self::Receiver)
	where
		Self::Input: Default,
	{
		Self::new(Self::Input::default())
	}
}

#[trait_variant::make(Send)]
pub trait Role: 'static + Sync + Send {
	type Payload: Sized + Send;
	type Return: Sized + Send;
	type Channel: Channel<Item = ReturnEnvelope<Self>>;
	fn send(&self, val: Self::Payload);
	async fn enqueue(&self, val: ReturnEnvelope<Self>) -> Result<(), Role2SendError<Self>>;
}
