//! # OpenRouter Provider
//!
//! [OpenRouter](https://openrouter.ai) serves models from OpenAI, Anthropic, Google, Meta,
//! Mistral, DeepSeek and others behind one OpenAI-compatible API and one API key. Model IDs
//! take the form `provider/model` (for example `anthropic/claude-opus-5.5` or
//! `openai/gpt-5.5`); the default, `openrouter/auto`, lets OpenRouter pick a model per prompt.
//!
//! ## Environment Variables
//!
//! - `OPENROUTER_API_KEY` (required by [`OpenRouter::from_env`])
//! - `OPENROUTER_API_BASE` (optional, defaults to `https://openrouter.ai/api/v1`)
//! - `OPENROUTER_APP_URL` / `OPENROUTER_APP_NAME` (optional, credit your app on
//!   openrouter.ai rankings via the `HTTP-Referer` and `X-OpenRouter-Title` headers)
//!
//! ## Example
//!
//! ```rust,no_run
//! use swarms_rs::llm::provider::openrouter::OpenRouter;
//! use swarms_rs::structs::agent::Agent;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = OpenRouter::from_env_with_model("anthropic/claude-opus-5.5");
//! let agent = client
//!     .agent_builder()
//!     .agent_name("ResearchAgent")
//!     .system_prompt("You are a helpful research assistant.")
//!     .build();
//!
//! let answer = agent.run("Summarize the history of Rust in two sentences.".to_string()).await?;
//! println!("{answer}");
//! # Ok(())
//! # }
//! ```

use std::env;

use futures::future::BoxFuture;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use crate::{
    agent::SwarmsAgentBuilder,
    llm::{
        CompletionError, Model,
        provider::{any::ModelNameError, openai::OpenAI},
        request::{CompletionRequest, CompletionResponse},
    },
};

/// OpenRouter's OpenAI-compatible API base.
pub const OPENROUTER_API_BASE: &str = "https://openrouter.ai/api/v1";

/// OpenRouter's auto router, which picks a model for each prompt.
pub const DEFAULT_MODEL: &str = "openrouter/auto";

const APP_URL_HEADER: &str = "http-referer";
const APP_NAME_HEADER: &str = "x-openrouter-title";

/// OpenRouter client. Implements [`Model`], so it plugs into agents and workflows like
/// any other provider.
#[derive(Clone)]
pub struct OpenRouter {
    inner: OpenAI,
    headers: HeaderMap,
}

impl OpenRouter {
    pub fn new<S: Into<String>>(api_key: S) -> Self {
        Self::from_url(OPENROUTER_API_BASE.to_owned(), api_key.into())
    }

    pub fn from_url<S: Into<String>>(base_url: S, api_key: S) -> Self {
        Self {
            inner: OpenAI::from_url(base_url, api_key).set_model(DEFAULT_MODEL),
            headers: HeaderMap::new(),
        }
    }

    pub fn from_env() -> Self {
        let base_url = env::var("OPENROUTER_API_BASE").unwrap_or(OPENROUTER_API_BASE.to_owned());
        let api_key = env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY is not set");
        let mut client = Self::from_url(base_url, api_key);
        if let Ok(url) = env::var("OPENROUTER_APP_URL") {
            client = client.with_app_url(url);
        }
        if let Ok(name) = env::var("OPENROUTER_APP_NAME") {
            client = client.with_app_name(name);
        }
        client
    }

    pub fn from_env_with_model<S: Into<String>>(model: S) -> Self {
        Self::from_env().set_model(model)
    }

    /// Read environment configuration, returning an error for a missing or empty API key.
    pub fn try_from_env() -> Result<Self, ModelNameError> {
        Self::try_from_env_with_model(DEFAULT_MODEL)
    }

    /// Read environment configuration and select a model, returning credential errors.
    pub fn try_from_env_with_model<S: Into<String>>(model: S) -> Result<Self, ModelNameError> {
        let model = model.into();
        let base_url = env::var("OPENROUTER_API_BASE").unwrap_or(OPENROUTER_API_BASE.to_owned());
        let api_key = super::utils::read_api_key(&model, "OPENROUTER_API_KEY")?;
        let mut client = Self::from_url(base_url, api_key).set_model(model);
        if let Ok(url) = env::var("OPENROUTER_APP_URL") {
            client = client.with_app_url(url);
        }
        if let Ok(name) = env::var("OPENROUTER_APP_NAME") {
            client = client.with_app_name(name);
        }
        Ok(client)
    }

    /// Choose the model, e.g. `"anthropic/claude-opus-5.5"` or `"openai/gpt-5.5"`.
    pub fn set_model<S: Into<String>>(mut self, model: S) -> Self {
        self.inner = self.inner.set_model(model);
        self
    }

    pub fn set_system_prompt<S: Into<String>>(&mut self, prompt: S) {
        self.inner.set_system_prompt(prompt);
    }

    /// Your app's URL, sent as `HTTP-Referer` for attribution on openrouter.ai.
    pub fn with_app_url<S: AsRef<str>>(self, url: S) -> Self {
        self.with_header(APP_URL_HEADER, url.as_ref())
    }

    /// Your app's name, sent as `X-OpenRouter-Title` for attribution on openrouter.ai.
    pub fn with_app_name<S: AsRef<str>>(self, name: S) -> Self {
        self.with_header(APP_NAME_HEADER, name.as_ref())
    }

    pub fn agent_builder(&self) -> SwarmsAgentBuilder<Self> {
        SwarmsAgentBuilder::new_with_model(self.clone())
    }

    fn with_header(mut self, name: &'static str, value: &str) -> Self {
        match HeaderValue::from_str(value) {
            Ok(value) => {
                self.headers.insert(HeaderName::from_static(name), value);
                self.inner = self.inner.with_default_headers(self.headers.clone());
            },
            Err(e) => tracing::warn!("Ignoring invalid OpenRouter {name} header value: {e}"),
        }
        self
    }
}

impl Model for OpenRouter {
    type RawCompletionResponse = async_openai::types::CreateChatCompletionResponse;

    fn completion(
        &self,
        request: CompletionRequest,
    ) -> BoxFuture<'_, Result<CompletionResponse<Self::RawCompletionResponse>, CompletionError>>
    {
        self.inner.completion(request)
    }
}
