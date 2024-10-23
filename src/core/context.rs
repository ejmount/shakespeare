use std::sync::Arc;

use super::{Shell, State};

#[derive(Debug)]
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

	pub fn sustains(&self) -> bool {
		Arc::strong_count(&self.shell_handle) > 1 && self.running
	}

	pub fn get_shell(&self) -> Arc<A::ShellType> {
		self.shell_handle.clone()
	}

	pub fn stop(&mut self) {
		self.running = false;
	}
}
