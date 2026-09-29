use dashmap::DashMap;
use serde::Deserialize;

use crate::agent::SwarmsAgent;
use crate::llm::Model;
use crate::llm::provider::openai::OpenAI;
use crate::prompts::multi_agent_collab_prompt::MULTI_AGENT_COLLAB_PROMPT;
use crate::structs::agent::Agent;
use crate::structs::concurrent_workflow::ConcurrentWorkflow;
use crate::structs::concurrent_workflow::ConcurrentWorkflowError;
use crate::structs::conversation::AgentConversation;
use crate::structs::rearrange::AgentRearrange;
use crate::structs::sequential_workflow::SequentialWorkflow;
use crate::structs::sequential_workflow::SequentialWorkflowError;

/// The different allowed types of Swarms
#[derive(Debug, Deserialize)]
pub enum SwarmType {
    SequentialWorkflow,
    ConcurrentWorkflow,
    AgentRearrange,
}

/// Configuration model for SwarmsRouter.
///
/// Generic over the agents' model, so agents built on any provider (Anthropic, OpenRouter,
/// `AnyModel`, ...) can be routed; it defaults to `OpenAI`.
pub struct SwarmRouterConfig<M = OpenAI>
where
    M: Model + Clone + Send + Sync + 'static,
    M::RawCompletionResponse: Clone + Send + Sync,
{
    /// Name identifier for the SwarmRouter instance.
    pub name: String,

    /// Description of the SwarmRouter's purpose.
    pub description: String,

    /// Type of swarm to use.
    pub swarm_type: SwarmType,

    /// List of the agents to use.
    pub agents: Vec<SwarmsAgent<M>>,

    /// Rules to inject in every agent
    pub rules: Option<String>,

    /// Whether to enable multi-agent collaboration prompts
    pub multi_agent_collab_prompt: bool,

    /// Flow pattern for AgentRearrange (only used when swarm_type is AgentRearrange)
    pub flow: Option<String>,

    /// Maximum loops for AgentRearrange (only used when swarm_type is AgentRearrange)
    pub max_loops: Option<u32>,
}

// Only for the default model, so `SwarmRouterConfig::default()` keeps inferring `OpenAI`;
// for other models use `SwarmRouterConfig::with_agents`.
impl Default for SwarmRouterConfig<OpenAI> {
    fn default() -> Self {
        Self::with_agents(Vec::new())
    }
}

impl<M> SwarmRouterConfig<M>
where
    M: Model + Clone + Send + Sync + 'static,
    M::RawCompletionResponse: Clone + Send + Sync,
{
    /// A config with the default settings for agents on any model, e.g.
    /// `SwarmRouterConfig::with_agents(vec![claude_agent])`.
    pub fn with_agents(agents: Vec<SwarmsAgent<M>>) -> Self {
        SwarmRouterConfig {
            name: String::from("swarm-router"),
            description: String::from("Routes your task to the desired swarm"),
            swarm_type: SwarmType::SequentialWorkflow,
            agents,
            rules: None,
            multi_agent_collab_prompt: true,
            flow: None,
            max_loops: None,
        }
    }

    /// Ensure that all preconditions are met.
    fn validate(&self) -> Result<(), SwarmRouterError> {
        tracing::info!("Initializing reliability checks");
        self.validate_agents()?;
        tracing::info!("Reliability checks completed your swarm is ready");
        Ok(())
    }

    /// Append the different rules provided by the user at the end of the prompt.
    fn handle_rules(&mut self) {
        let rules = match self.rules.as_deref() {
            Some(rules) => rules,
            None => return,
        };

        tracing::info!("Injecting rules to every agent!");
        let agents = std::mem::take(&mut self.agents);
        self.agents = agents
            .into_iter()
            .map(|agent| {
                let system_prompt = agent.get_system_prompt().unwrap_or("");
                let new_system_prompt = format!("{system_prompt}\n### SWARM RULES ###\n{rules}");
                agent.system_prompt(new_system_prompt)
            })
            .collect();
        tracing::info!("Finished injecting rules");
    }

    /// Activate automatic prompt engineering for agents that support it
    fn update_system_prompt_for_agent_in_swarm(&mut self) {
        tracing::info!("Injecting multi-agent prompt to every agent!");
        let agents = std::mem::take(&mut self.agents);
        self.agents = agents
            .into_iter()
            .map(|agent| {
                let system_prompt = agent.get_system_prompt().unwrap_or("");
                let new_system_prompt = format!("{system_prompt}\n{MULTI_AGENT_COLLAB_PROMPT}");
                agent.system_prompt(new_system_prompt)
            })
            .collect();
        tracing::info!("Finished injecting multi-agent prompt");
    }

    /// Validate that agents are valid
    fn validate_agents(&self) -> Result<(), SwarmRouterError> {
        if self.agents.is_empty() {
            return Err(SwarmRouterError::ValidationError(String::from(
                "No agents provided for the swarm.",
            )));
        }

        Ok(())
    }
}

