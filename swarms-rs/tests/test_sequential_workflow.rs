use futures::future::{BoxFuture, ready};
use swarms_rs::structs::agent::{Agent, AgentError};
use swarms_rs::structs::sequential_workflow::{SequentialWorkflow, SequentialWorkflowError};

// Mock Agent for testing
#[derive(Clone)]
struct MockAgent {
    name: String,
    response: String,
    should_fail: bool,
}

impl MockAgent {
    fn new(name: &str, response: &str) -> Self {
        Self {
            name: name.to_string(),
            response: response.to_string(),
            should_fail: false,
        }
    }

    fn new_failing(name: &str, response: &str) -> Self {
        Self {
            name: name.to_string(),
            response: response.to_string(),
            should_fail: true,
        }
    }
}

impl Agent for MockAgent {
    fn run(&self, _task: String) -> BoxFuture<'_, Result<String, AgentError>> {
        if self.should_fail {
            Box::pin(ready(Err(AgentError::NoChoiceFound)))
        } else {
            Box::pin(ready(Ok(self.response.clone())))
        }
    }

    fn run_multiple_tasks(
        &mut self,
        _tasks: Vec<String>,
    ) -> BoxFuture<'_, Result<Vec<String>, AgentError>> {
        Box::pin(ready(Ok(vec![self.response.clone()])))
    }

    fn plan(&self, _task: String) -> BoxFuture<'_, Result<(), AgentError>> {
        Box::pin(ready(Ok(())))
    }

    fn query_long_term_memory(&self, _task: String) -> BoxFuture<'_, Result<(), AgentError>> {
        Box::pin(ready(Ok(())))
    }

    fn save_task_state(&self, _task: String) -> BoxFuture<'_, Result<(), AgentError>> {
        Box::pin(ready(Ok(())))
    }

    fn is_response_complete(&self, _response: String) -> bool {
        true
    }

    fn id(&self) -> String {
        format!("mock-{}", self.name)
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn description(&self) -> String {
        format!("Mock agent: {}", self.name)
    }

    fn clone_box(&self) -> Box<dyn Agent> {
        Box::new(self.clone())
    }
}

