# Shakespeare

![Crates.io Version](https://img.shields.io/crates/v/shakespeare)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)
[![docs.rs](https://img.shields.io/docsrs/shakespeare)](https://docs.rs/shakespeare/latest/shakespeare/)
![GitHub Release Date](https://img.shields.io/github/release-date/ejmount/shakespeare?label=latest%20release)
[![Build](https://github.com/ejmount/shakespeare/actions/workflows/build.yml/badge.svg)](https://github.com/ejmount/shakespeare/actions/workflows/build.yml)
![Crates.io Total Downloads](https://img.shields.io/crates/d/shakespeare)

Shakespeare is a safe, pure Rust framework that focuses on ergonomics and extensibility in its implementation of the [actor model](https://en.wikipedia.org/wiki/Actor_model) for creating highly parallel yet safe and robust systems. Actors are run as tasks directly on top of [tokio](https://tokio.rs/) (with no "world" or "system" boundary) and run fully in parallel.

**Why do I want an actor system?** - The actor model describes a system in terms of interacting "actors," each of which has some sort of associated state, and which interact with other actors by asynchronously sending messages. Importantly, *only* the actor which owns any given data can directly read or write it, with other parts of the system needing to post a message to the owning actor to request the value change.

This has many of the same architectural benefits as micro-services bring to complex systems, but on a smaller scale and with less overhead. (Namely, that no serialization is needed for actors on the same host and heap-allocated data need not be moved at all) More specifically:

* Any particular operation never needs to take locks, as it is the only possible writer of any value it has direct access to
* Scheduling concerns do not intrude on the application logic embodied by the actors - the former can vary completely independently, and in most cases, an actor system will yield the same results whether the host has a single parallel thread or a hundred with no functionality changes.
* Actors' interfaces - and so allowed operations on the data - are declared explicitly rather than allowing a holder of a lock to make arbitrary changes to the shared value.
* Because actors run concurrently by default, parallel execution and vertical scaling fall out "for free."

Applications that can benefit from the actor model include network servers of almost all kinds, messaging protocols like IRC and Matrix, distributed services like Kubernetes and Docker Swarm (and anything else using Raft consensus), and even some styles of video games.

**Why do I want *Shakespeare*?** - Shakespeare focuses on generality and allowing the programmer maximum control over the results while minimizing runtime overhead. Its most significant features include:

* **Polymorphic actors** - actors' interfaces ("roles") are a first-class consideration in Shakespeare, and ergonomically allowing an actor to have multiple roles, and a role to have multiple implementations, were important design considerations. Allowing code to work with dynamically chosen actors of a given role as easily as they can work with dynamically chosen implementations of a trait with `dyn Trait` was a primary use case. This enables not only polymorphism within application code, but also substituting mock actors in integration testing and the like.
  * If a role is exported publicly, downstream crates can implement it on their own actors exactly as with normal traits.
* **Rich interfaces** - a single role for an actor can support any number of messages and accompanying methods with no more boilerplate than the equivalent trait definition. Messages can include any number of input parameters of any `'static` type. This includes methods that return values, which can be returned to the caller - however, the choice of whether or not to wait for the response remains with the caller and can be made on a call-by-call basis.
  * The common case of immediately `await`ing the response means the message works like an ordinary function call.
  * There is [special support](https://docs.rs/shakespeare/latest/shakespeare/struct.Envelope.html#method.forward_to) for relaying the returned value onwards to another actor without waiting for the remote processing to complete.
* **Static validation** - Shakespeare extensively leverages Rust's type system - the most common flows of a message to and from an actor are statically typed throughout, and API functions express their preconditions at the type level where possible. The output of Shakespeare's macros is similarly statically-typed throughout, minimizing runtime overhead and offering more visibility to the optimizer.
  * Sending a message to and then awaiting the response from an actor of known concrete type incurs *no* virtual function calls.
* **Interoperability** - linking actors into the wider ecosystem of async code is designed to be easy - actor messages implement `Future`, and actors can treat [`Future`](https://docs.rs/shakespeare/latest/shakespeare/fn.send_future_to.html) and `Stream` objects as incoming messages with no indirection.
  * Conversely, message handlers within an actor can be `async` and can await arbitrary Futures, with actor messages simply being a special case.
* **Recovery** - Shakespeare makes it easy to respond an actor stopping its processing, both by providing a cleanup hooks within the actor and providing a `Future` to the caller (that originally created the actor) that will become ready when the spawned actor stops.
  * Both the cleanup handling and the value from the cleanup `Future` indicate whether the shutdown was graceful (i.e. all references to the actor were dropped) or was a result of a panic within a message handler, so that error handling is more easily separated from the "happy path."
  * Recovery handling is entirely optional - the cleanup `Future` can simply be dropped with no ill effect.  (Note in this case, the panic from the actor will *not* propagate up)

For a full explanation of how all this works, see [the crate documentation](https://docs.rs/shakespeare/latest/).

It is worth noting that Shakespeare currently does **not** offer any built-in support for:

* Global/static "broadcasts" of any kind - by default, actors only have access to their own state and any static values your application code has defined.
* Networking - while actor messages conceptually have the same semantics as remote procedure calls, Shakespeare currently only supports messages within a single host process. That said, an actor can receive a `Stream` of packets as normal if one is constructed out of a network socket such as with a [tokio codec](https://docs.rs/tokio-util/latest/tokio_util/codec/index.html).

If needed, these capabilities are expected to be relatively straightforward to build in application code - one of the original imagined use cases involved "proxy" actors that implement a Role by forwarding the received messages to an actor on a remote host. If more support for these use cases is important to you, please raise an issue and leave your feedback.

Additionally, Shakespeare currently runs exclusively on [tokio](https://tokio.rs/) but this may change in the future. It also currently uses only unbounded channels, but improving this is planned future work.

## Example

The following is a demonstration of some of Shakespeare's capabilities. There is also a [chat server example](examples/chat.rs) showing these features in a more realistic context.

```rust
use shakespeare::{actor, performance, role, ActorHandles, Envelope};
use std::sync::Arc;
use tokio::sync::mpsc;

#[role]
trait BasicRole { // Defining a role is largely a normal trait definition, just with the attribute macro
    fn inform(&self, val: usize); // Role methods can take any number of arguments of any reasonable type
    fn get(&self) -> usize; // They can similarly return any reasonable type
}
// ("Reasonable" here means Sized, Send and with 'static lifetime)


#[actor]
mod AnActor { // The name of an actor type is defined by the module name here
    struct StateA(mpsc::Sender<usize>); // An actor's internal state type can be any struct, enum or union
                                        // with any name, any number of fields/variants and any size.
                                        // Any syntax for these is accepted - this example does not
                                        // name the struct field, but it could do
                                        // (Anonymous tuples are not allowed as state types)

    #[performance]
    impl BasicRole for StateA {
        async fn inform(&mut self, val: usize) {
            // Even though the role was defined as taking &self and as non-async, the implementation can be either or both
            // The macros will handle the glue

            // In role implementations, `self` is the state type, i.e. `StateA` here
            self.0.send(val).await;
            // this method doesn't do anything interesting, just passes the value back through the channel
            // that the actor was given at startup
        }
        fn get(&self) -> usize {
            4 // Chosen by fair dice roll, guaranteed to be random
            // ...not that it needs to be random, this is just providing a return value we can retrieve later
        }
    }
}


#[tokio::main]
async fn main() {
    // We will give the actor a channel to send values back to us
    // This is to demonstrate that actor methods can be async.
    // There's no ordinary need to do this because we can also get hold of return values directly
    let (tx, mut rx) = mpsc::channel(1);

    let actor_state = StateA(tx); // Nothing unusual is happening here, just initializing a value

    // This calls the generated constructor, which starts the actor as a new task
    // with the provided value as its state, i.e. with the sender we assigned.
    let actor_handles = AnActor::start(actor_state);

    // Starting the actor produces two handles
    // - the message handle for sending messages/calling functions
    // - the join handle to await the actor stopping and indicating successful shutdown or panic
    let ActorHandles { message_handle, join_handle: _, .. } = actor_handles;
    // In many cases, you may not care about the join_handle and can just ignore it

    // the message handle can be upcast to a Role trait object
    // Code working with actors via trait objects is more generic
    let actor: Arc<dyn BasicRole> = message_handle;

    // We send the actor messages by calling methods
    // These calls return an "Envelope" to let you decide what to do with the return value
    {
        let _: Envelope<dyn BasicRole, _> = actor.inform(100);
        // can let the Envelope drop to fire and forget, ignoring any return value from the actor
    }
    // The message was sent to the actor after the closing brace, but since it arrived on a separate
    // task, we must wait until that task is scheduled and sends us a value back on the sender we gave it.
    // NB: Putting the following line inside the braces above would cause a deadlock
    // because we would await the response before the Envelope dropped and sent the message
    let chan_response = rx.recv().await;

    assert_eq!(chan_response, Some(100));

    // We can also directly await the Envelope to get a synchronous, strongly-typed return value
    let ret_value = actor.get().await;
    assert_eq!(ret_value, Ok(4));
    // Its also possible in general the actor shuts down while we were waiting for the message,
    // which would give us an Err when we awaited.
}
```

## Compatibility

If you directly browse this crate's source code, you will come across functions marked `pub` but also `#[doc(hidden)]`. This is because the macros used in Shakespeare generate output code that then calls into these library functions. These calls technically originate from the *user's* crate, which means the called functions need to be `pub` to resolve. However, they are nonetheless not intended for direct client use, so they are suppressed from the documentation and **are exempt from SemVer** - code calling these functions by bypassing documented interfaces may break even in patch releases.

## Licence

Licensed under either of [Apache Licence, Version 2.0](LICENSE-APACHE) or [MIT licence](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 licence, shall
be dual licensed as above, without any additional terms or conditions.
