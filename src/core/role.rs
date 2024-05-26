use super::super::{ReturnEnvelope, Role2SendError};

#[doc(hidden)]
#[trait_variant::make(Send)]
pub trait Sender<T: Send>: Sync + Send + Clone {
	type Error: Send;
	async fn send(&self, msg: T) -> Result<(), Self::Error>;
}

#[doc(hidden)]
#[trait_variant::make(Send)]
pub trait Receiver<T: Send> {
	async fn recv(&mut self) -> Option<T>;
	fn is_empty(&self) -> bool;
}

#[doc(hidden)]
pub trait Channel {
	type Input;
	type Item: Send;
	type Sender: Sender<Self::Item>;
	type Receiver: Receiver<Self::Item>;
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
