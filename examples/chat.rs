//! This is an example shakespeare program that runs an all-to-all chatroom on telnet port 8000.
//!
//! Once started, press ctrl-C or equivalent to shut it down.
//!
//! Inputs are lines of UTF-8 text, which must end in `\n` specifically as per [`LinesCodec`](`tokio_util::codec::LinesCodec`)
//!
//! The system is set up as a single central relay actor, the [`Server`], and a [`Client`] per connected client.
//! The Server does the following:
//! 1) listens for each incoming [`TcpStream`] and spawns an actor for each. It then hands off the `TcpStream` and a handle to itself (that is, the Server, acting as a [`MsgRelay`]) to the newly created Client.
//! 2) Receives incoming text lines from Clients, and forwards each it receives to all clients, including the original sender
//! 3) If any Clients report IO failures, the Server deletes the client handle and sends a message to other clients that the client has left
//!
//! The Client actor:
//! 1) receives incoming text lines, and forwards them to the Server in its role as a [`MsgRelay`]
//! 2) sends any lines it receives via a [`Client::send_out`] method call (i.e. from the Server) and writes them to the network stream. If an IO failure (including finding that the client has gracefully hung up) happens at this stage, the client's event loop shuts down and new method calls will fail.
//! 3) is kept alive by two `Arc` references - one held by the Server, the other held implicitly as part of the `Stream` forwarding. The latter will be implcitly dropped if an IO condition (including a shutdown at the far end) occurs. The former will be dropped by the Server if it unsuccessfully tries to call a method on the Client, but the only way that can happen is if the actor is already explicitly shut down.
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use shakespeare::{ActorHandles, ActorOutcome, Context, Message, MessageStream, actor};
use tokio::net::{TcpListener, TcpStream};
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};

/// This counts up ID numbers to assign newly joining clients.
///
/// Shakespeare itself does not require any form of unique ID for each actor,
/// This is just to allow following the program's behaviour more easily
static ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

/// This actor represents a connected client and manages the network socket to that client.
#[actor]
pub mod Client {

	/// [`Client`] internal state, tracking an ID number to identify the client, the socket and handle to a Relay.
	pub struct ClientState {
		id:         usize,
		/// Incoming messages are forwarded to the relay to be sent on to everyone else
		relay:      Arc<dyn MsgRelay>,
		/// The stream of outgoing (text) lines
		out_stream: SplitSink<Framed<TcpStream, LinesCodec>, String>,
	}

	/// Called when the actor gracefully exits.
	///
	/// The TcpStream the client is subscribed to will hold an `Arc` until the socket closes, so even if the handles held by the Server are dropped, the Client actor would remain alive. However, calling `stop` inside the Context (as happens on an IO error) will shut down the actor and run *this* handler regardless of active handles.
	///
	/// This function (and `catch`) can have arbitrary return type, and that return type will be tracked as part of the `ActorOutcome`
	fn stop(self) -> usize {
		self.id
	}

	/// This would run when message handlers panic, such as on IO error on write.
	///
	/// (We don't distinguish this from a graceful exit for simplicity)
	fn catch(self, _err: Box<dyn Any + Send>) -> usize {
		self.id
	}

	impl Client {
		/// Create a new Client from a socket, which will then send incoming messages to the relay.
		pub fn new(relay: Arc<dyn MsgRelay>, client: TcpStream, id: usize) -> Arc<Client> {
			// This is normal tokio machinery to turn the TCP stream into a Stream of Strings
			// representing each line of input
			let framed = Framed::new(client, LinesCodec::new());
			let (out_stream, in_stream) = framed.split();

			// The starting values of the new actor.
			let client_state = ClientState {
				id,
				relay: relay.clone(),
				out_stream,
			};

			// Start the actor running and then break out the handles for it
			let ActorHandles {
				message_handle,
				join_handle,
				..
			} = Client::start(client_state);

			// Joins the incoming network stream to the actor, so each incoming String
			// is sent as a message as though [`NetClient::on_read`] had been called with it
			in_stream.send_to(message_handle.clone() as Arc<dyn NetClient>);

			// The join_handle is a Future that yields when the actor (the client we're building) stops
			// We register this future to send its value to the relay so that the relay can then tidy up a client shutting down for internal reasons
			// e.g. network or parse failure
			join_handle.send_to(relay);

			// Return the handle for message-passing back to the caller.
			message_handle
		}
	}

	/// This is a trait for actors that interact with a network socket through a [`LinesCodec`]
	#[performance(canonical)]
	impl NetClient for ClientState {
		/// Process a inputs decoded from the network stream.
		/// If we have a valid line, sent it to the MsgRelay to distribute.
		/// If decoding has failed, shuts the actor down.
		fn on_read(&self, ctx: &'_ mut Context<Self>, val: Result<String, LinesCodecError>) {
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
		/// Begins shut down if the send fails for any reason, but still sends the error back to the caller
		async fn send_out(
			&mut self,
			ctx: &'_ mut Context<Self>,
			msg: String,
		) -> Result<(), LinesCodecError> {
			let result = self.out_stream.send(msg).await;
			if result.is_err() {
				ctx.stop();
			}
			result
		}
	}
}

/// An actor representing the central network loop.
/// This both sets up incoming clients and relays incoming messages among the clients that already exist.
#[actor]
pub mod Server {
	#[derive(Default)]
	pub struct ServerState {
		users: HashMap<usize, Arc<dyn NetClient>>,
	}

