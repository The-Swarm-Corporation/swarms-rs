//! # Mixture of Agents
//!
//! This module implements the **Mixture-of-Agents (MoA)** architecture: a layered
//! ensemble where multiple "worker" agents generate responses in parallel on every
//! layer, and their outputs are progressively refined by subsequent layers before a
//! dedicated aggregator agent synthesises a final answer.
//!
//! ## Architecture
//!
//! - **Layer 0**: every worker agent receives only the original task.
//! - **Layer N (>0)**: every worker agent receives the original task plus the
//!   concatenated outputs produced by the previous layer, letting later layers
//!   refine earlier proposals.
//! - **Aggregation**: the aggregator agent receives the *full* transcript of every
//!   layer's outputs and produces the final response.
//!
//! All worker agents within a layer run concurrently via [`futures::future::join_all`],
//! so the wall-clock cost of a layer is roughly the cost of its slowest worker.
//!
//! This mirrors the `MixtureOfAgents` struct from the Python `swarms` framework
//! (`swarms/structs/mixture_of_agents.py`) and integrates with the existing
//! [`Agent`](crate::structs::agent::Agent) and [`Swarm`](crate::structs::swarm::Swarm)
//! abstractions.

use futures::future::{join_all, BoxFuture};
use thiserror::Error;

use crate::structs::agent::{Agent, AgentError};
use crate::structs::swarm::{Swarm, SwarmError};

/// Default system prompt used for the aggregator agent when one is supplied
/// explicitly by the caller via [`MixtureOfAgentsBuilder::aggregator_system_prompt`].
///
/// The aggregator is responsible for combining the (possibly contradictory) proposals
/// of the worker agents into a single, coherent, high-quality answer.
pub const DEFAULT_AGGREGATOR_SYSTEM_PROMPT: &str = "\
You are a meticulous aggregator. You are given an original task and a set of \
responses produced by multiple independent agents across several refinement layers.

Your job is to synthesise these responses into a single, coherent, and high-quality \
answer to the original task. Follow these rules:
1. Preserve every verifiable fact and useful detail present in the responses.
2. Resolve contradictions by favouring the most specific, best-reasoned claim.
3. Produce a final answer that reads as if written by one expert, not a committee.
4. Do not mention the existence of the other agents or layers in your answer.";

/// Errors that can occur while building or running a [`MixtureOfAgents`].
#[derive(Debug, Error)]
pub enum MoAError {
    #[error("Agent error: {0}")]
    AgentError(#[from] AgentError),

    #[error("No worker agents were provided to the mixture")]
    NoAgents,

    #[error("No aggregator agent was provided to the mixture")]
    NoAggregator,

    #[error("The number of layers must be greater than zero")]
    InvalidLayers,

    #[error("Aggregation produced no output")]
    EmptyAggregation,
}

/// A layered Mixture-of-Agents ensemble.
///
/// Build one with [`MixtureOfAgents::builder`].
pub struct MixtureOfAgents {
    name: String,
    description: String,
    /// Worker agents, executed concurrently on every layer.
    agents: Vec<Box<dyn Agent>>,
    /// Agent that synthesises the final answer from the full transcript.
    aggregator_agent: Option<Box<dyn Agent>>,
    /// System prompt memorised for documentation / re-export.
    aggregator_system_prompt: String,
    /// Number of worker layers to run before aggregation.
    layers: usize,
}

impl std::fmt::Debug for MixtureOfAgents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MixtureOfAgents")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("layers", &self.layers)
            .field("num_agents", &self.agents.len())
            .field("has_aggregator", &self.aggregator_agent.is_some())
            .finish()
    }
}

impl Clone for MixtureOfAgents {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            description: self.description.clone(),
            agents: self.agents.iter().map(|a| a.clone_box()).collect(),
            aggregator_agent: self.aggregator_agent.as_ref().map(|a| a.clone_box()),
            aggregator_system_prompt: self.aggregator_system_prompt.clone(),
            layers: self.layers,
        }
    }
}

