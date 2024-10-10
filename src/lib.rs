//! Hello

#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(unreachable_pub)]
#![warn(unused)]
#![warn(nonstandard_style)]
#![warn(clippy::pedantic)]
#![warn(clippy::missing_panics_doc)]
#![warn(clippy::dbg_macro)]
#![forbid(unsafe_code)]
#![warn(clippy::todo)]
#![warn(clippy::unimplemented)]

use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[doc(hidden)]
pub use ::async_trait as async_trait_export;
#[doc(hidden)]
pub use ::tokio as tokio_export;
pub use shakespeare_macro::{actor, performance, role};
#[doc(hidden)]
pub use tokio::TokioUnbounded;

mod core;
mod tokio;

#[doc(hidden)]
pub use core::{
	Channel, Receiver as RoleReceiver, ReturnCaster, ReturnEnvelope, ReturnPath,
	Sender as RoleSender,
};
pub use core::{
	Envelope, Handle as ActorHandle, Outcome as ActorOutcome, Role, Shell as ActorShell,
	Spawn as ActorSpawn, State as ActorState,
};

use futures::Stream;

#[doc(hidden)]
pub type Role2Payload<R> = <R as Role>::Payload;
#[doc(hidden)]
pub type Role2Receiver<R> = <<R as Role>::Channel as Channel>::Receiver;
/// Shortcut to resolve a Role's channel's sender type.
pub type Role2Sender<R> = <<R as Role>::Channel as Channel>::Sender;
/// Shortcut to resolve the sender's error type
pub type Role2SendError<R> = <Role2Sender<R> as RoleSender<ReturnEnvelope<R>>>::Error;

#[doc(hidden)]
/// Create a new future that will wrap the given future and catch any panic.
/// Used by [`::shakespeare_macro::actor::output::SpawningFunction`]
/// Included here to avoid clients having to depend on `futures` crate
pub fn catch_future<T>(fut: T) -> impl Future<Output = Result<T::Output, Box<dyn Any + Send>>>
where
	T: Future,
{
	futures::future::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(fut))
}

/// Subscribes an actor to a stream, delivering each item of the stream to the actor's mailbox.
///
/// The type constraints ensure that it is unambigious which method handler this will dispatch to.
///
/// This function does not do anything to inform the actor when the stream closes, successfuly or otherwise.
///
/// **N.B**: this function retains the `Arc<dyn Role>` for as long as the stream is still active, and will keep the actor alive for that time.
pub fn send_stream_to<R, S>(stream: S, actor: Arc<R>)
where
	R: Role + ?Sized + 'static,
	S: Stream + Send + 'static,
	R::Payload: From<S::Item>,
	<S as Stream>::Item: Send,
{
	use futures::StreamExt;
	tokio_export::spawn(async move {
		stream
			.for_each(|msg| async {
				let payload = msg.into();
				let envelope = ReturnEnvelope {
					payload,
					return_path: ReturnPath::Discard,
				};
				let _ = actor.enqueue(envelope).await;
			})
			.await;
	});
}

/// Send a future value to an actor.
///
/// The future's output will be delivered to the actor's mailbox when it resolves.
/// The type constraints ensure that the actor has an unambigious interpretation of the incoming value.
///
/// See also [`send_stream_to`] if you have a stream of items to deliver rather than a single value.
///
/// **N.B**: this function retains the `Arc<dyn Role>` for as long as the future is pending, and will keep the actor alive for that time.
pub fn send_future_to<R, F>(fut: F, actor: Arc<R>)
where
	R: Role + ?Sized + 'static,
	F: Future + Send + 'static,
	R::Payload: From<F::Output>,
{
	tokio_export::spawn(async move {
		let actor = actor;
		let payload = fut.await.into();
		let envelope = ReturnEnvelope {
			payload,
			return_path: ReturnPath::Discard,
		};
		let _ = actor.enqueue(envelope).await;
	});
}

/// Arranges for the *return value* produced by processing the given [`Envelope`] to be forwarded to the recipient actor.
///
/// Equivalent to, but more efficient than, passing the same parameters to [`send_future_to`] **including** that the recipient actor will be kept alive until the message is either processed or the source of the `Envelope` drops
///
/// Can return an Err if the actor originating the Envelope panics before the message is delivered
pub async fn send_to<R, Payload, Sender, RetType>(
	env: Envelope<Sender, RetType>,
	recipient: Arc<R>,
) -> Result<(), Role2SendError<Sender>>
where
	R: Role<Payload = Payload> + ?Sized + 'static,
	Sender: Role,
	Payload: TryFrom<Sender::Return> + Send + 'static,
	RetType: Send + 'static + TryFrom<Sender::Return>,
{
	let (payload, original) = env.unpack();

	let closure = |payload: Sender::Return| -> Pin<Box<dyn Future<Output = ()> + Send>> {
		let discard_envelope = ReturnEnvelope {
			return_path: ReturnPath::Discard,
			payload:     payload.try_into().unwrap_or_else(|_| unreachable!()),
		};
		Box::pin(async move {
			let _ = recipient.enqueue(discard_envelope).await;
		})
	};

	let val: ReturnEnvelope<Sender> = ReturnEnvelope {
		return_path: ReturnPath::Mailbox(Box::new(closure)),
		payload,
	};

	original.enqueue(val).await
}
