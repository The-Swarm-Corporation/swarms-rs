use std::{
    hash::{Hash, Hasher},
    path::Path,
};

use chrono::Local;
use dashmap::{DashMap, DashSet};
use futures::{StreamExt, future::BoxFuture, stream};
use thiserror::Error;
use tokio::sync::mpsc;
use twox_hash::XxHash3_64;
use uuid::Uuid;

use crate::structs::{
    agent::{Agent, AgentError},
    conversation::{AgentConversation, AgentShortMemory, Role},
    persistence::{self, PersistenceError},
    swarm::{MetadataSchema, Swarm, SwarmError},
    utils::run_agent_with_output_schema,
};

use super::swarm::MetadataSchemaMap;

#[derive(Debug, Error)]
pub enum ConcurrentWorkflowError {
    #[error("Agent error: {0}")]
    AgentError(#[from] AgentError),
    #[error("FilePersistence error: {0}")]
    FilePersistenceError(#[from] PersistenceError),
    #[error("Tasks or Agents are empty")]
    EmptyTasksOrAgents,
    #[error("Task already exists")]
    TaskAlreadyExists,
    #[error("Json error: {0}")]
    JsonError(#[from] serde_json::Error),
}

#[derive(Default)]
pub struct ConcurrentWorkflowBuilder {
    name: String,
    description: String,
    metadata_output_dir: String,
    agents: Vec<Box<dyn Agent>>,
}

impl ConcurrentWorkflowBuilder {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn metadata_output_dir(mut self, dir: impl Into<String>) -> Self {
        self.metadata_output_dir = dir.into();
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn add_agent(mut self, agent: Box<dyn Agent>) -> Self {
        self.agents.push(agent);
        self
    }

    pub fn agents(self, agents: Vec<Box<dyn Agent>>) -> Self {
        agents
            .into_iter()
            .fold(self, |builder, agent| builder.add_agent(agent))
    }

    pub fn build(self) -> ConcurrentWorkflow {
        ConcurrentWorkflow {
            name: self.name,
            metadata_output_dir: self.metadata_output_dir,
            description: self.description,
            agents: self.agents,
            ..Default::default()
        }
    }
}

#[derive(Default)]
pub struct ConcurrentWorkflow {
    name: String,
    description: String,
    metadata_map: MetadataSchemaMap,
    metadata_output_dir: String,
    tasks: DashSet<String>,
    agents: Vec<Box<dyn Agent>>,
    conversation: AgentShortMemory,
}

impl ConcurrentWorkflow {
    pub fn builder() -> ConcurrentWorkflowBuilder {
        ConcurrentWorkflowBuilder::default()
    }

    pub async fn run(
        &self,
        task: impl Into<String>,
    ) -> Result<AgentConversation, ConcurrentWorkflowError> {
        let task = task.into();

        if task.is_empty() || self.agents.is_empty() {
            return Err(ConcurrentWorkflowError::EmptyTasksOrAgents);
        }
        if !self.tasks.insert(task.clone()) {
            return Err(ConcurrentWorkflowError::TaskAlreadyExists);
        };

        self.conversation
            .add(&task, &self.name, Role::User("User".to_owned()), &task);

        let (tx, mut rx) = mpsc::channel(self.agents.len());
        let agents = &self.agents;
        stream::iter(agents)
            .for_each_concurrent(None, |agent| {
                let tx = tx.clone();
                let task = task.clone();
                async move {
                    let output =
                        match run_agent_with_output_schema(agent.as_ref(), task.clone()).await {
                            Ok(output) => output,
                            Err(e) => {
                                tracing::error!(
                                    "| concurrent workflow | Agent: {} | Task: {} | Error: {}",
                                    agent.name(),
                                    task,
                                    e
                                );
                                return;
                            },
                        };
                    tx.send(output).await.unwrap();
                }
            })
            .await;
        drop(tx);

        let mut agents_output_schema = Vec::with_capacity(self.agents.len());
        while let Some(output_schema) = rx.recv().await {
            self.conversation.add(
                &task,
                &self.name,
                Role::Assistant(output_schema.agent_name.clone()),
                &output_schema.output,
            );
            agents_output_schema.push(output_schema);
        }

        let metadata = MetadataSchema {
            swarm_id: Uuid::new_v4(),
            task: task.clone(),
            description: self.description.clone(),
            agents_output_schema,
            timestamp: Local::now(),
        };

        self.metadata_map.add(&task, metadata.clone());

        let mut hasher = XxHash3_64::default();
        task.hash(&mut hasher);
        let task_hash = hasher.finish();
        let metadata_path_dir = Path::new(&self.metadata_output_dir);
        let metadata_output_dir = metadata_path_dir
            .join(format!("{:x}", task_hash & 0xFFFFFFFF)) // Lower 32 bits of the hash
            .with_extension("json");
        let metadata_data = serde_json::to_string_pretty(&metadata)?;
        persistence::save_to_file(metadata_data, &metadata_output_dir).await?;

        // Safety: we know that the task exists
        Ok(self.conversation.0.get(&task).unwrap().clone())
    }

    /// Runs the workflow for a batch of tasks, executes agents concurrently for each task.
    pub async fn run_batch(
        &self,
        tasks: Vec<String>,
    ) -> Result<DashMap<String, AgentConversation>, ConcurrentWorkflowError> {
        if tasks.is_empty() || self.agents.is_empty() {
            return Err(ConcurrentWorkflowError::EmptyTasksOrAgents);
        }

        let results = DashMap::with_capacity(tasks.len());
        let (tx, mut rx) = mpsc::channel(tasks.len());
        stream::iter(tasks)
            .for_each_concurrent(None, |task| {
                let tx = tx.clone();
                let workflow = self;
                async move {
                    let result = workflow.run(&task).await;
                    tx.send((task, result)).await.unwrap(); // Safety: we know rx is not dropped
                }
            })
            .await;
        drop(tx);

        while let Some((task, result)) = rx.recv().await {
            match result {
                Ok(conversation) => {
                    results.insert(task, conversation);
                },
                Err(e) => {
                    tracing::error!("| concurrent workflow | Task: {} | Error: {}", task, e);
                },
            }
        }

        Ok(results)
    }
}

impl Swarm for ConcurrentWorkflow {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(&self, task: String) -> BoxFuture<'_, Result<Box<dyn erased_serde::Serialize>, SwarmError>> {
        Box::pin(async move {
            self.run(task)
                .await
                .map(|output| Box::new(output) as _)
                .map_err(|e| e.into())
        })
    }
}

// #[cfg(test)]
// mod tests {
//     use crate::structs::{conversation::Content, test_utils::create_mock_agent};

//     use super::*;

//     #[tokio::test]
//     async fn test_concurrent_workflow_failure_no_agents() {
//         let workflow = ConcurrentWorkflow::builder()
//             .name("Test Workflow")
//             .description("This is a test workflow")
//             .build();
//         let result = workflow.run("test task").await;
//         assert!(result.is_err());
//     }

//     #[tokio::test]
//     async fn test_concurrent_workflow_failure_no_tasks() {
//         let workflow = ConcurrentWorkflow::builder()
//             .name("Test Workflow")
//             .description("This is a test workflow")
//             .add_agent(create_mock_agent(
//                 "1",
//                 "agent1",
//                 "agent1 description",
//                 "response1",
//             ))
//             .build();
//         let result = workflow.run("").await;
//         assert!(result.is_err());
//     }

//     #[tokio::test]
//     async fn test_concurrent_workflow_run() {
//         let agents = vec![
//             create_mock_agent("1", "agent1", "agent1 description", "response1") as _,
//             create_mock_agent("2", "agent2", "agent2 description", "response2") as _,
//             create_mock_agent("3", "agent3", "agent3 description", "response3") as _,
//         ];

//         let workflow = ConcurrentWorkflow::builder()
//             .name("Test Workflow")
//             .description("This is a test workflow")
//             .metadata_output_dir("./temp/concurrent_workflow/unit_test/metadata")
//             .agents(agents)
//             .build();

//         let result = workflow.run("test task").await.expect("test failed");
//         let result_history = result.history;
//         assert_eq!(result_history.len(), 4); // 3 agents + 1 user
//         assert_eq!(result_history[0].role, Role::User("User".to_owned()));

//         let Content::Text(content0) = result_history[0].content.clone();
//         assert!(content0.contains("Timestamp(millis):"));
//         assert!(content0.contains("test task"));

//         let Content::Text(content1) = result_history[1].content.clone();
//         assert!(content1.contains("Timestamp(millis):"));
//         assert!(content1.contains("response1"));

//         let Content::Text(content2) = result_history[2].content.clone();
//         assert!(content2.contains("Timestamp(millis):"));
//         assert!(content2.contains("response2"));

//         let Content::Text(content3) = result_history[3].content.clone();
//         assert!(content3.contains("Timestamp(millis):"));
//         assert!(content3.contains("response3"));
//     }

//     #[tokio::test]
//     async fn test_concurrent_workflow_run_batch() {
//         let agents = vec![
//             create_mock_agent("1", "agent1", "agent1 description", "response1") as _,
//             create_mock_agent("2", "agent2", "agent2 description", "response2") as _,
//             create_mock_agent("3", "agent3", "agent3 description", "response3") as _,
//         ];

//         let workflow = ConcurrentWorkflow::builder()
//             .name("Test Workflow")
//             .description("This is a test workflow")
//             .agents(agents)
//             .metadata_output_dir("./temp/concurrent_workflow/unit_test/metadata")
//             .build();

//         let tasks = vec![
//             "test task 1".to_owned(),
//             "test task 2".to_owned(),
//             "test task 3".to_owned(),
//         ];
//         let result = workflow.run_batch(tasks).await.expect("test failed");
//         assert_eq!(result.len(), 3); // 3 tasks
//         for (_task, conversation) in result {
//             let result_history = conversation.history;
//             assert_eq!(result_history.len(), 4); // 3 agents + 1 user
//             assert_eq!(result_history[0].role, Role::User("User".to_owned()));

//             // Because agents are executed concurrently, the order of the responses is not guaranteed.
//             assert!(result_history.iter().skip(1).any(|msg| {
//                 let Content::Text(content) = msg.content.clone();
//                 content.contains("Timestamp(millis):") && content.contains("response1")
//             }));

//             assert!(result_history.iter().skip(1).any(|msg| {
//                 let Content::Text(content) = msg.content.clone();
//                 content.contains("Timestamp(millis):") && content.contains("response2")
//             }));

//             assert!(result_history.iter().skip(1).any(|msg| {
//                 let Content::Text(content) = msg.content.clone();
//                 content.contains("Timestamp(millis):") && content.contains("response3")
//             }));
//         }
//     }
// }
