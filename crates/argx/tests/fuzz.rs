//! Model-based fuzz testing for raw parsing and typed argument binding.
//!
//! Proptest generates valid command tables and arbitrary operating-system arguments. The raw
//! parser is checked against a deliberately separate reference grammar, while typed properties
//! exercise generated binding, conversion, entry-point equivalence, and byte preservation. The
//! fuzzing campaign is isolated from deterministic tests so its size and seed can be controlled
//! without changing the ordinary test suite.

#[cfg(test)]
#[path = "fuzz/mod.rs"]
mod tests;
