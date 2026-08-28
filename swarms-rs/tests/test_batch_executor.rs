use futures::future::BoxFuture;
use swarms_rs::structs::agent::{Agent, AgentError};
use swarms_rs::structs::execute_agent_batch::{
    AgentBatchExecutor, BatchConfig, BatchConfigBuilder, BatchExecutionError,
};

// Mock agent for testing
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

    fn new_failing(name: &str) -> Self {
        Self {
            name: name.to_string(),
            response: String::new(),
            should_fail: true,
        }
    }
}

impl Agent for MockAgent {
    fn run(&self, _task: String) -> BoxFuture<'_, Result<String, AgentError>> {
        let should_fail = self.should_fail;
        let response = self.response.clone();
        Box::pin(async move {
            if should_fail {
                Err(AgentError::NoChoiceFound)
            } else {
                Ok(response)
            }
        })
    }

    fn run_multiple_tasks(
        &mut self,
        _tasks: Vec<String>,
    ) -> BoxFuture<'_, Result<Vec<String>, AgentError>> {
        let response = self.response.clone();
        Box::pin(async move { Ok(vec![response]) })
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

#[test]
fn test_batch_config_default() {
    let config = BatchConfig::default();

    assert_eq!(config.max_concurrent_tasks, None);
    assert!(config.auto_cpu_optimization);
    assert_eq!(config.worker_threads, None);
}

#[test]
fn test_batch_config_builder_max_concurrent_tasks() {
    let config = BatchConfigBuilder::default()
        .max_concurrent_tasks(10)
        .build();

    assert_eq!(config.max_concurrent_tasks, Some(10));
}

#[test]
fn test_batch_config_builder_auto_cpu_optimization() {
    let config = BatchConfigBuilder::default()
        .auto_cpu_optimization(false)
        .build();

    assert!(!config.auto_cpu_optimization);
}

#[test]
fn test_batch_config_builder_worker_threads() {
    let config = BatchConfigBuilder::default().worker_threads(8).build();

    assert_eq!(config.worker_threads, Some(8));
}

#[test]
fn test_batch_config_builder_chaining() {
    let config = BatchConfigBuilder::default()
        .max_concurrent_tasks(5)
        .auto_cpu_optimization(true)
        .worker_threads(4)
        .build();

    assert_eq!(config.max_concurrent_tasks, Some(5));
    assert!(config.auto_cpu_optimization);
    assert_eq!(config.worker_threads, Some(4));
}

#[test]
fn test_agent_batch_executor_creation() {
    let agents: Vec<Box<dyn Agent>> = vec![
        Box::new(MockAgent::new("Agent1", "Response1")),
        Box::new(MockAgent::new("Agent2", "Response2")),
    ];

    let config = BatchConfig::default();
    let _executor = AgentBatchExecutor::new(agents, config);

    // If we reach here, the executor was created successfully
    assert!(true);
}

#[test]
fn test_agent_batch_executor_builder() {
    let _executor = AgentBatchExecutor::builder()
        .add_agent(Box::new(MockAgent::new("Agent1", "Response1")))
        .add_agent(Box::new(MockAgent::new("Agent2", "Response2")))
        .config(BatchConfig::default())
        .build();

    assert!(true);
}

