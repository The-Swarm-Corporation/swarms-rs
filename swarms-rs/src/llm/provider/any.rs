//! # One model type for every provider
//!
//! [`AnyModel`] picks the provider from a model name, so switching providers is a one-string
//! change:
//!
//! | Model name | Provider | Credentials |
//! |------------|----------|-------------|
//! | `openai/gpt-5.5`, or a bare `gpt-*`, `o1*`, `o3*`, `o4*` | OpenAI (`OPENAI_API_BASE` optional) | `OPENAI_API_KEY` |
//! | `anthropic/claude-opus-5-5`, or a bare `claude-*` | Anthropic | `ANTHROPIC_API_KEY` |
//! | `deepseek/deepseek-chat`, or a bare `deepseek-*` | DeepSeek (`DEEPSEEK_BASE_URL` optional) | `DEEPSEEK_API_KEY` |
//! | `openrouter/anthropic/claude-opus-5.5`, `openrouter/auto` | OpenRouter | `OPENROUTER_API_KEY` |
//! | any other `vendor/model`, e.g. `google/gemini-3.8-flash` | OpenRouter | `OPENROUTER_API_KEY` |
//!
//! ```rust,no_run
//! use swarms_rs::llm::provider::any::AnyModel;
//! use swarms_rs::structs::agent::Agent;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let agent = AnyModel::from_model_name("anthropic/claude-opus-5-5")?
//!     .agent_builder()
//!     .system_prompt("You are a helpful assistant.")
//!     .build();
//! println!("{}", agent.run("Hello!".to_string()).await?);
//! # Ok(())
//! # }
//! ```

use std::env;

use futures::future::BoxFuture;
use thiserror::Error;

use crate::{
    agent::SwarmsAgentBuilder,
    llm::{
        CompletionError, Model,
        provider::{anthropic::Anthropic, openai::OpenAI, openrouter::OpenRouter},
        request::{CompletionRequest, CompletionResponse},
    },
};

const DEEPSEEK_API_BASE: &str = "https://api.deepseek.com/v1";

/// A model from any supported provider, chosen by name with [`AnyModel::from_model_name`].
/// Tool calling works the same way on every provider.
// Built once per agent, so the size difference between variants doesn't matter.
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
pub enum AnyModel {
    OpenAI(OpenAI),
    Anthropic(Anthropic),
    OpenRouter(OpenRouter),
}

#[derive(Debug, Error, PartialEq)]
pub enum ModelNameError {
    #[error("Unknown model name '{0}'. Use a 'provider/model' name such as 'openai/gpt-5.5'")]
    Unknown(String),
    #[error("Model '{model}' needs the {var} environment variable")]
    MissingApiKey { model: String, var: &'static str },
}

/// Which provider a model name maps to, and the model ID to send to it.
#[derive(Debug, PartialEq)]
pub enum Route {
    OpenAI(String),
    Anthropic(String),
    DeepSeek(String),
    OpenRouter(String),
}

impl Route {
    /// Resolve a model name to a provider without touching the environment.
    pub fn parse(name: &str) -> Result<Route, ModelNameError> {
        let name = name.trim();
        let unknown = || ModelNameError::Unknown(name.to_string());
        if let Some((prefix, model)) = name.split_once('/') {
            if model.is_empty() {
                return Err(unknown());
            }
            return Ok(match prefix.to_ascii_lowercase().as_str() {
                "openai" => Route::OpenAI(model.to_string()),
                "anthropic" => Route::Anthropic(model.to_string()),
                "deepseek" => Route::DeepSeek(model.to_string()),
                // `openrouter/auto` is itself an OpenRouter model ID; otherwise strip the prefix.
                "openrouter" if model.contains('/') => Route::OpenRouter(model.to_string()),
                "openrouter" => Route::OpenRouter(name.to_string()),
                "" => return Err(unknown()),
                // Vendors without a native provider (Google, Meta, Mistral, ...) go via OpenRouter.
                _ => Route::OpenRouter(name.to_string()),
            });
        }
        let lower = name.to_ascii_lowercase();
        if lower.starts_with("gpt-")
            || lower.starts_with("chatgpt-")
            || ["o1", "o3", "o4"].iter().any(|p| lower.starts_with(p))
        {
            Ok(Route::OpenAI(name.to_string()))
        } else if lower.starts_with("claude-") {
            Ok(Route::Anthropic(name.to_string()))
        } else if lower.starts_with("deepseek-") {
            Ok(Route::DeepSeek(name.to_string()))
        } else {
            Err(unknown())
        }
    }
}

impl AnyModel {
    /// Build a model from a name like `"anthropic/claude-opus-5-5"`, reading the provider's
    /// API key from the environment. See the module docs for every supported form.
    pub fn from_model_name(name: &str) -> Result<Self, ModelNameError> {
        Ok(match Route::parse(name)? {
            Route::OpenAI(model) => {
                let key = super::utils::read_api_key(name, "OPENAI_API_KEY")?;
                let base = env::var("OPENAI_API_BASE")
                    .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
                AnyModel::OpenAI(OpenAI::from_url(base, key).set_model(model))
            },
            Route::Anthropic(model) => {
                let key = super::utils::read_api_key(name, "ANTHROPIC_API_KEY")?;
                let base = env::var("ANTHROPIC_BASE_URL")
                    .unwrap_or_else(|_| "https://api.anthropic.com".to_string());
                AnyModel::Anthropic(Anthropic::from_url(base, key).set_model(model))
            },
            Route::DeepSeek(model) => {
                let key = super::utils::read_api_key(name, "DEEPSEEK_API_KEY")?;
                let base =
                    env::var("DEEPSEEK_BASE_URL").unwrap_or_else(|_| DEEPSEEK_API_BASE.to_string());
                AnyModel::OpenAI(OpenAI::from_url(base, key).set_model(model))
            },
            Route::OpenRouter(model) => {
                let key = super::utils::read_api_key(name, "OPENROUTER_API_KEY")?;
                let base = env::var("OPENROUTER_API_BASE").unwrap_or_else(|_| {
                    crate::llm::provider::openrouter::OPENROUTER_API_BASE.to_string()
                });
                AnyModel::OpenRouter(OpenRouter::from_url(base, key).set_model(model))
            },
        })
    }

