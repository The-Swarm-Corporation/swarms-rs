//! Tests for Agent Configuration
//! This module tests the agent configuration builder and agent config struct

use swarms_rs::structs::agent::{AgentConfig, AgentError};
use tempfile::tempdir;

#[test]
fn test_agent_config_builder_creation() {
    let config = AgentConfig::builder()
        .agent_name("TestAgent")
        .user_name("TestUser")
        .description("A test agent")
        .temperature(0.7)
        .max_loops(5)
        .max_tokens(1000)
        .build();

    assert_eq!(config.name, "TestAgent");
    assert_eq!(config.user_name, "TestUser");
    assert_eq!(config.description, Some("A test agent".to_string()));
    assert_eq!(config.temperature, Some(0.7));
    assert_eq!(config.max_loops, 5);
    assert_eq!(config.max_tokens, 1000);
    assert!(!config.id.is_empty());
}

#[test]
fn test_agent_config_builder_defaults() {
    let config = AgentConfig::builder()
        .agent_name("DefaultAgent")
        .user_name("DefaultUser")
        .build();

    assert_eq!(config.name, "DefaultAgent");
    assert_eq!(config.user_name, "DefaultUser");
    assert_eq!(config.description, None);
    assert_eq!(config.temperature, None); // Default: left to the provider
    assert_eq!(config.max_loops, 1); // Default value
    assert!(!config.id.is_empty());
}

#[test]
fn test_agent_config_builder_plan_enabled() {
    let config = AgentConfig::builder()
        .agent_name("PlanAgent")
        .user_name("PlanUser")
        .enable_plan(Some("Test planning prompt".to_string()))
        .build();

    assert!(config.plan_enabled);
    assert_eq!(
        config.planning_prompt,
        Some("Test planning prompt".to_string())
    );
}

#[test]
fn test_agent_config_builder_autosave() {
    let config = AgentConfig::builder()
        .agent_name("AutosaveAgent")
        .user_name("AutosaveUser")
        .enable_autosave()
        .build();

    assert!(config.autosave);
}

#[test]
fn test_agent_config_builder_retry_attempts() {
    let config = AgentConfig::builder()
        .agent_name("RetryAgent")
        .user_name("RetryUser")
        .retry_attempts(5)
        .build();

    assert_eq!(config.retry_attempts, 5);
}

#[test]
fn test_agent_config_builder_rag_every_loop() {
    let config = AgentConfig::builder()
        .agent_name("RAGAgent")
        .user_name("RAGUser")
        .enable_rag_every_loop()
        .build();

    assert!(config.rag_every_loop);
}

#[test]
fn test_agent_config_builder_save_state_path() {
    let temp_dir = tempdir().unwrap();
    let path = temp_dir.path().join("agent_state.json");
    let path_str = path.to_str().unwrap();

    let config = AgentConfig::builder()
        .agent_name("SaveStateAgent")
        .user_name("SaveStateUser")
        .save_sate_path(path_str)
        .build();

    assert_eq!(config.save_state_dir, Some(path_str.to_string()));
}

#[test]
fn test_agent_config_builder_stop_words() {
    let config = AgentConfig::builder()
        .agent_name("StopWordsAgent")
        .user_name("StopWordsUser")
        .add_stop_word("STOP")
        .add_stop_word("END")
        .build();

    assert!(config.stop_words.contains("STOP"));
    assert!(config.stop_words.contains("END"));
    assert_eq!(config.stop_words.len(), 2);
}

#[test]
fn test_agent_config_builder_stop_words_batch() {
    let stop_words = vec!["STOP".to_string(), "END".to_string(), "QUIT".to_string()];

    let config = AgentConfig::builder()
        .agent_name("BatchStopWordsAgent")
        .user_name("BatchStopWordsUser")
        .stop_words(stop_words.clone())
        .build();

    assert_eq!(config.stop_words.len(), 3);
    for word in stop_words {
        assert!(config.stop_words.contains(&word));
    }
}

#[test]
fn test_agent_config_builder_verbose() {
    let config = AgentConfig::builder()
        .agent_name("VerboseAgent")
        .user_name("VerboseUser")
        .verbose(true)
        .build();

    assert!(config.verbose);
}

#[test]
fn test_agent_config_builder_chaining() {
    let config = AgentConfig::builder()
        .agent_name("ChainedAgent")
        .user_name("ChainedUser")
        .description("A chained configuration test")
        .temperature(0.8)
        .max_loops(3)
        .max_tokens(500)
        .enable_plan(Some("Chained plan".to_string()))
        .enable_autosave()
        .retry_attempts(2)
        .enable_rag_every_loop()
        .add_stop_word("CHAIN_STOP")
        .verbose(true)
        .build();

    assert_eq!(config.name, "ChainedAgent");
    assert_eq!(config.user_name, "ChainedUser");
    assert_eq!(
        config.description,
        Some("A chained configuration test".to_string())
    );
    assert_eq!(config.temperature, Some(0.8));
    assert_eq!(config.max_loops, 3);
    assert_eq!(config.max_tokens, 500);
    assert!(config.plan_enabled);
    assert_eq!(config.planning_prompt, Some("Chained plan".to_string()));
    assert!(config.autosave);
    assert_eq!(config.retry_attempts, 2);
    assert!(config.rag_every_loop);
    assert!(config.stop_words.contains("CHAIN_STOP"));
    assert!(config.verbose);
}

