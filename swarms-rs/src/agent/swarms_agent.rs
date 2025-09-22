//! # Swarms Agent Implementation
//! 
//! This module provides the core `SwarmsAgent` implementation - an autonomous AI agent
//! that can execute complex tasks through iterative loops, tool usage, and memory management.
//! 
//! ## Overview
//! 
//! The `SwarmsAgent` is designed to autonomously complete tasks by:
//! 
//! 1. **Planning**: Optional planning phase to break down complex tasks
//! 2. **Execution**: Iterative execution loops with configurable retry mechanisms
//! 3. **Tool Usage**: Integration with both native Rust tools and external MCP servers
//! 4. **Memory Management**: Short-term conversation memory and optional long-term storage
//! 5. **State Persistence**: Automatic saving and restoration of agent state
//! 6. **Error Recovery**: Built-in retry mechanisms and graceful error handling
//! 
//! ## Key Features
//! 
//! - **Autonomous Task Execution**: Can complete complex tasks without human intervention
//! - **Tool Integration**: Seamlessly integrates with Rust tools and external MCP servers
//! - **Memory Management**: Maintains conversation history and context throughout execution
//! - **State Persistence**: Optional automatic saving of agent state to disk
//! - **Configurable Execution**: Flexible configuration for different use cases
//! - **Concurrent Operations**: Supports concurrent tool calls and multiple task execution
//! - **Error Recovery**: Includes retry mechanisms and comprehensive error handling
//! - **Performance Considerations**:
//!     - **Memory Usage**: Short-term memory grows with conversation length
//!     - **Tool Execution**: Tools can be executed concurrently for better performance
//!     - **State Persistence**: Automatic saving can be disabled for performance-critical scenarios
//!     - **Loop Limits**: Configure max_loops to prevent infinite execution
//!     - **Retry Logic**: Balance reliability with performance through retry configuration
//!     - **Logging Overhead**: Disable verbose logging in production for better performance
//! 
//! ## Security Considerations
//! 
//! - **Tool Sandboxing**: Tools should implement their own security measures
//! - **Input Validation**: Validate all inputs before processing
//! - **State Isolation**: Each agent instance maintains isolated state
//! - **Error Information**: Avoid exposing sensitive information in error messages
//! - **Resource Limits**: Configure appropriate limits to prevent resource exhaustion
//! - **Access Control**: Implement access controls for sensitive operations

use std::{
    ffi::OsStr,
    hash::{Hash, Hasher},
    ops::Deref,
    path::Path,
    sync::Arc,
};
use twox_hash::XxHash3_64;

