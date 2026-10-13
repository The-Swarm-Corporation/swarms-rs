//! The agent engine.
//!
//! The agent is a swarms-rs [`SwarmsAgent`] carrying the coding tools. Each model call goes
//! through `SwarmsAgent::chat`, which sends the conversation, runs any tool calls the model
//! makes, and returns their results. [`Engine`] drives `chat` in a loop until the model
//! answers without calling a tool, and keeps the conversation across messages.
//!
//! The model is an [`ObservedModel`]: swarms-rs's provider-agnostic [`AnyModel`] wrapped so
//! that each response is seen before swarms-rs runs its tool calls. That is what lets the
//! agent's text print ahead of its tool activity, and lets the history record each tool call
//! with its real ID, so the next request carries proper provider-native tool-use turns.

use std::{
    ops::AddAssign,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow, bail};
use futures::future::BoxFuture;
use serde_json::Value;
use swarms_rs::{
    agent::{ChatResponse, SwarmsAgent, SwarmsAgentBuilder, ToolCallOutput},
    llm::{
        CompletionError, Model,
        completion::{AssistantContent, Message, ToolResultContent, UserContent},
        provider::any::AnyModel,
        request::{CompletionRequest, CompletionResponse},
    },
    structs::agent::AgentError,
};

use crate::{
    effort::{self, Effort, ROOMY_MAX_TOKENS},
    lock,
    models::{self, Backend, CatalogModel, Resolved},
    prompt,
    session::Session,
    tools,
    ui::AGENT_NAME,
};

const MAX_RETRIES: u32 = 3;
/// The output limit for each model call when `--max-tokens` isn't given.
pub const DEFAULT_MAX_TOKENS: u64 = 32_000;

#[derive(Debug, Clone, Copy, Default)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
}

impl Usage {
    /// Token counts from a provider's raw response, under either provider family's field names.
    fn from_raw(raw: &Value) -> Self {
        let usage = &raw["usage"];
        let field = |names: [&str; 2]| {
            names
                .iter()
                .find_map(|name| usage[name].as_u64())
                .unwrap_or(0)
        };
        Self {
            input: field(["input_tokens", "prompt_tokens"]),
            output: field(["output_tokens", "completion_tokens"]),
        }
    }
}

impl AddAssign for Usage {
    fn add_assign(&mut self, other: Self) {
        self.input += other.input;
        self.output += other.output;
    }
}

/// What the model returned for the latest call, recorded before swarms-rs runs the tools.
#[derive(Default)]
struct Capture {
    choice: Option<Vec<AssistantContent>>,
    usage: Usage,
}

/// [`AnyModel`] with a spinner while it thinks, and a hook that shows the model's text and
/// records its response as soon as it arrives.
#[derive(Clone)]
pub struct ObservedModel {
    inner: AnyModel,
    session: Arc<Session>,
    capture: Arc<Mutex<Capture>>,
}

impl Model for ObservedModel {
    type RawCompletionResponse = Value;

    fn completion(
        &self,
        request: CompletionRequest,
    ) -> BoxFuture<'_, Result<CompletionResponse<Value>, CompletionError>> {
        Box::pin(async move {
            let response = {
                let _spinner = self.session.ui.spinner("Thinking");
                self.inner.completion(request).await?
            };
            {
                let mut capture = lock(&self.capture);
                capture.usage += Usage::from_raw(&response.raw_response);
                capture.choice = Some(response.choice.clone());
            }
            let is_final = !response
                .choice
                .iter()
                .any(|part| matches!(part, AssistantContent::ToolCall(_)));
            for part in &response.choice {
                if let AssistantContent::Text(text) = part {
                    self.session.ui.agent_message(&text.text, is_final);
                }
            }
            Ok(response)
        })
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// The model answered without calling another tool.
    #[default]
    Finished,
    /// The user declined a tool call.
    Declined,
    /// The turn used `max_steps` model calls.
    StepLimit,
}

#[derive(Debug, Default)]
pub struct TurnOutcome {
    /// The model's final reply, if it wrote one.
    pub answer: Option<String>,
    pub tool_calls: usize,
    pub usage: Usage,
    pub elapsed: Duration,
    pub ending: Ending,
}

/// OpenRouter's catalog, fetched at most once per session, in the background.
enum Catalog {
    NotLoaded,
    Loading(tokio::task::JoinHandle<Result<Vec<CatalogModel>>>),
    Ready(Arc<[CatalogModel]>),
}

