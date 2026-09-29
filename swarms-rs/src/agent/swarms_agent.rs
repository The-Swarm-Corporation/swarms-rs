//! # Swarms Agent Implementation
//!
//! This module provides the core `SwarmsAgent` implementation - an autonomous AI agent
//! that can execute tasks using Large Language Models (LLMs) with tool integration,
//! memory management, and configurable execution patterns.
//!
//! ## Key Features
//!
//! - **LLM Integration**: Works with any LLM provider that implements the `llm::Model` trait
//! - **Tool System**: Supports both native Rust tools and MCP (Model Context Protocol) servers
//! - **Memory Management**: Short-term memory for conversation history and context
//! - **Task Planning**: Optional planning phase with configurable prompts
//! - **State Persistence**: Automatic saving and loading of agent state
//! - **Concurrent Execution**: Support for concurrent tool calls and multiple tasks
//! - **Configurable Logging**: Optional verbose logging for debugging and monitoring
//!
//! ## Basic Usage
//!
//! ```rust,no_run
//! use swarms_rs::agent::SwarmsAgentBuilder;
//! use swarms_rs::llm::provider::openai::OpenAI;
//! use swarms_rs::structs::agent::Agent;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create an LLM provider
//! let model = OpenAI::from_env();
//!
//! // Build an agent
//! let agent = SwarmsAgentBuilder::new_with_model(model)
//!     .agent_name("TaskAgent")
//!     .system_prompt("You are a helpful AI assistant.")
//!     .max_loops(3)
//!     .temperature(0.7)
//!     .verbose(true) // Enable logging
//!     .build();
//!
//! // Execute a task
//! let result = agent.run("Analyze the current market trends".to_string()).await?;
//! println!("Result: {}", result);
//! # Ok(())
//! # }
//! ```
//!
//! ## Advanced Configuration
//!
//! ```rust,no_run
//! use swarms_rs::agent::SwarmsAgentBuilder;
//! use swarms_rs::llm::provider::openai::OpenAI;
//! use swarms_rs::structs::agent::Agent;
//! use swarms_rs::structs::tool::Tool;
//!
//! # async fn advanced_example() -> Result<(), Box<dyn std::error::Error>> {
//! let model = OpenAI::from_env();
//!
//! let agent = SwarmsAgentBuilder::new_with_model(model)
//!     .agent_name("AdvancedAgent")
//!     .system_prompt("You are an advanced AI agent with specialized tools.")
//!     .max_loops(5)
//!     .temperature(0.3)
//!     .enable_plan(Some("Create a step-by-step plan for: ".to_string()))
//!     .enable_autosave()
//!     .save_state_dir("./agent_states")
//!     .retry_attempts(2)
//!     .add_stop_word("TASK_COMPLETE")
//!     .verbose(false) // Disable logging for production
//!     .build();
//!
//! let result = agent.run("Complex analytical task".to_string()).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Tool Integration
//!
//! The agent supports multiple ways to add tools:
//!
//! - **Native Rust Tools**: Implement the `Tool` trait
//! - **MCP Servers**: Connect to external MCP servers via SSE or stdio
//! - **Built-in Tools**: Task evaluator tool for autonomous task completion
//!
//! ## Memory and Persistence
//!
//! - **Short-term Memory**: Maintains conversation history during task execution
//! - **State Persistence**: Optional automatic saving of agent state to disk
//! - **Task Hashing**: Efficient state management using content-based hashing

use std::{
    collections::HashMap,
    ffi::OsStr,
    hash::{Hash, Hasher},
    ops::Deref,
    path::Path,
    sync::Arc,
};

use twox_hash::XxHash64;

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
use tabled::{
    builder::Builder,
    settings::{Alignment, Modify, Style, object::Rows},
};
use thiserror::Error;
use tokio::{process::Command, sync::Mutex};

use crate::{
    self as swarms_rs,
    llm::{
        self,
        request::{CompletionRequest, ToolDefinition},
    },
    log_agent, log_error_ctx, log_llm, log_memory, log_perf, log_task,
    structs::{
        conversation::{AgentConversation, AgentShortMemory, Role},
        persistence,
        tool::{MCPTool, Tool, ToolDyn},
    },
};

use crate::structs::agent::{Agent, AgentConfig, AgentError};

use super::delegation::{HandoffArgs, HandoffTool, SubAgentTool, tool_name_for};