impl MixtureOfAgents {
    /// Start building a [`MixtureOfAgents`].
    pub fn builder() -> MixtureOfAgentsBuilder {
        MixtureOfAgentsBuilder::default()
    }

    /// Human readable name of this mixture.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Human readable description of this mixture.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Number of worker layers that will be executed before aggregation.
    pub fn layers(&self) -> usize {
        self.layers
    }

    /// Returns the worker agents.
    pub fn agents(&self) -> &[Box<dyn Agent>] {
        &self.agents
    }

    /// Run the full Mixture-of-Agents workflow and return the final aggregated answer.
    ///
    /// This is the inherent async entry point. The [`Swarm`] trait implementation
    /// delegates to this method.
    pub async fn execute(&self, task: impl Into<String>) -> Result<String, MoAError> {
        let task = task.into();

        if self.agents.is_empty() {
            return Err(MoAError::NoAgents);
        }
        if self.layers == 0 {
            return Err(MoAError::InvalidLayers);
        }
        let aggregator = self
            .aggregator_agent
            .as_ref()
            .ok_or(MoAError::NoAggregator)?;

        // Collect every layer's outputs so the aggregator sees the full transcript.
        let mut all_layer_outputs: Vec<Vec<String>> = Vec::with_capacity(self.layers);

        for layer in 0..self.layers {
            let layer_task = if layer == 0 {
                task.clone()
            } else {
                format_layer_refinement_prompt(&task, &all_layer_outputs)
            };

            // Run every worker agent for this layer concurrently.
            let futures = self
                .agents
                .iter()
                .map(|agent| agent.run(layer_task.clone()))
                .collect::<Vec<_>>();

            let outputs = join_all(futures)
                .await
                .into_iter()
                .map(|r| r.map_err(MoAError::AgentError))
                .collect::<Result<Vec<String>, MoAError>>()?;

            all_layer_outputs.push(outputs);
        }

        let aggregator_prompt = format_aggregation_prompt(&task, &all_layer_outputs);
        let final_answer = aggregator
            .run(aggregator_prompt)
            .await
            .map_err(MoAError::AgentError)?;

        if final_answer.trim().is_empty() {
            return Err(MoAError::EmptyAggregation);
        }

        Ok(final_answer)
    }
}

impl Swarm for MixtureOfAgents {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(
        &self,
        task: String,
    ) -> BoxFuture<'_, Result<Box<dyn erased_serde::Serialize>, SwarmError>> {
        Box::pin(async move {
            let output = self.execute(task).await?;
            Ok(Box::new(output) as Box<dyn erased_serde::Serialize>)
        })
    }
}

/// Builder for [`MixtureOfAgents`].
#[derive(Default)]
pub struct MixtureOfAgentsBuilder {
    name: String,
    description: String,
    agents: Vec<Box<dyn Agent>>,
    aggregator_agent: Option<Box<dyn Agent>>,
    aggregator_system_prompt: String,
    layers: usize,
}

impl MixtureOfAgentsBuilder {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Add a single worker agent.
    pub fn add_agent(mut self, agent: Box<dyn Agent>) -> Self {
        self.agents.push(agent);
        self
    }

    /// Set all worker agents at once.
    pub fn agents(mut self, agents: Vec<Box<dyn Agent>>) -> Self {
        self.agents = agents;
        self
    }

    /// Provide the aggregator agent that synthesises the final answer.
    ///
    /// This is required; [`MixtureOfAgentsBuilder::build`] returns
    /// [`MoAError::NoAggregator`] if it is not set.
    pub fn aggregator_agent(mut self, agent: Box<dyn Agent>) -> Self {
        self.aggregator_agent = Some(agent);
        self
    }