	impl ServerState {
		/// Broadcast a message to all connected clients
		async fn broadcast(&mut self, msg: String) {
			println!("{msg}"); // Show the message on the local console as well, for visibility

			// Using this for mark-and-sweep deletes of dead clients.
			// This is somewhat convoluted because we want to announce each client leaving the system
			// and each announcement might discover that other clients have left, recursively.
			let mut dead = vec![];
			for (id, user) in &self.users {
				// After awaiting, `msg_success` will be the return value of `NetClient::send_out`
				// Which is a result of either 1) a successful send, 2) or a LinesCodec error
				// But our communication to the `user` may not be successful e.g. because it has shut down
				// So we have two layers of Result
				let msg_success: Result<Result<(), LinesCodecError>, _> =
					user.send_out(msg.clone()).await;
				// If the actor is still there *and* it successfully wrote the message to the stream, continue.
				// Any other case means the actor has failed somehow and we should disconnect it
				if !matches!(msg_success, Ok(Ok(_))) {
					dead.push(*id);
				}
			}
			for dead in dead {
				// The `remove_client` method calls back into `broadcast`,
				// so we need to have an indirection to avoid having a Future containing itself.
				Box::pin(self.remove_client(dead)).await;
			}
		}

		/// Removes the tracking data of a disconnected client and announce the departure.
		/// Needs to be separate from [`MsgRelay::client_leaves`] to be able to call from other methods inside the actor.
		async fn remove_client(&mut self, client_id: usize) {
			// Its possible this has been called twice for the same actor because e.g. we noticed the connection was dead before the actor wrapped up, so check something was actually removed before doing anything else.
			if self.users.remove(&client_id).is_some() {
				self.broadcast(format!("User {client_id} has left\n")).await;
			}
		}
	}

	/// The message relay shares client messages around to every other connected client
	#[performance(canonical)]
	impl MsgRelay for ServerState {
		/// A client wants a message broadcast
		async fn send_msg(&mut self, sender_id: usize, msg: String) {
			// The `format` call isn't needed for anything except readability, broadcast() takes any string
			self.broadcast(format!("{sender_id:000}: {msg}")).await;
		}

		/// A client left and the actor shutdown, so tell everyone.
		/// The existence of this method (and the fact that it is the only one that accepts `ActorOutcome` as its only parameter) triggers the macros to implement [`Accepts<ActorOutcome<Client>>`](`shakespeare::Accepts`) for `MsgRelay`, which then allows [`Message::send_to`] to accept the join handle from spawning a [`Client`] in [`NetListener::listen`]
		async fn client_leaves(&mut self, outcome: ActorOutcome<Client>) {
			// This happens to work because the Client returns the same type for both a graceful stop and a panic
			// This is not required in general
			let (ActorOutcome::Exit(client_id) | ActorOutcome::Panic(client_id)) = outcome;
			self.remove_client(client_id).await;
		}
	}

	/// Handling incoming new clients from a [`TcpListener`]
	#[performance(canonical)]
	impl NetListener for ServerState {
		/// Set up a newly arrived client up with a new actor to represent them and handle their messages.
		/// Also announces the new entry to existing clients.
		///
		/// As with `client_leaves`, this being the unique method taking a `TcpStream` on the `NetListener` role means `dyn NetListener` implements [`Accepts<TcpStream>`](`shakespeare::Accepts`), which in turn means that the initial setup can call [`MessageStream::send_to`](`shakespeare::MessageStream::send_to`) with the [`Stream<TcpStream>`](`futures::Stream`) it builds out of the [`TcpListener`](`tokio::net::TcpListener`)
		async fn listen(
			&mut self,
			ctx: &'_ mut Context<ServerState>, // this is handled specially by the macros, it does not count for `Accepts`
			tcp_client: TcpStream,
		) {
			// Accessing global state must be synchronised, because actor performances run in their own task, independent of all others
			let id = ID_COUNTER.fetch_add(1, Ordering::AcqRel);

			// This is a plain function call, all the interesting things happen in [`Client::new`]
			let actor = Client::new(ctx.get_shell(), tcp_client, id);
			self.users.insert(id, actor);

			// Make the announcement a new client has joined
			self.broadcast(format!("User {id} has entered\n")).await;
		}
	}
}

#[tokio::main]
async fn main() {
	// This is standard tokio TCP listening.
	let listener = TcpListener::bind("localhost:8000")
		.await
		.expect("Can't listen on port 8000, is it free?");

	// Converts the TcpListener into a `Stream<TcpStream>` representing incoming clients.
	// We drop anything that causes an IO failure at this stage - if this stream finishes or errors, we want existing clients to keep working
	// A production-ready system would have errors reported to the Server too, so that the stream can be restarted on failure
	let client_stream = TcpListenerStream::new(listener).filter_map(|r| async { r.ok() });

	// This starts the Server actor running,
	let ActorHandles {
		message_handle,
		join_handle,
		..
	} = Server::start(ServerState::default());

	// Joins the stream to the actor so that each new incoming `TcpStream` is sent as a message
	// In this case, [`Server::listen`] will be called as each new TcpStream becomes ready.
	client_stream.send_to(message_handle as Arc<dyn NetListener>);

	// Stops the main task until the Server actor shuts down. The value returned would indicate why the actor shut down.
	// This example does not expect that to actually happen - instead, use ctrl-C or equivalent to force shut down.
	let _ = join_handle.await;
}