pub struct Engine {
    session: Arc<Session>,
    capture: Arc<Mutex<Capture>>,
    /// The agent, or why it couldn't be set up (such as a missing API key). The chat still
    /// starts without one, so the welcome screen can say what's missing.
    agent: Result<SwarmsAgent<ObservedModel>, String>,
    model: Resolved,
    /// `--max-tokens`, if given. Without it, calls allow [`DEFAULT_MAX_TOKENS`], or
    /// [`ROOMY_MAX_TOKENS`] at the highest efforts; models that allow less get less.
    max_tokens: Option<u64>,
    /// The effort chosen with `/effort` or `--effort`; `None` leaves it to the model.
    effort: Option<Effort>,
    /// The effort sent to the current model: the chosen one, fitted to what the model takes.
    applied_effort: Option<Effort>,
    catalog: Catalog,
    max_steps: usize,
    history: Vec<Message>,
}

impl Engine {
    /// Fails only for model names that don't resolve to any provider.
    pub fn new(
        session: Arc<Session>,
        model: &str,
        max_tokens: Option<u64>,
        effort: Option<Effort>,
        max_steps: usize,
    ) -> Result<Self> {
        let model = models::resolve(model, models::has_key).map_err(|problem| anyhow!(problem))?;
        let capture = Arc::default();
        // OpenRouter models are fitted to the catalog in `prepare`.
        let setup = setup(&model, None, max_tokens, effort);
        let agent = build_agent(&session, &capture, &model, &setup);
        let mut engine = Self {
            session,
            capture,
            agent,
            model,
            max_tokens,
            effort,
            applied_effort: setup.effort,
            catalog: Catalog::NotLoaded,
            max_steps: max_steps.max(1),
            history: Vec::new(),
        };
        if engine.model.backend == Backend::OpenRouter {
            // Load it while the welcome screen plays; `prepare` needs it.
            engine.catalog = Catalog::Loading(tokio::spawn(models::fetch_catalog()));
        }
        Ok(engine)
    }

    pub fn model(&self) -> &str {
        &self.model.name
    }

    pub fn backend(&self) -> Backend {
        self.model.backend
    }

    /// The effort the current model runs at, when one is set.
    pub fn applied_effort(&self) -> Option<Effort> {
        self.applied_effort
    }

    /// Why the agent can't run, if it can't.
    pub fn problem(&self) -> Option<&str> {
        self.agent.as_ref().err().map(String::as_str)
    }

    /// OpenRouter's catalog of models Swarms Code can use, loading it on first use.
    pub async fn catalog(&mut self) -> Result<Arc<[CatalogModel]>> {
        let task = match std::mem::replace(&mut self.catalog, Catalog::NotLoaded) {
            Catalog::Ready(models) => {
                self.catalog = Catalog::Ready(Arc::clone(&models));
                return Ok(models);
            },
            Catalog::Loading(task) => task,
            Catalog::NotLoaded => tokio::spawn(models::fetch_catalog()),
        };
        // On failure the catalog stays unloaded, so the next use tries again.
        let models: Arc<[CatalogModel]> = task.await??.into();
        self.catalog = Catalog::Ready(Arc::clone(&models));
        Ok(models)
    }

    /// Check the starting model against OpenRouter's catalog, and fit its output limit and
    /// effort to it. Returns a warning to show, if any.
    pub async fn prepare(&mut self) -> Option<String> {
        if self.agent.is_err() {
            return None;
        }
        let model = self.model.clone();
        match self.setup_for(&model).await {
            Ok(setup) => {
                self.agent = build_agent(&self.session, &self.capture, &model, &setup);
                self.applied_effort = setup.effort;
                setup.note
            },
            Err(problem) => Some(format!("{problem:#}")),
        }
    }

    /// Switch models mid-conversation; the history carries over. Returns a note to show, if
    /// any. The current model stays if the new one can't be used.
    pub async fn set_model(&mut self, name: &str) -> Result<Option<String>> {
        let model = models::resolve(name, models::has_key).map_err(|problem| anyhow!(problem))?;
        let setup = self.setup_for(&model).await?;
        self.agent = Ok(build_agent(&self.session, &self.capture, &model, &setup)
            .map_err(|problem| anyhow!(problem))?);
        self.model = model;
        self.applied_effort = setup.effort;
        Ok(setup.note)
    }