    /// Override the aggregator system prompt stored for documentation / re-export.
    ///
    /// Note: the actual behavior of the aggregator is determined by the agent you
    /// supply via [`aggregator_agent`](Self::aggregator_agent). This value is kept
    /// for parity with the Python API and is surfaced via
    /// [`aggregator_system_prompt`](MixtureOfAgents::aggregator_system_prompt).
    pub fn aggregator_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.aggregator_system_prompt = prompt.into();
        self
    }

    /// Number of worker layers to run before aggregation (must be >= 1).
    pub fn layers(mut self, layers: usize) -> Self {
        self.layers = layers;
        self
    }

    /// Build the [`MixtureOfAgents`], validating the configuration.
    pub fn build(self) -> Result<MixtureOfAgents, MoAError> {
        if self.agents.is_empty() {
            return Err(MoAError::NoAgents);
        }
        if self.layers == 0 {
            return Err(MoAError::InvalidLayers);
        }
        if self.aggregator_agent.is_none() {
            return Err(MoAError::NoAggregator);
        }

        Ok(MixtureOfAgents {
            name: if self.name.is_empty() {
                "MixtureOfAgents".to_owned()
            } else {
                self.name
            },
            description: if self.description.is_empty() {
                "A layered mixture of agents that aggregates worker responses.".to_owned()
            } else {
                self.description
            },
            agents: self.agents,
            aggregator_agent: self.aggregator_agent,
            aggregator_system_prompt: if self.aggregator_system_prompt.is_empty() {
                DEFAULT_AGGREGATOR_SYSTEM_PROMPT.to_owned()
            } else {
                self.aggregator_system_prompt
            },
            layers: self.layers,
        })
    }
}

impl MixtureOfAgents {
    /// The aggregator system prompt associated with this mixture.
    pub fn aggregator_system_prompt(&self) -> &str {
        &self.aggregator_system_prompt
    }
}

/// Build the prompt for layer `> 0` workers: original task + previous layer outputs.
fn format_layer_refinement_prompt(task: &str, all_layer_outputs: &[Vec<String>]) -> String {
    let previous = all_layer_outputs
        .last()
        .map(|outputs| format_worker_outputs(outputs))
        .unwrap_or_default();

    format!(
        "Original task:\n{task}\n\n\
         Previous layer's responses:\n{previous}\n\n\
         Based on the original task and the previous layer's responses, \
         provide an improved and refined answer. Build on the strengths of the \
         previous responses and correct any of their weaknesses."
    )
}

/// Build the prompt handed to the aggregator: original task + every layer's outputs.
fn format_aggregation_prompt(task: &str, all_layer_outputs: &[Vec<String>]) -> String {
    let mut sections = String::new();
    for (layer_idx, outputs) in all_layer_outputs.iter().enumerate() {
        sections.push_str(&format!(
            "\n--- Layer {} responses ---\n{}",
            layer_idx + 1,
            format_worker_outputs(outputs)
        ));
    }

    format!(
        "Original task:\n{task}\n\n\
         The following responses were produced by multiple agents across {} layer(s):\
         {sections}\n\n\
         Synthesise these responses into a single, coherent, and high-quality \
         final answer to the original task.",
        all_layer_outputs.len()
    )
}