// Kept at its original path; it lives in `structs::tool` so conversations can store it.
pub use crate::structs::tool::ToolCallOutput;

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
/// use swarms_rs::llm::provider::openai::OpenAI;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let model = OpenAI::from_env();
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
    /// Handoff targets, keyed by the name of the tool that transfers to them
    handoffs: HashMap<String, Arc<dyn Agent>>,
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
    /// use swarms_rs::llm::provider::openai::OpenAI;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = OpenAI::from_env();
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
            handoffs: HashMap::new(),
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
    /// use swarms_rs::llm::provider::openai::OpenAI;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = OpenAI::from_env();
    /// let custom_config = AgentConfig::builder()
    ///     .agent_name("CustomAgent")
    ///     .max_loops(5)
    ///     .temperature(0.3)
    ///     .build();
    ///
    /// let agent = SwarmsAgentBuilder::new_with_model(model)
    ///     .config((*custom_config).clone())
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
    /// use swarms_rs::llm::provider::openai::OpenAI;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = OpenAI::from_env();
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
    /// or access external data. The agent can call these tools autonomously based on
    /// the task requirements.
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
    /// use swarms_rs::llm::provider::openai::OpenAI;
    /// use swarms_rs::structs::agent::Agent;
    /// use swarms_rs::structs::tool::Tool;
    /// use swarms_rs::llm::request::ToolDefinition;
    ///
    /// // Define a custom tool
    /// #[derive(Debug)]
    /// struct CalculatorTool;
    ///
    /// #[derive(Debug, serde::Deserialize)]
    /// struct CalculatorArgs {
    ///     expression: String,
    /// }
    ///
    /// impl Tool for CalculatorTool {
    ///     type Error = std::io::Error;
    ///     type Args = CalculatorArgs;
    ///     type Output = String;
    ///     const NAME: &'static str = "calculator";
    ///
    ///     fn definition(&self) -> ToolDefinition {
    ///         ToolDefinition {
    ///             name: "calculator".to_string(),
    ///             description: "Evaluate mathematical expressions".to_string(),
    ///             parameters: serde_json::json!({
    ///                 "type": "object",
    ///                 "properties": {
    ///                     "expression": {
    ///                         "type": "string",
    ///                         "description": "The mathematical expression to evaluate"
    ///                     }
    ///                 },
    ///                 "required": ["expression"]
    ///             }),
    ///         }
    ///     }
    ///
    ///     async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
    ///         // Simple calculator implementation
    ///         match args.expression.as_str() {
    ///             "2+2" => Ok("4".to_string()),
    ///             "10*5" => Ok("50".to_string()),
    ///             _ => Ok(format!("Result of {}: computed", args.expression)),
    ///         }
    ///     }
    /// }
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = OpenAI::from_env();
    ///
    /// let agent = SwarmsAgentBuilder::new_with_model(model)
    ///     .add_tool(CalculatorTool)
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_tool<T: Tool + 'static>(mut self, tool: T) -> Self {
        let definition = tool.definition();
        self.register_tool(definition, Arc::new(tool) as Arc<dyn ToolDyn>);
        self
    }

    /// Register a tool; a later tool with the same name replaces the earlier one, so the
    /// model is never offered duplicate names (which some providers reject).
    fn register_tool(&mut self, definition: ToolDefinition, tool: Arc<dyn ToolDyn>) {
        if let Some(existing) = self.tools.iter_mut().find(|t| t.name == definition.name) {
            tracing::warn!(
                "Tool '{}' registered more than once; keeping the last one",
                definition.name
            );
            *existing = definition.clone();
        } else {
            self.tools.push(definition.clone());
        }
        self.tools_impl.insert(definition.name, tool);
    }

    /// Adds a sub-agent the model can delegate subtasks to.
    ///
    /// The model sees a `delegate_to_<name>` tool that takes a `task`; calling it runs the
    /// sub-agent and returns its answer as the tool result, and this agent carries on. Use
    /// [`Self::add_handoff`] instead to pass control to another agent for good.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use swarms_rs::llm::provider::openrouter::OpenRouter;
    ///
    /// let client = OpenRouter::from_env_with_model("anthropic/claude-opus-5.5");
    /// let researcher = client
    ///     .agent_builder()
    ///     .agent_name("Researcher")
    ///     .description("Finds facts and sources")
    ///     .build();
    /// let coordinator = client
    ///     .agent_builder()
    ///     .agent_name("Coordinator")
    ///     .add_sub_agent(researcher)
    ///     .build();
    /// ```
    pub fn add_sub_agent<A: Agent + 'static>(self, agent: A) -> Self {
        self.add_sub_agent_boxed(Box::new(agent))
    }

    /// [`Self::add_sub_agent`] for an agent that is already boxed, such as one from a workflow.
    pub fn add_sub_agent_boxed(mut self, agent: Box<dyn Agent>) -> Self {
        let agent: Arc<dyn Agent> = Arc::from(agent);
        let tool_name = self.unused_tool_name("delegate_to_", &agent.name());
        let tool = SubAgentTool::new(tool_name, agent);
        let definition = tool.definition();
        self.register_tool(definition, Arc::new(tool));
        self
    }

    /// Adds an agent this agent can hand the task off to.
    ///
    /// The model sees a `transfer_to_<name>` tool that takes a `task` and optional `context`.
    /// When it calls it, the target agent gets the task, the context and this agent's
    /// conversation so far; this agent then stops, and its output ends with the target's
    /// answer. Only one handoff runs per turn.
    pub fn add_handoff<A: Agent + 'static>(self, agent: A) -> Self {
        self.add_handoff_boxed(Box::new(agent))
    }

    /// [`Self::add_handoff`] for an agent that is already boxed, such as one from a workflow.
    pub fn add_handoff_boxed(mut self, agent: Box<dyn Agent>) -> Self {
        let agent: Arc<dyn Agent> = Arc::from(agent);
        let tool_name = self.unused_tool_name("transfer_to_", &agent.name());
        let tool = HandoffTool::new(tool_name.clone(), agent.as_ref());
        let definition = tool.definition();
        self.register_tool(definition, Arc::new(tool));
        self.handoffs.insert(tool_name, agent);
        self
    }

    /// Adds several handoff targets at once; see [`Self::add_handoff`].
    pub fn handoffs(self, agents: Vec<Box<dyn Agent>>) -> Self {
        agents
            .into_iter()
            .fold(self, |builder, agent| builder.add_handoff_boxed(agent))
    }

    fn unused_tool_name(&self, prefix: &str, agent_name: &str) -> String {
        tool_name_for(prefix, agent_name, |name| {
            self.tools.iter().any(|tool| tool.name == name)
        })
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
    /// use swarms_rs::llm::provider::openai::OpenAI;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = OpenAI::from_env();
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
    /// use swarms_rs::llm::provider::openai::OpenAI;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = OpenAI::from_env();
    ///
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
            log::info!("🏗️  Building SwarmsAgent: {}", self.config.name);
        }

        if self.config.task_evaluator_tool_enabled {
            if self.config.verbose {
                log::debug!(
                    "📋 Adding task evaluator tool for agent: {}",
                    self.config.name
                );
            }
            let definition = ToolDyn::definition(&TaskEvaluator);
            self.tools.retain(|t| t.name != definition.name);
            self.tools.insert(0, definition);
            self.tools_impl.insert(
                ToolDyn::name(&TaskEvaluator),
                Arc::new(TaskEvaluator) as Arc<dyn ToolDyn>,
            );
        }

        // An agent handing off to itself would loop; the name is known only now.
        let self_handoffs: Vec<String> = self
            .handoffs
            .iter()
            .filter(|(_, target)| target.name() == self.config.name)
            .map(|(tool_name, _)| tool_name.clone())
            .collect();
        for tool_name in self_handoffs {
            tracing::warn!(
                "Agent '{}' can't hand off to itself; dropping tool '{}'",
                self.config.name,
                tool_name
            );
            self.handoffs.remove(&tool_name);
            self.tools.retain(|t| t.name != tool_name);
            self.tools_impl.remove(&tool_name);
        }

        let agent = SwarmsAgent {
            model: self.model,
            config: self.config.clone(),
            system_prompt: self.system_prompt,
            short_memory: AgentShortMemory::new(),
            tools: self.tools.clone(),
            tools_impl: self.tools_impl,
            handoffs: self.handoffs,
        };

        if agent.config.verbose && log::log_enabled!(log::Level::Info) {
            log::info!(
                "✅ SwarmsAgent built successfully: {} (ID: {}) with {} tools",
                agent.config.name,
                agent.config.id,
                self.tools.len()
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
        self.config.temperature = Some(temperature);
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
    /// use swarms_rs::llm::provider::openai::OpenAI;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = OpenAI::from_env();
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

    /// Enable or disable pretty printing with colored panels for this agent.
    ///
    /// When pretty printing is enabled, the agent will display outputs in colored
    /// panels similar to Python's Rich library, providing a more visually appealing
    /// and organized display of agent interactions, tool calls, and responses.
    ///
    /// # Arguments
    ///
    /// * `pretty_print_on` - `true` to enable pretty printing, `false` to use plain text
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use swarms_rs::agent::SwarmsAgentBuilder;
    /// use swarms_rs::llm::provider::openai::OpenAI;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = OpenAI::from_env();
    ///
    /// // Agent with pretty printing enabled
    /// let pretty_agent = SwarmsAgentBuilder::new_with_model(model.clone())
    ///     .pretty_print_on(true)
    ///     .verbose(true)
    ///     .build();
    ///
    /// // Agent with plain text output
    /// let plain_agent = SwarmsAgentBuilder::new_with_model(model)
    ///     .pretty_print_on(false)
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn pretty_print_on(mut self, pretty_print_on: bool) -> Self {
        self.config.pretty_print_on = pretty_print_on;
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
/// use swarms_rs::llm::provider::openai::OpenAI;
/// use swarms_rs::structs::agent::Agent;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let model = OpenAI::from_env();
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
/// use swarms_rs::llm::provider::openai::OpenAI;
/// use swarms_rs::structs::agent::Agent;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let model = OpenAI::from_env();
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
    /// Handoff targets, keyed by the name of the tool that transfers to them (not serialized)
    #[serde(skip)]
    handoffs: HashMap<String, Arc<dyn Agent>>,
}

impl<M> SwarmsAgent<M>
where
    M: llm::Model + Clone + Send + Sync + 'static,
    M::RawCompletionResponse: Clone + Send + Sync,
{
    /// Print agent thinking/output in a panel
    fn print_agent_output(&self, content: &str) {
        if self.config.pretty_print_on {
            let mut builder = Builder::default();
            builder.push_record(vec![format!("🤖 Agent: {} Output", self.config.name)]);
            builder.push_record(vec![content]);

            let mut table = builder.build();
            table.with(Style::rounded());
            table.with(Modify::new(Rows::first()).with(Alignment::center()));

            println!("{}", table);
        } else {
            println!("🤖 Agent: {}", content);
        }
    }

    /// Print tool execution in a panel
    fn print_tool_execution(&self, tool_name: &str, args: &str, result: &str) {
        if self.config.pretty_print_on {
            let mut builder = Builder::default();
            builder.push_record(vec![format!(
                "🔧 Agent: {} - Tool Execution",
                self.config.name
            )]);
            builder.push_record(vec![format!("📝 Tool: {}", tool_name)]);
            builder.push_record(vec![format!("📝 Arguments: {}", args)]);
            builder.push_record(vec![format!("✅ Result: {}", result)]);

            let mut table = builder.build();
            table.with(Style::rounded());
            table.with(Modify::new(Rows::first()).with(Alignment::center()));

            println!("{}", table);
        } else {
            println!("🔧 Tool {} executed with args: {}", tool_name, args);
            println!("✅ Result: {}", result);
        }
    }

    /// Print task completion in a panel
    fn print_task_complete(&self, task: &str, result: &str) {
        if self.config.pretty_print_on {
            let mut builder = Builder::default();
            builder.push_record(vec![format!(
                "🎯 Agent: {} - Task Completed",
                self.config.name
            )]);
            builder.push_record(vec![format!("📋 Task: {}", task)]);
            builder.push_record(vec![format!(
                "📋 Result: {}",
                result.chars().take(100).collect::<String>()
            )]);

            let mut table = builder.build();
            table.with(Style::rounded());
            table.with(Modify::new(Rows::first()).with(Alignment::center()));

            println!("{}", table);
        } else {
            println!("🎯 Task '{}' completed!", task);
            println!("📋 Result: {}", result);
        }
    }

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
    /// use swarms_rs::llm::provider::openai::OpenAI;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = OpenAI::from_env();
    /// let agent = SwarmsAgent::new(model, "You are a helpful assistant".to_string());
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
            handoffs: HashMap::new(),
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
    /// Returns a `ChatResponse` which is one of:
    /// - `ChatResponse::Text(String)` - A text response from the LLM
    /// - `ChatResponse::ToolCalls(Vec<ToolCallOutput>)` - Results from tool execution
    /// - `ChatResponse::TextWithToolCalls { text, tool_calls }` - Both, from the same reply
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use swarms_rs::agent::SwarmsAgentBuilder;
    /// use swarms_rs::llm::provider::openai::OpenAI;
    /// use swarms_rs::agent::ChatResponse;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = OpenAI::from_env();
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
    ///     ChatResponse::TextWithToolCalls { text, tool_calls } => {
    ///         println!("Agent: {}", text);
    ///         for call in tool_calls {
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
            temperature: self.config.temperature,
            max_tokens: Some(self.config.max_tokens),
        };

        let response = self.model.completion(request).await?;
        if response.choice.is_empty() {
            return Err(AgentError::NoChoiceFound);
        }

        // Models often write a sentence before calling a tool, so tool calls can follow
        // text in any position.
        let mut texts = Vec::new();
        let mut all_tool_calls = Vec::new();
        for choice in response.choice {
            match choice {
                llm::completion::AssistantContent::Text(text) => texts.push(text.text),
                llm::completion::AssistantContent::ToolCall(tool_call) => {
                    all_tool_calls.push(tool_call.function)
                },
            }
        }
        if all_tool_calls.is_empty() {
            return Ok(ChatResponse::Text(texts.join("\n")));
        }
        {
            {
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
                                            results.lock().await.push(ToolCallOutput {
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
                                        results.lock().await.push(ToolCallOutput {
                                            name: tool_call.name,
                                            args,
                                            result: e.to_string(),
                                        });
                                        return;
                                    },
                                };
                                results.lock().await.push(ToolCallOutput {
                                    name: tool_call.name,
                                    args,
                                    result,
                                });
                            }
                        })
                        .await;
                } else {
                    for tool_call in all_tool_calls {
                        let args = tool_call.arguments.to_string();
                        // Like the concurrent path, report failures to the model as the tool's
                        // result instead of aborting (and later re-running) the whole batch.
                        let result = match self.tools_impl.get(&tool_call.name) {
                            Some(tool) => {
                                let tool = Arc::clone(tool.deref());
                                match tool.call(args.clone()).await {
                                    Ok(result) => result,
                                    Err(e) => {
                                        tracing::error!(
                                            "Failed to call tool<{}>, args: {}, error: {}",
                                            tool_call.name,
                                            args,
                                            e
                                        );
                                        e.to_string()
                                    },
                                }
                            },
                            None => {
                                tracing::error!("Tool not found: {}", tool_call.name);
                                "Tool not found".to_owned()
                            },
                        };
                        results.lock().await.push(ToolCallOutput {
                            name: tool_call.name,
                            args,
                            result,
                        });
                    }
                }

                let tool_calls = Arc::clone(&results).lock().await.clone();
                // Keep any text the model wrote next to its tool calls (often the answer
                // itself, sent alongside a task_evaluator call).
                let text = texts.join("\n");
                if text.trim().is_empty() {
                    Ok(ChatResponse::ToolCalls(tool_calls))
                } else {
                    Ok(ChatResponse::TextWithToolCalls { text, tool_calls })
                }
            }
        }
    }

    pub async fn prompt(&self, prompt: impl Into<String>) -> Result<String, AgentError> {
        let prompt = prompt.into();
        let start_time = std::time::Instant::now();

        if self.config.verbose {
            log_llm!(
                info,
                &self.config.name,
                &self.config.id,
                "Prompt Request",
                "Sending prompt to LLM: '{}'",
                prompt.chars().take(100).collect::<String>()
            );
        }

        let request = CompletionRequest {
            prompt: llm::completion::Message::user(prompt.clone()),
            system_prompt: self.system_prompt.clone(),
            chat_history: vec![],
            tools: vec![],
            temperature: self.config.temperature,
            max_tokens: Some(self.config.max_tokens),
        };

        let response = self.model.completion(request).await.inspect_err(|e| {
            if self.config.verbose {
                log_error_ctx!(&self.config.name, &self.config.id, e, "LLM completion");
            }
        })?;

        // Replies can be split across several text blocks; no tools are offered here, so
        // any tool call block is ignored.
        let texts: Vec<String> = response
            .choice
            .into_iter()
            .filter_map(|choice| match choice {
                llm::completion::AssistantContent::Text(text) => Some(text.text),
                llm::completion::AssistantContent::ToolCall(_) => None,
            })
            .collect();
        if texts.is_empty() {
            return Err(AgentError::NoChoiceFound);
        }
        let text = texts.join("\n");

        {
            {
                let duration = start_time.elapsed().as_millis() as u64;
                if self.config.verbose {
                    log_perf!(info, "LLM", "completion_time", duration, "ms");
                    log_llm!(
                        debug,
                        &self.config.name,
                        &self.config.id,
                        "Prompt Response",
                        "Received response ({}ms): '{}'",
                        duration,
                        text.chars().take(100).collect::<String>()
                    );
                }
                Ok(text)
            }
        }
    }

    pub fn tool(mut self, tool: impl ToolDyn + 'static) -> Self {
        let definition = tool.definition();
        self.tools.retain(|t| t.name != definition.name);
        self.tools.push(definition);
        self.tools_impl.insert(tool.name(), Arc::new(tool));
        self
    }

    pub fn system_prompt(mut self, system_prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(system_prompt.into());
        self
    }

    pub fn get_system_prompt(&self) -> Option<&str> {
        self.system_prompt.as_deref()
    }

    /// A copy of the conversation this agent kept for `task`, if it has run it.
    ///
    /// Tool-call turns keep their typed results, so a workflow can read them back without
    /// parsing the text output:
    ///
    /// ```rust,no_run
    /// # use swarms_rs::{agent::SwarmsAgent, llm::provider::openai::OpenAI};
    /// # fn example(agent: &SwarmsAgent<OpenAI>) {
    /// if let Some(conversation) = agent.conversation("What is 2 + 3?") {
    ///     for output in conversation.tool_outputs() {
    ///         if let Ok(sum) = output.result_as::<i64>() {
    ///             println!("{} returned {sum}", output.name);
    ///         }
    ///     }
    /// }
    /// # }
    /// ```
    pub fn conversation(&self, task: &str) -> Option<AgentConversation> {
        self.short_memory
            .0
            .get(task)
            .map(|conversation| conversation.clone())
    }

    /// Handle error in attempts
    /// Run the first handoff among `tool_calls`, if any, and return the target agent's answer.
    ///
    /// The target gets the task and context the model wrote, any text it wrote alongside the
    /// call (`reply_text`), and this agent's conversation so far. The handoff call's result
    /// is replaced with the target's answer (or the error, in which case this agent keeps
    /// control), and any further handoff calls in the same turn are marked as not run.
    async fn run_handoff(
        &self,
        task: &str,
        reply_text: Option<&str>,
        tool_calls: &mut [ToolCallOutput],
    ) -> Option<String> {
        let first = tool_calls
            .iter()
            .position(|call| self.handoffs.contains_key(&call.name))?;
        let chosen = tool_calls[first].name.clone();
        for call in tool_calls.iter_mut().skip(first + 1) {
            if self.handoffs.contains_key(&call.name) {
                call.result = format!(
                    "Not run: only one handoff runs per turn, and this turn handed off via '{chosen}'"
                );
            }
        }

        let call = &mut tool_calls[first];
        let args = match serde_json::from_str::<HandoffArgs>(&call.args) {
            Ok(args) => args,
            Err(e) => {
                call.result = format!("Handoff failed: invalid arguments: {e}");
                return None;
            },
        };
        let target = Arc::clone(&self.handoffs[&chosen]);
        // Copy the conversation out so no DashMap guard is held across the target's run.
        let history = self
            .short_memory
            .0
            .get(task)
            .map(|conversation| conversation.to_string())
            .unwrap_or_default();

        let mut prompt = format!(
            "You are taking over a task from the agent '{}'.\n\nTask: {}\n",
            self.config.name, args.task
        );
        if let Some(context) = args.context.filter(|c| !c.trim().is_empty()) {
            prompt.push_str(&format!(
                "\nContext from {}: {}\n",
                self.config.name, context
            ));
        }
        prompt.push_str(&format!("\nConversation so far:\n{history}"));
        if let Some(text) = reply_text.filter(|t| !t.trim().is_empty()) {
            prompt.push_str(&format!(
                "\n{}'s last message: {}\n",
                self.config.name, text
            ));
        }

        match target.run(prompt).await {
            Ok(output) => {
                call.result = output.clone();
                Some(output)
            },
            Err(e) => {
                tracing::error!(
                    "Agent<{}> handoff to '{}' failed: {}",
                    self.config.name,
                    target.name(),
                    e
                );
                call.result = format!("Handoff to '{}' failed: {e}", target.name());
                None
            },
        }
    }

    async fn handle_error_in_attempts(&self, task: &str, error: &AgentError, attempt: u32) {
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
    fn run(&self, task: String) -> BoxFuture<'_, Result<String, AgentError>> {
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

            // Query long term memory
            // if self.long_term_memory.is_some() {
            //     self.query_long_term_memory(task.clone()).await?;
            // }

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
                } else if self.config.plan_enabled && self.config.planning_prompt.is_some() {
                    // The plan is the last (assistant) message in memory. Ending the request on
                    // an assistant turn makes it a prefill, which current models reject.
                    current_prompt = "Carry out the task above, following your plan.".to_owned();
                } else {
                    // first loop
                    // task is already in short_memory, short_memory will be passed to llm
                    // empty prompt should be ignored by LLM provider
                    current_prompt = "".to_owned();
                }

                let mut success = false;
                let mut last_error = None;
                // let task_prompt = self.short_memory.0.get(&task).unwrap().to_string(); // Safety: task is in short_memory
                for attempt in 0..self.config.retry_attempts.max(1) {
                    if success {
                        break;
                    }
                    if attempt > 0 {
                        // Back off before retrying so rate limits and transient errors can clear.
                        tokio::time::sleep(std::time::Duration::from_millis(500 << attempt.min(5)))
                            .await;
                    }

                    // if self.long_term_memory.is_some() && self.config.rag_every_loop {
                    //     // FIXME: if RAG success, but then LLM fails, then RAG is not removed and maybe causes issues
                    //     if let Err(e) = self.query_long_term_memory(task_prompt.clone()).await {
                    //         self.handle_error_in_attempts(&task, e, attempt).await;
                    //         continue;
                    //     };
                    // }

                    // Generate response using LLM. Copy the history out so no DashMap guard is
                    // held across the LLM and tool calls (other runs write to the same shard).
                    let history: Vec<llm::completion::Message> =
                        (&*self.short_memory.0.get(&task).unwrap()).into(); // Safety: task is in short_memory
                    let current_chat_response = match self.chat(&current_prompt, history).await {
                        Ok(response) => response,
                        Err(e) => {
                            self.handle_error_in_attempts(&task, &e, attempt).await;
                            last_error = Some(e);
                            continue;
                        },
                    };

                    // handle ChatResponse
                    let (reply_text, tool_calls) = match current_chat_response {
                        ChatResponse::Text(text) => (Some(text), None),
                        ChatResponse::ToolCalls(tool_calls) => (None, Some(tool_calls)),
                        ChatResponse::TextWithToolCalls { text, tool_calls } => {
                            (Some(text), Some(tool_calls))
                        },
                    };
                    if let Some(text) = &reply_text {
                        // Pretty print agent output
                        self.print_agent_output(text);
                    }
                    let mut assistant_tool_calls = None;
                    let mut is_task_evaluator_called = false;
                    match tool_calls {
                        None => {
                            last_response_text = reply_text.clone().unwrap_or_default();
                        },
                        Some(mut tool_calls) => {
                            // At most one handoff runs per turn; its call's result becomes the
                            // target agent's answer.
                            let handoff_output = self
                                .run_handoff(&task, reply_text.as_deref(), &mut tool_calls)
                                .await;

                            let mut formatted_tool_results = String::new();
                            for tool_call in &tool_calls {
                                // Pretty print tool execution
                                self.print_tool_execution(
                                    &tool_call.name,
                                    &tool_call.args,
                                    &tool_call.result,
                                );

                                let formatted = tool_call.to_string();
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
                                                },
                                                TaskStatus::Incomplete { context } => {
                                                    task_complete = false;
                                                    // If not complete, store the context for the next loop's prompt
                                                    last_response_text = context;
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
                                        },
                                    }
                                } else {
                                    // Handle other tool calls if necessary, for now just format them
                                    // If this is the *only* response part, update last_response_text
                                    if formatted_tool_results.len() == formatted.len() {
                                        // Check if it's the first/only tool result string being built
                                        last_response_text = formatted_tool_results.clone();
                                    } else {
                                        // Append to existing text/tool results for the final response string
                                        last_response_text.push_str(&formatted);
                                    }
                                }
                            }
                            let evaluator_incomplete = is_task_evaluator_called && !task_complete;
                            match &reply_text {
                                // Text written next to the calls (often the answer itself)
                                // counts for stop words too.
                                Some(text) if !evaluator_incomplete => {
                                    last_response_text =
                                        format!("{text}\n\n{formatted_tool_results}");
                                },
                                // Update last_response_text if it wasn't set by task_evaluator
                                _ if !is_task_evaluator_called => {
                                    last_response_text = formatted_tool_results;
                                },
                                // An incomplete task_evaluator keeps its context for the next prompt
                                _ => {},
                            }
                            if let Some(output) = handoff_output {
                                // The target agent took over and answered; this agent stops.
                                task_complete = true;
                                last_response_text = output;
                            }
                            // Memory keeps every tool's typed result, including those called
                            // alongside task_evaluator.
                            assistant_tool_calls = Some(tool_calls);
                        },
                    }

                    // Update the flag for the *next* iteration based on *this* iteration's call
                    was_prev_call_task_evaluator = is_task_evaluator_called && !task_complete;

                    let role = Role::Assistant(self.config.name.to_owned());
                    match assistant_tool_calls {
                        // The model's text comes first, then the tool lines, in one turn.
                        Some(outputs) => self.short_memory.add_tool_calls(
                            &task,
                            &self.config.name,
                            role,
                            reply_text,
                            outputs,
                        ),
                        None => self.short_memory.add(
                            &task,
                            &self.config.name,
                            role,
                            reply_text.unwrap_or_default(),
                        ),
                    }

                    success = true;
                }

                if !success {
                    let error = last_error.unwrap_or(AgentError::NoChoiceFound);
                    // Nothing was produced at all: report the failure instead of returning the
                    // bare task as if it were an answer.
                    if loop_count == 0 {
                        return Err(error);
                    }
                    // Later loops refine earlier output; keep what we have.
                    tracing::warn!(
                        "Agent<{}> stopping at loop {} after all retries failed: {}",
                        self.config.name,
                        loop_count + 1,
                        error
                    );
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

                // TODO: Loop interval, maybe add a sleep here
            }

            // TODO: Apply the cleaning function to the responses
            // clean and add to short memory. role: Assistant(Output Cleaner)

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
            let final_result = self
                .short_memory
                .0
                .get(&task)
                .expect("Task should exist in short memory")
                .to_string();

            // Pretty print the final result
            self.print_task_complete(&task, &final_result);

            Ok(final_result)
        })
    }

    fn run_multiple_tasks(
        &mut self,
        tasks: Vec<String>,
    ) -> BoxFuture<'_, Result<Vec<String>, AgentError>> {
        let agent_name = self.name();
        let mut results = Vec::with_capacity(tasks.len());

        Box::pin(async move {
            let agent_arc = Arc::new(self);
            let concurrency = tasks.len().max(1);
            // `buffered` runs every task concurrently and yields results in task order.
            let outcomes: Vec<(String, Result<String, AgentError>)> = stream::iter(tasks)
                .map(|task| {
                    let agent = Arc::clone(&agent_arc);
                    async move {
                        let result = agent.run(task.clone()).await;
                        (task, result)
                    }
                })
                .buffered(concurrency)
                .collect()
                .await;

            for (task, result) in outcomes {
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

    fn plan(&self, task: String) -> BoxFuture<'_, Result<(), AgentError>> {
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

    fn query_long_term_memory(&self, _task: String) -> BoxFuture<'_, Result<(), AgentError>> {
        unimplemented!("query_long_term_memory not implemented")
    }

    fn save_task_state(&self, task: String) -> BoxFuture<'_, Result<(), AgentError>> {
        let mut hasher = XxHash64::default();
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

                // Build the file name directly: `with_extension` would cut an agent name
                // such as "gpt-4.1-agent" at its first dot.
                let path = save_state_dir.join(format!(
                    "{}_{}.json",
                    self.name().replace(['/', '\\'], "_"),
                    task_hash
                ));

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
/// use swarms_rs::llm::provider::openai::OpenAI;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let model = OpenAI::from_env();
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
///     ChatResponse::TextWithToolCalls { text, tool_calls } => {
///         println!("Agent said '{}' and executed {} tools", text, tool_calls.len());
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

    /// Text the LLM wrote in the same reply as its tool calls, with the tools' outputs.
    ///
    /// Models often explain what they're doing, or give their answer, alongside a tool
    /// call; the text comes first in the reply.
    TextWithToolCalls {
        text: String,
        tool_calls: Vec<ToolCallOutput>,
    },
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