#[tokio::test]
async fn test_batch_executor_execute_batch_success() {
    let agents: Vec<Box<dyn Agent>> = vec![Box::new(MockAgent::new("Agent1", "Response1"))];

    let config = BatchConfig::default();
    let executor = AgentBatchExecutor::new(agents, config);

    let tasks = vec!["Task1".to_string(), "Task2".to_string()];
    let result = executor.execute_batch(tasks).await;

    assert!(result.is_ok());
    let results = result.unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_batch_executor_execute_batch_no_agents() {
    let agents: Vec<Box<dyn Agent>> = vec![];
    let config = BatchConfig::default();
    let executor = AgentBatchExecutor::new(agents, config);

    let tasks = vec!["Task1".to_string()];
    let result = executor.execute_batch(tasks).await;

    assert!(result.is_err());
    if let Err(e) = result {
        assert!(matches!(e, BatchExecutionError::NoAgents));
    }
}

#[tokio::test]
async fn test_batch_executor_execute_batch_no_tasks() {
    let agents: Vec<Box<dyn Agent>> = vec![Box::new(MockAgent::new("Agent1", "Response1"))];

    let config = BatchConfig::default();
    let executor = AgentBatchExecutor::new(agents, config);

    let tasks: Vec<String> = vec![];
    let result = executor.execute_batch(tasks).await;

    assert!(result.is_err());
    if let Err(e) = result {
        assert!(matches!(e, BatchExecutionError::NoTasks));
    }
}

#[tokio::test]
async fn test_batch_executor_with_multiple_agents() {
    let agents: Vec<Box<dyn Agent>> = vec![
        Box::new(MockAgent::new("Agent1", "Response1")),
        Box::new(MockAgent::new("Agent2", "Response2")),
        Box::new(MockAgent::new("Agent3", "Response3")),
    ];

    let config = BatchConfig::default();
    let executor = AgentBatchExecutor::new(agents, config);

    let tasks = vec!["Task1".to_string()];
    let result = executor.execute_batch(tasks).await;

    assert!(result.is_ok());
    let results = result.unwrap();
    // One task, three agents = 3 results
    assert_eq!(results.len(), 3);
}

#[tokio::test]
async fn test_batch_executor_with_custom_concurrency() {
    let agents: Vec<Box<dyn Agent>> = vec![Box::new(MockAgent::new("Agent1", "Response1"))];

    let config = BatchConfigBuilder::default()
        .max_concurrent_tasks(2)
        .build();

    let executor = AgentBatchExecutor::new(agents, config);

    let tasks = vec![
        "Task1".to_string(),
        "Task2".to_string(),
        "Task3".to_string(),
        "Task4".to_string(),
    ];

    let result = executor.execute_batch(tasks).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_batch_executor_with_failing_agent() {
    let agents: Vec<Box<dyn Agent>> = vec![
        Box::new(MockAgent::new("Agent1", "Response1")),
        Box::new(MockAgent::new_failing("Agent2")),
    ];

    let config = BatchConfig::default();
    let executor = AgentBatchExecutor::new(agents, config);

    let tasks = vec!["Task1".to_string()];
    let result = executor.execute_batch(tasks).await;

    // The executor should complete but with some failed results
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_batch_executor_with_worker_threads() {
    let agents: Vec<Box<dyn Agent>> = vec![Box::new(MockAgent::new("Agent1", "Response1"))];

    let config = BatchConfigBuilder::default().worker_threads(2).build();

    let executor = AgentBatchExecutor::new(agents, config);

    let tasks = vec!["Task1".to_string(), "Task2".to_string()];
    let result = executor.execute_batch(tasks).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_batch_executor_auto_cpu_optimization_disabled() {
    let agents: Vec<Box<dyn Agent>> = vec![Box::new(MockAgent::new("Agent1", "Response1"))];

    let config = BatchConfigBuilder::default()
        .auto_cpu_optimization(false)
        .build();

    let executor = AgentBatchExecutor::new(agents, config);

    let tasks = vec!["Task1".to_string()];
    let result = executor.execute_batch(tasks).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_batch_executor_large_batch() {
    let agents: Vec<Box<dyn Agent>> = vec![Box::new(MockAgent::new("Agent1", "Response1"))];

    let config = BatchConfigBuilder::default()
        .max_concurrent_tasks(10)
        .build();

    let executor = AgentBatchExecutor::new(agents, config);

    // Create 50 tasks
    let tasks: Vec<String> = (0..50).map(|i| format!("Task {}", i)).collect();

    let result = executor.execute_batch(tasks).await;
    assert!(result.is_ok());

    let results = result.unwrap();
    assert_eq!(results.len(), 50);
}

#[test]
fn test_batch_config_clone() {
    let config1 = BatchConfigBuilder::default()
        .max_concurrent_tasks(5)
        .worker_threads(4)
        .build();

    let config2 = config1.clone();

    assert_eq!(config1.max_concurrent_tasks, config2.max_concurrent_tasks);
    assert_eq!(config1.worker_threads, config2.worker_threads);
    assert_eq!(config1.auto_cpu_optimization, config2.auto_cpu_optimization);
}

#[test]
fn test_batch_execution_error_display() {
    let error = BatchExecutionError::NoAgents;
    assert_eq!(error.to_string(), "No agents provided");

    let error = BatchExecutionError::NoTasks;
    assert_eq!(error.to_string(), "No tasks provided");
}

#[test]
fn test_batch_execution_error_from_agent_error() {
    let agent_error = AgentError::NoChoiceFound;
    let batch_error: BatchExecutionError = agent_error.into();
    assert!(matches!(batch_error, BatchExecutionError::AgentError(_)));
}

#[tokio::test]
async fn test_batch_executor_builder_with_config() {
    let config = BatchConfigBuilder::default()
        .max_concurrent_tasks(3)
        .build();

    let executor = AgentBatchExecutor::builder()
        .add_agent(Box::new(MockAgent::new("Agent1", "Response1")))
        .config(config)
        .build();

    let tasks = vec!["Task1".to_string()];
    let result = executor.execute_batch(tasks).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_batch_executor_empty_agent_list() {
    let executor = AgentBatchExecutor::builder().build();

    let tasks = vec!["Task1".to_string()];
    let result = executor.execute_batch(tasks).await;

    assert!(result.is_err());
    if let Err(e) = result {
        assert!(matches!(e, BatchExecutionError::NoAgents));
    }
}

#[test]
fn test_batch_config_builder_default() {
    let builder = BatchConfigBuilder::default();
    let config = builder.build();

    assert_eq!(config.max_concurrent_tasks, None);
    assert!(config.auto_cpu_optimization);
    assert_eq!(config.worker_threads, None);
}

#[tokio::test]
async fn test_batch_executor_with_special_characters() {
    let agents: Vec<Box<dyn Agent>> = vec![Box::new(MockAgent::new("Agent1", "Response1"))];

    let config = BatchConfig::default();
    let executor = AgentBatchExecutor::new(agents, config);

    let tasks = vec![
        "Task with émojis 🚀".to_string(),
        "Task with unicode: 你好世界".to_string(),
        "Task with symbols: @#$%^&*()".to_string(),
    ];

    let result = executor.execute_batch(tasks).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_batch_executor_results_contain_conversations() {
    let agents: Vec<Box<dyn Agent>> = vec![Box::new(MockAgent::new("Agent1", "Response1"))];

    let config = BatchConfig::default();
    let executor = AgentBatchExecutor::new(agents, config);

    let tasks = vec!["Task1".to_string()];
    let result = executor.execute_batch(tasks).await;

    assert!(result.is_ok());
    let results = result.unwrap();

    // Check that the result contains a conversation
    let conversation = results.get("Task1");
    assert!(conversation.is_some());
}
