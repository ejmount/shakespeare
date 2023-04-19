# Shakespeare

Shakespeare is an actor framework written in Rust that focuses on ergonomics and extensibility while maintaining high performance.

Its most significant differences from existing frameworks include:

* __polymorphic actors__ - actors' interfaces are a first-class consideration in shakespeare, and allowing code to work over dynamically chosen actors is a primary use case
* __rich interfaces__ - a single interface for an actor can support any number of methods with no more boilerplate than the equivalent trait definition
* __code generation__ - the majority of functionality is implemented by procedural macros, both minimizing direct runtime overhead and offering more visibility to the optimizer.

## Example

```rust
use tokio::sync::mpsc::*;

#[shakespeare::actor]
mod Actor {
    struct ActorState {
        sender: UnboundedSender<usize>,
    }
    #[shakespeare::performance(canonical)]
    impl BasicRole for ActorState {
        fn speak(&mut self, val: usize) {
            self.sender.send(val).unwrap();
        }
    }
}

#[tokio::test]
async fn main() {
    let (sender, mut recv) = tokio::sync::mpsc::unbounded_channel();
    let state = ActorState { sender };
    let shakespeare::ActorSpawn { actor, .. } = ActorState::start(state);
    actor.speak(42).await.expect("Error sending");
    assert_eq!(recv.recv().await.unwrap(), 42);
}
```

## Licence

Licensed under either of [Apache Licence, Version 2.0](LICENSE-APACHE) or [MIT licence](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 licence, shall
be dual licensed as above, without any additional terms or conditions.
