//! Tests for Concurrent Workflow
//! This module tests the concurrent workflow builder and concurrent workflow struct

use futures::future::BoxFuture;
use swarms_rs::structs::{
    agent::{Agent, AgentError},
    concurrent_workflow::{ConcurrentWorkflow, ConcurrentWorkflowError},
};
use tempfile::tempdir;
use tokio::sync::Semaphore;

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

// Shared state to hold active counter
#[derive(Debug)]
struct Activity {
    active: AtomicUsize,
    peak: AtomicUsize,
    release: Semaphore,
}

impl Activity {
    fn new() -> Self {
        Self {
            active: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            release: Semaphore::new(0),
        }
    }
}

// Mock agent for testing
#[derive(Clone, Debug)]
struct MockAgent {
    name: String,
    response: String,
    should_error: bool,
    activity: Option<Arc<Activity>>,
}

impl MockAgent {
    fn new(name: &str, response: &str) -> Self {
        Self {
            name: name.to_string(),
            response: response.to_string(),
            should_error: false,
            activity: None,
        }
    }

    fn new_with_error(name: &str) -> Self {
        Self {
            name: name.to_string(),
            response: String::new(),
            should_error: true,
            activity: None,
        }
    }

    fn with_activity(mut self, activity: Arc<Activity>) -> Self {
        self.activity = Some(activity);
        self
    }
}

impl Agent for MockAgent {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn id(&self) -> String {
        format!("mock-{}", self.name)
    }

    fn description(&self) -> String {
        format!("Mock agent: {}", self.name)
    }

    fn run(&self, _task: String) -> BoxFuture<'_, Result<String, AgentError>> {
        Box::pin(async move {
            if let Some(activity) = &self.activity {
                let active = activity.active.fetch_add(1, Ordering::SeqCst) + 1;
                activity.peak.fetch_max(active, Ordering::SeqCst);

                let _permit = activity.release.acquire().await.unwrap();

                activity.active.fetch_sub(1, Ordering::SeqCst);
            }

            if self.should_error {
                Err(AgentError::NoChoiceFound)
            } else {
                Ok(self.response.clone())
            }
        })
    }

    fn run_multiple_tasks(
        &mut self,
        _tasks: Vec<String>,
    ) -> BoxFuture<'_, Result<Vec<String>, AgentError>> {
        Box::pin(async move {
            if self.should_error {
                Err(AgentError::NoChoiceFound)
            } else {
                Ok(vec![self.response.clone()])
            }
        })
    }

    fn plan(&self, _task: String) -> BoxFuture<'_, Result<(), AgentError>> {
        Box::pin(async move { Ok(()) })
    }

    fn query_long_term_memory(&self, _task: String) -> BoxFuture<'_, Result<(), AgentError>> {
        Box::pin(async move { Ok(()) })
    }

    fn save_task_state(&self, _task: String) -> BoxFuture<'_, Result<(), AgentError>> {
        Box::pin(async move { Ok(()) })
    }

    fn is_response_complete(&self, _response: String) -> bool {
        true
    }

    fn clone_box(&self) -> Box<dyn Agent> {
        Box::new(self.clone())
    }
}

#[test]
fn test_concurrent_workflow_builder_creation() {
    let agent1 = Box::new(MockAgent::new("Agent1", "Response1")) as Box<dyn Agent>;
    let agent2 = Box::new(MockAgent::new("Agent2", "Response2")) as Box<dyn Agent>;
    let temp_dir = tempdir().unwrap();
    let output_dir = temp_dir.path().to_str().unwrap();

    let _workflow = ConcurrentWorkflow::builder()
        .name("TestWorkflow")
        .description("A test workflow")
        .metadata_output_dir(output_dir)
        .add_agent(agent1)
        .add_agent(agent2)
        .build();

    // Can't directly access private fields, but we can test the builder pattern works
    // The actual functionality will be tested in integration tests
}

#[test]
fn test_concurrent_workflow_builder_defaults() {
    let _workflow = ConcurrentWorkflow::builder().build();
    // Test that the builder creates a workflow with defaults
}

