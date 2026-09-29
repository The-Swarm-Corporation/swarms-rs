use futures::future::BoxFuture;
use swarms_rs::structs::agent::{Agent, AgentError};
use swarms_rs::structs::rearrange::{AgentRearrange, AgentRearrangeError, OutputType, rearrange};
use uuid::Uuid;

// Mock agent for testing
#[derive(Clone)]
struct MockAgent {
    name: String,
    response: String,
}

impl MockAgent {
    fn new(name: impl Into<String>, response: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            response: response.into(),
        }
    }
}

impl Agent for MockAgent {
    fn run(&self, _task: String) -> BoxFuture<'_, Result<String, AgentError>> {
        let response = self.response.clone();
        Box::pin(async move { Ok(response) })
    }

    fn run_multiple_tasks(
        &mut self,
        tasks: Vec<String>,
    ) -> BoxFuture<'_, Result<Vec<String>, AgentError>> {
        let response = self.response.clone();
        Box::pin(async move { Ok(tasks.into_iter().map(|_| response.clone()).collect()) })
    }

    fn plan(&self, _task: String) -> BoxFuture<'_, Result<(), AgentError>> {
        Box::pin(async { Ok(()) })
    }

    fn query_long_term_memory(&self, _task: String) -> BoxFuture<'_, Result<(), AgentError>> {
        Box::pin(async { Ok(()) })
    }

    fn save_task_state(&self, _task: String) -> BoxFuture<'_, Result<(), AgentError>> {
        Box::pin(async { Ok(()) })
    }

    fn is_response_complete(&self, _response: String) -> bool {
        true
    }

    fn id(&self) -> String {
        Uuid::new_v4().to_string()
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

/// Agent whose reply depends on its input: "slow done" (after a delay) for tasks
/// containing "SLOW", otherwise whether the input contained "ORIGINAL".
#[derive(Clone)]
struct ProbeAgent {
    name: String,
}

impl ProbeAgent {
    fn boxed(name: &str) -> Box<dyn Agent> {
        Box::new(Self {
            name: name.to_string(),
        })
    }
}

impl Agent for ProbeAgent {
    fn run(&self, task: String) -> BoxFuture<'_, Result<String, AgentError>> {
        Box::pin(async move {
            if task.contains("SLOW") {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                return Ok("slow done".to_string());
            }
            if task.contains("ORIGINAL") {
                Ok("saw-original".to_string())
            } else {
                Ok("missing-original".to_string())
            }
        })
    }

    fn run_multiple_tasks(
        &mut self,
        _tasks: Vec<String>,
    ) -> BoxFuture<'_, Result<Vec<String>, AgentError>> {
        Box::pin(async { Ok(vec![]) })
    }

    fn plan(&self, _task: String) -> BoxFuture<'_, Result<(), AgentError>> {
        Box::pin(async { Ok(()) })
    }

    fn query_long_term_memory(&self, _task: String) -> BoxFuture<'_, Result<(), AgentError>> {
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
        format!("Probe agent: {}", self.name)
    }

    fn clone_box(&self) -> Box<dyn Agent> {
        Box::new(self.clone())
    }
}

#[tokio::test]
async fn test_concurrent_run_preserves_task_order() {
    let mut rearrange = AgentRearrange::builder()
        .add_agent(ProbeAgent::boxed("probe"))
        .flow("probe")
        .output_type(OutputType::Final)
        .build();

    // The first task finishes last; results must still follow input order
    let tasks = vec!["SLOW task".to_string(), "ORIGINAL task".to_string()];
    let results = rearrange
        .concurrent_run(tasks, None, Some(2))
        .await
        .unwrap();
    assert_eq!(results, vec!["slow done", "saw-original"]);
}

