//! Swarms-rs is a Rust implementation of the Swarms framework for building multi-agent systems.
//! This crate provides core abstractions and implementations for agents, workflows and swarms.
pub mod agent;
pub mod llm;
pub mod logging;
pub mod prompts;
pub mod structs;
pub use swarms_macro;

// Re-export commonly used traits and types
pub use structs::agent::Agent;
pub use structs::mixture_of_agents::{MixtureOfAgents, MixtureOfAgentsBuilder, MoAError};
