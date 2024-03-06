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
#![forbid(clippy::unimplemented)]

use std::any::Any;
use std::future::Future;
use std::sync::Arc;

use ::tokio::sync::mpsc::error::SendError;
use ::tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use ::tokio::task::JoinHandle;
use async_trait::async_trait;
pub use shakespeare_macro::{actor, performance, role};

#[doc(hidden)]
pub use crate::tokio::TokioUnbounded;

#[non_exhaustive]
#[derive(Debug)]
pub struct ActorSpawn<T> {
	pub actor:    Arc<T>,
	_join_handle: JoinHandle<Result<(), Box<dyn Any + Send>>>,
}

impl<T> ActorSpawn<T> {
	#[doc(hidden)]
	pub fn new(
		actor: T,
		join_handle: JoinHandle<Result<(), Box<dyn Any + Send>>>,
	) -> ActorSpawn<T> {
		ActorSpawn {
			actor:        Arc::new(actor),
			_join_handle: join_handle,
		}
	}
}

#[doc(hidden)]
#[async_trait]
pub trait RoleSender<T: Send>: Sync {
	type Error;
	async fn send(&self, msg: T) -> Result<(), Self::Error>;
}

#[doc(hidden)]
#[async_trait]
pub trait RoleReceiver<T: Send> {
	async fn recv(&mut self) -> Option<T>;
}

#[doc(hidden)]
mod tokio {
	#[allow(clippy::wildcard_imports)]
	use super::*;

	#[async_trait]
	impl<T: Send> RoleSender<T> for UnboundedSender<T> {
		type Error = SendError<T>;

		async fn send(&self, msg: T) -> Result<(), SendError<T>> {
			self.send(msg)
		}
	}

	#[async_trait]
	impl<T: Send> RoleReceiver<T> for UnboundedReceiver<T> {
		async fn recv(&mut self) -> Option<T> {
			self.recv().await
		}
	}

	#[doc(hidden)]
	#[allow(clippy::module_name_repetitions)]
	#[derive(Debug)]
	pub struct TokioUnbounded<T>(std::marker::PhantomData<T>);
	impl<T: Send> super::Channel for TokioUnbounded<T> {
		type Input = ();
		type Item = T;
		type Receiver = UnboundedReceiver<T>;
		type Sender = UnboundedSender<T>;

		fn new((): ()) -> (UnboundedSender<T>, UnboundedReceiver<T>) {
			unbounded_channel()
		}
	}
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
pub trait Role: 'static {
	type Payload: Sized + Send;
	type Channel: Channel<Item = Self::Payload>;
}

#[doc(hidden)]
pub fn catch_future<T>(fut: T) -> impl Future<Output = Result<T::Output, Box<dyn Any + Send>>>
where
	T: Future,
{
	futures::future::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(fut))
}