#[tokio::test]
async fn test_concurrent_run_zero_concurrency_does_not_hang() {
    let mut rearrange = AgentRearrange::builder()
        .add_agent(Box::new(MockAgent::new("agent1", "response1")))
        .flow("agent1")
        .build();

    let tasks = vec!["task1".to_string(), "task2".to_string()];
    let results = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        rearrange.concurrent_run(tasks, None, Some(0)),
    )
    .await
    .expect("concurrent_run hung with max_concurrent = 0")
    .unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_batch_run_zero_batch_size() {
    let mut rearrange = AgentRearrange::builder()
        .add_agent(Box::new(MockAgent::new("agent1", "response1")))
        .flow("agent1")
        .build();

    let tasks = vec!["task1".to_string(), "task2".to_string()];
    let results = rearrange.batch_run(tasks, 0, None).await.unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_parallel_group_sets_final_output_and_sees_task() {
    // Parallel-only flow: the final output is the group's responses, not the raw task
    let mut rearrange = AgentRearrange::builder()
        .add_agent(ProbeAgent::boxed("probe1"))
        .add_agent(ProbeAgent::boxed("probe2"))
        .flow("probe1, probe2")
        .output_type(OutputType::Final)
        .build();
    let result = rearrange.run("ORIGINAL task").await.unwrap();
    assert_eq!(result, "probe1: saw-original\nprobe2: saw-original");

    // After a sequential step, parallel agents still see the original task
    let mut rearrange = AgentRearrange::builder()
        .add_agent(Box::new(MockAgent::new("agent1", "response1")))
        .add_agent(ProbeAgent::boxed("probe2"))
        .add_agent(ProbeAgent::boxed("probe3"))
        .flow("agent1 -> probe2, probe3")
        .output_type(OutputType::Final)
        .build();
    let result = rearrange.run("ORIGINAL task").await.unwrap();
    assert_eq!(result, "probe2: saw-original\nprobe3: saw-original");
}

#[tokio::test]
async fn test_duplicate_agent_names_rejected() {
    let mut rearrange = AgentRearrange::builder()
        .add_agent(Box::new(MockAgent::new("agent1", "first")))
        .add_agent(Box::new(MockAgent::new("agent1", "second")))
        .flow("agent1")
        .build();
    assert!(matches!(
        rearrange.validate_flow(),
        Err(AgentRearrangeError::DuplicateAgentNames)
    ));
    assert!(matches!(
        rearrange.run("task").await,
        Err(AgentRearrangeError::DuplicateAgentNames)
    ));

    let mut rearrange = AgentRearrange::builder().build();
    rearrange
        .add_agent(Box::new(MockAgent::new("agent1", "first")))
        .unwrap();
    assert!(matches!(
        rearrange.add_agent(Box::new(MockAgent::new("agent1", "second"))),
        Err(AgentRearrangeError::DuplicateAgentNames)
    ));
    assert_eq!(rearrange.agent_count(), 1);
}

#[tokio::test]
async fn test_duplicate_agent_names_can_be_fixed_by_removing_the_agent() {
    let mut rearrange = AgentRearrange::builder()
        .add_agent(Box::new(MockAgent::new("agent1", "first")))
        .add_agent(Box::new(MockAgent::new("agent1", "second")))
        .flow("agent1")
        .build();
    assert!(rearrange.validate_flow().is_err());

    rearrange.remove_agent("agent1");
    rearrange
        .add_agent(Box::new(MockAgent::new("agent1", "fixed")))
        .unwrap();
    assert!(rearrange.validate_flow().is_ok());
    assert!(rearrange.run("task").await.is_ok());
}

#[tokio::test]
async fn test_list_output_keeps_every_response_in_order() {
    let mut rearrange = AgentRearrange::builder()
        .add_agent(Box::new(MockAgent::new("agent1", "response1")))
        .add_agent(Box::new(MockAgent::new("agent2", "response2")))
        .flow("agent1 -> agent2 -> agent1")
        .output_type(OutputType::List)
        .build();

    let result = rearrange.run("task").await.unwrap();
    assert_eq!(result, "response1\nresponse2\nresponse1");
}

#[tokio::test]
async fn test_agent_rearrange_builder() {
    let agent1 = Box::new(MockAgent::new("agent1", "response1")) as Box<dyn Agent>;
    let agent2 = Box::new(MockAgent::new("agent2", "response2")) as Box<dyn Agent>;

    let rearrange = AgentRearrange::builder()
        .name("TestRearrange")
        .description("Test description")
        .add_agent(agent1)
        .add_agent(agent2)
        .flow("agent1 -> agent2")
        .max_loops(2)
        .verbose(true)
        .build();

    assert_eq!(rearrange.name(), "TestRearrange");
    assert_eq!(rearrange.description(), "Test description");
    assert_eq!(rearrange.flow(), "agent1 -> agent2");
    assert_eq!(rearrange.max_loops(), 2);
    assert_eq!(rearrange.agent_count(), 2);
}

#[tokio::test]
async fn test_flow_validation() {
    let agent1 = Box::new(MockAgent::new("agent1", "response1")) as Box<dyn Agent>;
    let agent2 = Box::new(MockAgent::new("agent2", "response2")) as Box<dyn Agent>;

    let mut rearrange = AgentRearrange::builder()
        .add_agent(agent1)
        .add_agent(agent2)
        .flow("agent1 -> agent2")
        .build();

    // Valid flow
    assert!(rearrange.validate_flow().is_ok());

    // Invalid flow - space-separated without comma (ambiguous)
    rearrange.set_custom_flow("agent1 agent2");
    assert!(rearrange.validate_flow().is_err());

    // Invalid flow - nonexistent agent
    rearrange.set_custom_flow("agent1 -> nonexistent");
    assert!(rearrange.validate_flow().is_err());
}

#[tokio::test]
async fn test_sequential_execution() {
    let agent1 = Box::new(MockAgent::new("agent1", "response1")) as Box<dyn Agent>;
    let agent2 = Box::new(MockAgent::new("agent2", "response2")) as Box<dyn Agent>;

    let mut rearrange = AgentRearrange::builder()
        .add_agent(agent1)
        .add_agent(agent2)
        .flow("agent1 -> agent2")
        .output_type(OutputType::Final)
        .build();

    let result = rearrange.run("test task").await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "response2");
}

