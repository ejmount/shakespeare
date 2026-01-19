//! [![Github link](https://img.shields.io/badge/github-ejmount%2Fshakespeare-blue)](https://github.com/ejmount/shakespeare)
//! [![Crates.io Version](https://img.shields.io/crates/v/shakespeare)](https://crates.io/crates/shakespeare)
//!
//! Shakespeare is an [actor system](https://en.wikipedia.org/wiki/Actor_model) focused on rich interfaces and polymorphism without impacting runtime performance.
//!
//! Shakespeare is based around three concepts:
//! * [Actors](`shakespeare_macro::actor`) - objects that can asynchronously receive messages and run user-defined code in response. Actors contain state of an arbitrary type; message handlers can be arbitrary code, although will most commonly include updating their state, sending new messages and/or spawning additional actors.
//!    * Some parts of this documentation refer specifically to the "actor shell", which is the automatically generated proxy object that handles the message passing into and out of the code you have written defining the actor's methods and state.
//!    * There is no "world" or "system" boundary in Shakespeare - any part of your program can call methods on any actor for which it has a handle, and any part can spawn new actors.
//! * [Roles](`shakespeare_macro::role`) - traits for actors. A Role defines a set of methods that can be asynchronously called on an actor and which may be asynchronous themselves. These methods return a [`Envelope`] to external callers, which is a `Future` of a return value, if any - the caller can use the envelope object to decide whether (and how) to await for any return value that the method may produce.
//! * [Performances](`shakespeare_macro::performance`) - the block of code that actually defines how a given actor implements a role. This is in most ways a normal trait implementation, with the macro generating all of the glue code needed to call the defined methods as part of the actor's message loop.
//!
//! ## Using actors
//!
//! With a tokio runtime running, (e.g. via `#[tokio::main]`) an actor instance is created by calling the `start` function on the actor's shell type, which consumes a value of the corresponding state type. The `start` function is currently defined in a way that means it is private and a sibling of the `#[actor]` module that defines the overall actor - if you want to expose it more widely (including outside of the crate, i.e. `pub`) you will have to define a wrapper function for now. This requirement may be relaxed in a future version.
//!
//! The `start` function returns a [`ActorHandles`], which contains two values:
//! * an [`ExitHandle`] - this is a [`Future`][`std::future::Future`] that yields when the actor exits, the value of which indicates whether the actor exited gracefully (all handles were dropped or a handler explicitly shut it down) or by one of the message handlers panicking. Dropping this value doesn't affect the actor's execution
//! * the `message_handle` - an [`Arc`][`std::sync::Arc`] containing the shell type, the first handle to the new actor. If all copies of this value drop, the actor begins shutting down.
//!
//! At this point, the actor is active as a new task, separate from the one that called `start`. Calling a method on the shell type via the `message_handle` will send a message to the actor's mailbox containing the method's parameter values. The actor will then service this message by calling the corresponding method from the matching `performance` block. (This can be seen as the asynchronous version of calling a trait method on an trait object - the lookup is handled automatically.) If the method returns a value, this return value will be passed back to the caller automatically as described below.
//!
//! Where a role defines a method that returns a value of type `T`, the call on the shell type instead returns [`Envelope<T>`], which is then used to access the return value the actor produced. For full details, see that type's documentation, but in most cases, you will want to do one of two things with it:
//! * `await` it - an `Envelope` implements [`IntoFuture`][`std::future::IntoFuture`] and will yield a value of `Ok(T)` containing the return value, unless the actor that produced the `Envelope` shut down before processing the message.
//! * allow it to drop, which will dispatch the message to the destination actor's mailbox but not wait for any return value. This option does not allow checking for failure.
//!
//! Actors can also receive general [`Future`] and [`Stream`][`futures::Stream`] values as messages, using [`Message::send_when_ready`] and [`MessageStream::feed_to`]. These functions work on roles that implement the [`Accepts`] trait, which is implemented for roles which have exactly one method that takes a given tuple of parameters. (And is implemented separately for each such tuple.)
//!
//! **Note**: The API is designed to allow code to work with dynamically typed actors of a given role by using values of type `Arc<dyn Role>`, which `Arc<A>` can be upcast to by normal language rules. This construction does mean that the compiler may need help to correctly disambiguate [`Message::send_when_ready`] (and similar) calls.
//!
//!
//! ## Defining an actor
//!
//! The starting point for defining a new actor is an `#[actor]` module block. The name of the actor shell type (from which `start` is called) is defined by the name of the module. As an example, to start the following, one would call `MyActor::start(MyState)`:
//!
//! ```
//! # use shakespeare::{actor, performance};
//!
//! #[actor]
//! mod MyActor {
//! 	// ⇧ This name can be any valid identifier
//! 	enum MyState {
//! 		// ⇧ This name can also be any valid type name
//! 		// Could also be a struct or union
//! 		SomeData(usize),
//! 		OtherData(String),
//! 		Empty,
//! 	}
//! # 	#[performance(canonical)] impl Empty for MyState { fn method(&self) {}}
//! 	// ...
//! }
//! ```
//!
//! The module can be named anything. It needs to contain exactly one `struct`, `enum` *or* `union`, which serves as the state type of the actor. This type can similarly have any name that does not collide with the module name, but must be `'static`, `Sized`, and cannot contain free generic parameters (`enum MyState<T>` would not be allowed) but there are no other restrictions on its content. It does not move after the actor is started, so storing large amounts of data inline is not a performance concern.
//!
//! While inherent `impl MyState` blocks are allowed within the module, there is currently no support for calling those methods on the actor state from outside the actor, as the state value is not externally accessible - all externally callable methods must be defined on a performance, discussed below. (See also `canonical` performances later for a simplification of the common case.) Methods from such a block *can* be called normally on the state value from within a performance or on state values *before* they are consumed by `start` - there is nothing unusual about the state type itself, the relevant values are simply only accessible at the point they are passed as parameters to a performance. Free functions (i.e. with no `self`) defined inside inherent `impl` blocks can also be called as normal.
//!
//! ### Performances
//!
//! The module must also contain at least one `#[performance]` trait impl block:
//!
//! ```
//! # use shakespeare::{actor, performance, role};
//! #[role]
//! trait ARole {
//! 	fn a_method(&self) -> AnyReturnType;
//! }
//! # struct AnyReturnType;
//! #[actor]
//! mod MyActor {
//! 	struct MyState;
//! 	#[performance]
//! 	impl ARole for MyState {
//! 		//          ⇧ the type name should match the name you used previously
//! 		async fn a_method(&mut self /* any parameters */) -> AnyReturnType {
//! 			// any code can go here
//! 			unimplemented!()
//! 		}
//! 		// any other methods the ARole trait requires
//! 	}
//! }
//! ```
//!
//! A performance is written like an ordinary trait implementation with a caveat: all methods in the trait are in reality `async` and receiving `&mut self`, but as with normal traits, method implementations that do not `await` anything can leave off the `async` keyword, and implementations that do not mutate the state object can take plain `&self`. (As might be expected, the `Self` type is the state type, e.g. `MyState`)
//!
//!
//! **N.B.** While an actor's message handler can `await` futures (whether from an [`Envelope`] or otherwise) the event loop cannot resume until the method returns. This risks deadlocks: if actor A sends and then awaits a message to actor B, and in response actor B sends and awaits a message to actor A, the two wil deadlock because A cannot service further messages (including the one B sent it) until it receives a response from  B, which is waiting on A. If you need to handle the return value from calling another actor (B) without blocking the original sender (A) by waiting on it, consider using [`Message::send_when_ready`] or [`MessageStream::feed_to`] and passing the sender's handle.
//!
//! A performance implementation is allowed to be outside of the actor `mod` scope, in the same way that any other `impl ... for` block can be outside the module that defines the struct, but if it is elsewhere, the actor `mod` must contain a `#[performance] impl ARole for MyActor {}` block, including empty braces. A more detailed description along with other caveats can be found in [the full macro documentation](shakespeare_macro::performance).
//!
//! Defining a role is syntactically a normal trait declaration, just with the macro attached:
//!
//! ```
//! # use shakespeare::role;
//! #[role]
//! trait ARole {
//! 	fn a_method(&self);
//! 	fn another_method(&self, a: usize) -> String;
//! 	// Any number of other methods
//! 	// Methods can take any number of parameters and return almost any type
//! }
//! ```
//!
//! The methods on this trait should *not* be marked `async`, that is handled by the macro. While a role can have any number of methods, with any names and with any number of parameters, there are a number of restrictions on the types that can be used in the parameters and return values - see the [the macro documentation](shakespeare_macro::role) for the specifics.
//!
//! It's expected that many traits have a single "primary" implementation, such as an application object having a particular interface that testing mock implementations are then defined by. To simplify this situation, instead a role can be defined implicitly via a performance by providing:
//! ```ignore
//! #[performance(canonical)]
//! impl ARole for MyState {
//!     ... // the trait ARole will be defined as part of generating the performance
//! }
//! ```
//!
//! ### Miscellanea
//!
//! A method inside a performance can define its *second* parameter (i.e. the one immediately after the `self`) as having a type of `&'_ mut Context<Self>` to get access to the [`Context`] object for the current actor, which includes the capability of getting the current actor's handle or shutting it down early. (If only a shared borrow is required, `&'_ Context<Self>` is also allowed.) The context parameter should *not* be included in any explicitly defined roles, and roles defined by `canonical` performances take this into account.
//!
//!
//! ## Actor Lifetime
//!
//! There are several events in the actor's lifecycle that are accessed by optionally defining free-standing (i.e. outside of any `impl`) functions within the `#[actor]` module. Their names, inputs and events are:
//!
//! * `stop(self)` - is called with the final value of the actor's state object when the actor shuts down without panicking
//!	* `catch(self, Box<dyn Any + Send>)` - called in the event a method handler panics, being provided the final state value and the value passed to the `panic!` call
//!
//! Both of these functions can have any `'static + Sized` return type, and any return values from these functions will be passed back to the [`ExitHandle`].
//!
//! **N.B.**: The `catch` function is not technically running in an unwinding context, so a secondary panic will not abort the process. However, Shakespeare leaves behaviour in the event of panic within `catch` unspecified except that safety is upheld, and **the exact behaviour may change even in patch releases**.
//!
//!
//! ### Start
//!
//! Calling `Actor::start(state)` spawns a new task for handling the actor's event loop. It is expected that you do any setup needed for the actor to be in a ready state in the construction of the `state` value itself, and there is currently no interface for running user code after the actor is constructed but before the event loop proper begins. However, while the event loop has technically started by the time that `start` returns, the only way to provide messages to process is via the handle coming out of `start` - there is no global broadcasting that might pre-empt this. This means that if a "guaranteed first call" is needed, this can be achieved by simply sending a message and waiting for a response before sharing the handle.
//!
//! Once the event loop is established, it awaits a message being sent as a result of a call made to a method on the shell object, via any of the roles the actor might have, and then calls the appropriate method from the corresponding `performance` for any it receives.
//!
//! ### Synchronisation
//!
//! The order the actor responds to calls from different tasks is unspecified. The order the actor responds to calls made via two different roles is unspecified *even from the same task or from the same handle.* Calls made by an actor's own performances count as being made on the same task as each other.
//!
//! A call will *happen-before* another call if all of the following hold:
//!   1) both calls are made via methods (not necessarily the same method) defined by the same role
//!   2) both calls are made from the same task
//!   3) the [`Envelope`] returned from the first call is `await`ed before the second call is made. If you do not need the return value of the first call, you must use [`Envelope::ignore_return`] to await only the sending, as implicitly dropping the `Envelope` by letting it leave scope does *not* meet this condition. (Dropped envelopes have no particular ordering with respect to anything, including other such envelopes.)
//!
//! ### Shutting down
//!
//! The actor can stop processing messages and shut down in several circumstances:
//!
//! 1. If a message handler panics, `catch` is called (or the panic value passed straight up to the [`ExitHandle`] if there is no `catch`) immediately. No further messages are processed, and attempting to send messages to the actor will fail by returning `Err` to the caller via the [`Envelope`].
//! 2. If the [`Context::stop`] is called, no further messages are processed, calls against the actor will return `Err`, but the actor's `stop` function is called rather than `catch`. This similarly passes the returned value up to the [`ExitHandle`].
//! 3. If the `Arc` that was returned from `start` and all of its copies drop, *and* no further messages are waiting to be processed, `stop` will be called as in case 2. By definition, it is not possible for an external client to be sending messages to the actor at this point. (Note that functions directly subscribing the actor to a future result, such as [`MessageStream::feed_to`] implicitly hold an `Arc` and will preclude this case until that value yields to exhaustion.) As explained below, there is currently an indeterminate delay between this condition becoming true and `stop` actually being called - to be sure that the actor has completely stopped, `await` the [`ExitHandle`].
//!
//! **N.B:** Because method implementations can get hold of the actor's own handle via the [`Context`], then even if all other copies have dropped at any given time, a running event handler can "save" the actor by sending a new copy of the handle out of the actor. This is not treated as the actor being revived from having shut down, but instead it has not shut down in the first place.
//!
//! As an implementation detail of making all of the above work, *every actor* has a watchdog timer that fires intermittently to check for case 3 above, *whether or not* handles to the actor remain live. As a result, there is both a marginal amount of CPU use even by idle actors, and in some circumstances also a finite "finalization" interval between processing stopping (i.e. the later of the last handle dropping and the last message handler completing) and the actor beginning to shut down by calling `stop`. The exact length and behaviour of this watchdog **is not part of ``SemVer`` compatibility**, and the behaviour may vary in future versions. Currently, this timer goes off 1 second (1000ms) after the last message was received, and recurs at the same rate if the actor is still alive at that point. This is considered a design issue and may be removed entirely in future versions. If [`Context::stop`] is called or if the last message remaining in the queue is handled *after* the last message handle has dropped, there is no wait for the watchdog and the actor will shut down immediately after the last (or current, for `stop`) message handler finishes.