    /// Change the effort (`None` for the model's default); the history carries over. Returns
    /// a note to show, if any.
    pub async fn set_effort(&mut self, effort: Option<Effort>) -> Result<Option<String>> {
        let previous = std::mem::replace(&mut self.effort, effort);
        let model = self.model.clone();
        let setup = match self.setup_for(&model).await {
            Ok(setup) => setup,
            Err(error) => {
                self.effort = previous;
                return Err(error);
            },
        };
        // Without an API key this stays an error, but the effort is kept for when it works.
        self.agent = build_agent(&self.session, &self.capture, &model, &setup);
        self.applied_effort = setup.effort;
        Ok(setup.note)
    }

    /// The effort levels the current model takes.
    pub async fn effort_support(&mut self) -> effort::Support {
        let model = self.model.clone();
        let catalog = if model.backend == Backend::OpenRouter {
            self.catalog().await.ok()
        } else {
            None
        };
        let entry = catalog.as_deref().and_then(|catalog| {
            models::check_against_catalog(&model, catalog)
                .ok()
                .flatten()
        });
        effort::support(&model, entry)
    }

    /// [`setup`] for `model`, checking OpenRouter models against the catalog: many allow far
    /// less output than the default, and each lists the effort levels it takes.
    async fn setup_for(&mut self, model: &Resolved) -> Result<Setup> {
        if model.backend != Backend::OpenRouter {
            return Ok(setup(model, None, self.max_tokens, self.effort));
        }
        match self.catalog().await {
            Ok(catalog) => {
                let entry = models::check_against_catalog(model, &catalog)
                    .map_err(|problem| anyhow!(problem))?;
                Ok(setup(model, entry, self.max_tokens, self.effort))
            },
            Err(error) => {
                let mut planned = setup(model, None, self.max_tokens, self.effort);
                let problem = format!(
                    "Couldn't check {} against OpenRouter's model list ({error:#}).",
                    model.name
                );
                planned.note = Some(match planned.note {
                    Some(note) => format!("{problem} {note}"),
                    None => problem,
                });
                Ok(planned)
            },
        }
    }

    pub fn clear(&mut self) {
        self.history.clear();
        self.session.reset();
    }

    /// Record that the user stopped the last turn. Steps that finished are already in the
    /// history; this tells the model why the rest is missing.
    pub fn note_interrupted(&mut self) {
        self.history.push(Message::user(
            "[The user interrupted your previous response. Wait for their next instruction.]",
        ));
    }

    /// Handle one user message: call the model, run the tools it asks for, and repeat until
    /// it replies without a tool call.
    pub async fn run_turn(&mut self, input: &str) -> Result<TurnOutcome> {
        let agent = match &self.agent {
            Ok(agent) => agent,
            Err(problem) => bail!("{problem}"),
        };
        let started = Instant::now();
        self.session.begin_turn();
        lock(&self.capture).usage = Usage::default();
        self.history.push(Message::user(input));

        let mut outcome = TurnOutcome::default();
        let mut steps = 0;
        let result = loop {
            if steps == self.max_steps {
                self.session.ui.warn(&format!(
                    "Stopped after {steps} model calls (--max-steps). Send a message to continue."
                ));
                outcome.ending = Ending::StepLimit;
                break Ok(());
            }
            steps += 1;

            let response = match step(agent, &self.history, &self.session, &self.capture).await {
                Ok(response) => response,
                // The model ended its turn without writing anything.
                Err(AgentError::NoChoiceFound) => break Ok(()),
                Err(error) => break Err(error),
            };
            let choice = lock(&self.capture).choice.take().unwrap_or_default();
            let outputs = match response {
                ChatResponse::Text(text) => {
                    push_assistant(&mut self.history, choice);
                    outcome.answer = Some(text).filter(|text| !text.trim().is_empty());
                    break Ok(());
                },
                ChatResponse::ToolCalls(outputs)
                | ChatResponse::TextWithToolCalls {
                    tool_calls: outputs,
                    ..
                } => outputs,
            };
            outcome.tool_calls += outputs.len();
            push_tool_step(&mut self.history, choice, outputs);
            if self.session.halted() {
                outcome.ending = Ending::Declined;
                break Ok(());
            }
        };

        outcome.usage = lock(&self.capture).usage;
        outcome.elapsed = started.elapsed();
        result.map_err(|error| anyhow!(describe(&error)))?;
        Ok(outcome)
    }
}

