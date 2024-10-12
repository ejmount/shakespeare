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

#[role]
trait BasicRole {
    fn read(&mut self, val: usize);
    fn speak(&self) -> usize;
}

#[actor]
mod ActorA {
    struct StateA(usize);
    #[performance]
    impl BasicRole for State {
        fn read(&mut self, val: usize) {
            self.0 = val;
        }
        fn speak(&self) -> usize {
            2* self.0
        }
    }
}

#[actor]
mod ActorB {
    struct StateB(usize);
    #[performance]
    impl BasicRole for State {
        fn read(&mut self, val: usize) {
            self.0 = val;
        }
        fn speak(&self) -> usize {
            self.0
        }
    }
}


#[tokio::main]
async fn main() {
    let actor_a = ActorA::start(StateA(0)).msg_handle;
    let actor_b = ActorB::start(StateB(0)).msg_handle;

    let actors: Vec<Arc<dyn BasicRole>> = vec![actor_a, actor_b];

    for (ind, a) in actors.iter().enumerate() {
        a.read(ind+1); // Can fire and forget
    }

    let mut total = 0;
    for a in &actors {
        total += a.speak().await.expect("Actor shouldn't crash");
        // Only have to await to get the return value
    }

    assert_eq!(total, 4);

}
```

## Licence

Licensed under either of [Apache Licence, Version 2.0](LICENSE-APACHE) or [MIT licence](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 licence, shall
be dual licensed as above, without any additional terms or conditions.