use colored::*;
use dashmap::DashMap;
use futures::{StreamExt, future::BoxFuture, stream};
use reqwest::IntoUrl;
use rmcp::{
    ServiceExt,
    model::{ClientCapabilities, ClientInfo, Implementation},
    transport::{SseTransport, TokioChildProcess},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use swarms_macro::tool;
use thiserror::Error;
use tokio::{
    process::Command,
    sync::{Mutex, mpsc},
};

use crate::{
    self as swarms_rs,
    llm::{
        self,
        request::{CompletionRequest, ToolDefinition},
    },
    log_agent, log_error_ctx, log_llm, log_memory, log_perf, log_task,
    structs::{
        conversation::{AgentShortMemory, Role},
        agent::{Agent, AgentConfig, AgentError},
        persistence,
        tool::{MCPTool, Tool, ToolDyn},
    },
};

/// Builder pattern implementation for creating `SwarmsAgent` instances with customizable configuration.
/// 
/// The `SwarmsAgentBuilder` provides a fluent interface for configuring all aspects of an agent
/// before building the final instance. This includes LLM settings, tool integration, memory configuration,
/// and execution parameters.
/// 
/// # Type Parameters
/// 
/// - `M`: The LLM model type that implements `llm::Model`
/// 
/// # Examples
/// 
/// ```rust,no_run
/// use swarms_rs::agent::SwarmsAgentBuilder;
/// use swarms_rs::llm::provider::openai::OpenAIProvider;
/// 
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let model = OpenAIProvider::new("api-key")?;
/// 
/// let agent = SwarmsAgentBuilder::new_with_model(model)
///     .agent_name("DataAnalyst")
///     .system_prompt("You are a data analysis expert.")
///     .max_loops(3)
///     .temperature(0.5)
///     .enable_autosave()
///     .verbose(true)
///     .build();
/// # Ok(())
/// # }
/// ```
pub struct SwarmsAgentBuilder<M>
where
    M: llm::Model + Send + Sync,
    M::RawCompletionResponse: Send + Sync,
{
    /// The LLM model instance used for generating responses
    model: M,
    /// Agent configuration including execution parameters
    config: AgentConfig,
    /// Optional system prompt to guide agent behavior
    system_prompt: Option<String>,
    /// List of tool definitions available to the agent
    tools: Vec<ToolDefinition>,
    /// Implementation instances of tools, keyed by tool name
    tools_impl: DashMap<String, Arc<dyn ToolDyn>>,
}

impl<M> SwarmsAgentBuilder<M>
where
    M: llm::Model + Clone + Send + Sync,
    M::RawCompletionResponse: Clone + Send + Sync,
{
    /// Creates a new `SwarmsAgentBuilder` with the specified LLM model.
    /// 
    /// This is the entry point for building a new agent. The model will be used
    /// for all LLM interactions during agent execution.
    /// 
    /// # Arguments
    /// 
    /// * `model` - An LLM model instance that implements the `llm::Model` trait
    /// 
    /// # Examples
    /// 
    /// ```rust,no_run
    /// use swarms_rs::agent::SwarmsAgentBuilder;
    /// use swarms_rs::llm::provider::openai::OpenAIProvider;
    /// 
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = OpenAIProvider::new("your-api-key")?;
    /// let builder = SwarmsAgentBuilder::new_with_model(model);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new_with_model(model: M) -> Self {
        Self {
            model,
            config: AgentConfig::default(),
            system_prompt: None,
            tools: vec![],
            tools_impl: DashMap::new(),
        }
    }

    /// Sets a custom agent configuration.
    /// 
    /// This replaces the default configuration with a custom one. Use this when you
    /// need fine-grained control over agent behavior or when loading a saved configuration.
    /// 
    /// # Arguments
    /// 
    /// * `config` - The `AgentConfig` to use for this agent
    /// 
    /// # Examples
    /// 
    /// ```rust,no_run
    /// use swarms_rs::agent::SwarmsAgentBuilder;
    /// use swarms_rs::structs::agent::AgentConfig;
    /// use swarms_rs::llm::provider::openai::OpenAIProvider;
    /// 
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = OpenAIProvider::new("api-key")?;
    /// let custom_config = AgentConfig::builder()
    ///     .agent_name("CustomAgent")
    ///     .max_loops(5)
    ///     .temperature(0.3)
    ///     .build();
    /// 
    /// let agent = SwarmsAgentBuilder::new_with_model(model)
    ///     .config(*custom_config)
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn config(mut self, config: AgentConfig) -> Self {
        self.config = config;
        self
    }

    /// Sets the system prompt that guides the agent's behavior.
    /// 
    /// The system prompt is sent to the LLM before every interaction and defines
    /// the agent's role, personality, and general instructions.
    /// 
    /// # Arguments
    /// 
    /// * `system_prompt` - A string that will be used as the system prompt
    /// 
    /// # Examples
    /// 
    /// ```rust,no_run
    /// use swarms_rs::agent::SwarmsAgentBuilder;
    /// use swarms_rs::llm::provider::openai::OpenAIProvider;
    /// 
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = OpenAIProvider::new("api-key")?;
    /// 
    /// let agent = SwarmsAgentBuilder::new_with_model(model)
    ///     .system_prompt("You are a helpful assistant specialized in data analysis. Always provide detailed explanations.")
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn system_prompt(mut self, system_prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(system_prompt.into());
        self
    }

    /// Adds a native Rust tool to the agent's toolkit.
    /// 
    /// Tools extend the agent's capabilities by allowing it to perform specific actions
    /// or access external data. The agent can call these tools autonomously based
    /// on the task requirements.
    /// 
    /// # Type Parameters
    /// 
    /// * `T` - A type that implements the `Tool` trait
    /// 
    /// # Arguments
    /// 
    /// * `tool` - An instance of the tool to add
    /// 
    /// # Examples
    /// 
    /// ```rust,no_run
    /// use swarms_rs::agent::SwarmsAgentBuilder;
    /// use swarms_rs::llm::provider::openai::OpenAIProvider;
    /// use swarms_rs::structs::tool::Tool;
    /// 
    /// // Define a custom tool (implementation details omitted)
    /// struct CalculatorTool;
    /// 
    /// # impl Tool for CalculatorTool {
    /// #     fn name(&self) -> String { "calculator".to_string() }
    /// #     fn description(&self) -> String { "A calculator tool".to_string() }
    /// #     fn parameters(&self) -> serde_json::Value { serde_json::json!({}) }
    /// #     fn call(&self, args: String) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, swarms_rs::structs::tool::ToolError>> + Send + '_>> { todo!() }
    /// # }
    /// 
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = OpenAIProvider::new("api-key")?;
    /// 
    /// let agent = SwarmsAgentBuilder::new_with_model(model)
    ///     .add_tool(CalculatorTool)
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_tool<T: Tool + 'static>(mut self, tool: T) -> Self {
        self.tools.push(tool.definition());
        self.tools_impl
            .insert(tool.name().to_string(), Arc::new(tool) as Arc<dyn ToolDyn>);
        self
    }

    /// Adds tools from an MCP (Model Context Protocol) server via SSE (Server-Sent Events).
    /// 
    /// This method connects to an external MCP server over HTTP/SSE and automatically
    /// adds all available tools from that server to the agent. The connection is
    /// established asynchronously and tools are loaded during the build process.
    /// 
    /// # Arguments
    /// 
    /// * `name` - A name identifier for this MCP server connection
    /// * `url` - The HTTP/HTTPS URL of the MCP server's SSE endpoint
    /// 
    /// # Examples
    /// 
    /// ```rust,no_run
    /// use swarms_rs::agent::SwarmsAgentBuilder;
    /// use swarms_rs::llm::provider::openai::OpenAIProvider;
    /// 
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = OpenAIProvider::new("api-key")?;
    /// 
    /// let agent = SwarmsAgentBuilder::new_with_model(model)
    ///     .add_sse_mcp_server("weather_service", "https://weather-api.example.com/mcp")
    ///     .await
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    /// 
    /// # Panics
    /// 
    /// This method will panic if:
    /// - The SSE transport cannot be established
    /// - The MCP server handshake fails
    /// - Tool listing from the server fails
    pub async fn add_sse_mcp_server(self, name: impl Into<String>, url: impl IntoUrl) -> Self {
        let name = name.into();

        let transport = SseTransport::start(url)
            .await
            .expect("Failed to start SSE transport");

        let client_info = ClientInfo {
            protocol_version: Default::default(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: name.clone(),
                version: "".to_owned(),
            },
        };

        let client = Arc::new(
            client_info
                .into_dyn()
                .serve(transport)
                .await
                .expect("Failed to start MCP server"),
        );

        let mcp_tools = client.list_all_tools().await.expect("Failed to list tools");
        mcp_tools.into_iter().fold(self, |acc, tool| {
            acc.add_tool(MCPTool::from_server(tool, Arc::clone(&client)))
        })
    }

    /// Adds tools from an MCP server via stdio (standard input/output).
    /// 
    /// This method launches an external process that implements the MCP protocol
    /// over stdio and automatically adds all available tools from that process
    /// to the agent. This is useful for integrating with command-line tools or
    /// scripts that implement MCP.
    /// 
    /// # Type Parameters
    /// 
    /// * `I` - An iterator of command line arguments
    /// * `S` - A type that can be converted to an OS string (typically `&str` or `String`)
    /// 
    /// # Arguments
    /// 
    /// * `command` - The command/executable to run
    /// * `args` - Command line arguments to pass to the executable
    /// 
    /// # Examples
    /// 
    /// ```rust,no_run
    /// use swarms_rs::agent::SwarmsAgentBuilder;
    /// use swarms_rs::llm::provider::openai::OpenAIProvider;
    /// 
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = OpenAIProvider::new("api-key")?;
    /// let agent = SwarmsAgentBuilder::new_with_model(model)
    ///     .add_stdio_mcp_server("python", ["./my_mcp_tool.py", "--mcp"])
    ///     .await
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    /// 
    /// # Panics
    /// 
    /// This method will panic if:
    /// - The child process cannot be spawned
    /// - The MCP server handshake fails
    /// - Tool listing from the server fails
    pub async fn add_stdio_mcp_server<I, S>(self, command: S, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let service = Arc::new(
            ().into_dyn()
                .serve(TokioChildProcess::new(Command::new(command).args(args)).unwrap())
                .await
                .expect("Failed to start MCP server"),
        );

        let mcp_tools = service
            .list_all_tools()
            .await
            .expect("Failed to list tools");
        mcp_tools.into_iter().fold(self, |acc, tool| {
            acc.add_tool(MCPTool::from_server(tool, Arc::clone(&service)))
        })
    }

    pub fn build(mut self) -> SwarmsAgent<M> {
        if self.config.verbose && log::log_enabled!(log::Level::Info) {
            log::info!(
                "🏗️  Building SwarmsAgent: {}",
                self.config.name.bright_cyan().bold()
            );
        }

        if self.config.task_evaluator_tool_enabled {
            if self.config.verbose {
                log::debug!(
                    "📋 Adding task evaluator tool for agent: {}",
                    self.config.name.bright_cyan()
                );
            }
            self.tools.insert(0, ToolDyn::definition(&TaskEvaluator));
            self.tools_impl.insert(
                ToolDyn::name(&TaskEvaluator),
                Arc::new(TaskEvaluator) as Arc<dyn ToolDyn>,
            );
        }

        let agent = SwarmsAgent {
            model: self.model,
            config: self.config.clone(),
            system_prompt: self.system_prompt,
            short_memory: AgentShortMemory::new(),
            tools: self.tools.clone(),
            tools_impl: self.tools_impl,
        };

        if agent.config.verbose && log::log_enabled!(log::Level::Info) {
            log::info!(
                "✅ SwarmsAgent built successfully: {} (ID: {}) with {} tools",
                agent.config.name.bright_cyan().bold(),
                agent.config.id.bright_yellow(),
                self.tools.len().to_string().bright_green().bold()
            );
        }

        agent
    }

    // Configuration methods

    pub fn agent_name(mut self, name: impl Into<String>) -> Self {
        self.config.name = name.into();
        self
    }

    pub fn user_name(mut self, name: impl Into<String>) -> Self {
        self.config.user_name = name.into();
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.config.description = Some(description.into());
        self
    }

    pub fn temperature(mut self, temperature: f64) -> Self {
        self.config.temperature = temperature;
        self
    }

    pub fn max_tokens(mut self, max_tokens: u64) -> Self {
        self.config.max_tokens = max_tokens;
        self
    }

    pub fn max_loops(mut self, max_loops: u32) -> Self {
        self.config.max_loops = max_loops;
        self
    }

    pub fn enable_plan(mut self, planning_prompt: impl Into<Option<String>>) -> Self {
        self.config.plan_enabled = true;
        self.config.planning_prompt = planning_prompt.into();
        self
    }

    pub fn enable_autosave(mut self) -> Self {
        self.config.autosave = true;
        self
    }

    pub fn retry_attempts(mut self, retry_attempts: u32) -> Self {
        self.config.retry_attempts = retry_attempts;
        self
    }

    pub fn enable_rag_every_loop(mut self) -> Self {
        self.config.rag_every_loop = true;
        self
    }

    pub fn save_state_dir(mut self, dir: impl Into<String>) -> Self {
        self.config.save_state_dir = Some(dir.into());
        self
    }

    pub fn add_stop_word(mut self, stop_word: impl Into<String>) -> Self {
        self.config.stop_words.insert(stop_word.into());
        self
    }

    pub fn stop_words(self, stop_words: Vec<String>) -> Self {
        stop_words
            .into_iter()
            .fold(self, |builder, stop_word| builder.add_stop_word(stop_word))
    }

    pub fn disable_task_complete_tool(mut self) -> Self {
        self.config.task_evaluator_tool_enabled = false;
        self
    }

    /// Some tools doesn't support concurrent call, so we need to disable it
    pub fn disable_concurrent_tool_call(mut self) -> Self {
        self.config.concurrent_tool_call_enabled = false;
        self
    }

    /// Enable or disable verbose logging for this agent.
    ///
    /// When verbose logging is enabled, the agent will log detailed information
    /// about its execution process, including task progress, tool calls, memory
    /// operations, and performance metrics. When disabled, the agent runs silently.
    ///
    /// # Arguments
    ///
    /// * `verbose` - `true` to enable logging, `false` to disable all logging
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use swarms_rs::agent::SwarmsAgentBuilder;
    /// use swarms_rs::llm::provider::openai::OpenAIProvider;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = OpenAIProvider::new("api-key")?;
    ///
    /// // For development with detailed logs
    /// let debug_agent = SwarmsAgentBuilder::new_with_model(model.clone())
    ///     .verbose(true)
    ///     .build();
    ///
    /// // For production with no logging
    /// let production_agent = SwarmsAgentBuilder::new_with_model(model)
    ///     .verbose(false)
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn verbose(mut self, verbose: bool) -> Self {
        self.config.verbose = verbose;
        self
    }
}

