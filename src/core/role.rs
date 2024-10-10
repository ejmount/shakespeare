use super::super::Role2SendError;
use super::returnval::ReturnEnvelope;

/// The sender half of a channel used internally by a Role
#[trait_variant::make(Send)]
pub trait Sender<T: Send>: Sync + Send + Clone {
	/// An error indicating the message failed to deliver. This likely indicates the actor crashed before receiving the message
	type Error: Send;
	#[doc(hidden)]
	async fn send(&self, msg: T) -> Result<(), Self::Error>;
}

/// The receiver half of a channel used internally by a Role
#[trait_variant::make(Send)]
pub trait Receiver<T: Send> {
	#[doc(hidden)]
	async fn recv(&mut self) -> Option<T>;
	/// Used to avoid bailing out on the dispatch loop too early if all clients have dropped
	fn is_empty(&self) -> bool;
}

/// A marker trait describing a channel underlying a particular role
/// Currently the only implementation is for unbounded tokio channels, but more implementations are expected in the future
pub trait Channel {
	/// Parameters used to construct the channel pair. Currently unused, here for futureproofing
	type Input;
	/// The type of item sent across the channel.
	type Item: Send;
	/// The type of the sender half of the channel
	type Sender: Sender<Self::Item>;
	/// The type of the recv half of the channel
	type Receiver: Receiver<Self::Item>;
	/// Construct a new channel
	fn new(init: Self::Input) -> (Self::Sender, Self::Receiver);

	#[must_use]
	/// A convenience if the channel has a default parameters
	fn new_default() -> (Self::Sender, Self::Receiver)
	where
		Self::Input: Default,
	{
		Self::new(Self::Input::default())
	}
}

/// A Role that an Actor can implement.
///
/// Roles implement this trait, which describes the generic features all roles contain. See the [`role!`][`::shakespeare_macro::role`] macro for more information.
/// No internal details of this trait are relevant to external users, only whether it is implemented and its related implementations of ['Accepts'] and ['Emits']
#[trait_variant::make(Send)]
// This logically *should* be 'static but the compiler can't deal with the lifetime bounds properly. See https://github.com/rust-lang/rust/issues/131488
// The compiler seems to be OK if 'static is listed seperately in the signature of the functions that need it.
pub trait Role: Sync + Send {
	#[doc(hidden)]
	type Payload: Sized + Send + 'static;
	#[doc(hidden)]
	type Return: Sized + Send + 'static;
	#[doc(hidden)]
	type Channel: Channel<Item = ReturnEnvelope<Self>>;
	#[doc(hidden)]
	/// Can potentially error if the actor crashes before the message is received
	async fn enqueue(&self, val: ReturnEnvelope<Self>) -> Result<(), Role2SendError<Self>>;
}
