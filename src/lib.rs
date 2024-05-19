#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
//#![warn(missing_docs)]
#![warn(unreachable_pub)]
#![warn(unused)]
#![warn(nonstandard_style)]
#![warn(clippy::pedantic)]
#![warn(clippy::missing_panics_doc)]
#![warn(clippy::dbg_macro)]
//#![warn(unused_crate_dependencies)]
#![forbid(unsafe_code)]
#![warn(clippy::todo)]
#![warn(clippy::unimplemented)]

use std::any::Any;
use std::future::Future;
use std::sync::Arc;

#[doc(hidden)]
pub use ::async_trait as async_trait_export;
#[doc(hidden)]
pub use ::tokio as tokio_export;
use futures::Stream;
pub use shakespeare_macro::{actor, performance, role};
#[doc(hidden)]
pub use tokio::TokioUnbounded;

mod core;
mod returnval;
mod tokio;

pub use core::{
	ActorHandle, ActorOutcome, ActorShell, ActorSpawn, Channel, Role, RoleReceiver, RoleSender,
};

pub use returnval::{Envelope, ReturnEnvelope};

#[doc(hidden)]
pub type Role2Payload<R> = <R as Role>::Payload;
#[doc(hidden)]
pub type Role2Receiver<R> = <<R as Role>::Channel as Channel>::Receiver;
#[doc(hidden)]
pub type Role2Sender<R> = <<R as Role>::Channel as Channel>::Sender;
#[doc(hidden)]
pub type Role2SendError<R> = <Role2Sender<R> as RoleSender<ReturnEnvelope<R>>>::Error;

#[doc(hidden)]
pub fn catch_future<T>(fut: T) -> impl Future<Output = Result<T::Output, Box<dyn Any + Send>>>
where
	T: Future,
{
	futures::future::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(fut))
}

pub fn add_stream<R, S>(actor: Arc<R>, stream: S)
where
	R: Role + ?Sized,
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
					return_path: returnval::ReturnPath::Discard,
				};
				let _ = actor.enqueue(envelope).await;
			})
			.await;
	});
}

pub fn add_future<R, F>(actor: Arc<R>, fut: F)
where
	R: Role + ?Sized,
	F: Future + Send + 'static,
	R::Payload: From<F::Output>,
{
	tokio_export::spawn(async move {
		let actor = actor;
		let payload = fut.await.into();
		let envelope = ReturnEnvelope {
			payload,
			return_path: returnval::ReturnPath::Discard,
		};
		let _ = actor.enqueue(envelope).await;
	});
}
/*
pub async fn send_to<R, P, Sender, RetType>(
	actor: Arc<R>,
	env: Envelope<Sender, RetType>,
) -> Result<(), Role2SendError<Sender>>
where
	R: Role<Payload = P> + Debug,
	Sender: Role,
	P: TryInto<RetType> + Send + 'static,
	RetType: Send + 'static + TryFrom<Sender::Return>,
{
	let (payload, original) = env.unpack();

	let receiver: Arc<dyn ReceiverRole<_>> = actor;

	let val: ReturnEnvelope<Sender> = ReturnEnvelope {
		return_path: returnval::ReturnPath::Mailbox(receiver),
		payload,
	};

	original.enqueue(val).await
}
 */