#[test]
fn test_concurrent_workflow_builder_with_agents_vector() {
    let agents: Vec<Box<dyn Agent>> = vec![
        Box::new(MockAgent::new("Agent1", "Response1")),
        Box::new(MockAgent::new("Agent2", "Response2")),
        Box::new(MockAgent::new("Agent3", "Response3")),
    ];

    let _workflow = ConcurrentWorkflow::builder()
        .name("BatchWorkflow")
        .agents(agents)
        .build();
}

#[test]
fn test_concurrent_workflow_builder_chaining() {
    let temp_dir = tempdir().unwrap();
    let output_dir = temp_dir.path().to_str().unwrap();

    let _workflow = ConcurrentWorkflow::builder()
        .name("ChainedWorkflow")
        .description("A chained workflow test")
        .metadata_output_dir(output_dir)
        .add_agent(Box::new(MockAgent::new("Agent1", "Response1")))
        .add_agent(Box::new(MockAgent::new("Agent2", "Response2")))
        .build();
}

#[tokio::test]
async fn test_concurrent_workflow_run_empty_task() {
    let workflow = ConcurrentWorkflow::builder()
        .name("EmptyTaskWorkflow")
        .add_agent(Box::new(MockAgent::new("Agent1", "Response1")))
        .build();

    let result = workflow.run("").await;
    assert!(matches!(
        result,
        Err(ConcurrentWorkflowError::EmptyTasksOrAgents)
    ));
}

#[tokio::test]
async fn test_concurrent_workflow_run_no_agents() {
    let workflow = ConcurrentWorkflow::builder()
        .name("NoAgentsWorkflow")
        .build();

    let result = workflow.run("test task").await;
    assert!(matches!(
        result,
        Err(ConcurrentWorkflowError::EmptyTasksOrAgents)
    ));
}

#[tokio::test]
async fn test_concurrent_workflow_run_duplicate_task() {
    let output_dir = tempdir().unwrap();
    let workflow = ConcurrentWorkflow::builder()
        .name("DuplicateTaskWorkflow")
        .metadata_output_dir(output_dir.path().to_str().unwrap())
        .add_agent(Box::new(MockAgent::new("Agent1", "Response1")))
        .build();

    let task = "duplicate task";

    // The same task can run again once the previous run has finished,
    // and each run returns only its own history
    for _ in 0..2 {
        let conversation = workflow.run(task).await.unwrap();
        assert_eq!(conversation.history.len(), 2); // user + Agent1
    }

    // But not twice at the same time
    let (first, second) = tokio::join!(workflow.run(task), workflow.run(task));
    assert!(first.is_ok());
    assert!(matches!(
        second,
        Err(ConcurrentWorkflowError::TaskAlreadyExists)
    ));
}

#[tokio::test]
async fn test_concurrent_workflow_retry_after_failure() {
    // A failed run must not leave the task marked as running
    let workflow = ConcurrentWorkflow::builder()
        .name("RetryWorkflow")
        .add_agent(Box::new(MockAgent::new_with_error("ErrorAgent")))
        .build();

    for _ in 0..2 {
        assert!(matches!(
            workflow.run("retry task").await,
            Err(ConcurrentWorkflowError::AgentError(_))
        ));
    }
}

#[tokio::test]
async fn test_concurrent_workflow_all_agents_fail() {
    let workflow = ConcurrentWorkflow::builder()
        .name("AllFailWorkflow")
        .add_agent(Box::new(MockAgent::new_with_error("ErrorAgent1")))
        .add_agent(Box::new(MockAgent::new_with_error("ErrorAgent2")))
        .build();

    assert!(matches!(
        workflow.run("doomed task").await,
        Err(ConcurrentWorkflowError::AgentError(_))
    ));
}

#[tokio::test]
async fn test_concurrent_workflow_run_successful() {
    let workflow = ConcurrentWorkflow::builder()
        .name("SuccessfulWorkflow")
        .add_agent(Box::new(MockAgent::new("Agent1", "Response1")))
        .add_agent(Box::new(MockAgent::new("Agent2", "Response2")))
        .build();

    let result = workflow.run("test task").await;
    assert!(result.is_ok());

    let conversation = result.unwrap();
    // The conversation should contain the user message and agent responses
    assert!(!conversation.history.is_empty());
}

