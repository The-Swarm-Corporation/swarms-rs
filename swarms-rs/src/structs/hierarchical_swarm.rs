use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, error, info, warn};
use reqwest::{Client, Method};
use url::Url;

// ================================================================================================
// ERROR TYPES
// ================================================================================================

#[derive(Error, Debug)]
pub enum HierarchicalSwarmError {
    #[error("No agents found in the swarm. At least one agent must be provided to create a hierarchical swarm.")]
    NoAgents,

    #[error("Max loops must be greater than 0. Please set a valid number of loops.")]
    InvalidMaxLoops,

    #[error("Director not set for the swarm. A director agent is required to coordinate and orchestrate tasks among the agents.")]
    NoDirector,

    #[error("Agent '{agent_name}' not found in swarm. Available agents: {available_agents:?}")]
    AgentNotFound {
        agent_name: String,
        available_agents: Vec<String>,
    },

    #[error("Unable to parse orders from director output: {output}")]
    ParseOrdersError { output: String },

    #[error("Missing 'plan' or 'orders' in director output: {output}")]
    MissingPlanOrOrders { output: String },

    #[error("Unexpected output format from director: {output_type}")]
    UnexpectedOutputFormat { output_type: String },

    #[error("API error: {message}")]
    ApiError {
        message: String,
        status: Option<u16>,
        request_id: Option<String>,
    },

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Timeout error: {message}")]
    Timeout { message: String },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("Invalid configuration: {message}")]
    InvalidConfig { message: String },

    #[error("General error: {0}")]
    General(String),

    #[error("Conversation error: {0}")]
    ConversationError(String),
}

pub type Result<T> = std::result::Result<T, HierarchicalSwarmError>;

// ================================================================================================
// CORE TYPES (MOVED FROM SWARMS_CLIENT)
// ================================================================================================

/// Agent specification for creating agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    pub agent_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(default = "default_model")]
    pub model_name: String,
    #[serde(default)]
    pub auto_generate_prompt: bool,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default = "default_max_loops")]
    pub max_loops: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools_dictionary: Option<Vec<HashMap<String, serde_json::Value>>>,
    #[serde(default)]
    pub markdown: bool,
}

fn default_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_max_tokens() -> u32 {
    8192
}

fn default_temperature() -> f32 {
    0.5
}

fn default_max_loops() -> u32 {
    1
}

/// Agent completion request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCompletion {
    pub agent_config: AgentSpec,
    pub task: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<HashMap<String, serde_json::Value>>,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    #[serde(default)]
    pub img_cost: f64,
    #[serde(default)]
    pub total_cost: f64,
}

/// Response from an agent completion request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCompletionResponse {
    pub job_id: String,
    pub success: bool,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub temperature: f32,
    pub outputs: Vec<HashMap<String, serde_json::Value>>,
    pub usage: Usage,
    pub timestamp: String,
}

/// Generic error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub detail: String,
}

// ================================================================================================
// SIMPLIFIED HTTP CLIENT
// ================================================================================================

/// Configuration for the HTTP client
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub api_key: String,
    pub base_url: Url,
    pub timeout: Duration,
    pub max_retries: usize,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.swarms.world/".parse().unwrap(),
            timeout: Duration::from_secs(60),
            max_retries: 3,
        }
    }
}

/// Simplified HTTP client for API communication
#[derive(Debug, Clone)]
pub struct SwarmsClient {
    client: Client,
    config: ClientConfig,
}

impl SwarmsClient {
    /// Create a new client builder
    pub fn builder() -> Result<ClientBuilder> {
        Ok(ClientBuilder::new())
    }

    /// Create a client with custom configuration
    pub fn with_config(config: ClientConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| HierarchicalSwarmError::Network(e))?;

        Ok(Self { client, config })
    }

    /// Get agent resource
    pub fn agent(&self) -> AgentResource {
        AgentResource::new(self)
    }

    /// Make an HTTP request with retries
    async fn request<T: for<'de> Deserialize<'de>>(
        &self,
        method: Method,
        url: Url,
        body: Option<&impl Serialize>,
    ) -> Result<T> {
        let mut last_error = None;
        
        for attempt in 0..=self.config.max_retries {
            match self.make_request_attempt(method.clone(), url.clone(), body).await {
                Ok(response) => {
                    debug!("Request succeeded on attempt {}", attempt + 1);
                    return Ok(serde_json::from_value(response)?);
                },
                Err(e) => {
                    last_error = Some(e);
                    if attempt < self.config.max_retries {
                        let delay = Duration::from_secs(1) * 2_u32.pow(attempt as u32);
                        warn!("Request failed on attempt {}, retrying in {:?}", attempt + 1, delay);
                        tokio::time::sleep(delay).await;
                    }
                },
            }
        }

        Err(last_error.unwrap())
    }

    /// Make a single request attempt
    async fn make_request_attempt(
        &self,
        method: Method,
        url: Url,
        body: Option<&impl Serialize>,
    ) -> Result<serde_json::Value> {
        let mut request_builder = self.client.request(method, url);

        // Add headers
        request_builder = request_builder
            .header("Content-Type", "application/json")
            .header("x-api-key", &self.config.api_key);

        // Add body if provided
        if let Some(body) = body {
            request_builder = request_builder.json(body);
        }

        let response = tokio::time::timeout(self.config.timeout, request_builder.send())
            .await
            .map_err(|_| HierarchicalSwarmError::Timeout {
                message: format!("Request timed out after {:?}", self.config.timeout),
            })?
            .map_err(|e| HierarchicalSwarmError::Network(e))?;

        let status = response.status();
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());

        if !status.is_success() {
            let body: ErrorResponse = response.json().await.unwrap_or_else(|_| ErrorResponse {
                detail: "Unknown error".to_string(),
            });

            return Err(HierarchicalSwarmError::ApiError {
                message: body.detail,
                status: Some(status.as_u16()),
                request_id,
            });
        }

        let response_body: serde_json::Value = response.json().await?;
        debug!("Response: {}", serde_json::to_string_pretty(&response_body)?);

        Ok(response_body)
    }

    /// Build URL for endpoint
    fn build_url(&self, endpoint: &str) -> Result<Url> {
        Ok(self.config.base_url.join(endpoint)?)
    }
}

/// Builder for creating a Swarms API client
#[derive(Debug, Default)]
pub struct ClientBuilder {
    config: ClientConfig,
}

impl ClientBuilder {
    /// Create a new client builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Load API key from environment variables and .env file
    pub fn from_env() -> Result<Self> {
        // Load .env file if it exists
        dotenv::dotenv().ok();

        // Try to get API key from environment
        let api_key = std::env::var("SWARMS_API_KEY").map_err(|_| HierarchicalSwarmError::InvalidConfig {
            message: "SWARMS_API_KEY not found in environment or .env file".to_string(),
        })?;

        Ok(Self::new().api_key(api_key))
    }

    /// Set the API key
    pub fn api_key<S: Into<String>>(mut self, api_key: S) -> Self {
        self.config.api_key = api_key.into();
        self
    }

    /// Set the base URL
    pub fn base_url<S: AsRef<str>>(mut self, base_url: S) -> Result<Self> {
        self.config.base_url = base_url.as_ref().parse()?;
        Ok(self)
    }

    /// Set the request timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    /// Set the maximum number of retries
    pub fn max_retries(mut self, max_retries: usize) -> Self {
        self.config.max_retries = max_retries;
        self
    }

    /// Build the client
    pub fn build(self) -> Result<SwarmsClient> {
        if self.config.api_key.is_empty() {
            return Err(HierarchicalSwarmError::InvalidConfig {
                message: "API key is required".to_string(),
            });
        }

        SwarmsClient::with_config(self.config)
    }
}

/// Agent resource for agent operations
#[derive(Debug, Clone)]
pub struct AgentResource<'a> {
    client: &'a SwarmsClient,
}

impl<'a> AgentResource<'a> {
    fn new(client: &'a SwarmsClient) -> Self {
        Self { client }
    }

    /// Create an agent completion
    async fn create(&self, request: AgentCompletion) -> Result<AgentCompletionResponse> {
        let url = self.client.build_url("v1/agent/completions")?;
        self.client.request(Method::POST, url, Some(&request)).await
    }

    /// Start building an agent completion request
    pub fn completion(&'a self) -> AgentCompletionBuilder<'a> {
        AgentCompletionBuilder::new(self)
    }
}

/// Builder for agent completions
#[derive(Debug)]
pub struct AgentCompletionBuilder<'a> {
    resource: &'a AgentResource<'a>,
    request: AgentCompletion,
}

impl<'a> AgentCompletionBuilder<'a> {
    fn new(resource: &'a AgentResource<'a>) -> Self {
        Self {
            resource,
            request: AgentCompletion {
                agent_config: AgentSpec {
                    agent_name: String::new(),
                    description: None,
                    system_prompt: None,
                    model_name: default_model(),
                    auto_generate_prompt: false,
                    max_tokens: default_max_tokens(),
                    temperature: default_temperature(),
                    role: None,
                    max_loops: default_max_loops(),
                    tools_dictionary: None,
                    markdown: false,
                },
                task: String::new(),
                history: None,
            },
        }
    }

