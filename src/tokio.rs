use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use super::{RoleReceiver, RoleSender};

impl<T: Send> RoleSender<T> for UnboundedSender<T> {
	type Error = SendError<T>;

	async fn send(&self, msg: T) -> Result<(), SendError<T>> {
		self.send(msg)
	}
}

impl<T: Send> RoleReceiver<T> for UnboundedReceiver<T> {
	async fn recv(&mut self) -> Option<T> {
		self.recv().await
	}

	fn is_empty(&self) -> bool {
		self.is_empty()
	}
}

#[doc(hidden)]
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Default)]
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
