//! This is an example shakespeare program that runs an all-to-all chatroom on telnet port 8000.
//!
//! Inputs are lines of UTF-8 text, which must end in `\n` specifically as per [`LinesCodec`]
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use shakespeare::{ActorOutcome, ActorSpawn, Context, Message, MessageStream, actor};
use tokio::net::{TcpListener, TcpStream};
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};

static ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

/// This actor represents a connected client and manages the network socket to that client.
#[actor]
pub mod Client {

	/// [`Client`] internal state, tracking an ID number to identify the client, the socket and handle to a Relay.
	pub struct UserState {
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

	/// This would run when message handlers panic, such as on IO error on write
	/// (We don't distinguish this from a graceful exit for simplicity)
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
			in_stream.send_to(actor_handle.clone() as Arc<dyn NetClient>);
			join_handle.send_to(relay);

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

		/// Send a message to the network socket, passing up any error to the caller
		async fn send_out(&mut self, msg: String) -> Result<(), LinesCodecError> {
			self.out_stream.send(msg).await
		}
	}
}

/// An actor representing the central network loop.
/// This both sets incoming clients and relays incoming messages among the clients that already exist.
#[actor]
pub mod Server {
	#[derive(Default)]
	pub struct ServerState {
		users: HashMap<usize, Arc<dyn NetClient>>,
	}

	impl ServerState {
		/// Broadcast a message to all connected clients
		async fn broadcast(&mut self, msg: String) {
			println!("{msg}");
			let mut dead = vec![];
			for (id, user) in &self.users {
				let msg_success = user.send_out(msg.clone()).await;
				// Check for both successfully sending the message to the actor, and the message
				// executing successfully
				if !matches!(msg_success, Ok(Ok(_))) {
					dead.push(*id);
				}
			}
			for dead in dead {
				Box::pin(self.remove_client(dead)).await;
			}
		}

		/// Removes the tracking data of a disconnected client and announce the departure.
		/// Needs to be separate from [`MsgRelay::client_leaves`] to be able to call from other methods inside the actor.
		async fn remove_client(&mut self, client_id: usize) {
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
		/// The existence of this method triggers the Role macro to implement implement [`shakespeare::Accepts<ActorOutcome<Client>>`] for `MsgRelay`, which then allows [`Message::send_to`] to accept the join handle from spawning a `Client`.
		async fn client_leaves(&mut self, outcome: ActorOutcome<Client>) {
			match outcome {
				ActorOutcome::Exit(client_id) | ActorOutcome::Panic(client_id) => {
					self.remove_client(client_id).await;
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
			let id = ID_COUNTER.fetch_add(1, Ordering::AcqRel);
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

	// Get the client stream from the Listener. Fine to drop anything that causes an IO failure.
	let client_stream = TcpListenerStream::new(listener).filter_map(|r| async { r.ok() });

	let ActorSpawn {
		actor_handle,
		join_handle,
		..
	} = Server::start(ServerState::default());

	client_stream.send_to(actor_handle as Arc<dyn NetListener>);

	join_handle.await;
}
