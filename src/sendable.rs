use std::future::Future;
use std::sync::Arc;

use futures::{pin_mut, Stream, StreamExt};

use crate::{Accepts, ReturnEnvelope, ReturnPath};

/// Extension utilities for [`Future<T>`]. Blanket implemented for all values that meet the requirements.
pub trait Message: Future + Send + 'static {
	/// Send a future value to an actor.
	///
	/// The future's output will be delivered to the actor's mailbox when it resolves.
	/// See the [`Accepts`] documentation for the conditions that allow an actor to use this function.
	///
	/// See also [`MessageStream::send_to`] if you have a [`Stream`] of items to deliver rather than a single value.
	///
	/// **N.B**: this function retains the `Arc<dyn Role>` for as long as the future is pending, and will keep the actor alive for that time.
	fn send_to<R>(self, actor: Arc<R>)
	where
		Self: Sized,
		R: 'static + ?Sized + Accepts<<Self as Future>::Output>,
	{
		tokio::spawn(async move {
			let payload = R::into_payload(self.await);
			let envelope = ReturnEnvelope {
				payload,
				return_path: ReturnPath::Discard,
			};

			let _ = actor.enqueue(envelope).await;
		});
	}
}

impl<T> Message for T where T: Future + Send + 'static {}

/// Extension utilities for [`Stream<T>`]. Blanket implemented for all values that meet the requirements.
pub trait MessageStream: Stream<Item: Send> + Send + 'static {
	/// Subscribes an actor to a [`Stream`], delivering each item of the stream to the actor's mailbox.
	///
	/// See the [`Accepts`] documentation for the conditions that allow an actor to use this function.
	///
	/// This function does not do anything to inform the actor when the stream closes, successfuly or otherwise. If sending the stream item to the actor fails, the stream will be dropped. If an actor explicitly shuts down with an active stream, the stream will be dropped with any remaining items unread. A sent stream prevents an actor shutting down from zero remaining handles until the stream runs out, and conversely, the stream running out will release the held handle.
	fn send_to<R>(self, actor: Arc<R>)
	where
		Self: Sized,
		R: 'static + ?Sized + Accepts<Self::Item>,
	{
		let stream = self;
		tokio::spawn(async move {
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
}

impl<T> MessageStream for T where T: Stream<Item: Send> + Send + 'static {}
