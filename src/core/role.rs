use super::super::Role2SendError;
use super::returnval::ReturnEnvelope;

/// The sender half of a channel used internally by a Role
#[trait_variant::make(Send)]
pub trait Sender<T: Send>: Sync + Send + Clone {
	/// An error indicating the message failed to deliver. This likely indicates the actor stopped before receiving the message
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
	/// Parameters used to construct the channel pair. Currently unused, here for future-proofing
	type Input;
	/// The type of item sent across the channel.
	type Item: Send;
	/// The type of the sender half of the channel
	type Sender: Sender<Self::Item>;
	/// The type of the recv half of the channel
	type Receiver: Receiver<Self::Item>;
	/// Construct a new channel
	fn new(init: Self::Input) -> (Self::Sender, Self::Receiver);
}

/// A supertrait for Roles that an actor might implement
///
/// Roles (that is, the dyn type, `dyn ARole`) implement this trait, which describes the generic features all roles contain. See the [`role`](`super::super::role`) macro for more information.
/// No internal details of this trait are relevant to external users, only whether it is implemented and its related implementations of [`Accepts`] and [`Emits`]
#[trait_variant::make(Send)]
// This logically *should* be 'static but the compiler can't deal with the lifetime bounds properly. See https://github.com/rust-lang/rust/issues/131488
// The compiler seems to be OK if 'static is listed separately in the signature of the functions that need it.
pub trait Role: Sync + Send {
	#[doc(hidden)]
	type Payload: Sized + Send + 'static;
	#[doc(hidden)]
	type Return: Sized + Send + 'static;
	#[doc(hidden)]
	type Channel: Channel<Item = ReturnEnvelope<Self>>;
	#[doc(hidden)]
	/// Puts a message into the corresponding queue for the actor
	/// Can potentially error if the actor stops before the message is received
	async fn enqueue(&self, val: ReturnEnvelope<Self>) -> Result<(), Role2SendError<Self>>;
}

/// Denotes that a Role can be sent `T` values
///
/// A Role (specifically, the type, `dyn Role`) implementing this trait means that exactly one method of the Role has a parameter list corresponding to `T`. This means the actor can determine what method call is intended from the value alone - it is the only possibility - and so can work with [`Message::send_when_ready`](crate::Message::send_when_ready) and similar. Methods explicitly defined in the Role can be called whether or not an `Accepts` implementation exists.
///
/// Because a single actor can implement multiple roles, and each role may have an implementation of this trait for the same value of `T`, you may need to disambiguate the call like so:
/// ```ignore
/// future.send_when_ready(actor as Arc<dyn Role>)
/// ```
///
/// As an example of a role and what values of `T` it implements this trait for:
/// ```no_run
/// use std::sync::Arc;
///
/// use shakespeare::{Accepts, role};
/// use static_assertions::{assert_impl_one, assert_not_impl_any};
///
/// #[role]
/// trait ARole {
///     fn a_method(&self, a: usize) {}
///     fn a_different_method(&self, a: usize) {}
///     fn a_third_method(&self, a: usize, b: usize) {}
///     fn a_nullary_method(&self) {}
/// }
///
///
/// assert_impl_one!(dyn ARole: Accepts<(usize, usize)>);
/// assert_not_impl_any!(dyn ARole: Accepts<usize>);
/// // The empty tuple is special - this is currently never implemented even when a nullary method implies it should be
/// // This may be changed in the future
/// assert_not_impl_any!(dyn ARole: Accepts<()>);
/// ```
///
///
/// (This trait's implementations are normally automatically generated)
pub trait Accepts<T>: Role {
	#[doc(hidden)]
	fn into_payload(t: T) -> Self::Payload;
}

#[doc(hidden)]
/// Need this so that [`crate::Envelope`] can be dropped to send off the item
impl<R: Role + ?Sized> Accepts<R::Payload> for R {
	fn into_payload(t: R::Payload) -> Self::Payload {
		t
	}
}

/// Denotes that at least one method of a Role produces a `T`
///
/// `Emits` is the dual of [`Accepts`] - it indicates at least one of the Role's methods returns a value of type `T`. The primary use for this is to express bounds for the [`Envelope::forward_to`](crate::Envelope::forward_to) method.
///
/// (This trait's implementations are normally automatically generated)
pub trait Emits<T>: Role {
	#[doc(hidden)]
	fn from_return_payload(t: Self::Return) -> T;
}
// The raw Return type shouldn't be escaping anywhere else, so we don't need a reflexive impl