    /// Set the agent name
    pub fn agent_name<S: Into<String>>(mut self, name: S) -> Self {
        self.request.agent_config.agent_name = name.into();
        self
    }

    /// Set the task
    pub fn task<S: Into<String>>(mut self, task: S) -> Self {
        self.request.task = task.into();
        self
    }

    /// Set the model
    pub fn model<S: Into<String>>(mut self, model: S) -> Self {
        self.request.agent_config.model_name = model.into();
        self
    }

    /// Set the description
    pub fn description<S: Into<String>>(mut self, description: S) -> Self {
        self.request.agent_config.description = Some(description.into());
        self
    }

    /// Set the system prompt
    pub fn system_prompt<S: Into<String>>(mut self, prompt: S) -> Self {
        self.request.agent_config.system_prompt = Some(prompt.into());
        self
    }

    /// Set the temperature
    pub fn temperature(mut self, temperature: f32) -> Self {
        self.request.agent_config.temperature = temperature.clamp(0.0, 1.0);
        self
    }

    /// Set max tokens
    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.request.agent_config.max_tokens = max_tokens;
        self
    }

    /// Set max loops
    pub fn max_loops(mut self, max_loops: u32) -> Self {
        self.request.agent_config.max_loops = max_loops;
        self
    }

    /// Send the request
    pub async fn send(self) -> Result<AgentCompletionResponse> {
        self.resource.create(self.request).await
    }
}

// ================================================================================================
// DATA STRUCTURES
// ================================================================================================

/// Represents a hierarchical order for agent task assignment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchicalOrder {
    /// Specifies the name of the agent to which the task is assigned
    pub agent_name: String,
    /// Defines the specific task to be executed by the assigned agent
    pub task: String,
}

/// Represents the swarm specification with plan and orders
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmSpecResponse {
    /// Outlines the sequence of actions to be taken by the swarm
    pub plan: String,
    /// A collection of task assignments to specific agents within the swarm
    pub orders: Vec<HierarchicalOrder>,
}

/// Supported swarm types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum SwarmTypeEnum {
    AgentRearrange,
    MixtureOfAgents,
    SpreadSheetSwarm,
    SequentialWorkflow,
    ConcurrentWorkflow,
    GroupChat,
    MultiAgentRouter,
    AutoSwarmBuilder,
    HiearchicalSwarm,
    #[serde(rename = "auto")]
    Auto,
    MajorityVoting,
    #[serde(rename = "MALT")]
    Malt,
    DeepResearchSwarm,
}

/// Represents a swarm router call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmRouterCall {
    /// The goal of the swarm router call
    pub goal: String,
    /// The type of swarm to use
    pub swarm_type: SwarmTypeEnum,
    /// The task to be executed by the swarm router
    pub task: String,
}

// ================================================================================================
// HIERARCHICAL SWARM
// ================================================================================================

/// Represents a hierarchical swarm of agents, with a director that orchestrates tasks among the agents.
/// The workflow follows a hierarchical pattern:
/// 1. Task is received and sent to the director
/// 2. Director creates a plan and distributes orders to agents
/// 3. Agents execute tasks and report back to the director
/// 4. Director evaluates results and issues new orders if needed (up to max_loops)
/// 5. All context and conversation history is preserved throughout the process
/// 
#[derive(Clone)]
pub struct HierarchicalSwarm {
    /// The name of the swarm
    pub name: String,
    /// A description of the swarm
    pub description: String,
    /// The director agent that orchestrates tasks
    pub director: AgentSpec,
    /// A list of agents within the swarm
    pub agents: Vec<AgentSpec>,
    /// The maximum number of feedback loops between the director and agents
    pub max_loops: u32,
    /// The format in which to return the output
    pub output_type: String,
    /// The model name for the feedback director
    pub feedback_director_model_name: String,
    /// The name of the director
    pub director_name: String,
    /// The model name for the director
    pub director_model_name: String,
    /// Enable detailed logging
    pub verbose: bool,
    /// Whether to add collaboration prompt
    pub add_collaboration_prompt: bool,
    /// Planning director agent
    pub planning_director_agent: Option<AgentSpec>,
    /// Whether director feedback is enabled
    pub director_feedback_on: bool,
    /// Enable interactive dashboard (real-time monitoring)
    pub interactive: bool,
    /// Dashboard update callback for real-time monitoring
    pub dashboard_callback: Option<Arc<dyn Fn(DashboardUpdate) + Send + Sync>>,
    /// Whether to execute agents sequentially with memory (true) or in parallel (false)
    pub sequential_execution: bool,
    /// The Swarms API client
    client: Arc<SwarmsClient>,
    /// Conversation history (thread-safe mutable)
    conversation: Arc<Mutex<Vec<HashMap<String, serde_json::Value>>>>,
    /// Execution statistics for dashboard
    execution_stats: Arc<Mutex<ExecutionStats>>,
}

impl std::fmt::Debug for HierarchicalSwarm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HierarchicalSwarm")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("director", &self.director)
            .field("agents", &self.agents)
            .field("max_loops", &self.max_loops)
            .field("output_type", &self.output_type)
            .field("feedback_director_model_name", &self.feedback_director_model_name)
            .field("director_name", &self.director_name)
            .field("director_model_name", &self.director_model_name)
            .field("verbose", &self.verbose)
            .field("add_collaboration_prompt", &self.add_collaboration_prompt)
            .field("planning_director_agent", &self.planning_director_agent)
            .field("director_feedback_on", &self.director_feedback_on)
            .field("interactive", &self.interactive)
            .field("dashboard_callback", &"<callback>")
            .field("sequential_execution", &self.sequential_execution)
            .field("client", &"<client>")
            .field("conversation", &"<conversation>")
            .field("execution_stats", &"<execution_stats>")
            .finish()
    }
}

/// Dashboard update information for real-time monitoring
#[derive(Debug, Clone)]
pub struct DashboardUpdate {
    /// Current loop number
    pub current_loop: u32,
    /// Total loops
    pub total_loops: u32,
    /// Current agent being executed
    pub current_agent: Option<String>,
    /// Agent execution status
    pub agent_status: AgentStatus,
    /// Overall progress percentage
    pub progress: f32,
    /// Latest output from current agent
    pub latest_output: Option<String>,
    /// Error message if any
    pub error: Option<String>,
}

/// Agent execution status
#[derive(Debug, Clone)]
pub enum AgentStatus {
    /// Agent is waiting to be executed
    Waiting,
    /// Agent is currently executing
    Executing,
    /// Agent has completed successfully
    Completed,
    /// Agent execution failed
    Failed,
}

/// Execution statistics for monitoring
#[derive(Debug, Clone)]
pub struct ExecutionStats {
    /// Total loops completed
    pub loops_completed: u32,
    /// Total agents executed
    pub agents_executed: u32,
    /// Total execution time in seconds
    pub total_time: f64,
    /// Start time of execution
    pub start_time: std::time::Instant,
    /// Agent-specific statistics
    pub agent_stats: HashMap<String, AgentStats>,
}

/// Individual agent statistics
#[derive(Debug, Clone)]
pub struct AgentStats {
    /// Number of times this agent was executed
    pub execution_count: u32,
    /// Total execution time for this agent
    pub total_time: f64,
    /// Average execution time
    pub avg_time: f64,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f32,
    /// Last execution status
    pub last_status: AgentStatus,
}

impl HierarchicalSwarm {
    /// Creates a new HierarchicalSwarm with the given parameters
    /// 
    /// **Performance Optimized**: Core swarm prioritizes speed over visual formatting.
    pub fn new(
        name: String,
        description: String,
        director: AgentSpec,
        agents: Vec<AgentSpec>,
        max_loops: u32,
        output_type: String,
        feedback_director_model_name: String,
        director_name: String,
        director_model_name: String,
        verbose: bool,
        add_collaboration_prompt: bool,
        planning_director_agent: Option<AgentSpec>,
        director_feedback_on: bool,
        interactive: bool,
        dashboard_callback: Option<Arc<dyn Fn(DashboardUpdate) + Send + Sync>>,
        sequential_execution: bool,
        client: SwarmsClient,
    ) -> Result<Self> {
        let swarm = Self {
            name,
            description,
            director,
            agents,
            max_loops,
            output_type,
            feedback_director_model_name,
            director_name,
            director_model_name,
            verbose,
            add_collaboration_prompt,
            planning_director_agent,
            director_feedback_on,
            interactive,
            dashboard_callback,
            sequential_execution,
            client: Arc::new(client),
            conversation: Arc::new(Mutex::new(Vec::new())),
            execution_stats: Arc::new(Mutex::new(ExecutionStats {
                loops_completed: 0,
                agents_executed: 0,
                total_time: 0.0,
                start_time: std::time::Instant::now(),
                agent_stats: HashMap::new(),
            })),
        };

        swarm.init_swarm()?;
        Ok(swarm)
    }


    /// Updates the dashboard with current execution status
    async fn update_dashboard(&self, update: DashboardUpdate) {
        if self.interactive {
            if let Some(ref callback) = self.dashboard_callback {
                callback(update);
            }
        }
    }