/// Render a list of worker outputs as an indexed block of text.
fn format_worker_outputs(outputs: &[String]) -> String {
    outputs
        .iter()
        .enumerate()
        .map(|(i, o)| format!("[Response {}]\n{o}\n", i + 1))
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic in-memory agent used to validate the orchestration logic
    /// without issuing any network / LLM calls.
    #[derive(Clone)]
    struct MockAgent {
        name: String,
        /// Tag embedded into every response so tests can trace provenance.
        tag: String,
    }

    impl Agent for MockAgent {
        fn run(&self, task: String) -> BoxFuture<'_, Result<String, AgentError>> {
            let tag = self.tag.clone();
            Box::pin(async move { Ok(format!("[{tag}] {task}")) })
        }

        fn run_multiple_tasks(
            &mut self,
            tasks: Vec<String>,
        ) -> BoxFuture<'_, Result<Vec<String>, AgentError>> {
            let tag = self.tag.clone();
            Box::pin(async move {
                Ok(tasks
                    .into_iter()
                    .map(|t| format!("[{tag}] {t}"))
                    .collect())
            })
        }

        fn plan(&self, _task: String) -> BoxFuture<'_, Result<(), AgentError>> {
            Box::pin(async { Ok(()) })
        }

        fn query_long_term_memory(
            &self,
            _task: String,
        ) -> BoxFuture<'_, Result<(), AgentError>> {
            Box::pin(async { Ok(()) })
        }

        fn save_task_state(&self, _task: String) -> BoxFuture<'_, Result<(), AgentError>> {
            Box::pin(async { Ok(()) })
        }

        fn is_response_complete(&self, _response: String) -> bool {
            true
        }

        fn id(&self) -> String {
            self.name.clone()
        }

        fn name(&self) -> String {
            self.name.clone()
        }

        fn description(&self) -> String {
            self.name.clone()
        }

        fn clone_box(&self) -> Box<dyn Agent> {
            Box::new(self.clone())
        }
    }

    fn worker(tag: &str) -> Box<dyn Agent> {
        Box::new(MockAgent {
            name: format!("worker-{tag}"),
            tag: tag.to_owned(),
        })
    }

    #[test]
    fn builder_rejects_empty_agents() {
        let err = MixtureOfAgents::builder()
            .aggregator_agent(worker("agg"))
            .layers(2)
            .build()
            .unwrap_err();
        assert!(matches!(err, MoAError::NoAgents));
    }

    #[test]
    fn builder_rejects_zero_layers() {
        let err = MixtureOfAgents::builder()
            .agents(vec![worker("a")])
            .aggregator_agent(worker("agg"))
            .layers(0)
            .build()
            .unwrap_err();
        assert!(matches!(err, MoAError::InvalidLayers));
    }

    #[test]
    fn builder_rejects_missing_aggregator() {
        let err = MixtureOfAgents::builder()
            .agents(vec![worker("a")])
            .layers(2)
            .build()
            .unwrap_err();
        assert!(matches!(err, MoAError::NoAggregator));
    }

    #[tokio::test]
    async fn single_layer_forwards_original_task() {
        let moa = MixtureOfAgents::builder()
            .name("test")
            .agents(vec![worker("a"), worker("b")])
            .aggregator_agent(worker("agg"))
            .layers(1)
            .build()
            .unwrap();

        let out = moa.execute("What is 2+2?").await.unwrap();
        // Aggregator receives the full transcript, including the worker tags.
        assert!(out.contains("[agg]"));
        assert!(out.contains("[a]"));
        assert!(out.contains("[b]"));
        assert!(out.contains("What is 2+2?"));
    }

    #[tokio::test]
    async fn multi_layer_refines_previous_outputs() {
        let moa = MixtureOfAgents::builder()
            .agents(vec![worker("a"), worker("b")])
            .aggregator_agent(worker("agg"))
            .layers(3)
            .build()
            .unwrap();

        let out = moa.execute("task-X").await.unwrap();
        // The aggregator prompt for 3 layers must include all three layers' responses,
        // so every worker tag should appear in the aggregator's (mock) output.
        assert!(out.contains("task-X"));
        assert!(out.contains("[a]"));
        assert!(out.contains("[b]"));
        assert!(out.contains("[agg]"));
    }

    #[tokio::test]
    async fn clone_preserves_configuration() {
        let moa = MixtureOfAgents::builder()
            .agents(vec![worker("a")])
            .aggregator_agent(worker("agg"))
            .layers(2)
            .build()
            .unwrap();

        let cloned = moa.clone();
        assert_eq!(cloned.layers(), 2);
        assert_eq!(cloned.agents().len(), 1);
        let out = cloned.execute("hi").await.unwrap();
        assert!(out.contains("[agg]"));
    }
}