/// One model call (plus the tools it requests), retrying transient API failures. Retrying is
/// safe: when the call fails, no tools have run.
async fn step(
    agent: &SwarmsAgent<ObservedModel>,
    history: &[Message],
    session: &Session,
    capture: &Mutex<Capture>,
) -> Result<ChatResponse, AgentError> {
    let mut attempt = 0;
    loop {
        lock(capture).choice = None;
        match agent.chat("", history.to_vec()).await {
            Err(AgentError::CompletionError(error))
                if attempt < MAX_RETRIES && is_transient(&error) =>
            {
                attempt += 1;
                let delay = Duration::from_secs(2_u64.pow(attempt));
                session.ui.warn(&format!(
                    "{} · retrying in {}s ({attempt}/{MAX_RETRIES})",
                    first_line(&error.to_string()),
                    delay.as_secs()
                ));
                tokio::time::sleep(delay).await;
            },
            result => return result,
        }
    }
}

fn push_assistant(history: &mut Vec<Message>, choice: Vec<AssistantContent>) {
    let content = without_empty_text(choice);
    if !content.is_empty() {
        history.push(Message::Assistant { content });
    }
}

/// Record a tool-using step as the provider expects it: the assistant turn with its tool
/// calls, then a user turn with one result per call, matched by call ID. swarms-rs runs the
/// calls in order (concurrent tool calls are off), so results line up with calls.
fn push_tool_step(
    history: &mut Vec<Message>,
    choice: Vec<AssistantContent>,
    outputs: Vec<ToolCallOutput>,
) {
    let mut outputs = outputs.into_iter();
    let results: Vec<UserContent> = choice
        .iter()
        .filter_map(|part| match part {
            AssistantContent::ToolCall(call) => Some(call),
            AssistantContent::Text(_) => None,
        })
        .map(|call| {
            let result = outputs.next().map_or_else(
                || "The tool produced no result.".to_owned(),
                |output| output.result,
            );
            UserContent::tool_result(call.id.clone(), vec![ToolResultContent::text(result)])
        })
        .collect();
    push_assistant(history, choice);
    history.push(Message::User { content: results });
}

/// How the agent is set up for a model.
#[derive(Debug, PartialEq, Eq)]
struct Setup {
    max_tokens: u64,
    effort: Option<Effort>,
    /// Something the user should know, such as an effort level the model doesn't take.
    note: Option<String>,
}

/// Set up `model` (listed as `entry` in OpenRouter's catalog, if it is) for the `wanted`
/// effort: the nearest level it takes, and enough output for that level within its limit.
fn setup(
    model: &Resolved,
    entry: Option<&CatalogModel>,
    max_tokens: Option<u64>,
    wanted: Option<Effort>,
) -> Setup {
    let levels = effort::support(model, entry).levels;
    let (effort, note) = match wanted {
        None => (None, None),
        Some(wanted) => match effort::fit(wanted, &levels) {
            Some(level) if level == wanted => (Some(level), None),
            Some(level) => (
                Some(level),
                Some(format!(
                    "{} doesn't offer {wanted} effort, so it runs at {level}.",
                    model.name
                )),
            ),
            None => (
                None,
                Some(format!(
                    "{} has no effort setting, so {wanted} effort doesn't apply to it.",
                    model.name
                )),
            ),
        },
    };
    let wanted_tokens = max_tokens.unwrap_or(if effort.is_some_and(Effort::needs_room) {
        ROOMY_MAX_TOKENS
    } else {
        DEFAULT_MAX_TOKENS
    });
    let max_tokens = entry
        .and_then(CatalogModel::output_limit)
        .map_or(wanted_tokens, |limit| limit.min(wanted_tokens));
    Setup {
        max_tokens,
        effort,
        note,
    }
}

fn build_agent(
    session: &Arc<Session>,
    capture: &Arc<Mutex<Capture>>,
    model: &Resolved,
    setup: &Setup,
) -> Result<SwarmsAgent<ObservedModel>, String> {
    let model_impl = ObservedModel {
        inner: models::connect(model, setup.effort, models::env_var)?,
        session: Arc::clone(session),
        capture: Arc::clone(capture),
    };
    let mut agent = SwarmsAgentBuilder::new_with_model(model_impl)
        .agent_name(AGENT_NAME)
        .description("A coding agent for the terminal")
        .system_prompt(prompt::build(
            &session.cwd,
            &model.name,
            session.approvals(),
        ))
        .max_tokens(setup.max_tokens)
        // The engine's loop decides when a turn is done, and tools run one at a time so
        // permission prompts never overlap.
        .disable_task_complete_tool()
        .disable_concurrent_tool_call()
        .build();
    for tool in tools::all(session) {
        agent = agent.tool(tools::Boxed(tool));
    }
    Ok(agent)
}

