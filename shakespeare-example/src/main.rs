//! This is an example shakespeare program that runs an all-to-all chatroom on telnet port 8000.
//!
//! Inputs are lines of UTF-8 text, which must end in `\n` specifically as per [`LinesCodec`]
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use shakespeare::{actor, send_future_to, send_stream_to, ActorOutcome, ActorSpawn, Context};
use tokio::net::{TcpListener, TcpStream};
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};

static ID_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

/// This actor represents a connected client and manages the network socket to that client.
#[actor]
mod Client {
	/// [`Client`] internal state, tracking an ID number to identify the client, the socket and handle to a Relay.
	struct UserState {
		id:         usize,
		relay:      Arc<dyn MsgRelay>,
		out_stream: SplitSink<Framed<TcpStream, LinesCodec>, String>,
	}

	/// Called when the actor gracefully exits.
	///
	/// The TcpStream the client is subscribed to will hold an `Arc` until the socket closes, so even if the handles held by the Server are dropped, the Client would remain working.
	/// However, calling `stop` inside the Context (as happens on an IO error) will shut down the actor and run *this* handler regardless of active handles.
	fn stop(self) -> usize {
		self.id
	}

	/// This would run if any message handlers panic
	fn catch(self, _err: Box<dyn Any + Send>) -> usize {
		self.id
	}

	impl Client {
		/// Create a new Client from a socket, which will then send incoming messages to the relay.
		pub fn new(relay: Arc<dyn MsgRelay>, client: TcpStream, id: usize) -> Arc<Client> {
			let framed = Framed::new(client, LinesCodec::new());
			let (out_stream, in_stream) = framed.split();

			let ActorSpawn {
				actor_handle,
				join_handle,
				..
			} = Client::start(UserState {
				id,
				relay: relay.clone(),
				out_stream,
			});
			send_stream_to::<dyn NetClient, _>(in_stream, actor_handle.clone());
			send_future_to(join_handle, relay);

			actor_handle
		}
	}

	/// This is a trait for actors that interact with a network socket through a [`LinesCodec`]
	#[performance(canonical)]
	impl NetClient for UserState {
		/// Process a line read from the decoded network stream
		fn on_read(&self, ctx: &'_ mut Context<UserState>, val: Result<String, LinesCodecError>) {
			match val {
				Ok(msg) => {
					self.relay.send_msg(self.id, msg);
				}
				Err(LinesCodecError::MaxLineLengthExceeded) => { /* Do nothing, we don't have a maximum length */
				}
				Err(LinesCodecError::Io(_)) => {
					ctx.stop();
				}
			}
		}

		/// Send a message to the network socket
		async fn send_out(&mut self, msg: String) -> Result<(), LinesCodecError> {
			self.out_stream.send(msg).await
		}
	}
}

/// An actor representing the central network loop.
/// This both sets incoming clients and relays incoming messages among the clients that already exist.
#[actor]
mod Server {
	#[derive(Default)]
	struct ServerState {
		users: HashMap<usize, Arc<dyn NetClient>>,
	}

	impl ServerState {
		/// Broadcast a message to all connected clients
		async fn broadcast(&mut self, msg: String) {
			println!("{msg}");
			let mut dead = vec![];
			for (id, user) in &self.users {
				let msg_success = user.send_out(msg.clone()).await;
				if !matches!(msg_success, Ok(Ok(_))) {
					dead.push(*id);
				}
			}
			for dead in dead {
				Box::pin(self.client_leaves(dead)).await;
			}
		}

		/// Removes the tracking data of a disconnected client and announce the departure.
		/// Needs to be separate from [`MsgRelay::client_leaves`] to be able to call from other methods inside the actor.
		async fn client_leaves(&mut self, client_id: usize) {
			self.users.remove(&client_id);
			self.broadcast(format!("User {client_id} has left\n")).await;
		}
	}

	/// The message relay shares client messages around to every other connected client
	#[performance(canonical)]
	impl MsgRelay for ServerState {
		/// A client wants a message broadcast
		async fn send_msg(&mut self, sender_id: usize, msg: String) {
			self.broadcast(format!("{sender_id:000}: {msg}")).await;
		}

		/// A client left and the actor shutdown, so tell everyone.
		/// The existence of this method triggers the Role macro to implement implement [`shakespeare::Accepts<ActorOutcome<Client>>`] for `MsgRelay`, which then allows `send_future_to` to accept the join handle from spawning a `Client`.
		async fn client_leaves(&mut self, outcome: ActorOutcome<Client>) {
			match outcome {
				ActorOutcome::Exit(client_id) | ActorOutcome::Panic(client_id) => {
					self.client_leaves(client_id).await;
				}
				_ => unimplemented!(),
			}
		}
	}

	/// Handling incoming new clients from a [`TcpListener`]
	#[performance(canonical)]
	impl NetListener for ServerState {
		/// Set up a newly arrived client up with a new actor to represent them and handle their messages.
		/// Also announces the new entry to existing clients.
		async fn listen(&mut self, ctx: &'_ mut Context<ServerState>, tcp_client: TcpStream) {
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
		.expect("Can't listen on port 8000, is it free?");

	let client_stream = TcpListenerStream::new(listener).filter_map(|r| async { r.ok() });

	let server = Server::start(ServerState::default());

	send_stream_to::<dyn NetListener, _>(client_stream, server.actor_handle);

	server.join_handle.await;
}