/// The main Swarms Agent implementation providing autonomous task execution capabilities.
/// 
/// `SwarmsAgent` is the core agent implementation that combines an LLM with tools, memory,
/// and configurable execution patterns to autonomously complete tasks. The agent follows
/// an iterative execution loop where it:
/// 
/// 1. Receives a task
/// 2. Optionally creates a plan
/// 3. Executes the task through multiple loops
/// 4. Uses tools when necessary
/// 5. Maintains conversation history
/// 6. Saves state for persistence
/// 
/// ## Key Capabilities
/// 
/// - **Autonomous Task Execution**: Can complete complex tasks without human intervention
/// - **Tool Integration**: Seamlessly integrates with Rust tools and external MCP servers
/// - **Memory Management**: Maintains short-term conversation memory throughout task execution
/// - **State Persistence**: Can save and restore agent state across sessions
/// - **Error Recovery**: Includes retry mechanisms and error handling
/// - **Concurrent Operations**: Supports concurrent tool calls and multiple task execution
/// 
/// ## Task Execution Flow
/// 
/// 1. **Initialization**: Task is added to memory, optional planning phase
/// 2. **Execution Loop**: Agent iteratively works on the task up to `max_loops` times
/// 3. **Tool Usage**: Agent can call tools autonomously based on task requirements
/// 4. **Completion Detection**: Built-in task evaluator or custom stop words detect completion
/// 5. **State Saving**: Optional automatic saving of conversation history and state
/// 
/// # Type Parameters
/// 
/// - `M`: The LLM model type that implements `llm::Model`
/// 
/// # Examples
/// 
/// ## Basic Task Execution
/// 
/// ```rust,no_run
/// use swarms_rs::agent::SwarmsAgentBuilder;
/// use swarms_rs::llm::provider::openai::OpenAIProvider;
/// 
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let model = OpenAIProvider::new("your-api-key")?;
/// 
/// let agent = SwarmsAgentBuilder::new_with_model(model)
///     .agent_name("DataAnalyst")
///     .system_prompt("You are a data analysis expert.")
///     .max_loops(3)
///     .build();
/// 
/// let result = agent.run("Analyze sales trends for Q4".to_string()).await?;
/// println!("Analysis: {}", result);
/// # Ok(())
/// # }
/// ```
/// 
/// ## Multiple Tasks
/// 
/// ```rust,no_run
/// use swarms_rs::agent::SwarmsAgentBuilder;
/// use swarms_rs::llm::provider::openai::OpenAIProvider;
/// 
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let model = OpenAIProvider::new("your-api-key")?;
/// 
/// let mut agent = SwarmsAgentBuilder::new_with_model(model)
///     .agent_name("MultiTaskAgent")
///     .max_loops(2)
///     .build();
/// 
/// let tasks = vec![
///     "Create a summary of recent news".to_string(),
///     "Generate a weekly report".to_string(),
///     "Analyze user feedback".to_string(),
/// ];
/// 
/// let results = agent.run_multiple_tasks(tasks).await?;
/// for result in results {
///     println!("Result: {}", result);
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Serialize)]
pub struct SwarmsAgent<M>
where
    M: llm::Model + Clone + Send + Sync,
    M::RawCompletionResponse: Clone + Send + Sync,
{
    /// The LLM model used for generating responses
    model: M,
    /// Agent configuration including execution parameters
    config: AgentConfig,
    /// Optional system prompt that guides agent behavior
    system_prompt: Option<String>,
    /// Short-term memory for maintaining conversation history
    short_memory: AgentShortMemory,
    /// List of available tool definitions
    tools: Vec<ToolDefinition>,
    /// Tool implementation instances (not serialized)
    #[serde(skip)]
    tools_impl: DashMap<String, Arc<dyn ToolDyn>>,
    /// Enable or disable markdown output formatting
    markdown: bool,
}