#[tokio::test]
async fn test_sequential_workflow_builder() {
    let agent = MockAgent::new("test_agent", "test_response");
    let workflow = SequentialWorkflow::builder()
        .name("TestWorkflow")
        .description("A test workflow")
        .metadata_output_dir("/tmp/test")
        .add_agent(Box::new(agent))
        .build();

    // Test that the workflow was created successfully
    // We can't access private fields, so we'll test the functionality instead
    let result = workflow.run("test task").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_sequential_workflow_builder_multiple_agents() {
    let agent1 = MockAgent::new("agent1", "response1");
    let agent2 = MockAgent::new("agent2", "response2");

    let workflow = SequentialWorkflow::builder()
        .name("MultiAgentWorkflow")
        .add_agent(Box::new(agent1))
        .add_agent(Box::new(agent2))
        .build();

    // Test that the workflow with multiple agents works
    let result = workflow.run("test task").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_sequential_workflow_builder_agents_vec() {
    let agents: Vec<Box<dyn Agent>> = vec![
        Box::new(MockAgent::new("agent1", "response1")),
        Box::new(MockAgent::new("agent2", "response2")),
    ];

    let workflow = SequentialWorkflow::builder()
        .name("VecAgentsWorkflow")
        .agents(agents)
        .build();

    // Test that the workflow with agents vector works
    let result = workflow.run("test task").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_sequential_workflow_builder_defaults() {
    let workflow = SequentialWorkflow::builder()
        .name("DefaultWorkflow")
        .build();

    // Test that default workflow was created successfully
    // Since there are no agents, running should return NoAgents error
    let result = workflow.run("test task").await;
    assert!(matches!(result, Err(SequentialWorkflowError::NoAgents)));
}

#[tokio::test]
async fn test_sequential_workflow_run_empty_task() {
    let workflow = SequentialWorkflow::builder()
        .name("EmptyTaskWorkflow")
        .add_agent(Box::new(MockAgent::new("Agent1", "Response1")))
        .build();

    let result = workflow.run("").await;
    assert!(matches!(result, Err(SequentialWorkflowError::NoTasks)));
}

#[tokio::test]
async fn test_sequential_workflow_run_no_agents() {
    let workflow = SequentialWorkflow::builder()
        .name("NoAgentsWorkflow")
        .build();

    let result = workflow.run("test task").await;
    assert!(matches!(result, Err(SequentialWorkflowError::NoAgents)));
}

#[tokio::test]
async fn test_sequential_workflow_run_successful() {
    let workflow = SequentialWorkflow::builder()
        .name("SuccessfulWorkflow")
        .add_agent(Box::new(MockAgent::new("Agent1", "Response1")))
        .add_agent(Box::new(MockAgent::new("Agent2", "Response2")))
        .build();

    let result = workflow.run("test task").await;
    assert!(result.is_ok());

    let conversation = result.unwrap();
    assert!(conversation.history.len() > 0);
}

#[tokio::test]
async fn test_sequential_workflow_run_with_failure() {
    let workflow = SequentialWorkflow::builder()
        .name("FailureWorkflow")
        .add_agent(Box::new(MockAgent::new("Agent1", "Response1")))
        .add_agent(Box::new(MockAgent::new_failing("Agent2", "Response2")))
        .build();

    let result = workflow.run("test task").await;
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(SequentialWorkflowError::AgentError(_))
    ));
}

#[tokio::test]
async fn test_sequential_workflow_single_agent() {
    let workflow = SequentialWorkflow::builder()
        .name("SingleAgentWorkflow")
        .add_agent(Box::new(MockAgent::new("OnlyAgent", "Single response")))
        .build();

    let result = workflow.run("single task").await;
    assert!(result.is_ok());

    let conversation = result.unwrap();
    assert!(conversation.history.len() >= 2); // User message + Agent response
}

#[tokio::test]
async fn test_sequential_workflow_empty_task_string() {
    let workflow = SequentialWorkflow::builder()
        .name("EmptyStringWorkflow")
        .add_agent(Box::new(MockAgent::new("Agent", "Response")))
        .build();

    let result = workflow.run("   ").await; // whitespace only
    assert!(result.is_ok()); // whitespace is not considered empty
}

#[test]
fn test_sequential_workflow_error_types() {
    // Test different error types
    let agent_error = AgentError::NoChoiceFound;
    let workflow_error = SequentialWorkflowError::AgentError(agent_error);
    assert!(matches!(
        workflow_error,
        SequentialWorkflowError::AgentError(_)
    ));

    let workflow_error = SequentialWorkflowError::NoAgents;
    assert!(matches!(workflow_error, SequentialWorkflowError::NoAgents));

    let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
    let workflow_error = SequentialWorkflowError::JsonError(json_error);
    assert!(matches!(
        workflow_error,
        SequentialWorkflowError::JsonError(_)
    ));
}

#[test]
fn test_sequential_workflow_error_display() {
    let error = SequentialWorkflowError::NoAgents;
    assert!(error.to_string().contains("No agents provided"));

    let error = SequentialWorkflowError::NoTasks;
    assert!(error.to_string().contains("No tasks provided"));
}

#[test]
fn test_sequential_workflow_error_from_agent_error() {
    let agent_error = AgentError::NoChoiceFound;
    let workflow_error: SequentialWorkflowError = agent_error.into();
    assert!(matches!(
        workflow_error,
        SequentialWorkflowError::AgentError(_)
    ));
}

#[test]
fn test_sequential_workflow_error_from_json_error() {
    let json_error = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
    let workflow_error: SequentialWorkflowError = json_error.into();
    assert!(matches!(
        workflow_error,
        SequentialWorkflowError::JsonError(_)
    ));
}

#[tokio::test]
async fn test_sequential_workflow_long_task() {
    let long_task = "a".repeat(1000);
    let workflow = SequentialWorkflow::builder()
        .name("LongTaskWorkflow")
        .add_agent(Box::new(MockAgent::new("Agent", "Response")))
        .build();

    let result = workflow.run(long_task).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_sequential_workflow_special_characters() {
    let special_task = "Task with special chars: !@#$%^&*()_+{}|:<>?[]\\;'\"./,`~";
    let workflow = SequentialWorkflow::builder()
        .name("SpecialCharsWorkflow")
        .add_agent(Box::new(MockAgent::new("Agent", "Response")))
        .build();

    let result = workflow.run(special_task).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_sequential_workflow_unicode_task() {
    let unicode_task = "Unicode task: 你好世界 🚀 αβγδε";
    let workflow = SequentialWorkflow::builder()
        .name("UnicodeWorkflow")
        .add_agent(Box::new(MockAgent::new("Agent", "Response")))
        .build();

    let result = workflow.run(unicode_task).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_sequential_workflow_builder_chaining() {
    let workflow = SequentialWorkflow::builder()
        .name("ChainedWorkflow")
        .description("Test chaining")
        .metadata_output_dir("/tmp/chained")
        .add_agent(Box::new(MockAgent::new("Agent1", "Response1")))
        .add_agent(Box::new(MockAgent::new("Agent2", "Response2")))
        .build();

    // Test that the chained workflow works properly
    let result = workflow.run("test task").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_sequential_workflow_conversation_history() {
    let workflow = SequentialWorkflow::builder()
        .name("HistoryWorkflow")
        .add_agent(Box::new(MockAgent::new("Agent1", "First response")))
        .add_agent(Box::new(MockAgent::new("Agent2", "Second response")))
        .build();

    let result = workflow.run("test task").await;
    assert!(result.is_ok());

    let conversation = result.unwrap();
    // Should have at least: user message + agent1 response + agent2 response
    assert!(conversation.history.len() >= 3);
}

#[tokio::test]
async fn test_sequential_workflow_many_agents() {
    let mut builder = SequentialWorkflow::builder().name("ManyAgentsWorkflow");

    // Add 10 agents
    for i in 0..10 {
        builder = builder.add_agent(Box::new(MockAgent::new(
            &format!("Agent{}", i),
            &format!("Response{}", i),
        )));
    }

    let workflow = builder.build();
    let result = workflow.run("test task").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_sequential_workflow_with_metadata_dir() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let metadata_path = temp_dir.path().to_str().unwrap();

    let workflow = SequentialWorkflow::builder()
        .name("MetadataWorkflow")
        .metadata_output_dir(metadata_path)
        .add_agent(Box::new(MockAgent::new("Agent1", "Response1")))
        .build();

    let result = workflow.run("test task").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_sequential_workflow_error_propagation() {
    let workflow = SequentialWorkflow::builder()
        .name("ErrorPropagationWorkflow")
        .add_agent(Box::new(MockAgent::new("Agent1", "Success")))
        .add_agent(Box::new(MockAgent::new_failing("Agent2", "Fail")))
        .add_agent(Box::new(MockAgent::new("Agent3", "Never reached")))
        .build();

    let result = workflow.run("test task").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_sequential_workflow_first_agent_fails() {
    let workflow = SequentialWorkflow::builder()
        .name("FirstAgentFailsWorkflow")
        .add_agent(Box::new(MockAgent::new_failing("Agent1", "Fail")))
        .add_agent(Box::new(MockAgent::new("Agent2", "Never reached")))
        .build();

    let result = workflow.run("test task").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_sequential_workflow_last_agent_fails() {
    let workflow = SequentialWorkflow::builder()
        .name("LastAgentFailsWorkflow")
        .add_agent(Box::new(MockAgent::new("Agent1", "Success")))
        .add_agent(Box::new(MockAgent::new("Agent2", "Success")))
        .add_agent(Box::new(MockAgent::new_failing("Agent3", "Fail")))
        .build();

    let result = workflow.run("test task").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_sequential_workflow_complex_task() {
    let complex_task = format!(
        "Analyze the following data:\n{}\n\nProvide insights and recommendations.",
        "x".repeat(5000)
    );

    let workflow = SequentialWorkflow::builder()
        .name("ComplexTaskWorkflow")
        .add_agent(Box::new(MockAgent::new("Analyzer", "Analysis complete")))
        .build();

    let result = workflow.run(complex_task).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_sequential_workflow_empty_description() {
    let workflow = SequentialWorkflow::builder()
        .name("EmptyDescWorkflow")
        .description("")
        .add_agent(Box::new(MockAgent::new("Agent1", "Response1")))
        .build();

    let result = workflow.run("test task").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_sequential_workflow_empty_name() {
    let workflow = SequentialWorkflow::builder()
        .name("")
        .add_agent(Box::new(MockAgent::new("Agent1", "Response1")))
        .build();

    let result = workflow.run("test task").await;
    assert!(result.is_ok());
}