#[test]
fn test_agent_config_serialization() {
    let config = AgentConfig::builder()
        .agent_name("SerializableAgent")
        .user_name("SerializableUser")
        .description("Test serialization")
        .temperature(0.5)
        .max_loops(7)
        .build();

    // Test serialization to JSON
    let json = serde_json::to_string(&config).unwrap();
    assert!(!json.is_empty());
    assert!(json.contains("SerializableAgent"));

    // Test deserialization from JSON
    let deserialized: AgentConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name, config.name);
    assert_eq!(deserialized.user_name, config.user_name);
    assert_eq!(deserialized.description, config.description);
    assert_eq!(deserialized.temperature, config.temperature);
    assert_eq!(deserialized.max_loops, config.max_loops);
}

#[test]
fn test_agent_error_types() {
    // Test different error types
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
    let agent_error = AgentError::IoError(io_error);
    assert!(matches!(agent_error, AgentError::IoError(_)));

    let serde_error = serde_json::from_str::<AgentConfig>("invalid json").unwrap_err();
    let agent_error = AgentError::SerdeError(serde_error);
    assert!(matches!(agent_error, AgentError::SerdeError(_)));

    let agent_error = AgentError::NoChoiceFound;
    assert!(matches!(agent_error, AgentError::NoChoiceFound));

    let agent_error = AgentError::ToolNotFound("nonexistent_tool".to_string());
    assert!(matches!(agent_error, AgentError::ToolNotFound(_)));
}

#[test]
fn test_agent_config_cache_response() {
    let mut config = AgentConfig::builder()
        .agent_name("CacheAgent")
        .user_name("CacheUser")
        .build();

    let arc_config = std::sync::Arc::make_mut(&mut config);

    // Test caching a response
    let input = "test input".to_string();
    let response = "test response".to_string();
    arc_config.cache_response(input.clone(), response.clone());

    // Test retrieving cached response
    let cached = arc_config.get_cached_response(&input);
    assert!(cached.is_some());
    assert_eq!(cached.unwrap(), &response);

    // Test retrieving non-existent cached response
    let non_existent = arc_config.get_cached_response("non_existent");
    assert!(non_existent.is_none());
}

#[test]
fn test_agent_config_compute_hash() {
    let config = AgentConfig::builder()
        .agent_name("HashAgent")
        .user_name("HashUser")
        .build();

    let input1 = "test input";
    let input2 = "test input";
    let input3 = "different input";

    // Same inputs should produce same hash
    let hash1 = config.compute_hash(input1);
    let hash2 = config.compute_hash(input2);
    assert_eq!(hash1, hash2);

    // Different inputs should produce different hashes
    let hash3 = config.compute_hash(input3);
    assert_ne!(hash1, hash3);
}

#[test]
fn test_agent_config_default_values() {
    let config = AgentConfig::default();

    assert!(!config.id.is_empty());
    assert_eq!(config.name, "Agent");
    assert_eq!(config.user_name, "User");
    assert_eq!(config.description, None);
    assert_eq!(config.temperature, None);
    assert_eq!(config.max_loops, 1);
    assert_eq!(config.max_tokens, 8192);
    assert!(!config.plan_enabled);
    assert_eq!(config.planning_prompt, None);
    assert!(!config.autosave);
    assert_eq!(config.retry_attempts, 3);
    assert!(!config.rag_every_loop);
    assert_eq!(config.save_state_dir, None);
    assert!(config.stop_words.is_empty());
    assert!(config.task_evaluator_tool_enabled);
    assert!(config.concurrent_tool_call_enabled);
    assert!(!config.verbose);
    assert!(!config.pretty_print_on);
}

#[test]
fn test_agent_config_pretty_print() {
    let config = AgentConfig::builder()
        .agent_name("PrettyPrintAgent")
        .user_name("PrettyPrintUser")
        .pretty_print_on(true)
        .build();

    assert!(config.pretty_print_on);
}

#[test]
fn test_agent_config_enable_plan_without_prompt() {
    let config = AgentConfig::builder()
        .agent_name("PlanNoPromptAgent")
        .user_name("PlanNoPromptUser")
        .enable_plan(None)
        .build();

    assert!(config.plan_enabled);
    assert_eq!(config.planning_prompt, None);
}

#[test]
fn test_agent_config_temperature_bounds() {
    // Test low temperature
    let config_low = AgentConfig::builder()
        .agent_name("LowTempAgent")
        .temperature(0.0)
        .build();
    assert_eq!(config_low.temperature, Some(0.0));

    // Test high temperature
    let config_high = AgentConfig::builder()
        .agent_name("HighTempAgent")
        .temperature(2.0)
        .build();
    assert_eq!(config_high.temperature, Some(2.0));
}

#[test]
fn test_agent_config_max_loops_edge_cases() {
    // Test with 0 loops
    let config_zero = AgentConfig::builder()
        .agent_name("ZeroLoopAgent")
        .max_loops(0)
        .build();
    assert_eq!(config_zero.max_loops, 0);

    // Test with very high loops
    let config_high = AgentConfig::builder()
        .agent_name("HighLoopAgent")
        .max_loops(1000)
        .build();
    assert_eq!(config_high.max_loops, 1000);
}

#[test]
fn test_agent_config_unique_ids() {
    let config1 = AgentConfig::default();
    let config2 = AgentConfig::default();

    // Each config should have a unique ID
    assert_ne!(config1.id, config2.id);
}

#[cfg(test)]
mod agent_error_tests {
    use super::*;

    #[test]
    fn test_agent_error_display() {
        let error = AgentError::InvalidSaveStatePath("invalid/path".to_string());
        assert_eq!(error.to_string(), "Invalid save state path: invalid/path");

        let error = AgentError::ToolNotFound("missing_tool".to_string());
        assert_eq!(error.to_string(), "Tool missing_tool not found");

        let error = AgentError::NoChoiceFound;
        assert_eq!(error.to_string(), "No choice found");
    }
}