#[test]
fn test_concurrent_workflow_error_types() {
    // Test different error types
    let agent_error = AgentError::NoChoiceFound;
    let workflow_error = ConcurrentWorkflowError::AgentError(agent_error);
    assert!(matches!(
        workflow_error,
        ConcurrentWorkflowError::AgentError(_)
    ));

    let workflow_error = ConcurrentWorkflowError::EmptyTasksOrAgents;
    assert!(matches!(
        workflow_error,
        ConcurrentWorkflowError::EmptyTasksOrAgents
    ));

    let workflow_error = ConcurrentWorkflowError::TaskAlreadyExists;
    assert!(matches!(
        workflow_error,
        ConcurrentWorkflowError::TaskAlreadyExists
    ));

    let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
    let workflow_error = ConcurrentWorkflowError::JsonError(json_error);
    assert!(matches!(
        workflow_error,
        ConcurrentWorkflowError::JsonError(_)
    ));
}

#[test]
fn test_concurrent_workflow_error_display() {
    let error = ConcurrentWorkflowError::EmptyTasksOrAgents;
    assert_eq!(error.to_string(), "Tasks or Agents are empty");

    let error = ConcurrentWorkflowError::TaskAlreadyExists;
    assert_eq!(error.to_string(), "Task already exists");

    let agent_error = AgentError::NoChoiceFound;
    let error = ConcurrentWorkflowError::AgentError(agent_error);
    assert_eq!(error.to_string(), "Agent error: No choice found");
}

#[tokio::test]
async fn test_concurrent_workflow_with_mixed_agents() {
    let workflow = ConcurrentWorkflow::builder()
        .name("MixedAgentsWorkflow")
        .add_agent(Box::new(MockAgent::new("SuccessAgent", "Success")))
        .add_agent(Box::new(MockAgent::new_with_error("ErrorAgent")))
        .build();

    let result = workflow.run("mixed test").await;
    // Even if one agent fails, the workflow should still return a result
    // The error handling is done internally and logged
    assert!(result.is_ok());
}

#[test]
fn test_concurrent_workflow_builder_multiple_add_agent_calls() {
    let _workflow = ConcurrentWorkflow::builder()
        .name("MultipleAgentsWorkflow")
        .add_agent(Box::new(MockAgent::new("Agent1", "Response1")))
        .add_agent(Box::new(MockAgent::new("Agent2", "Response2")))
        .add_agent(Box::new(MockAgent::new("Agent3", "Response3")))
        .add_agent(Box::new(MockAgent::new("Agent4", "Response4")))
        .build();
}

