//! Swarms-rs is a Rust implementation of the Swarms framework for building multi-agent systems.
//! This crate provides core abstractions and implementations for agents, workflows and swarms.
pub mod agent;
pub mod llm;
pub mod logging;
pub mod prompts;
pub mod structs;
pub use swarms_macro;

// Spending caps (`budget` feature), so callers can build a shared budget or fleet without
// their own `llm-budget` dependency.
#[cfg(feature = "budget")]
pub use llm_budget;

// Used by the exported logging macros, so callers don't need their own `log` dependency.
#[doc(hidden)]
pub use log;

// Re-export commonly used traits and types
pub use structs::agent::Agent;
