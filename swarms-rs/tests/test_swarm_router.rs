//! Tests for SwarmRouter configuration and error types
//! Note: Full SwarmRouter integration tests require actual SwarmsAgent instances
//! which are tested in other integration test files

use futures::future::BoxFuture;
use swarms_rs::structs::agent::{Agent, AgentError};
use swarms_rs::structs::rearrange::AgentRearrange;
use swarms_rs::structs::swarms_router::{
    SwarmRouter, SwarmRouterConfig, SwarmRouterError, SwarmType,
};

#[derive(Clone)]
struct MockAgent {
    name: String,
    response: String,
}

impl MockAgent {
    fn boxed(name: &str, response: &str) -> Box<dyn Agent> {
        Box::new(Self {
            name: name.to_string(),
            response: response.to_string(),
        })
    }
}

impl Agent for MockAgent {
    fn run(&self, _task: String) -> BoxFuture<Result<String, AgentError>> {
        let response = self.response.clone();
        Box::pin(async move { Ok(response) })
    }

    fn run_multiple_tasks(
        &mut self,
        _tasks: Vec<String>,
    ) -> BoxFuture<Result<Vec<String>, AgentError>> {
        Box::pin(async { Ok(vec![]) })
    }

    fn plan(&self, _task: String) -> BoxFuture<Result<(), AgentError>> {
        Box::pin(async { Ok(()) })
    }

    fn query_long_term_memory(&self, _task: String) -> BoxFuture<Result<(), AgentError>> {
        Box::pin(async { Ok(()) })
    }

    fn save_task_state(&self, _task: String) -> BoxFuture<Result<(), AgentError>> {
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
        format!("Mock agent: {}", self.name)
    }

    fn clone_box(&self) -> Box<dyn Agent> {
        Box::new(self.clone())
    }
}

#[tokio::test]
async fn test_router_agent_rearrange_runs_flow() {
    let rearrange = AgentRearrange::builder()
        .add_agent(MockAgent::boxed("agent1", "response1"))
        .add_agent(MockAgent::boxed("agent2", "response2"))
        .flow("agent1 -> agent2")
        .build();
    let router = SwarmRouter::AgentRearrange(rearrange);

    let transcript = router.run("route this").await.unwrap().to_string();
    assert!(transcript.contains("route this"));
    assert!(transcript.contains("response1"));
    assert!(transcript.contains("response2"));

    let results = router
        .batch_run(vec!["task1".to_string(), "task2".to_string()])
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
    for entry in results.iter() {
        let transcript = entry.value().to_string();
        assert!(transcript.contains(entry.key().as_str()));
        assert!(transcript.contains("response2"));
    }
}

#[tokio::test]
async fn test_router_agent_rearrange_surfaces_errors() {
    // No flow configured: the rearrange fails validation, and the router must say so
    let rearrange = AgentRearrange::builder()
        .add_agent(MockAgent::boxed("agent1", "response1"))
        .build();
    let router = SwarmRouter::AgentRearrange(rearrange);

    assert!(matches!(
        router.run("task").await,
        Err(SwarmRouterError::AgentRearrangeError(_))
    ));
}

#[test]
fn test_swarm_router_config_default() {
    let config = SwarmRouterConfig::default();

    assert_eq!(config.name, "swarm-router");
    assert_eq!(config.description, "Routes your task to the desired swarm");
    assert!(matches!(config.swarm_type, SwarmType::SequentialWorkflow));
    assert_eq!(config.agents.len(), 0);
    assert_eq!(config.rules, None);
    assert!(config.multi_agent_collab_prompt);
    assert_eq!(config.flow, None);
    assert_eq!(config.max_loops, None);
}

#[test]
fn test_swarm_router_config_custom_name() {
    let mut config = SwarmRouterConfig::default();
    config.name = "custom-router".to_string();

    assert_eq!(config.name, "custom-router");
}

#[test]
fn test_swarm_router_config_custom_description() {
    let mut config = SwarmRouterConfig::default();
    config.description = "Custom description".to_string();

    assert_eq!(config.description, "Custom description");
}

#[test]
fn test_swarm_router_config_with_rules() {
    let mut config = SwarmRouterConfig::default();
    config.rules = Some("Always be helpful and accurate".to_string());

    assert!(config.rules.is_some());
    assert_eq!(config.rules.unwrap(), "Always be helpful and accurate");
}