    /// Updates execution statistics
    async fn update_execution_stats(&self, agent_name: &str, status: AgentStatus, execution_time: f64) -> Result<()> {
        let mut stats = self.execution_stats.lock()
            .map_err(|e| HierarchicalSwarmError::ConversationError(format!("Failed to lock stats: {}", e)))?;
        
        let agent_stat = stats.agent_stats.entry(agent_name.to_string()).or_insert(AgentStats {
            execution_count: 0,
            total_time: 0.0,
            avg_time: 0.0,
            success_rate: 1.0,
            last_status: AgentStatus::Waiting,
        });
        
        agent_stat.execution_count += 1;
        agent_stat.total_time += execution_time;
        agent_stat.avg_time = agent_stat.total_time / agent_stat.execution_count as f64;
        agent_stat.last_status = status.clone();
        
        // Update success rate based on status
        if matches!(status, AgentStatus::Completed) {
            agent_stat.success_rate = (agent_stat.success_rate * (agent_stat.execution_count - 1) as f32 + 1.0) / agent_stat.execution_count as f32;
        } else if matches!(status, AgentStatus::Failed) {
            agent_stat.success_rate = (agent_stat.success_rate * (agent_stat.execution_count - 1) as f32) / agent_stat.execution_count as f32;
        }
        
        stats.agents_executed += 1;
        stats.total_time = stats.start_time.elapsed().as_secs_f64();
        
        Ok(())
    }

    /// Initializes the swarm
    fn init_swarm(&self) -> Result<()> {
        if self.verbose {
            info!(" Initializing HierarchicalSwarm: {}", self.name);
            info!(" Configuration - Max loops: {}", self.max_loops);
            if self.interactive {
                info!("  Interactive dashboard enabled");
            }
        }

        // Reliability checks
        self.reliability_checks()?;

        if self.verbose {
            info!(" HierarchicalSwarm initialized successfully: Name {}", self.name);
        }

        Ok(())
    }

    /// Checks if there are any agents and a director set for the swarm
    fn reliability_checks(&self) -> Result<()> {
        if self.verbose {
            info!(" Running reliability checks for swarm: {}", self.name);
        }

        if self.agents.is_empty() {
            return Err(HierarchicalSwarmError::NoAgents);
        }

        if self.max_loops <= 0 {
            return Err(HierarchicalSwarmError::InvalidMaxLoops);
        }

        if self.verbose {
            info!(" Reliability checks passed for swarm: {}", self.name);
            info!(" Swarm stats - Agents: {}, Max loops: {}", self.agents.len(), self.max_loops);
        }

        Ok(())
    }

    /// Runs a task through the director agent with the current conversation context
    pub async fn run_director(&self, task: &str, _img: Option<&str>) -> Result<SwarmSpecResponse> {
        if self.verbose {
            info!(" Running director with task: {}...", &task[..task.len().min(100)]);
        }

        let mut director_task = task.to_string();

        // If planning director agent is provided, run it first
        if let Some(ref planning_agent) = self.planning_director_agent {
            let history = self.get_conversation_str().await?;
            let planning_task = format!(
                "History: {} \n\n Create a detailed step by step comprehensive plan for the director to execute the task: {}",
                history, task
            );

            let planning_response = self.client
                .agent()
                .completion()
                .agent_name(&planning_agent.agent_name)
                .task(&planning_task)
                .model(&planning_agent.model_name)
                .system_prompt(planning_agent.system_prompt.as_deref().unwrap_or(""))
                .temperature(planning_agent.temperature)
                .max_tokens(planning_agent.max_tokens)
                .send()
                .await?;

            let planning_content = if let Some(first_output) = planning_response.outputs.first() {
                if let Some(content) = first_output.get("content") {
                    content.as_str().unwrap_or("").to_string()
                } else {
                    serde_json::to_string(&first_output).unwrap_or_default()
                }
            } else {
                "No planning output received".to_string()
            };

            director_task.push_str(&format!("\n\nPlanning: {}", planning_content));
        }

        // Run the director with the context
        let history = self.get_conversation_str().await?;
        let director_task_with_history = format!("History: {} \n\n Task: {}", history, director_task);

        let response = self.client
            .agent()
            .completion()
            .agent_name(&self.director.agent_name)
            .task(&director_task_with_history)
            .model(&self.director.model_name)
            .system_prompt(self.director.system_prompt.as_deref().unwrap_or(""))
            .temperature(self.director.temperature)
            .max_tokens(self.director.max_tokens)
            .send()
            .await?;

        // Add to conversation
        let output_content = if let Some(first_output) = response.outputs.first() {
            if let Some(content) = first_output.get("content") {
                content.as_str().unwrap_or("").to_string()
            } else {
                serde_json::to_string(&first_output).unwrap_or_default()
            }
        } else {
            "No output received".to_string()
        };
        self.add_to_conversation("Director", &output_content).await?;

        // Parse the response to extract plan and orders
        let response_content = if let Some(first_output) = response.outputs.first() {
            if let Some(content) = first_output.get("content") {
                serde_json::Value::String(content.as_str().unwrap_or("").to_string())
            } else {
                serde_json::to_value(first_output).unwrap_or(serde_json::Value::Null)
            }
        } else {
            serde_json::Value::String("No output received".to_string())
        };

        let parsed_response = self.parse_director_response(&response_content)?;

        // Markdown functionality removed from core hierarchical swarm

        if self.verbose {
            info!(" Director execution completed");
            debug!(" Director output type: {:?}", response.outputs);
        }

        Ok(parsed_response)
    }


