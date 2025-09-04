use std::{
    hash::{Hash, Hasher},
    ops::Deref,
    path::Path,
};

use chrono::Local;
use thiserror::Error;
use twox_hash::XxHash64;
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
            metadata_output_dir: "./temp/sequential_workflow/metadata".to_string(),
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

        let metadata = MetadataSchema {
            swarm_id: Uuid::new_v4(),
            task: task.clone(),
            description: self.description.clone(),
            agents_output_schema,
            timestamp: Local::now(),
        };

        let mut hasher = XxHash64::default();
        task.hash(&mut hasher);
        let task_hash = hasher.finish();
        let metadata_path_dir = Path::new(&self.metadata_output_dir);
        let metadata_output_dir = metadata_path_dir
            .join(format!("{:x}", task_hash & 0xFFFFFFFF)) // Lower 32 bits of the hash
            .with_extension("json");
        let metadata_data = serde_json::to_string_pretty(&metadata)?;
        persistence::save_to_file(metadata_data, &metadata_output_dir).await?;

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

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::structs::{
//         conversation::Content,
//         test_utils::{create_failing_agent, create_mock_agent},
//     };

//     #[test]
//     fn test_builder() {
//         let workflow = SequentialWorkflow::builder()
//             .name("TestWorkflow")
//             .description("Test Description")
//             .metadata_output_dir("/tmp/test")
//             .build();

//         assert_eq!(workflow.name, "TestWorkflow");
//         assert_eq!(workflow.description, "Test Description");
//         assert_eq!(workflow.metadata_output_dir, "/tmp/test");
//         assert!(workflow.agents.is_empty());
//     }

//     #[test]
//     fn test_builder_add_agent() {
//         let agent = create_mock_agent("1", "agent1", "Test Agent", "response1");
//         let workflow = SequentialWorkflow::builder().add_agent(agent).build();

//         assert_eq!(workflow.agents.len(), 1);
//     }

//     #[test]
//     fn test_builder_add_multiple_agents() {
//         let agent1 = create_mock_agent("1", "agent1", "First Agent", "response1");
//         let agent2 = create_mock_agent("2", "agent2", "Second Agent", "response2");

//         let workflow = SequentialWorkflow::builder()
//             .add_agent(agent1)
//             .add_agent(agent2)
//             .build();

//         assert_eq!(workflow.agents.len(), 2);
//     }

//     #[test]
//     fn test_builder_set_agents() {
//         let agents = vec![
//             create_mock_agent("1", "agent1", "First Agent", "response1") as _,
//             create_mock_agent("2", "agent2", "Second Agent", "response2") as _,
//         ];

//         let workflow = SequentialWorkflow::builder().agents(agents).build();

//         assert_eq!(workflow.agents.len(), 2);
//     }

//     #[tokio::test]
//     async fn test_run_no_agents() {
//         let workflow = SequentialWorkflow::builder().build();
//         let result = workflow.run("test task").await;

//         assert!(result.is_err());
//         match result {
//             Err(SequentialWorkflowError::NoAgents) => {},
//             _ => panic!("Expected NoAgents error"),
//         }
//     }

//     #[tokio::test]
//     async fn test_run_empty_task() {
//         let agent = create_mock_agent("1", "agent1", "Test Agent", "response1");
//         let workflow = SequentialWorkflow::builder().add_agent(agent).build();

//         let result = workflow.run("").await;

//         assert!(result.is_err());
//         match result {
//             Err(SequentialWorkflowError::NoTasks) => {},
//             _ => panic!("Expected NoTasks error"),
//         }
//     }

//     #[tokio::test]
//     async fn test_run_single_agent() {
//         let agent = create_mock_agent("1", "agent1", "Test Agent", "response1");
//         let workflow = SequentialWorkflow::builder().add_agent(agent).build();

//         let result = workflow.run("test task").await;

//         assert!(result.is_ok());
//         let conversation = result.unwrap();
//         assert_eq!(conversation.history.len(), 2); // User message + Agent response

//         let messages = conversation.history;
//         assert!(matches!(messages[0].role, Role::User(_)));
//         let Content::Text(user_content) = messages[0].content.clone();
//         assert!(user_content.contains("test task"));

//         assert!(matches!(messages[1].role, Role::Assistant(_)));
//         let Content::Text(assistant_content) = messages[1].content.clone();
//         assert!(assistant_content.contains("response1"));
//     }

//     #[tokio::test]
//     async fn test_run_multiple_agents() {
//         let agent1 = create_mock_agent("1", "agent1", "First Agent", "response1");
//         let agent2 = create_mock_agent("2", "agent2", "Second Agent", "response2");

//         let workflow = SequentialWorkflow::builder()
//             .add_agent(agent1)
//             .add_agent(agent2)
//             .build();

//         let result = workflow.run("test task").await;

//         assert!(result.is_ok());
//         let conversation = result.unwrap();
//         assert_eq!(conversation.history.len(), 3); // User message + 2 Agent responses

//         let messages = conversation.history;
//         assert!(matches!(messages[0].role, Role::User(_)));
//         let Content::Text(user_content) = messages[0].content.clone();
//         assert!(user_content.contains("test task"));

//         assert!(matches!(messages[1].role, Role::Assistant(_)));
//         let Content::Text(assistant_content) = messages[1].content.clone();
//         assert!(assistant_content.contains("response1"));

//         assert!(matches!(messages[2].role, Role::Assistant(_)));
//         let Content::Text(assistant_content) = messages[2].content.clone();
//         assert!(assistant_content.contains("response2"));
//     }

//     #[tokio::test]
//     async fn test_run_agent_error() {
//         let failing_agent = create_failing_agent("1", "failing_agent", "test error");
//         let workflow = SequentialWorkflow::builder()
//             .add_agent(failing_agent)
//             .build();

//         let result = workflow.run("test task").await;

//         assert!(result.is_err());
//         match result {
//             Err(SequentialWorkflowError::AgentError(_)) => {},
//             _ => panic!("Expected AgentError"),
//         }
//     }
// }
