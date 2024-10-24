#![warn(missing_docs)]

//! This is an example shakespeare program that runs an all-to-all chatroom on telnet port 8000.
use std::collections::HashMap;
use std::sync::Arc;

use futures::{SinkExt, StreamExt};
use shakespeare::{actor, send_future_to, send_stream_to, ActorOutcome, ActorSpawn, Context};
use tokio::net::{TcpListener, TcpStream};
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec, LinesCodecError};

static ID_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

/// Docstring for the Client actor
#[actor]
mod Client {
	struct UserState {
		id:         usize,
		relay:      Arc<dyn MsgRelay>,
		out_stream: FramedWrite<tokio::net::tcp::OwnedWriteHalf, LinesCodec>,
	}

	fn stop(s: UserState) -> usize {
		s.id
	}

	/// This is a trait for actors that interact with a network socket
	#[performance(canonical)]
	impl NetClient for UserState {
		/// Process a line read from the decoded network stream
		fn on_read(&self, ctx: &'_ mut Context<UserState>, val: Result<String, LinesCodecError>) {
			match val {
				Ok(msg) => {
					self.relay.send_msg(self.id, msg);
				}
				Err(LinesCodecError::MaxLineLengthExceeded) => { /* Do nothing */ }
				Err(LinesCodecError::Io(_)) => {
					ctx.stop();
				}
			}
		}

		/// Send a message to the network socket
		async fn send_out(&mut self, msg: String) {
			self.out_stream.send(msg).await.unwrap();
		}
	}
}

impl Client {
	/// Create a new Client from a socket, which will then send incoming messages to the relay.
	pub fn new(relay: Arc<dyn MsgRelay>, client: TcpStream, id: usize) -> Arc<Client> {
		let (in_stream, out_stream) = client.into_split();

		let codec_writer = FramedWrite::new(out_stream, LinesCodec::new());
		let codec_reader = FramedRead::new(in_stream, LinesCodec::new());

		let ActorSpawn {
			msg_handle,
			join_handle,
			..
		} = Client::start(UserState {
			id,
			relay: relay.clone(),
			out_stream: codec_writer,
		});
		send_stream_to::<dyn NetClient, _>(codec_reader, msg_handle.clone());
		send_future_to(join_handle, relay);

		msg_handle
	}
}

#[actor]
mod Server {
	#[derive(Default)]
	struct ServerState {
		users: HashMap<usize, Arc<dyn NetClient>>,
	}

	impl ServerState {
		// Broadcast a message to all connected clients
		async fn broadcast(&mut self, msg: String) {
			println!("{}", msg);
			let mut dead = vec![];
			for (id, user) in &self.users {
				if user.send_out(msg.clone()).await.is_err() {
					dead.push(*id);
				}
			}
			for dead in dead {
				Box::pin(self.client_leaves(dead)).await;
			}
		}

		async fn client_leaves(&mut self, client_id: usize) {
			self.users.remove(&client_id);
			self.broadcast(format!("User {client_id} has left\n")).await;
		}
	}

	#[performance(canonical)]
	impl MsgRelay for ServerState {
		/// A client wants a message broadcast
		async fn send_msg(&mut self, sender_id: usize, msg: String) {
			self.broadcast(format!("{sender_id:000}: {msg}")).await;
		}

		// A client left and the actor shutdown
		async fn client_leaves(&mut self, outcome: ActorOutcome<Client>) {
			match outcome {
				ActorOutcome::Exit(client_id) => {
					self.client_leaves(client_id).await;
				}
				_ => unimplemented!(),
			}
		}
	}

	#[performance(canonical)]
	impl NetListener for ServerState {
		async fn listen<'a>(
			&mut self,
			ctx: &'a mut shakespeare::Context<ServerState>,
			tcp_client: TcpStream,
		) {
			let id = ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
			let actor = Client::new(ctx.get_shell(), tcp_client, id);
			self.users.insert(id, actor);
			self.broadcast(format!("User {id} has entered\n")).await;
		}
	}
}

#[tokio::main]
async fn main() {
	let listener = TcpListener::bind("localhost:8000")
		.await
		.expect("Can't listen");

	let client_stream = TcpListenerStream::new(listener).filter_map(|r| async { r.ok() });

	let server = Server::start(ServerState::default());

	send_stream_to::<dyn NetListener, _>(client_stream, server.msg_handle);

	server.join_handle.await;
}