/// Providers reject empty text blocks in the history.
fn without_empty_text(content: Vec<AssistantContent>) -> Vec<AssistantContent> {
    content
        .into_iter()
        .filter(|part| !matches!(part, AssistantContent::Text(text) if text.text.trim().is_empty()))
        .collect()
}

fn is_transient(error: &CompletionError) -> bool {
    match error {
        CompletionError::Http(_) => true,
        CompletionError::Provider(message)
        | CompletionError::Other(message)
        | CompletionError::Response(message) => {
            let message = message.to_ascii_lowercase();
            [
                "overloaded",
                "rate_limit",
                "rate limit",
                "api_error",
                "server_error",
                "(status 429",
                "(status 5",
                "http request failed",
                "failed to read response",
                "empty response body",
                "timed out",
            ]
            .iter()
            .any(|pattern| message.contains(pattern))
        },
        _ => false,
    }
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default()
}

fn describe(error: &AgentError) -> String {
    match error {
        AgentError::CompletionError(error) => format!("Model request failed: {error}"),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_usage_from_either_provider_shape() {
        let anthropic =
            Usage::from_raw(&json!({ "usage": { "input_tokens": 12, "output_tokens": 3 } }));
        assert_eq!((anthropic.input, anthropic.output), (12, 3));
        let openai =
            Usage::from_raw(&json!({ "usage": { "prompt_tokens": 7, "completion_tokens": 2 } }));
        assert_eq!((openai.input, openai.output), (7, 2));
        let missing = Usage::from_raw(&json!({}));
        assert_eq!((missing.input, missing.output), (0, 0));
    }

    #[test]
    fn setup_fits_effort_and_output_to_the_model() {
        let keys = |var: &str| var == "DEEPSEEK_API_KEY";
        let opus = models::resolve("opus", keys).unwrap();
        assert_eq!(
            setup(&opus, None, None, None),
            Setup {
                max_tokens: DEFAULT_MAX_TOKENS,
                effort: None,
                note: None
            }
        );
        // The highest levels get room to think, unless --max-tokens says otherwise.
        let deep = setup(&opus, None, None, Some(Effort::XHigh));
        assert_eq!(
            (deep.max_tokens, deep.effort),
            (ROOMY_MAX_TOKENS, Some(Effort::XHigh))
        );
        assert_eq!(
            setup(&opus, None, Some(20_000), Some(Effort::Max)).max_tokens,
            20_000
        );

        let catalog: Vec<CatalogModel> = serde_json::from_str(
            r#"[{"id": "openai/gpt-5.5", "name": "GPT-5.5", "context_length": 400000,
                 "supported_parameters": ["tools", "reasoning_effort"],
                 "top_provider": {"max_completion_tokens": 128000},
                 "reasoning": {"supported_efforts": ["xhigh", "high", "medium", "low"]}}]"#,
        )
        .unwrap();
        let gpt = models::resolve("openrouter/openai/gpt-5.5", keys).unwrap();
        let fitted = setup(&gpt, catalog.first(), None, Some(Effort::Max));
        assert_eq!(
            (fitted.max_tokens, fitted.effort),
            (ROOMY_MAX_TOKENS, Some(Effort::XHigh))
        );
        assert!(fitted.note.unwrap().contains("runs at xhigh"));

        let deepseek = models::resolve("deepseek/deepseek-chat", keys).unwrap();
        let unsupported = setup(&deepseek, None, None, Some(Effort::High));
        assert_eq!(unsupported.effort, None);
        assert!(unsupported.note.unwrap().contains("no effort setting"));
    }

    #[test]
    fn only_transient_errors_are_retried() {
        let provider = |message: &str| CompletionError::Provider(message.to_owned());
        assert!(is_transient(&provider(
            "Anthropic API error: overloaded_error - Overloaded"
        )));
        assert!(is_transient(&provider(
            "Anthropic API error (status 529 <unknown status code>): "
        )));
        assert!(!is_transient(&provider(
            "Anthropic API error: invalid_request_error - max_tokens: 150000 > 128000"
        )));
        assert!(!is_transient(&provider(
            "Anthropic API error: authentication_error - invalid x-api-key"
        )));
    }
}