impl<M> SwarmsAgent<M>
where
    M: llm::Model + Clone + Send + Sync + 'static,
    M::RawCompletionResponse: Clone + Send + Sync,
{
    /// Creates a new `SwarmsAgent` with minimal configuration.
    /// 
    /// This is a simple constructor for creating an agent with just a model and optional
    /// system prompt. For more advanced configuration, use `SwarmsAgentBuilder`.
    /// 
    /// # Arguments
    /// 
    /// * `model` - The LLM model to use for generating responses
    /// * `system_prompt` - Optional system prompt to guide agent behavior
    /// 
    /// # Examples
    /// 
    /// ```rust,no_run
    /// use swarms_rs::agent::SwarmsAgent;
    /// use swarms_rs::llm::provider::openai::OpenAIProvider;
    /// 
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = OpenAIProvider::new("api-key")?;
    /// let agent = SwarmsAgent::new(model, "You are a helpful assistant");
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(model: M, system_prompt: impl Into<Option<String>>) -> Self {
        Self {
            model,
            system_prompt: system_prompt.into(),
            config: AgentConfig::default(),
            short_memory: AgentShortMemory::new(),
            tools: vec![],
            tools_impl: DashMap::new(),
            markdown: false,
        }
    }

    /// Set the configuration for this agent
    pub fn with_config(mut self, config: AgentConfig) -> Self {
        self.config = config;
        self
    }

    pub fn get_config(&self) -> &AgentConfig {
        &self.config
    }

    /// Get a formatter instance configured for this agent
    pub fn get_formatter(&self) -> crate::utils::formatter::Formatter {
        let formatter = crate::utils::formatter::Formatter::new(self.markdown);
        formatter
    }

    /// Check if markdown output is enabled for this agent
    pub fn is_markdown_enabled(&self) -> bool {
        self.markdown
    }

    /// Enable markdown output for this agent
    pub fn enable_markdown(&mut self) {
        self.markdown = true;
    }

    /// Disable markdown output for this agent
    pub fn disable_markdown(&mut self) {
        self.markdown = false;
    }

    /// Auto-render agent output with markdown formatting if enabled
    pub fn auto_render_output(&self, content: &str) {
        if self.markdown {
            let mut formatter = self.get_formatter();
            formatter.render_agent_output(&self.config.name, content);
        }
    }

    /// Render agent output with explicit markdown control
    pub fn render_output(&self, content: &str, use_markdown: bool) {
        if use_markdown {
            let mut formatter = crate::utils::formatter::Formatter::new(true);
            formatter.render_agent_output(&self.config.name, content);
        } else {
            // Plain text output
            println!("[{}]: {}", self.config.name, content);
        }
    }

    /// Performs a single chat interaction with the agent.
    /// 
    /// This method allows for direct conversation with the agent without the full
    /// autonomous task execution loop. It's useful for interactive scenarios or
    /// when you need more control over the conversation flow.
    /// 
    /// The agent will either return a text response or execute tool calls based
    /// on the conversation context.
    /// 
    /// # Arguments
    /// 
    /// * `prompt` - The user message/prompt to send to the agent
    /// * `chat_history` - Previous conversation messages for context
    /// 
    /// # Returns
    /// 
    /// Returns a `ChatResponse` which is either:
    /// - `ChatResponse::Text(String)` - A text response from the LLM
    /// - `ChatResponse::ToolCalls(Vec<ToolCallOutput>)` - Results from tool execution
    /// 
    /// # Examples
    /// 
    /// ```rust,no_run
    /// use swarms_rs::agent::SwarmsAgentBuilder;
    /// use swarms_rs::llm::provider::openai::OpenAIProvider;
    /// use swarms_rs::agent::ChatResponse;
    /// 
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = OpenAIProvider::new("api-key")?;
    /// let agent = SwarmsAgentBuilder::new_with_model(model)
    ///     .system_prompt("You are a helpful math tutor")
    ///     .build();
    /// 
    /// let response = agent.chat("What is 2 + 2?", vec![]).await?;
    /// 
    /// match response {
    ///     ChatResponse::Text(text) => println!("Agent: {}", text),
    ///     ChatResponse::ToolCalls(calls) => {
    ///         for call in calls {
    ///             println!("Tool {}: {}", call.name, call.result);
    ///         }
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    /// 
    /// # Errors
    /// 
    /// Returns an `AgentError` if:
    /// - The LLM request fails
    /// - Tool execution fails
    /// - No response choice is available
    pub async fn chat(
        &self,
        prompt: impl Into<String>,
        chat_history: impl Into<Vec<llm::completion::Message>>,
    ) -> Result<ChatResponse, AgentError> {
        let chat_history = chat_history.into();

        let request = CompletionRequest {
            prompt: llm::completion::Message::user(prompt),
            system_prompt: self.system_prompt.clone(),
            chat_history,
            tools: self.tools.clone(),
            temperature: Some(self.config.temperature),
            max_tokens: Some(self.config.max_tokens),
        };

        let response = self.model.completion(request).await?;

        let choice = response.choice.first().ok_or(AgentError::NoChoiceFound)?;
        match ToOwned::to_owned(choice) {
            llm::completion::AssistantContent::Text(text) => Ok(ChatResponse::Text(text.text)), // <--- return Text
            llm::completion::AssistantContent::ToolCall(tool_call) => {
                let mut all_tool_calls = vec![tool_call.function];
                all_tool_calls.extend(response.choice.iter().skip(1).filter_map(|choice| {
                    match ToOwned::to_owned(choice) {
                        llm::completion::AssistantContent::Text(_) => None,
                        llm::completion::AssistantContent::ToolCall(tool_call) => {
                            Some(tool_call.function)
                        },
                    }
                }));

                // Call tools concurrently
                let results = Arc::new(Mutex::new(Vec::new()));
                if self.config.concurrent_tool_call_enabled {
                    stream::iter(all_tool_calls)
                        .for_each_concurrent(None, |tool_call| {
                            let results = Arc::clone(&results);
                            async move {
                                let tool = Arc::clone(
                                    match self.tools_impl.get(&tool_call.name) {
                                        Some(tool) => tool,
                                        None => {
                                            tracing::error!("Tool not found: {}", tool_call.name);
                                            results.lock().unwrap().push(ToolCallOutput {
                                                name: tool_call.name,
                                                args: tool_call.arguments.to_string(),
                                                result: "Tool not found".to_owned(),
                                            });
                                            return;
                                        },
                                    }
                                    .deref(),
                                );
                                let args = tool_call.arguments.to_string();
                                // execute tool
                                let result = match tool.call(args.clone()).await {
                                    Ok(result) => result,
                                    Err(e) => {
                                        tracing::error!(
                                            "Failed to call tool<{}>, args: {}, error: {}",
                                            tool.name(),
                                            args,
                                            e
                                        );
                                        results.lock().unwrap().push(ToolCallOutput {
                                            name: tool_call.name,
                                            args,
                                            result: e.to_string(),
                                        });
                                        return;
                                    },
                                };
                                results.lock().unwrap().push(ToolCallOutput {
                                    name: tool_call.name,
                                    args,
                                    result,
                                });
                            }
                        })
                        .await;
                } else {
                    for tool_call in all_tool_calls {
                        let tool = Arc::clone(
                            self.tools_impl
                                .get(&tool_call.name)
                                .ok_or(AgentError::ToolNotFound(tool_call.name.clone()))?
                                .deref(),
                        );
                        let args = tool_call.arguments.to_string();
                        // execute tool
                        let result_str = tool.call(args.clone()).await?;
                        // collect results
                        results.lock().unwrap().push(ToolCallOutput {
                            name: tool_call.name.clone(),
                            args,
                            result: result_str,
                        });
                    }
                }

                Ok(ChatResponse::ToolCalls(
                    Arc::clone(&results).lock().unwrap().clone(),
                ))
            },
        }
    }

    pub async fn prompt(&self, prompt: impl Into<String>) -> Result<String, AgentError> {
        let prompt = prompt.into();
        let start_time = std::time::Instant::now();

        let request = CompletionRequest {
            prompt: llm::completion::Message::user(prompt.clone()),
            system_prompt: self.system_prompt.clone(),
            chat_history: vec![],
            tools: vec![],
            temperature: Some(self.config.temperature),
            max_tokens: Some(self.config.max_tokens),
        };

        let response = self.model.completion(request).await.map_err(|e| {
            e
        })?;

        let choice = response.choice.first().ok_or(AgentError::NoChoiceFound)?;
        let result = match ToOwned::to_owned(choice) {
            llm::completion::AssistantContent::Text(text) => {
                Ok(text.text)
            },
            llm::completion::AssistantContent::ToolCall(_) => {
                unreachable!("We don't provide tools")
            },
        };

        result
    }

    pub fn tool(mut self, tool: impl ToolDyn + 'static) -> Self {
        let toolname = tool.name();
        // Create tool definition from the tool's parameters
        let definition = llm::request::ToolDefinition {
            name: toolname.clone(),
            description: tool.description(),
            parameters: tool.parameters(),
        };
        self.tools.push(definition);
        self.tools_impl.insert(toolname, Arc::new(tool));
        self
    }

    pub fn system_prompt(mut self, system_prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(system_prompt.into());
        self
    }

    pub fn get_system_prompt(&self) -> Option<&str> {
        self.system_prompt.as_deref()
    }

    /// Handle error in attempts
    async fn handle_error_in_attempts(&self, task: &str, error: AgentError, attempt: u32) {
        let err_msg = format!("Attempt {}, task: {}, failed: {}", attempt + 1, task, error);
        tracing::error!(err_msg);

        if self.config.autosave {
            let _ = self.save_task_state(task.to_owned()).await.map_err(|e| {
                tracing::error!(
                    "Failed to save agent<{}> task<{}>,  state: {}",
                    self.config.name,
                    task,
                    e
                )
            });
        }
    }
}