    /// Parses the director's response to extract plan and orders
    fn parse_director_response(&self, output: &serde_json::Value) -> Result<SwarmSpecResponse> {
        if self.verbose {
            info!(" Parsing director orders");
            debug!(" Output type: {:?}", output);
        }

        // Try to parse as direct JSON first
        if let Ok(spec) = serde_json::from_value::<SwarmSpecResponse>(output.clone()) {
            if self.verbose {
                info!(" Successfully parsed plan and {} orders", spec.orders.len());
            }
            return Ok(spec);
        }

        // Try to extract from function call format
        if let Some(function_data) = output.get("function") {
            if let Some(arguments) = function_data.get("arguments") {
                if let Ok(args) = serde_json::from_value::<serde_json::Value>(arguments.clone()) {
                    if let (Some(plan), Some(orders)) = (args.get("plan"), args.get("orders")) {
                        let plan_str = plan.as_str().unwrap_or("").to_string();
                        let orders_vec = serde_json::from_value::<Vec<HierarchicalOrder>>(orders.clone())?;
                        
                        let spec = SwarmSpecResponse {
                            plan: plan_str,
                            orders: orders_vec,
                        };

                        if self.verbose {
                            info!(" Successfully parsed plan and {} orders", spec.orders.len());
                        }
                        return Ok(spec);
                    }
                }
            }
        }

        // Try to extract from conversation format
        if let Some(conversation) = output.as_array() {
            for item in conversation {
                if let Some(content) = item.get("content") {
                    if let Some(content_array) = content.as_array() {
                        for content_item in content_array {
                            if let Some(function) = content_item.get("function") {
                                if let Some(arguments) = function.get("arguments") {
                                    if let Ok(args) = serde_json::from_value::<serde_json::Value>(arguments.clone()) {
                                        if let (Some(plan), Some(orders)) = (args.get("plan"), args.get("orders")) {
                                            let plan_str = plan.as_str().unwrap_or("").to_string();
                                            let orders_vec = serde_json::from_value::<Vec<HierarchicalOrder>>(orders.clone())?;
                                            
                                            let spec = SwarmSpecResponse {
                                                plan: plan_str,
                                                orders: orders_vec,
                                            };

                                            if self.verbose {
                                                info!(" Successfully parsed plan and {} orders", spec.orders.len());
                                            }
                                            return Ok(spec);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // If all parsing attempts fail, try to extract tasks from natural language
        let output_str = output.as_str().unwrap_or("");
        if !output_str.is_empty() {
            // Create a simple plan and extract agent tasks from the text
            let plan = output_str.to_string();
            
            // Extract agent tasks from the text by looking for patterns
            let mut orders = Vec::new();
            
            // Look for patterns like "Agent X: Task Y" or "Research Agent X" followed by tasks
            let lines: Vec<&str> = output_str.lines().collect();
            for line in lines {
                let line = line.trim();
                if line.contains("Agent") || line.contains("Focus:") || line.contains("Task:") {
                    // Try to extract agent name and task
                    if let Some(agent_match) = line.find("Agent") {
                        let agent_part = &line[agent_match..];
                        if let Some(task_start) = agent_part.find(":") {
                            let agent_name = agent_part[..task_start].trim();
                            let task = agent_part[task_start + 1..].trim();
                            
                            // Clean up agent name - remove asterisks, numbers, and extra formatting
                            let mut clean_agent_name = agent_name.to_string();
                            clean_agent_name = clean_agent_name.replace("*", "").replace("#", "").trim().to_string();
                            
                            // If agent name is just "Agent" or similar, try to extract more context
                            if clean_agent_name == "Agent" || clean_agent_name.len() < 5 {
                                // Look for the next line that might contain the actual agent name
                                continue;
                            }
                            
                            if !clean_agent_name.is_empty() && !task.is_empty() {
                                orders.push(HierarchicalOrder {
                                    agent_name: clean_agent_name,
                                    task: task.to_string(),
                                });
                            }
                        }
                    }
                }
            }
            
            // If we found some orders, return them
            if !orders.is_empty() {
                let spec = SwarmSpecResponse {
                    plan,
                    orders,
                };
                
                if self.verbose {
                    info!(" Successfully extracted {} orders from natural language", spec.orders.len());
                }
                return Ok(spec);
            }
            
            // If no specific orders found, create default orders for available agents
            if !self.agents.is_empty() {
                let mut default_orders = Vec::new();
                for (_i, agent) in self.agents.iter().enumerate() {
                    let task = format!("Execute the research task focusing on your area of expertise: {}", agent.description.as_deref().unwrap_or("general research"));
                    default_orders.push(HierarchicalOrder {
                        agent_name: agent.agent_name.clone(),
                        task,
                    });
                }
                
                let spec = SwarmSpecResponse {
                    plan,
                    orders: default_orders,
                };
                
                if self.verbose {
                    info!(" Created {} default orders for available agents", spec.orders.len());
                }
                return Ok(spec);
            }
        }

        Err(HierarchicalSwarmError::ParseOrdersError {
            output: serde_json::to_string_pretty(output)?,
        })
    }

    /// Executes a single step of the hierarchical swarm following the architecture:
    /// 1. Director creates Plan & Orders
    /// 2. Distribute to specialized Agents
    /// 3. Agents execute tasks in parallel
    /// 4. Collect and report results
    /// 5. Director evaluates results
    pub async fn step(&self, task: &str, img: Option<&str>) -> Result<Vec<String>> {
        if self.verbose {
            info!("👣 Executing hierarchical step for task: {}...", &task[..task.len().min(100)]);
        }

        // Step 1: Director creates Plan & Orders
        let swarm_spec = self.run_director(task, img).await?;

        if self.verbose {
            info!(" Director created plan and {} orders", swarm_spec.orders.len());
        }

        // Step 2: Execute orders (distribute to agents and collect results)
        let agent_outputs = if self.sequential_execution {
            self.execute_orders(&swarm_spec.orders).await?
        } else {
            self.execute_orders_parallel(&swarm_spec.orders).await?
        };

        if self.verbose {
            info!(" Executed {} agent tasks", agent_outputs.len());
        }

        // Step 3: Director evaluates results and provides feedback
        let final_outputs = if self.director_feedback_on {
            self.feedback_director(&agent_outputs).await?
        } else {
            agent_outputs
        };

        if self.verbose {
            info!(" Hierarchical step completed successfully");
        }

        Ok(final_outputs)
    }

    /// Executes the hierarchical swarm for a specified number of feedback loops
    pub async fn run(&self, task: &str, img: Option<&str>) -> Result<Vec<String>> {
        let mut current_loop = 0;
        let mut last_output = Vec::new();

        if self.verbose {
            info!(" Starting hierarchical swarm run: {}", self.name);
            info!(" Configuration - Max loops: {}", self.max_loops);
        }

        // Reset execution stats
        {
            let mut stats = self.execution_stats.lock()
                .map_err(|e| HierarchicalSwarmError::ConversationError(format!("Failed to lock stats: {}", e)))?;
            stats.loops_completed = 0;
            stats.agents_executed = 0;
            stats.start_time = std::time::Instant::now();
            stats.agent_stats.clear();
        }

        while current_loop < self.max_loops {
            if self.verbose {
                info!(" Loop {}/{} - Processing task", current_loop + 1, self.max_loops);
            }

            // Update dashboard for loop start
            self.update_dashboard(DashboardUpdate {
                current_loop: current_loop + 1,
                total_loops: self.max_loops,
                current_agent: None,
                agent_status: AgentStatus::Waiting,
                progress: (current_loop as f32 / self.max_loops as f32) * 100.0,
                latest_output: None,
                error: None,
            }).await;

            // For the first loop, use the original task.
            // For subsequent loops, use the feedback from the previous loop as context.
            let loop_task = if current_loop == 0 {
                task.to_string()
            } else {
                format!(
                    "Previous loop results: {:?}\n\nOriginal task: {}\n\nBased on the previous results and any feedback, continue with the next iteration of the task. Refine, improve, or complete any remaining aspects of the analysis.",
                    last_output, task
                )
            };

            // Execute one step of the swarm
            match self.step(&loop_task, img).await {
                Ok(output) => {
                    last_output = output;
                    if self.verbose {
                        info!(" Loop {} completed successfully", current_loop + 1);
                    }

                    // Update execution stats
                    {
                        let mut stats = self.execution_stats.lock()
                            .map_err(|e| HierarchicalSwarmError::ConversationError(format!("Failed to lock stats: {}", e)))?;
                        stats.loops_completed += 1;
                    }
                }
                Err(e) => {
                    error!(" Loop {} failed: {:?}", current_loop + 1, e);
                    
                    // Update dashboard with error
                    self.update_dashboard(DashboardUpdate {
                        current_loop: current_loop + 1,
                        total_loops: self.max_loops,
                        current_agent: None,
                        agent_status: AgentStatus::Failed,
                        progress: (current_loop as f32 / self.max_loops as f32) * 100.0,
                        latest_output: None,
                        error: Some(e.to_string()),
                    }).await;
                    
                    return Err(e);
                }
            }

            current_loop += 1;

            // Add loop completion marker to conversation
            self.add_to_conversation(
                "System",
                &format!("--- Loop {}/{} completed ---", current_loop, self.max_loops),
            ).await?;
        }

        if self.verbose {
            info!(" Hierarchical swarm run completed: {}", self.name);
            info!(" Total loops executed: {}", current_loop);
        }

        // Markdown functionality removed from core hierarchical swarm

        // Final dashboard update
        self.update_dashboard(DashboardUpdate {
            current_loop: self.max_loops,
            total_loops: self.max_loops,
            current_agent: None,
            agent_status: AgentStatus::Completed,
            progress: 100.0,
            latest_output: Some("All loops completed successfully".to_string()),
            error: None,
        }).await;

        Ok(last_output)
    }


    /// Executes the hierarchical swarm with streaming output
    pub async fn run_stream(
        &self,
        task: &str,
        img: Option<&str>,
    ) -> Result<impl futures::Stream<Item = Result<String>> + '_> {
        if self.verbose {
            info!(" Starting hierarchical swarm run with streaming: {}", self.name);
            info!("Configuration - Max loops: {}", self.max_loops);
        }

        // Create a stream that yields results as they become available
        let task_clone = task.to_string();
        let img_clone = img.map(|s| s.to_string());
        let swarm_clone = self.clone();
        
        let stream = futures::stream::unfold(
            (swarm_clone, task_clone, img_clone, 0, Vec::new()),
            |(swarm, task, img, loop_count, mut last_output)| async move {
                if loop_count >= swarm.max_loops {
                    return None;
                }

                if swarm.verbose {
                    info!(" Loop {}/{} - Processing task", loop_count + 1, swarm.max_loops);
                }

                // For the first loop, use the original task.
                // For subsequent loops, use the feedback from the previous loop as context.
                let loop_task = if loop_count == 0 {
                    task.clone()
                } else {
                    format!(
                        "Previous loop results: {:?}\n\nOriginal task: {}\n\nBased on the previous results and any feedback, continue with the next iteration of the task. Refine, improve, or complete any remaining aspects of the analysis.",
                        last_output, task
                    )
                };

                // Execute one step of the swarm
                match swarm.step(&loop_task, img.as_deref()).await {
                    Ok(output) => {
                        last_output = output.clone();
                        if swarm.verbose {
                            info!(" Loop {} completed successfully", loop_count + 1);
                        }

                        // Add loop completion marker to conversation
                        if let Err(e) = swarm.add_to_conversation(
                            "System",
                            &format!("--- Loop {}/{} completed ---", loop_count + 1, swarm.max_loops),
                        ).await {
                            return Some((Err(e), (swarm, task, img, loop_count + 1, last_output)));
                        }

                        // Yield the output as a single string
                        let output_str = output.join("\n\n");
                        Some((Ok(output_str), (swarm, task, img, loop_count + 1, last_output)))
                    }
                    Err(e) => {
                        error!(" Loop {} failed: {:?}", loop_count + 1, e);
                        Some((Err(e), (swarm, task, img, loop_count + 1, last_output)))
                    }
                }
            },
        );

        Ok(stream)
    }

    /// Executes the orders from the director's output (distribute to agents)
    /// This represents the "Distribute to Agents" and "Sequential Agent Execution" steps
    /// Each agent can build upon the previous agent's output
    async fn execute_orders(&self, orders: &[HierarchicalOrder]) -> Result<Vec<String>> {
        if self.verbose {
            info!("⚡ Distributing {} orders to specialized agents with memory", orders.len());
        }

        let mut outputs = Vec::new();
        let mut previous_outputs = Vec::new();

        for (i, order) in orders.iter().enumerate() {
            if self.verbose {
                info!(" Agent {}: {} - {}", i + 1, order.agent_name, order.task);
            }

            // Build context from previous outputs (optimized)
            let enhanced_task = if !previous_outputs.is_empty() {
                let context = previous_outputs.join("\n\n--- Previous Agent Output ---\n\n");
                format!(
                    "Previous agent outputs:\n{}\n\nYour task: {}\n\nBuild upon, refine, or continue from the previous work. If you see gaps or areas for improvement, address them. If the previous work is solid, expand on it.",
                    context, order.task
                )
            } else {
                order.task.clone()
            };

            let output = self.call_single_agent_optimized(&order.agent_name, &enhanced_task).await?;
            outputs.push(output.clone());
            previous_outputs.push(format!("{}: {}", order.agent_name, output));
        }

        if self.verbose {
            info!(" All {} agent tasks completed successfully with memory", orders.len());
        }

        Ok(outputs)
    }

    /// Executes the orders from the director's output in parallel (without memory)
    /// This is the original parallel execution method
    async fn execute_orders_parallel(&self, orders: &[HierarchicalOrder]) -> Result<Vec<String>> {
        if self.verbose {
            info!(" Executing {} orders in parallel", orders.len());
        }

        use futures::future::join_all;

        let futures: Vec<_> = orders
            .iter()
            .enumerate()
            .map(|(i, order)| {
                let verbose = self.verbose;
                let agent_name = order.agent_name.clone();
                let task = order.task.clone();
                
                async move {
                    if verbose {
                        info!(" Executing order {}/{}: {}", i + 1, orders.len(), agent_name);
                    }
                    
                    // Use optimized agent call for better performance
                    self.call_single_agent_optimized(&agent_name, &task).await
                }
            })
            .collect();

        let results = join_all(futures).await;
        
        // Collect results and handle errors
        let mut outputs = Vec::new();
        for result in results {
            match result {
                Ok(output) => outputs.push(output),
                Err(e) => {
                    error!(" Agent execution failed: {:?}", e);
                    return Err(e);
                }
            }
        }

        if self.verbose {
            info!(" All {} orders executed successfully in parallel", orders.len());
        }

        Ok(outputs)
    }

    /// Calls a single agent with the given task (optimized for speed)
    async fn call_single_agent_optimized(&self, agent_name: &str, task: &str) -> Result<String> {
        if self.verbose {
            info!(" Calling agent: {}", agent_name);
        }

        // Find agent by name with flexible matching
        let agent = self.find_agent_by_name(agent_name)?;

        // Only get conversation history if verbose or interactive mode is enabled
        let agent_task = if self.verbose || self.interactive {
            let history = self.get_conversation_str().await?;
            format!("History: {} \n\n Task: {}", history, task)
        } else {
            task.to_string()
        };

        let response = self.client
            .agent()
            .completion()
            .agent_name(&agent.agent_name)
            .task(&agent_task)
            .model(&agent.model_name)
            .system_prompt(agent.system_prompt.as_deref().unwrap_or(""))
            .temperature(agent.temperature)
            .max_tokens(agent.max_tokens)
            .send()
            .await?;

        let output_content = if let Some(first_output) = response.outputs.first() {
            if let Some(content) = first_output.get("content") {
                content.as_str().unwrap_or("").to_string()
            } else {
                serde_json::to_string(&first_output).unwrap_or_default()
            }
        } else {
            "No output received".to_string()
        };
        
        // Only add to conversation if verbose or interactive mode is enabled
        if self.verbose || self.interactive {
            self.add_to_conversation(agent_name, &output_content).await?;
        }

        // Markdown functionality removed from core implementation

        if self.verbose {
            info!(" Agent {} completed task successfully", agent_name);
        }

        Ok(output_content)
    }


    /// Provides feedback from the director based on agent outputs
    async fn feedback_director(&self, outputs: &[String]) -> Result<Vec<String>> {
        if self.verbose {
            info!(" Generating director feedback");
        }

        let history = self.get_conversation_str().await?;
        let outputs_str = outputs.join("\n\n");

        let feedback_task = format!(
            "You are the Director. Carefully review the outputs generated by all the worker agents in the previous step. \
            Provide specific, actionable feedback for each agent, highlighting strengths, weaknesses, and concrete suggestions for improvement. \
            If any outputs are unclear, incomplete, or could be enhanced, explain exactly how. \
            Your feedback should help the agents refine their work in the next iteration. \
            Worker Agent Responses: {}\n\nHistory: {}",
            outputs_str, history
        );

        let response = self.client
            .agent()
            .completion()
            .agent_name("Director")
            .task(&feedback_task)
            .model(&self.feedback_director_model_name)
            .system_prompt("You are a Director module that provides feedback to the worker agents")
            .temperature(0.7)
            .max_tokens(2000)
            .send()
            .await?;

        let feedback_content = if let Some(first_output) = response.outputs.first() {
            if let Some(content) = first_output.get("content") {
                content.as_str().unwrap_or("").to_string()
            } else {
                serde_json::to_string(&first_output).unwrap_or_default()
            }
        } else {
            "No feedback received".to_string()
        };
        self.add_to_conversation(&self.director.agent_name, &feedback_content).await?;

        // Markdown functionality removed from core hierarchical swarm

        if self.verbose {
            info!(" Director feedback generated successfully");
        }

        Ok(vec![feedback_content])
    }


    /// Executes the hierarchical swarm for a list of tasks
    pub async fn batched_run(&self, tasks: &[String], img: Option<&str>) -> Result<Vec<Vec<String>>> {
        if self.verbose {
            info!(" Starting batched hierarchical swarm run: {}", self.name);
            info!(" Configuration - Max loops: {}", self.max_loops);
        }

        let mut results = Vec::new();

        // Process each task sequentially (could be made parallel if needed)
        for task in tasks {
            let result = self.run(task, img).await?;
            results.push(result);
        }

        if self.verbose {
            info!(" Batched hierarchical swarm run completed: {}", self.name);
            info!(" Total tasks processed: {}", tasks.len());
        }

        Ok(results)
    }

    /// Executes the hierarchical swarm for a list of tasks in parallel
    pub async fn batched_run_parallel(&self, tasks: &[String], img: Option<&str>) -> Result<Vec<Vec<String>>> {
        if self.verbose {
            info!(" Starting parallel batched hierarchical swarm run: {}", self.name);
            info!(" Configuration - Max loops: {}", self.max_loops);
        }

        use futures::future::join_all;

        let futures: Vec<_> = tasks
            .iter()
            .map(|task| {
                let task = task.clone();
                async move {
                    self.run(&task, img).await
                }
            })
            .collect();

        let results = join_all(futures).await;
        
        // Collect results and handle errors
        let mut outputs = Vec::new();
        for result in results {
            match result {
                Ok(output) => outputs.push(output),
                Err(e) => {
                    error!(" Task execution failed: {:?}", e);
                    return Err(e);
                }
            }
        }

        if self.verbose {
            info!(" Parallel batched hierarchical swarm run completed: {}", self.name);
            info!(" Total tasks processed: {}", tasks.len());
        }

        Ok(outputs)
    }

    /// Adds a message to the conversation history
    async fn add_to_conversation(&self, role: &str, content: &str) -> Result<()> {
        let mut message = HashMap::new();
        message.insert("role".to_string(), serde_json::Value::String(role.to_string()));
        message.insert("content".to_string(), serde_json::Value::String(content.to_string()));
        
        let mut conversation = self.conversation.lock()
            .map_err(|e| HierarchicalSwarmError::ConversationError(format!("Failed to lock conversation: {}", e)))?;
        
        conversation.push(message);
        
        if self.verbose {
            debug!("Added to conversation: {}: {}", role, &content[..content.len().min(100)]);
        }

        Ok(())
    }

    /// Gets the conversation history as a string
    async fn get_conversation_str(&self) -> Result<String> {
        let conversation = self.conversation.lock()
            .map_err(|e| HierarchicalSwarmError::ConversationError(format!("Failed to lock conversation: {}", e)))?;
        
        let result = conversation
            .iter()
            .map(|msg| {
                let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("Unknown");
                let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
                format!("{}: {}", role, content)
            })
            .collect::<Vec<_>>()
            .join("\n");
        
        Ok(result)
    }

    /// Gets the conversation history as a vector of messages
    pub async fn get_conversation(&self) -> Result<Vec<HashMap<String, serde_json::Value>>> {
        let conversation = self.conversation.lock()
            .map_err(|e| HierarchicalSwarmError::ConversationError(format!("Failed to lock conversation: {}", e)))?;
        
        Ok(conversation.clone())
    }

    /// Clears the conversation history
    pub async fn clear_conversation(&self) -> Result<()> {
        let mut conversation = self.conversation.lock()
            .map_err(|e| HierarchicalSwarmError::ConversationError(format!("Failed to lock conversation: {}", e)))?;
        
        conversation.clear();
        
        if self.verbose {
            info!(" Conversation history cleared");
        }

        Ok(())
    }

    /// Gets the number of messages in the conversation
    pub async fn conversation_length(&self) -> Result<usize> {
        let conversation = self.conversation.lock()
            .map_err(|e| HierarchicalSwarmError::ConversationError(format!("Failed to lock conversation: {}", e)))?;
        
        Ok(conversation.len())
    }

    /// Gets the last message from the conversation
    pub async fn get_last_message(&self) -> Result<Option<HashMap<String, serde_json::Value>>> {
        let conversation = self.conversation.lock()
            .map_err(|e| HierarchicalSwarmError::ConversationError(format!("Failed to lock conversation: {}", e)))?;
        
        Ok(conversation.last().cloned())
    }

    /// Gets messages by role
    pub async fn get_messages_by_role(&self, role: &str) -> Result<Vec<HashMap<String, serde_json::Value>>> {
        let conversation = self.conversation.lock()
            .map_err(|e| HierarchicalSwarmError::ConversationError(format!("Failed to lock conversation: {}", e)))?;
        
        let messages: Vec<_> = conversation
            .iter()
            .filter(|msg| {
                msg.get("role")
                    .and_then(|v| v.as_str())
                    .map(|r| r == role)
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        
        Ok(messages)
    }

    /// Gets execution statistics for monitoring and dashboard
    pub async fn get_execution_statistics(&self) -> Result<ExecutionStats> {
        let stats = self.execution_stats.lock()
            .map_err(|e| HierarchicalSwarmError::ConversationError(format!("Failed to lock stats: {}", e)))?;
        
        Ok(stats.clone())
    }

    /// Gets the swarm configuration as a string
    pub fn get_configuration(&self) -> String {
        format!(
            "HierarchicalSwarm Configuration:\n\
            - Name: {}\n\
            - Description: {}\n\
            - Director: {}\n\
            - Agents: {}\n\
            - Max Loops: {}\n\
            - Output Type: {}\n\
            - Verbose: {}\n\
            - Director Feedback: {}\n\
            - Interactive Dashboard: {}",
            self.name,
            self.description,
            self.director.agent_name,
            self.agents.len(),
            self.max_loops,
            self.output_type,
            self.verbose,
            self.director_feedback_on,
            self.interactive
        )
    }

    /// Gets the list of agent names
    pub fn get_agent_names(&self) -> Vec<String> {
        self.agents
            .iter()
            .map(|agent| agent.agent_name.clone())
            .collect()
    }

    /// Checks if an agent exists by name
    pub fn has_agent(&self, agent_name: &str) -> bool {
        self.agents
            .iter()
            .any(|agent| agent.agent_name == agent_name)
    }

    /// Gets agent by name
    pub fn get_agent(&self, agent_name: &str) -> Option<&AgentSpec> {
        self.agents
            .iter()
            .find(|agent| agent.agent_name == agent_name)
    }

    /// Gets the director agent
    pub fn get_director(&self) -> &AgentSpec {
        &self.director
    }

    /// Gets the planning director agent if set
    pub fn get_planning_director(&self) -> Option<&AgentSpec> {
        self.planning_director_agent.as_ref()
    }

    /// Validates the swarm configuration
    pub fn validate_configuration(&self) -> Result<()> {
        // Check if agents exist
        if self.agents.is_empty() {
            return Err(HierarchicalSwarmError::NoAgents);
        }

        // Check max loops
        if self.max_loops <= 0 {
            return Err(HierarchicalSwarmError::InvalidMaxLoops);
        }

        // Check for duplicate agent names
        let mut agent_names = std::collections::HashSet::new();
        for agent in &self.agents {
            if !agent_names.insert(&agent.agent_name) {
                return Err(HierarchicalSwarmError::General(
                    format!("Duplicate agent name found: {}", agent.agent_name)
                ));
            }
        }

        // Check if director name conflicts with agent names
        if agent_names.contains(&self.director.agent_name) {
            return Err(HierarchicalSwarmError::General(
                format!("Director name conflicts with agent name: {}", self.director.agent_name)
            ));
        }

        // Check planning director name conflicts
        if let Some(planning_agent) = &self.planning_director_agent {
            if agent_names.contains(&planning_agent.agent_name) {
                return Err(HierarchicalSwarmError::General(
                    format!("Planning director name conflicts with agent name: {}", planning_agent.agent_name)
                ));
            }
            if planning_agent.agent_name == self.director.agent_name {
                return Err(HierarchicalSwarmError::General(
                    "Planning director name conflicts with director name".to_string()
                ));
            }
        }

        Ok(())
    }

    /// Resets the swarm state (clears conversation and resets counters)
    pub async fn reset(&self) -> Result<()> {
        if self.verbose {
            info!(" Resetting hierarchical swarm state");
        }

        // Clear conversation
        self.clear_conversation().await?;

        // Validate configuration
        self.validate_configuration()?;

        if self.verbose {
            info!(" Hierarchical swarm reset successfully");
        }

        Ok(())
    }

    /// Gets swarm statistics
    pub async fn get_statistics(&self) -> Result<HashMap<String, serde_json::Value>> {
        let mut stats = HashMap::new();
        
        stats.insert("name".to_string(), serde_json::Value::String(self.name.clone()));
        stats.insert("description".to_string(), serde_json::Value::String(self.description.clone()));
        stats.insert("max_loops".to_string(), serde_json::Value::Number(self.max_loops.into()));
        stats.insert("output_type".to_string(), serde_json::Value::String(self.output_type.clone()));
        stats.insert("verbose".to_string(), serde_json::Value::Bool(self.verbose));
        stats.insert("director_feedback_on".to_string(), serde_json::Value::Bool(self.director_feedback_on));
        stats.insert("add_collaboration_prompt".to_string(), serde_json::Value::Bool(self.add_collaboration_prompt));
        
        let agent_count = self.agents.len();
        stats.insert("agent_count".to_string(), serde_json::Value::Number(agent_count.into()));
        
        let conversation_length = self.conversation_length().await?;
        stats.insert("conversation_length".to_string(), serde_json::Value::Number(conversation_length.into()));
        
        let agent_names: Vec<serde_json::Value> = self.get_agent_names()
            .into_iter()
            .map(|name| serde_json::Value::String(name))
            .collect();
        stats.insert("agent_names".to_string(), serde_json::Value::Array(agent_names));
        
        stats.insert("director_name".to_string(), serde_json::Value::String(self.director.agent_name.clone()));
        stats.insert("director_model".to_string(), serde_json::Value::String(self.director.model_name.clone()));
        
        if let Some(planning_agent) = &self.planning_director_agent {
            stats.insert("planning_director_name".to_string(), serde_json::Value::String(planning_agent.agent_name.clone()));
            stats.insert("planning_director_model".to_string(), serde_json::Value::String(planning_agent.model_name.clone()));
        } else {
            stats.insert("planning_director_name".to_string(), serde_json::Value::Null);
            stats.insert("planning_director_model".to_string(), serde_json::Value::Null);
        }

        Ok(stats)
    }

    /// Find agent by name with flexible matching
    fn find_agent_by_name(&self, agent_name: &str) -> Result<&AgentSpec> {
        // Clean the agent name first
        let clean_agent_name = agent_name
            .replace("*", "")
            .replace("#", "")
            .replace("**", "")
            .trim()
            .to_string();
        
        // First try exact match with cleaned name
        if let Some(agent) = self.agents.iter().find(|a| a.agent_name == clean_agent_name) {
            return Ok(agent);
        }

        // Try partial matches
        let agent_name_lower = clean_agent_name.to_lowercase();
        
        // Map common patterns to actual agent names
        let agent_mappings = [
            ("research", "Research Coordinator"),
            ("coordinator", "Research Coordinator"),
            ("technology", "Technology Researcher"),
            ("tech", "Technology Researcher"),
            ("economic", "Economic Analyst"),
            ("economy", "Economic Analyst"),
            ("ethics", "Ethics Specialist"),
            ("ethical", "Ethics Specialist"),
            ("future", "Future Trends Analyst"),
            ("trends", "Future Trends Analyst"),
            ("trend", "Future Trends Analyst"),
            ("agent a", "Research Coordinator"),
            ("agent b", "Technology Researcher"),
            ("agent c", "Economic Analyst"),
            ("agent d", "Ethics Specialist"),
            ("agent e", "Future Trends Analyst"),
            ("agent f", "Future Trends Analyst"), // Map Agent F to Future Trends Analyst
        ];

        for (keyword, actual_name) in &agent_mappings {
            if agent_name_lower.contains(keyword) {
                if let Some(agent) = self.agents.iter().find(|a| a.agent_name == *actual_name) {
                    return Ok(agent);
                }
            }
        }

        // Try fuzzy matching on agent names
        for agent in &self.agents {
            let agent_name_lower_actual = agent.agent_name.to_lowercase();
            if agent_name_lower.contains(&agent_name_lower_actual) || 
               agent_name_lower_actual.contains(&agent_name_lower) {
                return Ok(agent);
            }
        }

        // If no match found, return error with available agents
        let available_agents: Vec<String> = self.agents
            .iter()
            .map(|a| a.agent_name.clone())
            .collect();
        Err(HierarchicalSwarmError::AgentNotFound {
            agent_name: clean_agent_name,
            available_agents,
        })
    }
}

// ================================================================================================
// BUILDER PATTERN
// ================================================================================================

/// Builder for creating HierarchicalSwarm instances
pub struct HierarchicalSwarmBuilder {
    name: String,
    description: String,
    director: Option<AgentSpec>,
    agents: Vec<AgentSpec>,
    max_loops: u32,
    output_type: String,
    feedback_director_model_name: String,
    director_name: String,
    director_model_name: String,
    verbose: bool,
    markdown_enabled: bool,
    add_collaboration_prompt: bool,
    planning_director_agent: Option<AgentSpec>,
    director_feedback_on: bool,
    interactive: bool,
    dashboard_callback: Option<Arc<dyn Fn(DashboardUpdate) + Send + Sync>>,
    sequential_execution: bool,
    client: Option<SwarmsClient>,
}

impl HierarchicalSwarmBuilder {
    /// Creates a new builder with default values
    pub fn new() -> Self {
        Self {
            name: "HierarchicalAgentSwarm".to_string(),
            description: "Distributed task swarm".to_string(),
            director: None,
            agents: Vec::new(),
            max_loops: 1,
            output_type: "dict-all-except-first".to_string(),
            feedback_director_model_name: "gpt-4o-mini".to_string(),
            director_name: "Director".to_string(),
            director_model_name: "gpt-4o-mini".to_string(),
            verbose: false,
            markdown_enabled: false,
            add_collaboration_prompt: true,
            planning_director_agent: None,
            director_feedback_on: true,
            interactive: false,
            dashboard_callback: None,
            sequential_execution: false, // Default to parallel execution for speed
            client: None,
        }
    }

    /// Sets the swarm name
    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = name.into();
        self
    }

    /// Sets the swarm description
    pub fn description<S: Into<String>>(mut self, description: S) -> Self {
        self.description = description.into();
        self
    }

    /// Sets the director agent
    pub fn director(mut self, director: AgentSpec) -> Self {
        self.director = Some(director);
        self
    }

    /// Adds an agent to the swarm
    pub fn agent(mut self, agent: AgentSpec) -> Self {
        self.agents.push(agent);
        self
    }

    /// Sets the maximum number of loops
    pub fn max_loops(mut self, max_loops: u32) -> Self {
        self.max_loops = max_loops;
        self
    }

    /// Sets the output type
    pub fn output_type<S: Into<String>>(mut self, output_type: S) -> Self {
        self.output_type = output_type.into();
        self
    }

    /// Sets the feedback director model name
    pub fn feedback_director_model_name<S: Into<String>>(mut self, model_name: S) -> Self {
        self.feedback_director_model_name = model_name.into();
        self
    }

    /// Sets the director name
    pub fn director_name<S: Into<String>>(mut self, director_name: S) -> Self {
        self.director_name = director_name.into();
        self
    }

    /// Sets the director model name
    pub fn director_model_name<S: Into<String>>(mut self, model_name: S) -> Self {
        self.director_model_name = model_name.into();
        self
    }

    /// Sets verbose mode
    pub fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Enables markdown rendering for agent outputs
    pub fn md(self, _enabled: bool) -> Self {
        // Markdown functionality removed from core implementation
        // This method exists for API compatibility only
        self
    }

    /// Sets whether to add collaboration prompt
    pub fn add_collaboration_prompt(mut self, add: bool) -> Self {
        self.add_collaboration_prompt = add;
        self
    }

    /// Sets the planning director agent
    pub fn planning_director_agent(mut self, agent: AgentSpec) -> Self {
        self.planning_director_agent = Some(agent);
        self
    }

    /// Sets whether director feedback is enabled
    pub fn director_feedback_on(mut self, enabled: bool) -> Self {
        self.director_feedback_on = enabled;
        self
    }

    /// Enables interactive dashboard with real-time monitoring
    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    /// Sets a dashboard callback for real-time monitoring
    pub fn dashboard_callback<F>(mut self, callback: F) -> Self 
    where
        F: Fn(DashboardUpdate) + Send + Sync + 'static,
    {
        self.dashboard_callback = Some(Arc::new(callback));
        self
    }

    /// Sets whether to execute agents sequentially with memory (true) or in parallel (false)
    pub fn sequential_execution(mut self, sequential: bool) -> Self {
        self.sequential_execution = sequential;
        self
    }



    /// Sets the Swarms API client
    pub fn client(mut self, client: SwarmsClient) -> Self {
        self.client = Some(client);
        self
    }

    /// Builds the HierarchicalSwarm
    pub fn build(self) -> Result<HierarchicalSwarm> {
        let client = self.client.ok_or_else(|| {
            HierarchicalSwarmError::General("SwarmsClient is required".to_string())
        })?;

        let director = self.director.unwrap_or_else(|| {
            let agent_names = self.agents.iter()
                .map(|a| a.agent_name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            
            AgentSpec {
                agent_name: self.director_name.clone(),
                description: Some("A director agent that can create a plan and distribute orders to agents".to_string()),
                system_prompt: Some(format!(
                    "You are a director agent that orchestrates tasks among worker agents. \
                    Available agents: {}. \
                    Create detailed plans and distribute specific orders to these agents only. \
                    Do not reference agents that are not in this list. \
                    Use the exact agent names when assigning tasks.",
                    agent_names
                )),
                model_name: self.director_model_name.clone(),
                auto_generate_prompt: false,
                max_tokens: 8192,
                temperature: 0.3,
                role: Some("director".to_string()),
                max_loops: 1,
                tools_dictionary: None,
                markdown: false,
            }
        });

        HierarchicalSwarm::new(
            self.name,
            self.description,
            director,
            self.agents,
            self.max_loops,
            self.output_type,
            self.feedback_director_model_name,
            self.director_name,
            self.director_model_name,
            self.verbose,
            self.add_collaboration_prompt,
            self.planning_director_agent,
            self.director_feedback_on,
            self.interactive,
            self.dashboard_callback,
            self.sequential_execution,
            client,
        )
    }
}

impl Default for HierarchicalSwarmBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ================================================================================================
// CONVERSION IMPLEMENTATIONS
// ================================================================================================

// No external conversions needed since we're self-contained

// ================================================================================================
// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_agent(name: &str) -> AgentSpec {
        AgentSpec {
            agent_name: name.to_string(),
            description: Some(format!("Test agent: {}", name)),
            system_prompt: Some(format!("You are a test agent named {}", name)),
            model_name: "gpt-4o-mini".to_string(),
            auto_generate_prompt: false,
            max_tokens: 1000,
            temperature: 0.1,
            role: Some("test".to_string()),
            max_loops: 1,
            tools_dictionary: None,
        }
    }

    fn create_test_director() -> AgentSpec {
        AgentSpec {
            agent_name: "Test Director".to_string(),
            description: Some("Test director agent".to_string()),
            system_prompt: Some("You are a test director agent".to_string()),
            model_name: "gpt-4o-mini".to_string(),
            auto_generate_prompt: false,
            max_tokens: 2000,
            temperature: 0.1,
            role: Some("director".to_string()),
            max_loops: 1,
            tools_dictionary: None,
        }
    }

    async fn create_test_client() -> SwarmsClient {
        // Note: This would require a real API key for actual testing
        // For unit tests, we'll create a mock client or skip API-dependent tests
        SwarmsClient::builder()
            .unwrap()
            .api_key("test-key")
            .timeout(std::time::Duration::from_secs(5))
            .max_retries(1)
            .build()
            .expect("Failed to create test client")
    }

    #[tokio::test]
    async fn test_hierarchical_swarm_creation() {
        let client = create_test_client().await;
        let director = create_test_director();
        let agent1 = create_test_agent("Agent1");
        let agent2 = create_test_agent("Agent2");

        let swarm = HierarchicalSwarm::new(
            "Test Swarm".to_string(),
            "Test description".to_string(),
            director,
            vec![agent1, agent2],
            2,
            "dict-all-except-first".to_string(),
            "gpt-4o-mini".to_string(),
            "Director".to_string(),
            "gpt-4o-mini".to_string(),
            true,
            true,
            None,
            true,
            false, // interactive
            None,  // dashboard_callback
            client,
        );

        assert!(swarm.is_ok());
        let swarm = swarm.unwrap();
        assert_eq!(swarm.name, "Test Swarm");
        assert_eq!(swarm.agents.len(), 2);
        assert_eq!(swarm.max_loops, 2);
    }

    #[tokio::test]
    async fn test_hierarchical_swarm_builder() {
        let client = create_test_client().await;
        let director = create_test_director();
        let agent1 = create_test_agent("Agent1");
        let agent2 = create_test_agent("Agent2");

        let swarm = HierarchicalSwarmBuilder::new()
            .name("Test Swarm")
            .description("Test description")
            .director(director)
            .agent(agent1)
            .agent(agent2)
            .max_loops(3)
            .verbose(true)
            .client(client)
            .build();

        assert!(swarm.is_ok());
        let swarm = swarm.unwrap();
        assert_eq!(swarm.name, "Test Swarm");
        assert_eq!(swarm.agents.len(), 2);
        assert_eq!(swarm.max_loops, 3);
        assert!(swarm.verbose);
    }

    #[tokio::test]
    async fn test_validation_errors() {
        let client = create_test_client().await;
        let director = create_test_director();

        // Test no agents error
        let swarm = HierarchicalSwarm::new(
            "Test Swarm".to_string(),
            "Test description".to_string(),
            director.clone(),
            vec![], // Empty agents
            2,
            "dict-all-except-first".to_string(),
            "gpt-4o-mini".to_string(),
            "Director".to_string(),
            "gpt-4o-mini".to_string(),
            true,
            true,
            None,
            true,
            false, // interactive
            None,  // dashboard_callback
            client.clone(),
        );

        assert!(swarm.is_err());
        match swarm.unwrap_err() {
            HierarchicalSwarmError::NoAgents => {},
            _ => panic!("Expected NoAgents error"),
        }

        // Test invalid max loops
        let swarm = HierarchicalSwarm::new(
            "Test Swarm".to_string(),
            "Test description".to_string(),
            director,
            vec![create_test_agent("Agent1")],
            0, // Invalid max loops
            "dict-all-except-first".to_string(),
            "gpt-4o-mini".to_string(),
            "Director".to_string(),
            "gpt-4o-mini".to_string(),
            true,
            true,
            None,
            true,
            false, // interactive
            None,  // dashboard_callback
            client,
        );

        assert!(swarm.is_err());
        match swarm.unwrap_err() {
            HierarchicalSwarmError::InvalidMaxLoops => {},
            _ => panic!("Expected InvalidMaxLoops error"),
        }
    }

    #[tokio::test]
    async fn test_agent_management() {
        let client = create_test_client().await;
        let director = create_test_director();
        let agent1 = create_test_agent("Agent1");
        let agent2 = create_test_agent("Agent2");

        let swarm = HierarchicalSwarm::new(
            "Test Swarm".to_string(),
            "Test description".to_string(),
            director,
            vec![agent1, agent2],
            2,
            "dict-all-except-first".to_string(),
            "gpt-4o-mini".to_string(),
            "Director".to_string(),
            "gpt-4o-mini".to_string(),
            true,
            true,
            None,
            true,
            false, // interactive
            None,  // dashboard_callback
            client,
        ).unwrap();

        // Test agent names
        let agent_names = swarm.get_agent_names();
        assert_eq!(agent_names.len(), 2);
        assert!(agent_names.contains(&"Agent1".to_string()));
        assert!(agent_names.contains(&"Agent2".to_string()));

        // Test agent existence
        assert!(swarm.has_agent("Agent1"));
        assert!(swarm.has_agent("Agent2"));
        assert!(!swarm.has_agent("NonExistentAgent"));

        // Test get agent
        let agent = swarm.get_agent("Agent1");
        assert!(agent.is_some());
        assert_eq!(agent.unwrap().agent_name, "Agent1");

        let agent = swarm.get_agent("NonExistentAgent");
        assert!(agent.is_none());
    }

    #[tokio::test]
    async fn test_conversation_management() {
        let client = create_test_client().await;
        let director = create_test_director();

        let swarm = HierarchicalSwarm::new(
            "Test Swarm".to_string(),
            "Test description".to_string(),
            director,
            vec![create_test_agent("Agent1")],
            1,
            "dict-all-except-first".to_string(),
            "gpt-4o-mini".to_string(),
            "Director".to_string(),
            "gpt-4o-mini".to_string(),
            true,
            true,
            None,
            true,
            false, // interactive
            None,  // dashboard_callback
            client,
        ).unwrap();

        // Test initial conversation length
        let length = swarm.conversation_length().await.unwrap();
        assert_eq!(length, 0);

        // Test adding to conversation
        swarm.add_to_conversation("TestRole", "Test content").await.unwrap();
        
        let length = swarm.conversation_length().await.unwrap();
        assert_eq!(length, 1);

        // Test getting conversation string
        let conv_str = swarm.get_conversation_str().await.unwrap();
        assert!(conv_str.contains("TestRole"));
        assert!(conv_str.contains("Test content"));

        // Test getting last message
        let last_msg = swarm.get_last_message().await.unwrap();
        assert!(last_msg.is_some());
        let last_msg = last_msg.unwrap();
        assert_eq!(last_msg.get("role").unwrap().as_str().unwrap(), "TestRole");
        assert_eq!(last_msg.get("content").unwrap().as_str().unwrap(), "Test content");

        // Test getting messages by role
        let messages = swarm.get_messages_by_role("TestRole").await.unwrap();
        assert_eq!(messages.len(), 1);

        let messages = swarm.get_messages_by_role("NonExistentRole").await.unwrap();
        assert_eq!(messages.len(), 0);

        // Test clearing conversation
        swarm.clear_conversation().await.unwrap();
        let length = swarm.conversation_length().await.unwrap();
        assert_eq!(length, 0);
    }

    #[tokio::test]
    async fn test_configuration_validation() {
        let client = create_test_client().await;
        let director = create_test_director();

        let swarm = HierarchicalSwarm::new(
            "Test Swarm".to_string(),
            "Test description".to_string(),
            director,
            vec![create_test_agent("Agent1")],
            1,
            "dict-all-except-first".to_string(),
            "gpt-4o-mini".to_string(),
            "Director".to_string(),
            "gpt-4o-mini".to_string(),
            true,
            true,
            None,
            true,
            false, // interactive
            None,  // dashboard_callback
            client,
        ).unwrap();

        // Test valid configuration
        let result = swarm.validate_configuration();
        assert!(result.is_ok());

        // Test configuration string
        let config = swarm.get_configuration();
        assert!(config.contains("Test Swarm"));
        assert!(config.contains("Test description"));
        assert!(config.contains("1")); // max_loops
    }

    #[tokio::test]
    async fn test_statistics() {
        let client = create_test_client().await;
        let director = create_test_director();

        let swarm = HierarchicalSwarm::new(
            "Test Swarm".to_string(),
            "Test description".to_string(),
            director,
            vec![create_test_agent("Agent1")],
            1,
            "dict-all-except-first".to_string(),
            "gpt-4o-mini".to_string(),
            "Director".to_string(),
            "gpt-4o-mini".to_string(),
            true,
            true,
            None,
            true,
            false, // interactive
            None,  // dashboard_callback
            client,
        ).unwrap();

        let stats = swarm.get_statistics().await.unwrap();
        
        assert_eq!(stats.get("name").unwrap().as_str().unwrap(), "Test Swarm");
        assert_eq!(stats.get("description").unwrap().as_str().unwrap(), "Test description");
        assert_eq!(stats.get("max_loops").unwrap().as_u64().unwrap(), 1);
        assert_eq!(stats.get("agent_count").unwrap().as_u64().unwrap(), 1);
        assert!(stats.get("verbose").unwrap().as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_reset_functionality() {
        let client = create_test_client().await;
        let director = create_test_director();

        let swarm = HierarchicalSwarm::new(
            "Test Swarm".to_string(),
            "Test description".to_string(),
            director,
            vec![create_test_agent("Agent1")],
            1,
            "dict-all-except-first".to_string(),
            "gpt-4o-mini".to_string(),
            "Director".to_string(),
            "gpt-4o-mini".to_string(),
            true,
            true,
            None,
            true,
            false, // interactive
            None,  // dashboard_callback
            client,
        ).unwrap();

        // Add some conversation
        swarm.add_to_conversation("TestRole", "Test content").await.unwrap();
        assert_eq!(swarm.conversation_length().await.unwrap(), 1);

        // Reset
        swarm.reset().await.unwrap();
        assert_eq!(swarm.conversation_length().await.unwrap(), 0);
        assert_eq!(swarm.get_conversation_str().await.unwrap(), "");
    }

    #[test]
    fn test_hierarchical_order_serialization() {
        let order = HierarchicalOrder {
            agent_name: "TestAgent".to_string(),
            task: "Test task".to_string(),
        };

        let json = serde_json::to_string(&order).unwrap();
        let deserialized: HierarchicalOrder = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.agent_name, "TestAgent");
        assert_eq!(deserialized.task, "Test task");
    }

    #[test]
    fn test_swarm_spec_response_serialization() {
        let spec = SwarmSpecResponse {
            plan: "Test plan".to_string(),
            orders: vec![
                HierarchicalOrder {
                    agent_name: "Agent1".to_string(),
                    task: "Task1".to_string(),
                },
                HierarchicalOrder {
                    agent_name: "Agent2".to_string(),
                    task: "Task2".to_string(),
                },
            ],
        };

        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: SwarmSpecResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.plan, "Test plan");
        assert_eq!(deserialized.orders.len(), 2);
        assert_eq!(deserialized.orders[0].agent_name, "Agent1");
        assert_eq!(deserialized.orders[1].agent_name, "Agent2");
    }

    #[test]
    fn test_error_conversion() {
        let error = HierarchicalSwarmError::NoAgents;
        // Test that the error can be converted to string
        let error_string = error.to_string();
        assert!(error_string.contains("No agents found"));
    }
} 