#![forbid(unsafe_code)]
#![forbid(future_incompatible)]
#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(unreachable_pub)]
#![warn(unused)]
#![warn(nonstandard_style)]
#![warn(clippy::pedantic)]
#![warn(clippy::missing_panics_doc)]
#![warn(clippy::dbg_macro)]
#![warn(clippy::todo)]
#![warn(clippy::unimplemented)]
#![allow(clippy::tabs_in_doc_comments)]

use std::any::Any;
use std::future::Future;

#[doc(hidden)]
pub use ::async_trait as async_trait_export;
#[doc(hidden)]
pub use ::tokio as tokio_export;
pub use shakespeare_macro::{actor, performance, role};
#[doc(hidden)]
pub use tokio::TokioUnbounded;

mod core;
mod sendable;
mod tokio;

pub use core::{
	Accepts, ActorHandles, Context, Emits, Envelope, EnvelopeErr, ExitHandle,
	Outcome as ActorOutcome, Role, Shell as ActorShell, State as ActorState,
};
#[doc(hidden)]
pub use core::{
	Channel, Receiver as RoleReceiver, ReturnCaster, ReturnEnvelope, ReturnPath,
	Sender as RoleSender,
};

pub use sendable::{Message, MessageStream};

#[doc(hidden)]
pub type Role2Payload<R> = <R as Role>::Payload;
#[doc(hidden)]
pub type Role2Receiver<R> = <<R as Role>::Channel as Channel>::Receiver;
/// Internal use only
///
/// Shortcut to resolve a Role's channel's sender type.
type Role2Sender<R> = <<R as Role>::Channel as Channel>::Sender;
/// Internal - the type returned if a Role fails to send a message. For now, this is always [`SendError`](`crate::tokio_export::sync::mpsc::error::SendError`)
pub type Role2SendError<R> = <Role2Sender<R> as RoleSender<ReturnEnvelope<R>>>::Error;

#[doc(hidden)]
/// Create a new future that will wrap the given future and catch any panic.
/// Used by [`::shakespeare_macro::actor::output::SpawningFunction`]
/// Included here to avoid clients having to depend on `futures` crate
pub fn catch_future<T>(fut: T) -> impl Future<Output = Result<T::Output, Box<dyn Any + Send>>>
where
	T: Future,
{
	futures::future::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(fut))
}

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