#[test]
fn test_swarm_router_config_disable_multi_agent_collab() {
    let mut config = SwarmRouterConfig::default();
    config.multi_agent_collab_prompt = false;

    assert!(!config.multi_agent_collab_prompt);
}

#[test]
fn test_swarm_router_config_with_flow() {
    let mut config = SwarmRouterConfig::default();
    config.flow = Some("agent1 -> agent2 -> agent3".to_string());

    assert!(config.flow.is_some());
    assert_eq!(config.flow.unwrap(), "agent1 -> agent2 -> agent3");
}

#[test]
fn test_swarm_router_config_with_max_loops() {
    let mut config = SwarmRouterConfig::default();
    config.max_loops = Some(5);

    assert!(config.max_loops.is_some());
    assert_eq!(config.max_loops.unwrap(), 5);
}

#[test]
fn test_swarm_type_sequential_workflow() {
    let json = r#""SequentialWorkflow""#;
    let swarm_type: SwarmType = serde_json::from_str(json).unwrap();

    assert!(matches!(swarm_type, SwarmType::SequentialWorkflow));
}

#[test]
fn test_swarm_type_concurrent_workflow() {
    let json = r#""ConcurrentWorkflow""#;
    let swarm_type: SwarmType = serde_json::from_str(json).unwrap();

    assert!(matches!(swarm_type, SwarmType::ConcurrentWorkflow));
}

#[test]
fn test_swarm_type_agent_rearrange() {
    let json = r#""AgentRearrange""#;
    let swarm_type: SwarmType = serde_json::from_str(json).unwrap();

    assert!(matches!(swarm_type, SwarmType::AgentRearrange));
}

#[test]
fn test_swarm_type_debug() {
    let swarm_type = SwarmType::SequentialWorkflow;
    let debug_str = format!("{:?}", swarm_type);

    assert_eq!(debug_str, "SequentialWorkflow");
}

#[test]
fn test_swarm_router_error_display_validation() {
    let error = SwarmRouterError::ValidationError("No agents provided".to_string());
    let error_str = error.to_string();

    assert!(error_str.contains("SwarmRouter validation error"));
    assert!(error_str.contains("No agents provided"));
}

#[test]
fn test_swarm_router_error_debug() {
    let error = SwarmRouterError::ValidationError("Test error".to_string());
    let debug_str = format!("{:?}", error);

    assert!(debug_str.contains("ValidationError"));
}

#[test]
fn test_swarm_router_config_empty_rules() {
    let mut config = SwarmRouterConfig::default();
    config.rules = Some("".to_string());

    assert!(config.rules.is_some());
    assert_eq!(config.rules.unwrap(), "");
}

#[test]
fn test_swarm_router_config_long_description() {
    let mut config = SwarmRouterConfig::default();
    let long_desc = "a".repeat(10000);
    config.description = long_desc.clone();

    assert_eq!(config.description, long_desc);
}

#[test]
fn test_swarm_router_config_special_characters() {
    let mut config = SwarmRouterConfig::default();
    config.name = "router-🚀".to_string();
    config.description = "Description with 你好世界".to_string();

    assert_eq!(config.name, "router-🚀");
    assert_eq!(config.description, "Description with 你好世界");
}

#[test]
fn test_swarm_router_config_complex_flow() {
    let mut config = SwarmRouterConfig::default();
    config.flow = Some("agent1 -> agent2, agent3 -> H -> agent4".to_string());

    assert!(config.flow.is_some());
    assert!(config.flow.unwrap().contains("->"));
}

#[test]
fn test_swarm_router_config_zero_max_loops() {
    let mut config = SwarmRouterConfig::default();
    config.max_loops = Some(0);

    assert_eq!(config.max_loops.unwrap(), 0);
}

#[test]
fn test_swarm_router_config_large_max_loops() {
    let mut config = SwarmRouterConfig::default();
    config.max_loops = Some(1000);

    assert_eq!(config.max_loops.unwrap(), 1000);
}

#[test]
fn test_swarm_router_config_multiline_rules() {
    let mut config = SwarmRouterConfig::default();
    let rules = "Rule 1: Be accurate\nRule 2: Be helpful\nRule 3: Be concise".to_string();
    config.rules = Some(rules.clone());

    assert_eq!(config.rules.unwrap(), rules);
}