impl<M> Agent for SwarmsAgent<M>
where
    M: llm::Model + Clone + Send + Sync + 'static,
    M::RawCompletionResponse: Clone + Send + Sync,
{
    fn run(&self, task: String) -> BoxFuture<Result<String, AgentError>> {
        Box::pin(async move {
            let start_time = std::time::Instant::now();

            if self.config.verbose {
                log_task!(
                    info,
                    &self.config.name,
                    &self.config.id,
                    &task,
                    "Task initializing - Agent starting autonomous execution loop"
                );
            }

            self.short_memory.add(
                &task,
                &self.config.name,
                Role::User(self.config.user_name.clone()),
                &task,
            );

            if self.config.verbose {
                log_memory!(
                    debug,
                    &self.config.name,
                    &self.config.id,
                    "Save Task",
                    "Added task to short-term memory"
                );
            }

            // Plan
            if self.config.plan_enabled {
                if self.config.verbose {
                    log_agent!(
                        info,
                        &self.config.name,
                        &self.config.id,
                        "Planning phase initiated"
                    );
                }
                self.plan(task.clone()).await?;
            }

            // Save state
            if self.config.autosave {
                if self.config.verbose {
                    log_memory!(
                        debug,
                        &self.config.name,
                        &self.config.id,
                        "Autosave",
                        "Saving agent state to disk"
                    );
                }
                self.save_task_state(task.clone()).await?;
            }

            // Run agent loop
            let mut last_response_text = String::new();
            let mut task_complete = false;
            let mut was_prev_call_task_evaluator = false;

            if self.config.verbose {
                log_agent!(
                    info,
                    &self.config.name,
                    &self.config.id,
                    "Starting autonomous execution loop - Max loops: {}",
                    self.config.max_loops
                );
            }

            for loop_count in 0..self.config.max_loops {
                if task_complete {
                    if self.config.verbose {
                        log_agent!(
                            info,
                            &self.config.name,
                            &self.config.id,
                            "Task completed early at loop {} of {}",
                            loop_count,
                            self.config.max_loops
                        );
                    }
                    break;
                }

                if self.config.verbose {
                    log_agent!(
                        debug,
                        &self.config.name,
                        &self.config.id,
                        "Starting loop iteration {} of {}",
                        loop_count + 1,
                        self.config.max_loops
                    );
                }

                let current_prompt: String;

                if was_prev_call_task_evaluator {
                    current_prompt = format!(
                        "You previously called task_evaluator and indicated the task was not complete. The required next step or context provided was: '{}'. \
                        Focus ONLY on addressing this context. DO NOT call task_evaluator again in this turn. Proceed with the task based on the context.",
                        last_response_text // last_response is the context provided by task_evaluator
                    );

                    was_prev_call_task_evaluator = false;
                } else if loop_count > 0 {
                    current_prompt = format!(
                        "Now, you are in loop {} of {}, The dialogue will terminate upon reaching maximum iteration count. You must:
                         - Complete the user's task before termination
                         - Optimize loop efficiency
                         - Minimize resource consumption through minimal iterations

                        You should consider to use tools if they can help, but only if they are relevant to the task and are necessary for the task.
                        origin task:\n{}",
                        loop_count + 1,
                        self.config.max_loops,
                        task
                    )
                } else {
                    // first loop
                    // task is already in short_memory, short_memory will be passed to llm
                    // empty prompt should be ignored by LLM provider
                    current_prompt = "".to_owned();
                }

                let mut success = false;
                for attempt in 0..self.config.retry_attempts {
                    if success {
                        break;
                    }

                    // Generate response using LLM
                    let history = self.short_memory.0.get(&task).unwrap(); // Safety: task is in short_memory
                    let current_chat_response =
                        match self.chat(&current_prompt, history.deref()).await {
                            Ok(response) => response,
                            Err(e) => {
                                self.handle_error_in_attempts(&task, e, attempt).await;
                                continue;
                            },
                        };
                    drop(history);

                    // handle ChatResponse
                    let mut assistant_memory_content = String::new();
                    let mut is_task_evaluator_called = false;
                    match current_chat_response {
                        ChatResponse::Text(text) => {
                            last_response_text = text.clone();
                            assistant_memory_content = text;
                        },
                        ChatResponse::ToolCalls(tool_calls) => {
                            let mut formatted_tool_results = String::new();
                            for tool_call in tool_calls {
                                let formatted = format!(
                                    "[Tool name]: {}\n[Tool args]: {}\n[Tool result]: {}\n\n",
                                    tool_call.name, tool_call.args, tool_call.result
                                );
                                formatted_tool_results.push_str(&formatted);
                                if tool_call.name == ToolDyn::name(&TaskEvaluator) {
                                    is_task_evaluator_called = true;
                                    match serde_json::from_str::<TaskStatus>(&tool_call.result) {
                                        Ok(task_status) => {
                                            tracing::info!(
                                                "Task evaluator tool called, task status: {:#?}",
                                                task_status,
                                            );

                                            match task_status {
                                                TaskStatus::Complete => {
                                                    task_complete = true;
                                                    assistant_memory_content = formatted;
                                                },
                                                TaskStatus::Incomplete { context } => {
                                                    task_complete = false;
                                                    last_response_text = context;
                                                    assistant_memory_content = formatted;
                                                },
                                            }
                                        },
                                        Err(e) => {
                                            tracing::error!(
                                                "Failed to parse task status from task_evaluator: {}. Raw result: {}",
                                                e,
                                                tool_call.result
                                            );

                                            task_complete = false;

                                            last_response_text = format!(
                                                "Error parsing task_evaluator result. Raw output: {}",
                                                tool_call.result
                                            );
                                            assistant_memory_content = formatted;
                                        },
                                    }
                                } else {
                                    // Handle other tool calls if necessary, for now just format them
                                    if formatted_tool_results.len() == formatted.len() {
                                        last_response_text = formatted_tool_results.clone();
                                    } else {
                                        last_response_text.push_str(&formatted);
                                    }
                                }
                            }
                            // If multiple tools were called, or if task_evaluator wasn't the only one,
                            // ensure assistant_memory_content reflects all calls.
                            if assistant_memory_content.is_empty() || !is_task_evaluator_called {
                                assistant_memory_content = formatted_tool_results.clone();
                                if !is_task_evaluator_called {
                                    last_response_text = formatted_tool_results;
                                }
                            }
                        },
                    }

                    // Update the flag for the *next* iteration based on *this* iteration's call
                    was_prev_call_task_evaluator = is_task_evaluator_called && !task_complete;

                    self.short_memory.add(
                        &task,
                        &self.config.name,
                        Role::Assistant(self.config.name.to_owned()),
                        assistant_memory_content.clone(),
                    );

                    // Render output with markdown formatting if enabled
                    if self.markdown {
                        self.auto_render_output(&assistant_memory_content);
                    }

                    success = true;
                }

                if !success {
                    break;
                }

                // Save state in each loop
                if self.config.autosave {
                    self.save_task_state(task.clone()).await?;
                }

                if self.is_response_complete(last_response_text.clone()) {
                    if self.config.verbose {
                        log_agent!(
                            info,
                            &self.config.name,
                            &self.config.id,
                            "Response marked as complete by completion checker"
                        );
                    }
                    break;
                }
            }

            // Save state
            if self.config.autosave {
                if self.config.verbose {
                    log_memory!(
                        debug,
                        &self.config.name,
                        &self.config.id,
                        "Final Autosave",
                        "Saving final agent state after task completion"
                    );
                }
                self.save_task_state(task.clone()).await?;
            }

            let total_duration = start_time.elapsed().as_millis() as u64;
            if self.config.verbose {
                log_perf!(info, "Agent", "total_execution_time", total_duration, "ms");

                log_task!(
                    info,
                    &self.config.name,
                    &self.config.id,
                    &task,
                    "Task execution completed successfully in {}ms",
                    total_duration
                );
            }

            // TODO: Handle artifacts

            // TODO: More flexible output types, e.g. JSON, CSV, etc.
            Ok(self
                .short_memory
                .0
                .get(&task)
                .expect("Task should exist in short memory")
                .to_string())
        })
    }

    fn run_multiple_tasks(
        &mut self,
        tasks: Vec<String>,
    ) -> BoxFuture<Result<Vec<String>, AgentError>> {
        let agent_name = self.name();
        let mut results = Vec::with_capacity(tasks.len());

        Box::pin(async move {
            let agent_arc = Arc::new(self);
            let (tx, mut rx) = mpsc::channel(100);
            stream::iter(tasks)
                .for_each_concurrent(None, |task| {
                    let tx = tx.clone();
                    let agent = Arc::clone(&agent_arc);
                    async move {
                        let result = agent.run(task.clone()).await;
                        let _ = tx.send((task, result)).await; // Safety: we know rx is not dropped
                    }
                })
                .await;
            drop(tx);

            while let Some((task, result)) = rx.recv().await {
                match result {
                    Ok(result) => {
                        results.push(result);
                    },
                    Err(e) => {
                        tracing::error!("| Agent: {} | Task: {} | Error: {}", agent_name, task, e);
                    },
                }
            }

            Ok(results)
        })
    }

    fn plan(&self, task: String) -> BoxFuture<Result<(), AgentError>> {
        Box::pin(async move {
            if let Some(planning_prompt) = &self.config.planning_prompt {
                let planning_prompt = format!("{} {}", planning_prompt, task);
                let plan = self.prompt(planning_prompt).await?;
                tracing::debug!("Plan: {}", plan);
                // Add plan to memory
                self.short_memory.add(
                    task,
                    self.config.name.clone(),
                    Role::Assistant(self.config.name.clone()),
                    plan,
                );
            };
            Ok(())
        })
    }

    fn query_long_term_memory(&self, _task: String) -> BoxFuture<Result<(), AgentError>> {
        unimplemented!("query_long_term_memory not implemented")
    }

    fn save_task_state(&self, task: String) -> BoxFuture<Result<(), AgentError>> {
        let mut hasher = XxHash3_64::default();
        task.hash(&mut hasher);
        let task_hash = hasher.finish();
        let task_hash = format!("{:x}", task_hash & 0xFFFFFFFF); // lower 32 bits of the hash

        Box::pin(async move {
            let save_state_dir = self.config.save_state_dir.clone();
            if let Some(save_state_dir) = save_state_dir {
                let save_state_dir = Path::new(&save_state_dir);
                if !save_state_dir.exists() {
                    tokio::fs::create_dir_all(save_state_dir).await?;
                }

                let path = save_state_dir
                    .join(format!("{}_{}", self.name(), task_hash))
                    .with_extension("json");

                let json = serde_json::to_string_pretty(&self.short_memory.0.get(&task).unwrap())?; // TODO: Safety?
                persistence::save_to_file(&json, path).await?;
            }
            Ok(())
        })
    }

    fn is_response_complete(&self, response: String) -> bool {
        self.config
            .stop_words
            .iter()
            .any(|word| response.contains(word))
    }

    fn id(&self) -> String {
        self.config.id.clone()
    }

    fn name(&self) -> String {
        self.config.name.clone()
    }

    fn description(&self) -> String {
        self.config.description.clone().unwrap_or_default()
    }

    fn md(&mut self, enabled: bool) {
        self.markdown = enabled;
    }

    fn clone_box(&self) -> Box<dyn Agent> {
        Box::new(self.clone())
    }
}