/// Runs tasks on the swarm type chosen in [`SwarmRouterConfig::swarm_type`].
/// The SwarmRouter builds that structure from the config, injects rules and collaboration prompts into
/// every agent, and handles task execution and logging.
/// Available Swarm Types:
///     - SequentialWorkflow: Executes tasks sequentially
///     - ConcurrentWorkflow: Executes tasks in parallel
///     - AgentRearrange: Executes tasks with custom flow patterns
pub enum SwarmRouter {
    SequentialWorkflow(SequentialWorkflow),
    ConcurrentWorkflow(ConcurrentWorkflow),
    AgentRearrange(AgentRearrange),
}

impl SwarmRouter {
    /// Create a SwarmRouter from a SwarmConfig.
    ///
    ///  # Params
    ///
    ///  -  config: The configuration to initialize the SwarmRouter
    ///
    ///  # Returns:
    ///
    ///  - The configured and ready to execute SwarmRouter
    ///
    ///  # Error
    ///     
    ///  - SwarmRouterError::ValidationError: If fails during config validation
    pub fn new_with_config<M>(config: SwarmRouterConfig<M>) -> Result<SwarmRouter, SwarmRouterError>
    where
        M: Model + Clone + Send + Sync + 'static,
        M::RawCompletionResponse: Clone + Send + Sync,
    {
        config.validate()?;

        tracing::info!(
            "SwarmRouter initialized with swarm type: {:?}",
            config.swarm_type
        );

        let mut config = config;
        config.handle_rules();

        if config.multi_agent_collab_prompt {
            config.update_system_prompt_for_agent_in_swarm();
        }

        let swarm_router = SwarmRouter::create_swarm_router(config);
        Ok(swarm_router)
    }

    ///  Execute a task on the configured swarm type.
    ///
    ///  # Params
    ///
    ///  -  task: The task to be executed by the swarm.
    ///
    ///  # Returns:
    ///
    ///  - The result of the swarm's execution.
    ///
    ///  # Error
    ///     
    ///  - SwarmRouterError::SequentialWorkflow: If fails during execution of sequential workflow
    ///  - SwarmRouterError::ConcurrentWorkflow: If fails during execution of concurrent workflow
    ///  - SwarmRouterError::AgentRearrange: If fails during execution of agent rearrange
    pub async fn run(&self, task: &str) -> Result<AgentConversation, SwarmRouterError> {
        let result = self.inner_run(task).await;

        if let Err(err) = &result {
            tracing::error!("Error executing task on swarm: {err}");
        }

        result
    }

    /// Execute a batch of tasks on the configured swarm type.
    ///
    /// # Params
    ///
    /// - tasks: A list of tasks to be executed by the swarm.
    ///
    /// # Returns:
    ///
    /// - A list of results from the swarm's execution.
    ///
    /// # Error
    ///    
    /// - SwarmRouterError::SequentialWorkflow: If fails during execution of sequential workflow for any of the tasks
    /// - SwarmRouterError::ConcurrentWorkflow: If fails during execution of concurrent workflow for any of the tasks
    pub async fn batch_run(
        &self,
        tasks: Vec<String>,
    ) -> Result<DashMap<String, AgentConversation>, SwarmRouterError> {
        let result = self.inner_batch_run(tasks).await;

        if let Err(err) = &result {
            tracing::error!("Error executing task on swarm: {err}");
        }

        result
    }

    async fn inner_run(&self, task: &str) -> Result<AgentConversation, SwarmRouterError> {
        tracing::info!("Running task on {:?} swarm with task: {task}", self.kind());
        let result = match self {
            SwarmRouter::SequentialWorkflow(wf) => wf.run(task).await?,
            SwarmRouter::ConcurrentWorkflow(wf) => wf.run(task).await?,
            SwarmRouter::AgentRearrange(ar) => ar.run_to_conversation(task).await?,
        };
        tracing::info!("Swarm completed successfully");

        Ok(result)
    }