#[test]
fn test_swarm_router_config_with_all_options() {
    let mut config = SwarmRouterConfig::default();
    config.name = "full-router".to_string();
    config.description = "Fully configured router".to_string();
    config.rules = Some("Custom rules".to_string());
    config.multi_agent_collab_prompt = false;
    config.flow = Some("a -> b -> c".to_string());
    config.max_loops = Some(10);

    assert_eq!(config.name, "full-router");
    assert_eq!(config.description, "Fully configured router");
    assert!(config.rules.is_some());
    assert!(!config.multi_agent_collab_prompt);
    assert!(config.flow.is_some());
    assert_eq!(config.max_loops.unwrap(), 10);
}

#[test]
fn test_swarm_type_deserialization_case_sensitive() {
    // Test that exact case is required
    let json = r#""sequentialworkflow""#;
    let result: Result<SwarmType, _> = serde_json::from_str(json);

    assert!(result.is_err());
}

#[test]
fn test_swarm_router_config_empty_name() {
    let mut config = SwarmRouterConfig::default();
    config.name = "".to_string();

    assert_eq!(config.name, "");
}

#[test]
fn test_swarm_router_config_empty_description() {
    let mut config = SwarmRouterConfig::default();
    config.description = "".to_string();

    assert_eq!(config.description, "");
}

#[test]
fn test_swarm_router_config_none_flow() {
    let config = SwarmRouterConfig::default();

    assert!(config.flow.is_none());
}

#[test]
fn test_swarm_router_config_none_max_loops() {
    let config = SwarmRouterConfig::default();

    assert!(config.max_loops.is_none());
}

#[test]
fn test_swarm_router_config_none_rules() {
    let config = SwarmRouterConfig::default();

    assert!(config.rules.is_none());
}

#[test]
fn test_swarm_type_all_variants() {
    // Test that all three variants can be deserialized
    let variants = vec![
        r#""SequentialWorkflow""#,
        r#""ConcurrentWorkflow""#,
        r#""AgentRearrange""#,
    ];

    for variant in variants {
        let result: Result<SwarmType, _> = serde_json::from_str(variant);
        assert!(result.is_ok());
    }
}

#[test]
fn test_swarm_router_config_modification_chain() {
    let mut config = SwarmRouterConfig::default();

    config.name = "test".to_string();
    config.description = "test desc".to_string();
    config.multi_agent_collab_prompt = false;

    assert_eq!(config.name, "test");
    assert_eq!(config.description, "test desc");
    assert!(!config.multi_agent_collab_prompt);
}

#[test]
fn test_swarm_router_error_types() {
    use swarms_rs::structs::concurrent_workflow::ConcurrentWorkflowError;
    use swarms_rs::structs::sequential_workflow::SequentialWorkflowError;

    // Test ValidationError
    let error = SwarmRouterError::ValidationError("test".to_string());
    assert!(matches!(error, SwarmRouterError::ValidationError(_)));

    // Test SequentialWorkflowError conversion
    let seq_error = SequentialWorkflowError::NoAgents;
    let router_error: SwarmRouterError = seq_error.into();
    assert!(matches!(
        router_error,
        SwarmRouterError::SequentialWorkflowError(_)
    ));

    // Test ConcurrentWorkflowError conversion
    let concurrent_error = ConcurrentWorkflowError::EmptyTasksOrAgents;
    let router_error: SwarmRouterError = concurrent_error.into();
    assert!(matches!(
        router_error,
        SwarmRouterError::ConcurrentWorkflowError(_)
    ));
}

#[test]
fn test_swarm_router_config_default_swarm_type() {
    let config = SwarmRouterConfig::default();

    // Default should be SequentialWorkflow
    match config.swarm_type {
        SwarmType::SequentialWorkflow => assert!(true),
        _ => panic!("Expected SequentialWorkflow as default"),
    }
}

#[test]
fn test_swarm_router_config_change_swarm_type() {
    let mut config = SwarmRouterConfig::default();

    // Can deserialize and assign different types
    config.swarm_type = serde_json::from_str(r#""ConcurrentWorkflow""#).unwrap();
    assert!(matches!(config.swarm_type, SwarmType::ConcurrentWorkflow));

    config.swarm_type = serde_json::from_str(r#""AgentRearrange""#).unwrap();
    assert!(matches!(config.swarm_type, SwarmType::AgentRearrange));
}
