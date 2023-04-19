#![allow(warnings)]
use std::sync::Arc;
use std::time::Instant;

use shakespeare_macro::actor;
use tokio::sync::oneshot;

use crate::{self as shakespeare, ActorSpawn};

pub struct AType;
pub struct Pattern;
pub struct Return;

#[shakespeare::actor]
mod OlafActor {
	struct Olaf {
		alive: bool,
	}
	impl Olaf {
		fn speak(&mut self) {
			println!("Hello")
		}
	}

	#[performance(canonical)]
	impl Funcoot for Olaf {
		fn do_thing(&mut self, a_name: AType, another: Pattern) -> Return {
			self.speak();
		}
	}
}

#[tokio::main]
#[test]
async fn ping() {
	let state = Olaf { alive: true };
	let ActorSpawn { actor, .. } = Olaf::start(state);
	assert!(actor.do_thing(AType {}, Pattern {}).await.is_ok());
}

#[shakespeare::actor]
mod ChainActor {
	use std::sync::Arc;
	use std::time::Instant;

	struct ChainIm {
		next: Option<Arc<dyn Chain + Send + Sync>>,
	}

	#[performance(canonical)]
	impl Chain for ChainIm {
		async fn poke(&mut self, start: Instant, sender: tokio::sync::oneshot::Sender<()>) {
			match &self.next {
				Some(next) => next.poke(start, sender).await.unwrap_or_else(|_| panic!()),
				None => println!("{:?}", start.elapsed()),
			}
		}
	}
}

#[tokio::main]
#[test]
async fn chain() {
	let ActorSpawn { actor, .. } = ChainIm::start(ChainIm { next: None });
	let mut actor = actor;
	let begin = Instant::now();

	for _ in 0..1000000 {
		let new_state = ChainIm { next: Some(actor) };
		ActorSpawn { actor, .. } = ChainIm::start(new_state);
	}

	let (sender, receiver) = oneshot::channel();

	let mid = Instant::now();
	println!("{:?}", mid - begin);
	actor.poke(begin, sender);

	receiver.await;
}