#[tokio::test]
async fn test_concurrent_workflow_run_with_long_task() {
    let long_task = "a".repeat(1000); // Very long task string

    let workflow = ConcurrentWorkflow::builder()
        .name("LongTaskWorkflow")
        .add_agent(Box::new(MockAgent::new("Agent1", "Response to long task")))
        .build();

    let result = workflow.run(long_task).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_concurrent_workflow_run_with_special_characters() {
    let special_task = "Task with special chars: 你好, émoji 🚀, and symbols @#$%";

    let workflow = ConcurrentWorkflow::builder()
        .name("SpecialCharsWorkflow")
        .add_agent(Box::new(MockAgent::new(
            "Agent1",
            "Response to special chars",
        )))
        .build();

    let result = workflow.run(special_task).await;
    assert!(result.is_ok());
}

#[test]
fn test_concurrent_workflow_builder_empty_name() {
    let _workflow = ConcurrentWorkflow::builder()
        .name("")
        .add_agent(Box::new(MockAgent::new("Agent1", "Response1")))
        .build();
}

#[test]
fn test_concurrent_workflow_builder_empty_description() {
    let _workflow = ConcurrentWorkflow::builder()
        .name("EmptyDescWorkflow")
        .description("")
        .add_agent(Box::new(MockAgent::new("Agent1", "Response1")))
        .build();
}

#[tokio::test]
async fn test_run_respects_max_concurrency() {
    let activity = Arc::new(Activity::new());
    let mut builder = ConcurrentWorkflow::builder()
        .name("MaxConcurrencyWorkflow")
        .max_concurrency(2);
    for i in 0..5 {
        builder = builder.add_agent(Box::new(
            MockAgent::new(&format!("Agent{i}"), "Response").with_activity(Arc::clone(&activity)),
        ));
    }
    let workflow = builder.build();
    let run = workflow.run("test task");
    tokio::pin!(run);

    // Poll while the release gate is closed to observe how many agents start.
    assert!(futures::poll!(&mut run).is_pending());
    assert_eq!(activity.active.load(Ordering::SeqCst), 2);

    activity.release.add_permits(5);
    let conversation = tokio::time::timeout(std::time::Duration::from_secs(5), run)
        .await
        .expect("workflow should finish after release")
        .unwrap();

    assert_eq!(activity.peak.load(Ordering::SeqCst), 2);
    assert_eq!(activity.active.load(Ordering::SeqCst), 0);
    assert_eq!(conversation.history.len(), 6);
}

#[tokio::test]
async fn test_batch_respects_max_concurrency() {
    let activity = Arc::new(Activity::new());
    let workflow = ConcurrentWorkflow::builder()
        .name("BatchMaxConcurrencyWorkflow")
        .max_concurrency(2)
        .add_agent(Box::new(
            MockAgent::new("Agent1", "Response1").with_activity(Arc::clone(&activity)),
        ))
        .build();
    let tasks: Vec<String> = (0..5).map(|i| format!("task{i}")).collect();
    let run = workflow.run_batch(tasks.clone());
    tokio::pin!(run);

    // One agent per task isolates the outer batch limit.
    assert!(futures::poll!(&mut run).is_pending());
    assert_eq!(activity.active.load(Ordering::SeqCst), 2);

    activity.release.add_permits(5);
    let results = tokio::time::timeout(std::time::Duration::from_secs(5), run)
        .await
        .expect("batch should finish after release")
        .unwrap();

    assert_eq!(activity.peak.load(Ordering::SeqCst), 2);
    assert_eq!(activity.active.load(Ordering::SeqCst), 0);
    assert_eq!(results.len(), tasks.len());
    for task in tasks {
        assert_eq!(results.get(&task).unwrap().history.len(), 2);
    }
}

#[tokio::test]
async fn test_default_concurrency_is_unlimited() {
    let activity = Arc::new(Activity::new());
    let mut builder = ConcurrentWorkflow::builder().name("DefaultConcurrencyWorkflow");
    for i in 0..5 {
        builder = builder.add_agent(Box::new(
            MockAgent::new(&format!("Agent{i}"), "Response").with_activity(Arc::clone(&activity)),
        ));
    }
    let workflow = builder.build();
    let run = workflow.run("test task");
    tokio::pin!(run);

    assert!(futures::poll!(&mut run).is_pending());
    assert_eq!(activity.active.load(Ordering::SeqCst), 5);

    activity.release.add_permits(5);
    let conversation = tokio::time::timeout(std::time::Duration::from_secs(5), run)
        .await
        .expect("workflow should finish after release")
        .unwrap();

    assert_eq!(activity.peak.load(Ordering::SeqCst), 5);
    assert_eq!(activity.active.load(Ordering::SeqCst), 0);
    assert_eq!(conversation.history.len(), 6);
}

#[tokio::test]
async fn test_zero_concurrency() {
    let activity = Arc::new(Activity::new());
    let mut builder = ConcurrentWorkflow::builder()
        .name("ZeroConcurrencyWorkflow")
        .max_concurrency(0);
    for i in 0..5 {
        builder = builder.add_agent(Box::new(
            MockAgent::new(&format!("Agent{i}"), "Response").with_activity(Arc::clone(&activity)),
        ));
    }
    let workflow = builder.build();
    let run = workflow.run("test task");
    tokio::pin!(run);

    assert!(futures::poll!(&mut run).is_pending());
    assert_eq!(activity.active.load(Ordering::SeqCst), 5);

    activity.release.add_permits(5);
    let conversation = tokio::time::timeout(std::time::Duration::from_secs(5), run)
        .await
        .expect("workflow should finish after release")
        .unwrap();

    assert_eq!(activity.peak.load(Ordering::SeqCst), 5);
    assert_eq!(activity.active.load(Ordering::SeqCst), 0);
    assert_eq!(conversation.history.len(), 6);
}