/// Represents the response from a chat interaction with the agent.
/// 
/// The agent can respond in two ways: with plain text or by executing tools.
/// This enum distinguishes between these response types and provides the
/// appropriate data for each case.
/// 
/// # Examples
/// 
/// ```rust,no_run
/// use swarms_rs::agent::{ChatResponse, SwarmsAgentBuilder};
/// use swarms_rs::llm::provider::openai::OpenAIProvider;
/// 
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let model = OpenAIProvider::new("api-key")?;
/// let agent = SwarmsAgentBuilder::new_with_model(model).build();
/// 
/// let response = agent.chat("Hello!", vec![]).await?;
/// 
/// match response {
///     ChatResponse::Text(text) => {
///         println!("Agent responded with text: {}", text);
///     }
///     ChatResponse::ToolCalls(tool_outputs) => {
///         println!("Agent executed {} tools:", tool_outputs.len());
///         for output in tool_outputs {
///             println!("- Tool '{}' returned: {}", output.name, output.result);
///         }
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub enum ChatResponse {
    /// Plain text response from the LLM.
    /// 
    /// This variant contains the raw text response when the agent doesn't
    /// need to use any tools to complete the request.
    Text(String),
    /// Results from executing one or more tool calls requested by the LLM.
    /// 
    /// This variant contains the outputs from all tools that were called
    /// during the chat interaction. The agent may call multiple tools
    /// concurrently or sequentially based on the task requirements.
    ToolCalls(Vec<ToolCallOutput>),
}