#[tokio::test]
async fn test_parallel_execution() {
    let agent1 = Box::new(MockAgent::new("agent1", "response1")) as Box<dyn Agent>;
    let agent2 = Box::new(MockAgent::new("agent2", "response2")) as Box<dyn Agent>;

    let mut rearrange = AgentRearrange::builder()
        .add_agent(agent1)
        .add_agent(agent2)
        .flow("agent1, agent2")
        .output_type(OutputType::Dict)
        .build();

    let result = rearrange.run("test task").await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("agent1"));
    assert!(output.contains("agent2"));
}

#[tokio::test]
async fn test_batch_run() {
    let agent1 = Box::new(MockAgent::new("agent1", "response1")) as Box<dyn Agent>;

    let mut rearrange = AgentRearrange::builder()
        .add_agent(agent1)
        .flow("agent1")
        .build();

    let tasks = vec!["task1".to_string(), "task2".to_string()];
    let results = rearrange.batch_run(tasks, 2, None).await;

    assert!(results.is_ok());
    let results = results.unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_concurrent_run() {
    let agent1 = Box::new(MockAgent::new("agent1", "response1")) as Box<dyn Agent>;

    let mut rearrange = AgentRearrange::builder()
        .add_agent(agent1)
        .flow("agent1")
        .build();

    let tasks = vec!["task1".to_string(), "task2".to_string()];
    let results = rearrange.concurrent_run(tasks, None, Some(2)).await;

    assert!(results.is_ok());
    let results = results.unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_output_types() {
    let agent1 = Box::new(MockAgent::new("agent1", "response1")) as Box<dyn Agent>;

    // Test All output type
    let mut rearrange = AgentRearrange::builder()
        .add_agent(agent1.clone_box())
        .flow("agent1")
        .output_type(OutputType::All)
        .build();

    let result = rearrange.run("test").await.unwrap();
    assert!(result.contains("agent1: response1"));

    // Test Final output type
    let mut rearrange = AgentRearrange::builder()
        .add_agent(agent1.clone_box())
        .flow("agent1")
        .output_type(OutputType::Final)
        .build();

    let result = rearrange.run("test").await.unwrap();
    assert_eq!(result, "response1");
}

#[tokio::test]
async fn test_convenience_function() {
    let agent1 = Box::new(MockAgent::new("agent1", "response1")) as Box<dyn Agent>;
    let agent2 = Box::new(MockAgent::new("agent2", "response2")) as Box<dyn Agent>;

    let agents = vec![agent1, agent2];

    let result = rearrange(
        "TestProcessor",
        "Test rearrange function",
        agents,
        "agent1 -> agent2",
        "Test task",
        None,
    )
    .await;

    assert!(result.is_ok());
    let output = result.unwrap();
    // Default output type is All, so it should contain both agent responses
    assert!(output.contains("agent1: response1"));
    assert!(output.contains("agent2: response2"));
}

#[tokio::test]
async fn test_agent_management() {
    let mut rearrange = AgentRearrange::builder().name("TestRearrange").build();

    // Test adding agents
    let agent1 = Box::new(MockAgent::new("agent1", "response1")) as Box<dyn Agent>;
    let agent2 = Box::new(MockAgent::new("agent2", "response2")) as Box<dyn Agent>;

    rearrange.add_agent(agent1).unwrap();
    rearrange.add_agent(agent2).unwrap();

    assert_eq!(rearrange.agent_count(), 2);
    assert!(rearrange.agent_names().contains(&&"agent1".to_string()));
    assert!(rearrange.agent_names().contains(&&"agent2".to_string()));

    // Test removing agents
    let removed = rearrange.remove_agent("agent1");
    assert!(removed.is_some());
    assert_eq!(rearrange.agent_count(), 1);

    // Test removing non-existent agent
    let not_found = rearrange.remove_agent("nonexistent");
    assert!(not_found.is_none());
}

#[tokio::test]
async fn test_flow_patterns() {
    let agent1 = Box::new(MockAgent::new("agent1", "response1")) as Box<dyn Agent>;
    let agent2 = Box::new(MockAgent::new("agent2", "response2")) as Box<dyn Agent>;
    let agent3 = Box::new(MockAgent::new("agent3", "response3")) as Box<dyn Agent>;

    let mut rearrange = AgentRearrange::builder()
        .add_agent(agent1)
        .add_agent(agent2)
        .add_agent(agent3)
        .flow("agent1 -> agent2, agent3")
        .output_type(OutputType::Dict)
        .build();

    let result = rearrange.run("test task").await;
    assert!(result.is_ok());
    let output = result.unwrap();

    // Should contain results from all agents
    assert!(output.contains("agent1"));
    assert!(output.contains("agent2"));
    assert!(output.contains("agent3"));
}

#[tokio::test]
async fn test_human_in_the_loop_placeholder() {
    let agent1 = Box::new(MockAgent::new("agent1", "response1")) as Box<dyn Agent>;
    let agent2 = Box::new(MockAgent::new("agent2", "response2")) as Box<dyn Agent>;

    let mut rearrange = AgentRearrange::builder()
        .add_agent(agent1)
        .add_agent(agent2)
        .flow("agent1 -> H -> agent2")
        .output_type(OutputType::Final)
        .verbose(true)
        .build();

    let result = rearrange.run("test task").await;
    assert!(result.is_ok());
    // Should still execute successfully, skipping the human intervention point
    assert_eq!(result.unwrap(), "response2");
}

#[tokio::test]
async fn test_multiple_loops() {
    let agent1 = Box::new(MockAgent::new("agent1", "response1")) as Box<dyn Agent>;

    let mut rearrange = AgentRearrange::builder()
        .add_agent(agent1)
        .flow("agent1")
        .max_loops(3)
        .output_type(OutputType::Final)
        .build();

    let result = rearrange.run("test task").await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "response1");
}

#[tokio::test]
async fn test_json_output() {
    let agent1 = Box::new(MockAgent::new("agent1", "response1")) as Box<dyn Agent>;
    let agent2 = Box::new(MockAgent::new("agent2", "response2")) as Box<dyn Agent>;

    let mut rearrange = AgentRearrange::builder()
        .add_agent(agent1)
        .add_agent(agent2)
        .flow("agent1, agent2")
        .output_type(OutputType::Dict)
        .return_json(true)
        .build();

    let result = rearrange.run("test task").await;
    assert!(result.is_ok());
    let output = result.unwrap();

    // Should be valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&output).expect("Should be valid JSON");
    assert!(parsed.is_object());
}

#[tokio::test]
async fn test_empty_flow_validation() {
    let agent1 = Box::new(MockAgent::new("agent1", "response1")) as Box<dyn Agent>;

    let rearrange = AgentRearrange::builder().add_agent(agent1).flow("").build();

    assert!(rearrange.validate_flow().is_err());
}

#[tokio::test]
async fn test_rules_injection() {
    let agent1 = Box::new(MockAgent::new("agent1", "response1")) as Box<dyn Agent>;

    let mut rearrange = AgentRearrange::builder()
        .add_agent(agent1)
        .flow("agent1")
        .rules("Always be helpful and accurate")
        .build();

    let result = rearrange.run("test task").await;
    assert!(result.is_ok());

    // Check that rules were added to conversation
    let conversation = rearrange.conversation();
    let conversation_str = conversation.to_string();
    assert!(conversation_str.contains("Rules: Always be helpful and accurate"));
}
