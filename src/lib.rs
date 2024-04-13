#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
//#![warn(missing_docs)]
#![warn(unused)]
#![warn(nonstandard_style)]
#![warn(clippy::pedantic)]
#![warn(clippy::missing_panics_doc)]
#![warn(clippy::dbg_macro)]
//#![warn(unused_crate_dependencies)]
#![forbid(unsafe_code)]
#![forbid(clippy::todo)]
//#![forbid(clippy::unimplemented)]

use std::any::Any;
use std::future::Future;

#[doc(hidden)]
pub use ::tokio as tokio_export;
use futures::{stream, Stream};
pub use shakespeare_macro::{actor, performance, role};
#[doc(hidden)]
pub use tokio::TokioUnbounded;

mod core;
mod tokio;

pub use core::{ActorShell, ActorSpawn, Channel, Role, RoleReceiver, RoleSender};

#[doc(hidden)]
pub type Role2Payload<R> = <R as Role>::Payload;
#[doc(hidden)]
pub type Role2Receiver<R> = <<R as Role>::Channel as Channel>::Receiver;
#[doc(hidden)]
pub type Role2Sender<R> = <<R as Role>::Channel as Channel>::Sender;
#[doc(hidden)]
pub type Role2SendError<R> = <Role2Sender<R> as RoleSender<<R as Role>::Payload>>::Error;

#[doc(hidden)]
pub fn catch_future<T>(fut: T) -> impl Future<Output = Result<T::Output, Box<dyn Any + Send>>>
where
	T: Future,
{
	futures::future::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(fut))
}

pub fn add_stream<R, S>(actor: &R, stream: S)
where
	R: Role + ?Sized,
	S: Stream<Item: Send + 'static> + Send + 'static,
	R::Payload: From<S::Item>,
{
	use futures::StreamExt;
	let sender = actor.clone_sender();
	crate::tokio_export::spawn(async move {
		let sender = sender;
		stream
			.for_each(|msg| async {
				let _ = sender.send(msg.into()).await;
			})
			.await;
	});
}

pub fn add_future<R, F>(actor: &R, fut: F)
where
	R: Role + ?Sized,
	F: Future<Output: Send + 'static> + Send + 'static,
	R::Payload: From<F::Output>,
{
	add_stream(actor, stream::once(fut));
}
