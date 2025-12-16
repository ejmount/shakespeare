use std::fmt::Debug;
use std::future::IntoFuture;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::Future;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::oneshot::{Receiver, Sender};

use crate::{Accepts, Emits, Message, Role, Role2SendError};

type PinnedAction = Pin<Box<dyn Send + Future<Output = ()>>>;

#[doc(hidden)]
#[derive(Default)]
// Represents what to do with the return value from a performance handler
pub enum ReturnPath<Payload: Send> {
	#[default]
	Discard,
	// Send it onwards to another actor's mailbox by runnin a function
	Mailbox(Box<dyn Send + FnOnce(Payload) -> PinnedAction>),
	// Send it directly back to the caller via the given sender
	Immediate(Sender<Payload>),
}

impl<Payload: Send> std::fmt::Debug for ReturnPath<Payload> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Discard => write!(f, "<ReturnPath::Discard>"),
			Self::Mailbox(_) => write!(f, "<ReturnPath::Mailbox>"),
			Self::Immediate(_) => write!(f, "<ReturnPath::Immediate>"),
		}
	}
}

impl<Payload: Send + 'static> ReturnPath<Payload> {
	pub(crate) fn create_immediate() -> (ReturnPath<Payload>, Receiver<Payload>) {
		let (send, recv) = tokio::sync::oneshot::channel();
		(ReturnPath::Immediate(send), recv)
	}

	pub async fn send(self, val: Payload) {
		use ReturnPath::{Discard, Immediate, Mailbox};

		match self {
			Discard => (),
			Mailbox(callback) => callback(val).await,
			Immediate(channel) => {
				let _ = channel.send(val);
			}
		}
	}
}

/// A message that has been prepared to be (*but not yet*) sent to an actor, produced by calling a Role method on the actor shell.
///
/// This type allows the caller to control how the return value, of type `Output`, produced by the actor processing the message will be handled. As a result, while this value exists the message has not been sent.
///
/// The caller is expected to do one of four things with this value:
/// 1. nothing - that is, allowing it to drop will dispatch the message and have any return value thrown away, but *will not* wait for the message delivery to complete.
/// 2. awaiting this value will wait for the actor to receive and process the message, then yield the return value to the caller
/// 3. calling [`ignore()`][`Envelope::ignore`] and awaiting the resulting future *will wait* for the message to be sent, but will not wait for any return value.
/// 4. calling [`forward_to`][`Envelope::forward_to`] will send the return value directly to a given actor's mailbox.
///
/// **NB**: In case 1, there is no ordering established with other messages sent to the same receiver, even from the same sender. In all other cases, multiple messages to the same receiver from a given sender sent through the same handle will be received in sending order. In all cases, ordering between messages sent to different receivers or from different senders is unspecified.
#[derive(Debug)]
pub struct Envelope<DestRole, Output>
where
	DestRole: Role + ?Sized + 'static,
{
	val:  Option<DestRole::Payload>,
	dest: Option<Arc<DestRole>>,
	// "Type parameter Output is never used"
	_v:   PhantomData<Output>,
}

