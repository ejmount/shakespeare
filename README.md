# Shakespeare

![Build](https://github.com/ejmount/shakespeare/actions/workflows/rust.yml/badge.svg)
![Version](https://img.shields.io/crates/v/shakespeare)
![release date](https://img.shields.io/github/release-date/ejmount/shakespeare)
[![codecov](https://codecov.io/gh/ejmount/shakespeare/branch/main/graph/badge.svg?token=2L6ZS8OK32)](https://codecov.io/gh/ejmount/shakespeare)
![Licence](https://img.shields.io/github/license/ejmount/shakespeare)
![Downloads](https://img.shields.io/crates/d/shakespeare)

Shakespeare is a framework written in Rust that focuses on ergonomics and extensibility in its implementation of the [actor model](https://en.wikipedia.org/wiki/Actor_model) for creating highly parallel yet safe and robust systems.

Its most significant features include:

* __Polymorphic actors__ - actors' interfaces are a first-class consideration in Shakespeare, and allowing code to work over dynamically chosen actors is a primary use case.
* __Rich interfaces__ - a single interface for an actor can support any number of methods with no more boilerplate than the equivalent trait definition. This includes methods returning values, while the choice of whether or not to wait for the response remains with the caller and can be made on a call-by-call basis.
* __Static validation__ - the majority of functionality is implemented by procedural macros, minimizing direct runtime overhead and offering more visibility to the optimizer without sacrificing static typing.
* __Interoperability__ - linking actors into the wider ecosystem of async code is designed to be easy - actor messages implement `Future`, and actors can interact with  `Future` and `Stream` generically.
* __Recovery__ - an actor ceasing both runs a cleanup function within the actor and sends out a value indicating whether the shutdown was graceful (i.e. all references were dropped) or was a result of a panic within a message handler, along with any return value from the cleanup. Another actor can then subscribe to receive this value and respond if the actor's drop is unexpected.

Shakespeare currently runs exclusively on tokio but this may change in the future. It also currently uses only unbounded channels and so has no handling of backpressure, but improving this is planned future work.

## Example

```rust
use std::sync::Arc;
use shakespeare::{actor, performance, role, ActorSpawn};
use tokio::time::{Duration, sleep};
use tokio::sync::mpsc::*;

#[role]
trait BasicRole {
    fn inform(&mut self, val: usize);
    fn get(&self) -> usize;
}

#[actor]
mod Actor {
    struct StateA(UnboundedSender<usize>);
    #[performance]
    impl BasicRole for State {
        async fn inform(&mut self, val: usize) {
            self.0.send(val);
        }
        fn get(&self) -> usize {
            42
        }
    }
}


#[tokio::main]
async fn main() {
    let (tx, mut rx) = unbounded_channel();

    let actor: Arc<dyn BasicRole> = Actor::start(StateA(tx)).msg_handle;
    {
        actor.inform(100);
        // can let the message drop to fire and forget
    }
    let value = rx.recv().await;
    assert_eq!(value, Some(100));

    let ret_value = actor.get().await; // But also await to get a syncronous return value

    assert_eq!(ret_value, Ok(42));
}
```

## Licence

Licensed under either of [Apache Licence, Version 2.0](LICENSE-APACHE) or [MIT licence](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 licence, shall
be dual licensed as above, without any additional terms or conditions.
