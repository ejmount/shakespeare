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

pub use core::{
	Accepts, Emits, Envelope, Handle as ActorHandle, Outcome as ActorOutcome, Role,
	Shell as ActorShell, Spawn as ActorSpawn, State as ActorState,
};
#[doc(hidden)]
pub use core::{
	Channel, Context, Receiver as RoleReceiver, ReturnCaster, ReturnEnvelope, ReturnPath,
	Sender as RoleSender,
};

use futures::{pin_mut, Stream, StreamExt};

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
/// This function does not do anything to inform the actor when the stream closes, successfuly or otherwise. If sending the message to the actor fails, the stream will be dropped.
///
/// **N.B**: this function retains the `Arc<dyn Role>` for as long as the stream is still active, and will keep the actor alive for that time.
pub fn send_stream_to<R, S>(stream: S, actor: Arc<R>)
where
	R: Accepts<S::Item> + ?Sized + 'static,
	S: Stream + Send + 'static,
	<S as Stream>::Item: Send,
{
	tokio_export::spawn(async move {
		pin_mut!(stream);
		while let Some(msg) = stream.next().await {
			let payload = R::into_payload(msg);
			let envelope = ReturnEnvelope {
				payload,
				return_path: ReturnPath::Discard,
			};
			if actor.enqueue(envelope).await.is_err() {
				break;
			}
		}
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
	R: Accepts<F::Output> + ?Sized + 'static,
	F: Future + Send + 'static,
{
	tokio_export::spawn(async move {
		let actor = actor;
		let payload = R::into_payload(fut.await);
		let envelope = ReturnEnvelope {
			payload,
			return_path: ReturnPath::Discard,
		};
		let _ = actor.enqueue(envelope).await;
	});
}

/// Arranges for the *return value* produced by processing the given [`Envelope`] to be forwarded to the recipient actor. Any return value produced by the receipient is ignored.
///
/// Equivalent to, but more efficient than, passing the same parameters to [`send_future_to`] **including** that the recipient actor will be kept alive until the message is either processed or the source of the `Envelope` drops
///
/// Can return an Err if the actor originating the Envelope panics before the message is delivered
pub async fn send_return_to<RxRole, SendingRole, BridgeType>(
	env: Envelope<SendingRole, BridgeType>,
	recipient: Arc<RxRole>,
) -> Result<(), Role2SendError<SendingRole>>
where
	SendingRole: Emits<BridgeType> + ?Sized + 'static,
	RxRole: Accepts<BridgeType> + ?Sized + 'static,
{
	let (payload, original) = env.unpack();

	let bridge_to_rx_role = |sender_payload| -> Pin<Box<dyn Future<Output = ()> + Send>> {
		let discard_envelope = ReturnEnvelope {
			return_path: ReturnPath::Discard,
			payload:     RxRole::into_payload(SendingRole::from_return_payload(sender_payload)),
		};
		Box::pin(async move {
			let _ = recipient.enqueue(discard_envelope).await;
		})
	};

	let val: ReturnEnvelope<SendingRole> = ReturnEnvelope {
		return_path: ReturnPath::Mailbox(Box::new(bridge_to_rx_role)),
		payload,
	};

	original.enqueue(val).await
}

#[doc(hidden)]
#[doc = include_str!("../README.md")]
#[expect(dead_code)]
struct ReadmeDoctests;
