use crate::structs::persistence;
use crate::structs::tool::ToolError;
// Logging imports available for future use
// use crate::{log_agent, log_error_ctx};
use colored::*;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::broadcast;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Serde json error: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("Broadcast error: {0}")]
    BroadcastError(#[from] broadcast::error::SendError<Result<String, String>>),
    #[error("Persistence error: {0}")]
    PersistenceError(#[from] persistence::PersistenceError),
    #[error("Invalid save state path: {0}")]
    InvalidSaveStatePath(String),
    #[error("Completion error: {0}")]
    CompletionError(#[from] crate::llm::CompletionError),
    #[error("No choice found")]
    NoChoiceFound,
    #[error("Tool {0} not found")]
    ToolNotFound(String),
    #[error("Tool error: {0}")]
    ToolError(#[from] ToolError),

    #[cfg(test)]
    #[error("Test error")]
    TestError(String),
}

#[derive(Clone)]
pub struct AgentConfigBuilder {
    config: Arc<AgentConfig>,
}

impl AgentConfigBuilder {
    pub fn agent_name(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        if self.config.verbose {
            log::debug!("🏷️  Setting agent name: {}", name.bright_cyan().bold());
        }
        Arc::make_mut(&mut self.config).name = name;
        self
    }

    pub fn user_name(mut self, name: impl Into<String>) -> Self {
        Arc::make_mut(&mut self.config).user_name = name.into();
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        Arc::make_mut(&mut self.config).description = Some(description.into());
        self
    }

    pub fn temperature(mut self, temperature: f64) -> Self {
        Arc::make_mut(&mut self.config).temperature = temperature;
        self
    }

    pub fn max_loops(mut self, max_loops: u32) -> Self {
        Arc::make_mut(&mut self.config).max_loops = max_loops;
        self
    }

    /// Set max tokens
    pub fn max_tokens(mut self, max_tokens: u64) -> Self {
        Arc::make_mut(&mut self.config).max_tokens = max_tokens;
        self
    }

    /// Set markdown enabled/disabled
    pub fn md(mut self, enabled: bool) -> Self {
        Arc::make_mut(&mut self.config).markdown = enabled;
        self
    }

    /// Enable plan
    pub fn enable_plan(mut self, planning_prompt: impl Into<Option<String>>) -> Self {
        let config = Arc::make_mut(&mut self.config);
        config.plan_enabled = true;
        config.planning_prompt = planning_prompt.into();
        self
    }

    pub fn enable_autosave(mut self) -> Self {
        Arc::make_mut(&mut self.config).autosave = true;
        self
    }

    pub fn retry_attempts(mut self, retry_attempts: u32) -> Self {
        Arc::make_mut(&mut self.config).retry_attempts = retry_attempts;
        self
    }

    pub fn enable_rag_every_loop(mut self) -> Self {
        Arc::make_mut(&mut self.config).rag_every_loop = true;
        self
    }

    pub fn save_sate_path(mut self, path: impl Into<String>) -> Self {
        Arc::make_mut(&mut self.config).save_state_dir = Some(path.into());
        self
    }

    pub fn add_stop_word(mut self, stop_word: impl Into<String>) -> Self {
        Arc::make_mut(&mut self.config)
            .stop_words
            .insert(stop_word.into());
        self
    }

    pub fn stop_words(mut self, stop_words: Vec<String>) -> Self {
        let config = Arc::make_mut(&mut self.config);
        config.stop_words = stop_words.into_iter().collect();
        self
    }

    pub fn verbose(mut self, verbose: bool) -> Self {
        Arc::make_mut(&mut self.config).verbose = verbose;
        self
    }

    pub fn build(self) -> Arc<AgentConfig> {
        let config = &self.config;
        if config.verbose {
            log::info!(
                "🎯 Agent configuration built: {} (ID: {}) - Max loops: {}, Temperature: {}, Max tokens: {}",
                config.name.bright_cyan().bold(),
                config.id.bright_yellow(),
                config.max_loops.to_string().bright_green(),
                config.temperature.to_string().bright_blue(),
                config.max_tokens.to_string().bright_purple()
            );
        }
        self.config
    }
}

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub id: String,
    pub name: String,
    pub user_name: String,
    pub description: Option<String>,
    pub temperature: f64,
    pub max_loops: u32,
    pub max_tokens: u64,
    pub plan_enabled: bool,
    pub planning_prompt: Option<String>,
    pub autosave: bool,
    pub retry_attempts: u32,
    pub rag_every_loop: bool,
    pub save_state_dir: Option<String>,
    #[serde(with = "hashset_serde")]
    pub stop_words: HashSet<String>,
    pub task_evaluator_tool_enabled: bool,
    pub concurrent_tool_call_enabled: bool,
    pub verbose: bool,
    pub markdown: bool,
    #[serde(skip)]
    pub response_cache: HashMap<String, String>,
}

// Helper module for HashSet serialization
mod hashset_serde {
    use super::*;
    use serde::{Deserializer, Serializer};
    use std::collections::HashSet;

    pub fn serialize<S>(set: &HashSet<String>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let vec: Vec<_> = set.iter().collect();
        vec.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<HashSet<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vec: Vec<String> = Vec::deserialize(deserializer)?;
        Ok(vec.into_iter().collect())
    }
}

impl AgentConfig {
    pub fn builder() -> AgentConfigBuilder {
        AgentConfigBuilder {
            config: Arc::new(AgentConfig::default()),
        }
    }

    // Add a method to compute a hash for caching
    pub fn compute_hash(&self, input: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        input.hash(&mut hasher);
        hasher.finish()
    }

    // Add a method to check cache
    pub fn get_cached_response(&self, input: &str) -> Option<&String> {
        self.response_cache.get(input)
    }

    // Add a method to cache response
    pub fn cache_response(&mut self, input: String, response: String) {
        self.response_cache.insert(input, response);
    }

    /// Get a formatter instance configured for this agent
    pub fn get_formatter(&self) -> crate::utils::formatter::Formatter {
        crate::utils::formatter::Formatter::new(self.markdown)
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        let id = uuid::Uuid::new_v4().to_string();

        let config = Self {
            id: id.clone(),
            name: "Agent".to_owned(),
            user_name: "User".to_owned(),
            description: None,
            temperature: 0.7,
            max_loops: 1,
            max_tokens: 8192,
            plan_enabled: false,
            planning_prompt: None,
            autosave: false,
            retry_attempts: 3,
            rag_every_loop: false,
            save_state_dir: None,
            stop_words: HashSet::with_capacity(16), // Pre-allocate capacity
            task_evaluator_tool_enabled: true,
            concurrent_tool_call_enabled: true,
            verbose: false,                              // Default to verbose logging
            markdown: true,
            response_cache: HashMap::with_capacity(100), // Pre-allocate cache capacity
        };

        if config.verbose && log::log_enabled!(log::Level::Debug) {
            log::debug!(
                "🆕 Creating default agent configuration with ID: {}",
                id.bright_yellow()
            );
        }

        config
    }
}

pub trait Agent: Send + Sync {
    /// Runs the autonomous agent loop to complete the given task.
    fn run(&self, task: String) -> BoxFuture<Result<String, AgentError>>;

    /// Run multiple tasks concurrently
    fn run_multiple_tasks(
        &mut self,
        tasks: Vec<String>,
    ) -> BoxFuture<Result<Vec<String>, AgentError>>;

    /// Plan the task and add it to short term memory
    fn plan(&self, task: String) -> BoxFuture<Result<(), AgentError>>;

    /// Query long term memory and add the results to short term memory
    fn query_long_term_memory(&self, task: String) -> BoxFuture<Result<(), AgentError>>;

    /// Save the agent state to a file
    fn save_task_state(&self, task: String) -> BoxFuture<Result<(), AgentError>>;

    /// Check a response to determine if it is complete
    fn is_response_complete(&self, response: String) -> bool;

    /// Get agent ID
    fn id(&self) -> String;

    /// Get agent name
    fn name(&self) -> String;

    /// Get agent description
    fn description(&self) -> String;

    /// Set markdown enabled/disabled for this agent
    fn md(&mut self, enabled: bool);

    fn clone_box(&self) -> Box<dyn Agent>;
}

impl Clone for Box<dyn Agent> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
