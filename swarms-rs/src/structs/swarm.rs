use chrono::{DateTime, Local};
use dashmap::DashMap;
use erased_serde::Serialize as ErasedSerialize;
use futures::future::BoxFuture;
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::structs::concurrent_workflow::ConcurrentWorkflowError;
use crate::structs::mixture_of_agents::MoAError;

pub trait Swarm {
    fn name(&self) -> &str;

    fn run(&self, task: String) -> BoxFuture<'_, Result<Box<dyn ErasedSerialize>, SwarmError>>;
}

#[derive(Debug, Error)]
pub enum SwarmError {
    #[error("ConcurrentWorkflowError: {0}")]
    ConcurrentWorkflowError(#[from] ConcurrentWorkflowError),
    #[error("AgentRearrangeError: {0}")]
    AgentRearrangeError(#[from] crate::structs::rearrange::AgentRearrangeError),
    #[error("MoAError: {0}")]
    MoAError(#[from] MoAError),
}

#[derive(Clone, Default, Serialize)]
pub struct MetadataSchemaMap(DashMap<String, MetadataSchema>);

impl MetadataSchemaMap {
    pub fn add(&self, task: impl Into<String>, metadata: MetadataSchema) {
        self.0.insert(task.into(), metadata);
    }
}

#[derive(Clone, Default, Serialize)]
pub struct MetadataSchema {
    pub swarm_id: Uuid,
    pub task: String,
    pub description: String,
    pub agents_output_schema: Vec<AgentOutputSchema>,
    pub timestamp: DateTime<Local>,
}

#[derive(Clone, Serialize)]
pub struct AgentOutputSchema {
    pub run_id: Uuid,
    pub agent_name: String,
    pub task: String,
    pub output: String,
    pub start: DateTime<Local>,
    pub end: DateTime<Local>,
    pub duration: i64,
}
