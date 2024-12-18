use std::sync::Arc;

use super::State;

#[derive(Debug)]
/// Various options for controlling the behaviour of the currently running actor.
///
/// This is what you need if you want to:
/// * get a copy of the currently running actor's handle
/// * stop the currently running actor without waiting for all handles to drop
///
/// To access this, the performance signature should take a `&mut Context<Self>` as its second parameter after the receiver.
pub struct Context<A: State + ?Sized> {
	shell_handle: Arc<A::ShellType>,
	running:      bool,
}

impl<A: State + ?Sized> Context<A> {
	#[doc(hidden)]
	pub fn new(shell_handle: Arc<A::ShellType>) -> Self {
		Context {
			shell_handle,
			running: true,
		}
	}

	#[doc(hidden)]
	/// Whether the message queue should still be held open
	/// TODO: Find a better name
	#[must_use]
	pub fn sustains(&self) -> bool {
		Arc::strong_count(&self.shell_handle) > 1 && self.running
	}

	#[must_use]
	/// Gets a handle to the surrounding actor
	pub fn get_shell(&self) -> Arc<A::ShellType> {
		self.shell_handle.clone()
	}

	/// Stops the actor and runs the exit function after the current performance handler is completed
	pub fn stop(&mut self) {
		self.running = false;
	}
}
