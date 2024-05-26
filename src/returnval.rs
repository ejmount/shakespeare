use std::fmt::Debug;
use std::future::IntoFuture;
use std::marker::PhantomData;
use std::pin::{pin, Pin};
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::Future;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::oneshot::{Receiver, Sender};

use crate::{add_future, Role};

type PinnedAction = Pin<Box<dyn Future<Output = ()>>>;

#[derive(Default)]
pub enum ReturnPath<Payload: Send> {
	#[default]
	Discard,
	Mailbox(Box<dyn Send + FnOnce(Payload) -> PinnedAction>),
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

		println!("Sending return... ",);

		match self {
			Discard => (),
			Mailbox(callback) => callback(val).await,
			Immediate(channel) => {
				let _ = channel.send(val).map_err(drop);
			}
		};
	}
}

#[derive(Debug)]
pub struct Envelope<R, V>
where
	R: Role + ?Sized,
	V: TryFrom<R::Return>,
{
	val:  Option<R::Payload>,
	dest: Option<Arc<R>>,
	_v:   PhantomData<V>,
}

impl<R: Role + ?Sized, V: TryFrom<R::Return>> Envelope<R, V> {
	pub fn new(val: impl Into<R::Payload>, dest: Arc<R>) -> Envelope<R, V> {
		Envelope {
			val:  Some(val.into()),
			dest: Some(dest),
			_v:   PhantomData {},
		}
	}

	pub(crate) fn unpack(mut self) -> (R::Payload, Arc<R>) {
		let val = (self.val.take().unwrap(), self.dest.take().unwrap());
		std::mem::forget(self);
		val
	}
}

impl<R: Role + ?Sized> Envelope<R, R::Return> {
	pub fn downcast<V: TryFrom<R::Return>>(self) -> Envelope<R, V> {
		let (val, dest) = self.unpack();
		Envelope {
			val:  Some(val),
			dest: Some(dest),
			_v:   PhantomData {},
		}
	}
}

impl<R: Role + ?Sized, V: TryFrom<R::Return>> IntoFuture for Envelope<R, V> {
	type IntoFuture = ReturnCaster<R, V>;
	type Output = std::result::Result<V, RecvError>;

	fn into_future(self) -> Self::IntoFuture {
		let (payload, dest) = self.unpack();

		let (return_path, rx) = ReturnPath::create_immediate();

		let envelope: ReturnEnvelope<R> = ReturnEnvelope {
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

impl<R: Role + ?Sized, V: TryFrom<R::Return>> Drop for Envelope<R, V> {
	fn drop(&mut self) {
		let val = self.val.take().unwrap();
		let dest = self.dest.take().unwrap();

		add_future(dest, std::future::ready(val));
	}
}

#[doc(hidden)]
#[pin_project::pin_project]
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
	R: Role + ?Sized,
	V: TryFrom<R::Return>,
{
	type Output = std::result::Result<V, RecvError>;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let inner = self.project().future;

		inner.poll(cx).map(|val| {
			val.map(|returned_payload| {
				returned_payload
					.try_into()
					.unwrap_or_else(|_| unreachable!("Conversion error"))
			})
		})
	}
}

pub struct ReturnEnvelope<R: Role + ?Sized> {
	pub payload:     R::Payload,
	pub return_path: ReturnPath<R::Return>,
}

impl<R: Role + ?Sized> Debug for ReturnEnvelope<R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "<ReturnEnvelope>")
	}
}