    async fn inner_batch_run(
        &self,
        tasks: Vec<String>,
    ) -> Result<DashMap<String, AgentConversation>, SwarmRouterError> {
        tracing::info!("Running batch tasks on {:?} swarm", self.kind());
        let result = match self {
            SwarmRouter::SequentialWorkflow(wf) => {
                let results = DashMap::with_capacity(tasks.len());
                for task in tasks {
                    let result = wf.run(&task).await?;
                    results.insert(task, result);
                }
                results
            },
            SwarmRouter::ConcurrentWorkflow(wf) => wf.run_batch(tasks).await?,
            SwarmRouter::AgentRearrange(ar) => {
                let results = DashMap::with_capacity(tasks.len());
                for task in tasks {
                    let result = ar.run_to_conversation(task.as_str()).await?;
                    results.insert(task, result);
                }
                results
            },
        };
        tracing::info!("Swarm completed successfully");

        Ok(result)
    }

    fn kind(&self) -> SwarmType {
        match self {
            SwarmRouter::SequentialWorkflow(_) => SwarmType::SequentialWorkflow,
            SwarmRouter::ConcurrentWorkflow(_) => SwarmType::ConcurrentWorkflow,
            SwarmRouter::AgentRearrange(_) => SwarmType::AgentRearrange,
        }
    }

    fn create_swarm_router<M>(config: SwarmRouterConfig<M>) -> SwarmRouter
    where
        M: Model + Clone + Send + Sync + 'static,
        M::RawCompletionResponse: Clone + Send + Sync,
    {
        let agents = config
            .agents
            .into_iter()
            .map(|agent| Box::new(agent) as Box<dyn Agent>)
            .collect::<Vec<_>>();

        match config.swarm_type {
            SwarmType::SequentialWorkflow => {
                let workflow = SequentialWorkflow::builder()
                    .name(config.name)
                    .description(config.description)
                    .agents(agents)
                    .build();
                SwarmRouter::SequentialWorkflow(workflow)
            },
            SwarmType::ConcurrentWorkflow => {
                let workflow = ConcurrentWorkflow::builder()
                    .name(config.name)
                    .description(config.description)
                    .agents(agents)
                    .build();
                SwarmRouter::ConcurrentWorkflow(workflow)
            },
            SwarmType::AgentRearrange => {
                let mut builder = AgentRearrange::builder()
                    .name(config.name)
                    .description(config.description)
                    .agents(agents);

                if let Some(flow) = config.flow {
                    builder = builder.flow(flow);
                }

                if let Some(max_loops) = config.max_loops {
                    builder = builder.max_loops(max_loops);
                }

                // Rules were already added to every agent's system prompt (handle_rules);
                // passing them to the rearrange too would repeat them in the conversation.

                let rearrange = builder.build();
                SwarmRouter::AgentRearrange(rearrange)
            },
        }
    }
}

/// Create and run a SwarmRouter instance with the given configuration.
///
/// # Params
///
/// - task: Task to execute.
/// - config: The SwarmConfig used to create the SwarmRouter
///
/// # Returns
///
/// - Result from executing the swarm router
///
/// # Error
///
/// - SwarmRouterError::ValidationError: If fails during config validation
/// - SwarmRouterError::SequentialWorkflow: If fails during execution of sequential workflow
/// - SwarmRouterError::ConcurrentWorkflow: If fails during execution of concurrent workflow
pub async fn swarm_router<M>(
    task: &str,
    config: SwarmRouterConfig<M>,
) -> Result<AgentConversation, SwarmRouterError>
where
    M: Model + Clone + Send + Sync + 'static,
    M::RawCompletionResponse: Clone + Send + Sync,
{
    tracing::info!(
        "Creating SwarmRouter with name: {}, swarm_type: {:?}",
        config.name,
        config.swarm_type
    );

    let router = SwarmRouter::new_with_config(config)?;
    tracing::info!("Executing task with SwarmRouter: {}", task);

    let result = router.run(task).await?;
    tracing::info!("Task execution completed successfully");

    Ok(result)
}

#[derive(Debug, thiserror::Error)]
pub enum SwarmRouterError {
    #[error("SwarmRouter validation error: {0}")]
    ValidationError(String),

    #[error(transparent)]
    SequentialWorkflowError(#[from] SequentialWorkflowError),

    #[error(transparent)]
    ConcurrentWorkflowError(#[from] ConcurrentWorkflowError),

    #[error(transparent)]
    AgentRearrangeError(#[from] crate::structs::rearrange::AgentRearrangeError),
}
