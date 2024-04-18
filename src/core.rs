use std::sync::Arc;

use ::tokio::task::JoinHandle;
use async_trait::async_trait;

#[non_exhaustive]
#[derive(Debug)]
pub struct ActorSpawn<A>
where
	A: ActorShell,
{
	pub actor:       Arc<A>,
	pub join_handle: JoinHandle<Result<A::ExitType, A::PanicType>>,
}

impl<A: ActorShell> ActorSpawn<A> {
	#[doc(hidden)]
	pub fn new(
		actor: Arc<A>,
		join_handle: JoinHandle<Result<A::ExitType, A::PanicType>>,
	) -> ActorSpawn<A> {
		ActorSpawn { actor, join_handle }
	}
}

#[doc(hidden)]
#[async_trait]
pub trait RoleSender<T: Send>: Sync + Send + Clone {
	type Error;
	async fn send(&self, msg: T) -> Result<(), Self::Error>;
}

#[doc(hidden)]
#[async_trait]
pub trait RoleReceiver<T: Send> {
	async fn recv(&mut self) -> Option<T>;
	fn is_empty(&self) -> bool;
}

#[doc(hidden)]
pub trait ActorShell {
	type ExitType;
	type PanicType;
}

#[doc(hidden)]
pub trait Channel {
	type Input;
	type Item: Send + Sized;
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

#[doc(hidden)]
pub trait Role: 'static + Sync + Send {
	type Payload: Sized + Send;
	type Channel: Channel<Item = Self::Payload>;
	fn clone_sender(&self) -> <Self::Channel as Channel>::Sender;
}