/// Contains the complete information about a single tool execution.
/// 
/// When an agent executes a tool, this structure captures all the relevant
/// information about the call, including the tool name, arguments passed,
/// and the result returned. This is useful for debugging, logging, and
/// understanding the agent's decision-making process.
/// 
/// # Examples
/// 
/// ```rust,no_run
/// use swarms_rs::agent::ToolCallOutput;
/// 
/// let tool_output = ToolCallOutput {
///     name: "calculator".to_string(),
///     args: r#"{"operation": "add", "a": 5, "b": 3}"#.to_string(),
///     result: "8".to_string(),
/// };
/// 
/// println!("Tool {} with args {} returned: {}", 
///          tool_output.name, tool_output.args, tool_output.result);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallOutput {
    /// The name of the tool that was executed.
    /// 
    /// This corresponds to the tool's identifier as registered with the agent.
    pub name: String,
    /// The arguments passed to the tool as a JSON string.
    /// 
    /// The arguments are serialized as JSON to provide a consistent format
    /// regardless of the tool's specific parameter structure.
    pub args: String,
    /// The result returned by the tool's execution as a string.
    /// 
    /// All tool results are converted to strings for consistent handling,
    /// even if the tool internally works with other data types.
    pub result: String,
}

#[tool(
    description = r#"
    **Important**
    If previous message is a `task_evaluator` call, then you shouldn't call this tool.

    **Task Evaluator**
    Finalize or request refinement for the current task.
    
    Call this when:
    - All user requirements are fully satisfied (set status to "Complete")
    - Avoids unnecessary iterations, redundancy, or waste.
    - Additional input/clarification is needed (set status to "Incomplete" with context)
    
    When status is "Complete", the context is ignored, because the dialogue will terminate.
    When status is "Incomplete", your context becomes the system's next prompt, enabling iterative task refinement.
    Provide clear, actionable contexts to guide the next steps, the context should be used to guide yourself to complete the task.