impl<DestRole, Output> Envelope<DestRole, Output>
where
	DestRole: Role + ?Sized,
{
	#[doc(hidden)]
	pub fn new(val: DestRole::Payload, dest: Arc<DestRole>) -> Envelope<DestRole, Output> {
		Envelope {
			val:  Some(DestRole::into_payload(val)),
			dest: Some(dest),
			_v:   PhantomData {},
		}
	}

	pub(crate) fn unpack(mut self) -> (DestRole::Payload, Arc<DestRole>) {
		let val = (self.val.take().unwrap(), self.dest.take().unwrap());
		std::mem::forget(self);
		val
	}

	/// This method will wait for the message to arrive at the receiving actor, but will not wait for any return value, which will be dropped.
	///
	/// # Errors
	///
	/// This function may return `Err` if the actor has already stopped.
	#[must_use = "The message will not be sent to the actor if this Future isn't processed"]
	pub async fn ignore(self) -> Result<(), Role2SendError<DestRole>> {
		let (payload, dest) = self.unpack();

		let return_path = ReturnPath::Discard;

		dest.enqueue(ReturnEnvelope {
			payload,
			return_path,
		})
		.await
	}

	/// Arranges for the *return value* produced by processing this [`Envelope`] to be forwarded to the given actor, discarding any return value that the second actor produces.
	///
	/// The returned future becomes `Ready`
	///
	/// An actor may want to call this method using its own handle as the destination, so that it receives the return value without `await`ing inside the message handler that's making the call, because an actor cannot service further messages until the handler returns. This means that an actor A sending a message to another actor B and awaiting the [`Envelope`] will not service messages until B's handler returns a value - if B also sends and then awaits a message to A, the two will deadlock, because A will not service the message B is waiting on until B returns a value.
	///
	/// While using this method and passing the actor's own handle as the `recipient` will avoid deadlocks - actor A will continue servicing messages - tying the returned value back to the original context currently has no specific support and is left to the developer.
	///
	/// Because [`Envelope`] is [`IntoFuture`], this is equivalent to, but more efficient than, passing the same parameters to [`Message::send_to`] **including** that the recipient actor will be kept alive until the message is either processed or the actor originating the [`Envelope`] shuts down.
	///
	/// # Errors
	///
	/// Can return an Err if the actor originating the Envelope stops before the Envelope's return value is delivered to the recipient
	pub async fn forward_to<RxRole>(
		self,
		recipient: Arc<RxRole>,
	) -> Result<(), Role2SendError<DestRole>>
	where
		DestRole: Emits<Output>,
		RxRole: Accepts<Output> + 'static,
	{
		let (payload, original) = self.unpack();

		let bridge_to_rx_role = |sender_payload| -> Pin<Box<dyn Future<Output = ()> + Send>> {
			let discard_envelope = ReturnEnvelope {
				return_path: ReturnPath::Discard,
				payload:     RxRole::into_payload(DestRole::from_return_payload(sender_payload)),
			};
			Box::pin(async move {
				let _ = recipient.enqueue(discard_envelope).await;
			})
		};

		let val: ReturnEnvelope<DestRole> = ReturnEnvelope {
			return_path: ReturnPath::Mailbox(Box::new(bridge_to_rx_role)),
			payload,
		};

		original.enqueue(val).await
	}
}

impl<DestRole, Output> IntoFuture for Envelope<DestRole, Output>
where
	DestRole: Emits<Output> + ?Sized + 'static,
{
	#[doc(hidden)]
	type IntoFuture = ReturnCaster<DestRole, Output>;
	/// The return received from the envelope can fail if the message handler doesn't complete
	type Output = std::result::Result<Output, RecvError>;

	fn into_future(self) -> Self::IntoFuture {
		let (payload, dest) = self.unpack();

		let (return_path, rx) = ReturnPath::create_immediate();

		let envelope = ReturnEnvelope {
			payload,
			return_path,
		};

		tokio::spawn(async move {
			let _ = dest.enqueue(envelope).await;
		});

		ReturnCaster {
			future: rx.into_future(),
			typ:    PhantomData {},
		}
	}
}

impl<DestRole, Output> Drop for Envelope<DestRole, Output>
where
	DestRole: Role + ?Sized,
{
	fn drop(&mut self) {
		let val = self.val.take().unwrap();
		let dest = self.dest.take().unwrap();

		std::future::ready(val).send_to(dest);
	}
}

#[doc(hidden)]
#[pin_project::pin_project]
/// A future that awaits on an [`Envelope`] being processed and appropriately casts the return value
/// This can fail and produce an `Err` if the actor's message handler aborts without completing.
///
/// This exists because the type for [`Envelope::into_future`] needs to be nameable, which Future::map is not.
pub struct ReturnCaster<R, V>
where
	R: Role + ?Sized,
{
	#[pin]
	future: <Receiver<<R as crate::Role>::Return> as IntoFuture>::IntoFuture,
	typ:    PhantomData<V>,
}

impl<R, V> Future for ReturnCaster<R, V>
where
	R: Emits<V> + ?Sized,
{
	type Output = std::result::Result<V, RecvError>;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let inner = self.project().future;

		inner
			.poll(cx)
			.map(|val| val.map(|returned_payload| R::from_return_payload(returned_payload)))
	}
}

impl<R, V> Debug for ReturnCaster<R, V>
where
	R: Role + ?Sized,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"ReturnCaster<{}, {}>",
			core::any::type_name::<R>(),
			core::any::type_name::<V>()
		)
	}
}

#[doc(hidden)]
/// This is the internal type for actor message queues, representing the input parameters and what to do with the return value
pub struct ReturnEnvelope<R: Role + ?Sized> {
	pub payload:     R::Payload,
	pub return_path: ReturnPath<R::Return>,
}

impl<R: Role + ?Sized> Debug for ReturnEnvelope<R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "<ReturnEnvelope>")
	}
}
