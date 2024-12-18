#[cfg(test)]
mod expanded {

	#[path = "shakespeare-macro/tests/successes/mod.rs"]
	pub mod successes;
}

// If the above module is missing, run `xtask expand`

#[cfg(test)]
#[path = "expanded/shakespeare-macro/tests/mod.rs"]
mod macros;