"#,
    arg(status, description = "Task status: either 'Complete' or 'Incomplete'"),
    arg(
        context,
        description = "Context for incomplete tasks - guidance for next steps"
    )
)]
fn task_evaluator(
    status: String,
    context: Option<String>,
) -> Result<TaskStatus, TaskEvaluatorError> {
    match status.as_str() {
        "Complete" => Ok(TaskStatus::Complete),
        "Incomplete" => {
            let context = context.unwrap_or_else(|| "Task needs further work".to_string());
            Ok(TaskStatus::Incomplete { context })
        },
        _ => Ok(TaskStatus::Incomplete {
            context: format!("Invalid status '{}', treating as incomplete", status),
        }),
    }
}

/// Represents the completion status of a task being executed by the agent.
///
/// This enum is used by the built-in task evaluator tool to communicate
/// whether a task has been completed or needs further work. When a task
/// is incomplete, the context provides guidance for the next steps.
///
/// # Task Evaluation Flow
///
/// 1. The agent calls the `task_evaluator` tool during task execution
/// 2. The tool returns a `TaskStatus` indicating completion status
/// 3. If `Complete`, the task execution loop terminates
/// 4. If `Incomplete`, the context becomes the prompt for the next iteration
///
/// # Examples
///
/// ```rust,no_run
/// use swarms_rs::agent::TaskStatus;
///
/// // Task is complete
/// let complete = TaskStatus::Complete;
///
/// // Task needs more work with specific guidance
/// let incomplete = TaskStatus::Incomplete {
///     context: "Please provide more details about the analysis methodology".to_string()
/// };
/// ```
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub enum TaskStatus {
    /// Indicates that the task has been completed successfully.
    ///
    /// When this status is returned, the agent will terminate its execution
    /// loop and return the final result. No further iterations will be performed.
    Complete,

    /// Indicates that the task is not yet complete and requires additional work.
    ///
    /// The `context` field provides specific guidance for what needs to be done
    /// next, which becomes the system prompt for the subsequent iteration.
    Incomplete {
        /// Guidance for the next steps in task completion.
        ///
        /// This context should be:
        /// - **Specific**: Clear description of what's missing or needed
        /// - **Actionable**: Concrete steps the agent can take
        /// - **Focused**: Targeted guidance rather than general instructions
        ///
        /// Examples of good context:
        /// - "Add error handling for the database connection"
        /// - "Include a summary of the key findings"
        /// - "Verify the calculations in the financial analysis"
        context: String,
    },
}

/// Error type for the task evaluator tool.
///
/// Currently, the task evaluator tool is designed to handle all input gracefully
/// and doesn't return errors, but this type is reserved for future error handling
/// scenarios.
#[derive(Debug, Error)]
pub enum TaskEvaluatorError {}