    pub fn agent_builder(&self) -> SwarmsAgentBuilder<Self> {
        SwarmsAgentBuilder::new_with_model(self.clone())
    }
}

fn to_value<T: serde::Serialize>(
    response: CompletionResponse<T>,
) -> Result<CompletionResponse<serde_json::Value>, CompletionError> {
    Ok(CompletionResponse {
        choice: response.choice,
        raw_response: serde_json::to_value(response.raw_response)?,
    })
}

impl Model for AnyModel {
    /// The provider's raw response as JSON, since each provider has its own response type.
    type RawCompletionResponse = serde_json::Value;

    fn completion(
        &self,
        request: CompletionRequest,
    ) -> BoxFuture<'_, Result<CompletionResponse<Self::RawCompletionResponse>, CompletionError>>
    {
        Box::pin(async move {
            match self {
                AnyModel::OpenAI(model) => to_value(model.completion(request).await?),
                AnyModel::Anthropic(model) => to_value(model.completion(request).await?),
                AnyModel::OpenRouter(model) => to_value(model.completion(request).await?),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_model_names() {
        assert_eq!(
            Route::parse("openai/gpt-5.5"),
            Ok(Route::OpenAI("gpt-5.5".into()))
        );
        assert_eq!(Route::parse("gpt-4o"), Ok(Route::OpenAI("gpt-4o".into())));
        assert_eq!(Route::parse("o3-mini"), Ok(Route::OpenAI("o3-mini".into())));
        assert_eq!(
            Route::parse("anthropic/claude-opus-5-5"),
            Ok(Route::Anthropic("claude-opus-5-5".into()))
        );
        assert_eq!(
            Route::parse("claude-haiku-4-5"),
            Ok(Route::Anthropic("claude-haiku-4-5".into()))
        );
        assert_eq!(
            Route::parse("deepseek/deepseek-chat"),
            Ok(Route::DeepSeek("deepseek-chat".into()))
        );
        assert_eq!(
            Route::parse("openrouter/anthropic/claude-opus-5.5"),
            Ok(Route::OpenRouter("anthropic/claude-opus-5.5".into()))
        );
        assert_eq!(
            Route::parse("openrouter/auto"),
            Ok(Route::OpenRouter("openrouter/auto".into()))
        );
        assert_eq!(
            Route::parse("google/gemini-3.8-flash"),
            Ok(Route::OpenRouter("google/gemini-3.8-flash".into()))
        );
        assert!(Route::parse("mystery-model").is_err());
        assert!(Route::parse("openai/").is_err());
        assert!(Route::parse("/gpt-4o").is_err());
    }
}
