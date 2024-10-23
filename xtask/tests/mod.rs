#[cfg(test)]
mod expanded;

// If the above module is missing, run `xtask expand`

#[cfg(test)]
#[path = "expanded/shakespeare-macro/tests/mod.rs"]
mod macros;
