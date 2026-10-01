use std::{
    hash::{Hash, Hasher},
    ops::Deref,
    path::Path,
};

use chrono::Local;
use thiserror::Error;
use twox_hash::XxHash3_64;
use uuid::Uuid;

use crate::structs::{
    agent::{Agent, AgentError},
    conversation::{AgentConversation, Role},
    persistence,
    swarm::MetadataSchema,
    utils::run_agent_with_output_schema,
};

pub struct SequentialWorkflowBuilder {
    name: String,
    description: String,
    metadata_output_dir: String,
    agents: Vec<Box<dyn Agent>>,
}

impl SequentialWorkflowBuilder {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn metadata_output_dir(mut self, metadata_output_dir: impl Into<String>) -> Self {
        self.metadata_output_dir = metadata_output_dir.into();
        self
    }

    pub fn add_agent(mut self, agent: Box<dyn Agent>) -> Self {
        self.agents.push(agent);
        self
    }

    pub fn agents(mut self, agents: Vec<Box<dyn Agent>>) -> Self {
        self.agents = agents;
        self
    }

    pub fn build(self) -> SequentialWorkflow {
        SequentialWorkflow {
            name: self.name,
            description: self.description,
            metadata_output_dir: self.metadata_output_dir,
            agents: self.agents,
        }
    }
}

pub struct SequentialWorkflow {
    name: String,
    description: String,
    metadata_output_dir: String,
    agents: Vec<Box<dyn Agent>>,
}

impl SequentialWorkflow {
    pub fn builder() -> SequentialWorkflowBuilder {
        SequentialWorkflowBuilder {
            name: "SequentialWorkflow".to_string(),
            description: "A Workflow to solve a problem with sequential agents, each agent's output becomes the input for the next agent.".to_string(),
            metadata_output_dir: String::new(),
            agents: Vec::new(),
        }
    }

    pub async fn run(
        &self,
        task: impl Into<String>,
    ) -> Result<AgentConversation, SequentialWorkflowError> {
        let task = task.into();

        if self.agents.is_empty() {
            return Err(SequentialWorkflowError::NoAgents);
        }

        if task.is_empty() {
            return Err(SequentialWorkflowError::NoTasks);
        }

        let mut conversation = AgentConversation::new(self.name.clone());
        conversation.add(Role::User("User".to_owned()), task.clone());

        let mut next_input = task.clone();
        let mut agents_output_schema = Vec::with_capacity(self.agents.len());
        for agent in &self.agents {
            let output = run_agent_with_output_schema(agent.deref(), next_input.clone()).await?;
            conversation.add(Role::Assistant(agent.name()), output.output.clone());
            next_input = format!("[From Agent] {}:\n{}", agent.name(), output.output);
            agents_output_schema.push(output);
        }

        if !self.metadata_output_dir.is_empty() {
            // Construct metadata only as needed
            let metadata = MetadataSchema {
                swarm_id: Uuid::new_v4(),
                task: task.clone(),
                description: self.description.clone(),
                agents_output_schema,
                timestamp: Local::now(),
            };

            let mut hasher = XxHash3_64::default();
            task.hash(&mut hasher);
            let task_hash = hasher.finish();
            let metadata_path_dir = Path::new(&self.metadata_output_dir);
            let metadata_output_dir = metadata_path_dir
                .join(format!("{:x}", task_hash & 0xFFFFFFFF)) // Lower 32 bits of the hash
                .with_extension("json");
            let metadata_data = serde_json::to_string_pretty(&metadata)?;

            if let Err(error) = persistence::save_to_file(metadata_data, &metadata_output_dir).await
            {
                tracing::warn!(
                    "Sequential workflow '{}' could not save metadata to '{}': {}",
                    self.name,
                    metadata_output_dir.display(),
                    error,
                );
            }
        }

        Ok(conversation)
    }
}

#[derive(Debug, Error)]
pub enum SequentialWorkflowError {
    #[error("No agents provided.")]
    NoAgents,
    #[error("No tasks provided.")]
    NoTasks,
    #[error("Agent error: {0}")]
    AgentError(#[from] AgentError),
    #[error("Persistence error: {0}")]
    PersistenceError(#[from] persistence::PersistenceError),
    #[error("Json error: {0}")]
    JsonError(#[from] serde_json::Error),
}
