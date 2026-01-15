//! # OpenRouter LLM Provider
//!
//! This module provides an OpenRouter API client for the Swarms framework.
//! OpenRouter provides unified access to multiple LLM providers through a single API.
//!
//! ## Features
//!
//! - **Multiple LLM Models**: Access to Claude, GPT-4, Gemini, Llama, and more
//! - **Cost-Effective**: Aggregates prices from different providers
//! - **Fallback Support**: Automatically falls back to alternative models if primary is unavailable
//! - **Full Tool Integration**: Complete support for tool calling and function execution
//! - **Environment Configuration**: Easy setup via environment variables
//!
//! ## Setup
//!
//! ### Environment Variables
//!
//! ```bash
//! export OPENROUTER_API_KEY="your-api-key-here"
//! export OPENROUTER_BASE_URL="https://openrouter.ai/api/v1"  # Optional, defaults to official API
//! ```
//!
//! ### Get API Key
//!
//! 1. Visit https://openrouter.ai
//! 2. Sign up for a free account
//! 3. Generate an API key from your dashboard
//! 4. Set the `OPENROUTER_API_KEY` environment variable
//!
//! ## Supported Models
//!
//! - `openai/gpt-4o` - OpenAI's latest model
//! - `openai/gpt-4-turbo` - OpenAI GPT-4 Turbo
//! - `anthropic/claude-3.5-sonnet` - Claude 3.5 Sonnet
//! - `anthropic/claude-3-opus` - Claude 3 Opus
//! - `google/gemini-2.5-pro` - Google Gemini Pro
//! - `meta-llama/llama-2-70b-chat` - Meta Llama 2
//! - `mistralai/mistral-large` - Mistral Large
//! - And many more...
//!
//! ## Usage Examples
//!
//! ### Basic Usage
//!
//! ```rust,no_run
//! use swarms_rs::agent::SwarmsAgentBuilder;
//! use swarms_rs::llm::provider::openrouter::OpenRouter;
//! use swarms_rs::structs::agent::Agent;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create OpenRouter client from environment
//! let model = OpenRouter::from_env();
//!
//! // Build agent with OpenRouter
//! let agent = SwarmsAgentBuilder::new_with_model(model)
//!     .agent_name("AIAssistant")
//!     .system_prompt("You are a helpful AI assistant.")
//!     .build();
//!
//! let result = agent.run("Hello!".to_string()).await?;
//! println!("Response: {}", result);
//! # Ok(())
//! # }
//! ```
//!
//! ### Using Different Models
//!
//! ```rust,no_run
//! use swarms_rs::agent::SwarmsAgentBuilder;
//! use swarms_rs::llm::provider::openrouter::OpenRouter;
//! use swarms_rs::structs::agent::Agent;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Use Claude through OpenRouter
//! let model = OpenRouter::from_env()
//!     .set_model("anthropic/claude-3.5-sonnet");
//!
//! let agent = SwarmsAgentBuilder::new_with_model(model)
//!     .agent_name("ClaudeViaOpenRouter")
//!     .system_prompt("You are Claude, a helpful AI assistant.")
//!     .build();
//!
//! let result = agent.run("What is 2+2?".to_string()).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### With Custom Configuration
//!
//! ```rust,no_run
//! use swarms_rs::agent::SwarmsAgentBuilder;
//! use swarms_rs::llm::provider::openrouter::OpenRouter;
//! use swarms_rs::structs::agent::Agent;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let model = OpenRouter::new("your-api-key")
//!     .set_model("openai/gpt-4o")
//!     .set_temperature(0.5)
//!     .set_max_tokens(2048);
//!
//! let agent = SwarmsAgentBuilder::new_with_model(model)
//!     .agent_name("GPT4Agent")
//!     .system_prompt("You are a helpful assistant.")
//!     .build();
//!
//! # Ok(())
//! # }
//! ```

use std::env;

use async_openai::types::{
    ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessageArgs,
    ChatCompletionRequestAssistantMessageContent,
    ChatCompletionRequestAssistantMessageContentPart, ChatCompletionRequestMessage,
    ChatCompletionRequestMessageContentPartAudio, ChatCompletionRequestMessageContentPartImage,
    ChatCompletionRequestMessageContentPartText, ChatCompletionRequestSystemMessageArgs,
    ChatCompletionRequestToolMessage, ChatCompletionRequestToolMessageContent,
    ChatCompletionRequestToolMessageContentPart, ChatCompletionRequestUserMessageArgs,
    ChatCompletionRequestUserMessageContentPart, ChatCompletionToolArgs,
    ChatCompletionToolType, CreateChatCompletionRequestArgs, FunctionCall, FunctionObjectArgs,
    ImageUrl, InputAudio, InputAudioFormat,
};
use futures::future::BoxFuture;
use reqwest::Client;

use crate::{
    agent::SwarmsAgentBuilder,
    llm::{
        self, CompletionError, Model,
        request::{CompletionRequest, CompletionResponse},
    },
};

/// OpenRouter LLM Provider
///
/// This struct represents the OpenRouter API client for making completions requests.
#[derive(Clone)]
pub struct OpenRouter {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    system_prompt: Option<String>,
}

impl OpenRouter {
    /// Create a new OpenRouter client with the specified API key
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your OpenRouter API key
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// let model = OpenRouter::new("sk-or-xxxxx");
    /// ```
    pub fn new<S: Into<String>>(api_key: S) -> Self {
        let http_client = reqwest::ClientBuilder::new()
            .user_agent("swarms-rs")
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client: http_client,
            api_key: api_key.into(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            model: "openai/gpt-4o-mini".to_string(),
            temperature: None,
            max_tokens: None,
            system_prompt: None,
        }
    }

    /// Create OpenRouter client from environment variables
    ///
    /// Expects `OPENROUTER_API_KEY` to be set. Optionally uses `OPENROUTER_BASE_URL`
    /// if provided.
    ///
    /// # Panics
    ///
    /// Panics if `OPENROUTER_API_KEY` is not set
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// let model = OpenRouter::from_env();
    /// ```
    pub fn from_env() -> Self {
        let api_key = env::var("OPENROUTER_API_KEY")
            .expect("OPENROUTER_API_KEY environment variable is not set");

        let base_url = env::var("OPENROUTER_BASE_URL")
            .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string());

        Self::new(api_key).with_base_url(base_url)
    }

    /// Create OpenRouter client from environment with specified model
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// let model = OpenRouter::from_env_with_model("anthropic/claude-3.5-sonnet");
    /// ```
    pub fn from_env_with_model<S: Into<String>>(model: S) -> Self {
        Self::from_env().set_model(model)
    }

    /// Set the base URL for OpenRouter API
    ///
    /// Useful if you're using a proxy or different endpoint
    pub fn with_base_url<S: Into<String>>(mut self, base_url: S) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Set the model to use
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// let model = OpenRouter::from_env()
    ///     .set_model("anthropic/claude-3.5-sonnet");
    /// ```
    pub fn set_model<S: Into<String>>(mut self, model: S) -> Self {
        self.model = model.into();
        self
    }

    /// Set the temperature for responses
    ///
    /// # Arguments
    ///
    /// * `temperature` - Temperature between 0.0 and 2.0. Higher values make responses more random.
    pub fn set_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set the maximum number of tokens to generate
    pub fn set_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set the system prompt
    pub fn set_system_prompt<S: Into<String>>(&mut self, prompt: S) {
        self.system_prompt = Some(prompt.into());
    }

    /// Get the current model name
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Get the current base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Create a SwarmsAgentBuilder with this model
    pub fn agent_builder(&self) -> SwarmsAgentBuilder<Self> {
        SwarmsAgentBuilder::new_with_model(self.clone())
    }
}

impl Model for OpenRouter {
    type RawCompletionResponse = serde_json::Value;

    fn completion(
        &self,
        request: CompletionRequest,
    ) -> BoxFuture<Result<CompletionResponse<Self::RawCompletionResponse>, CompletionError>> {
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let base_url = self.base_url.clone();
        let model = self.model.clone();
        let temperature = self.temperature;
        let max_tokens = self.max_tokens;

        Box::pin(async move {
            let mut msgs = Vec::new();

            // Add system prompt if provided in request
            if let Some(system_prompt) = request.system_prompt {
                msgs.push(
                    ChatCompletionRequestSystemMessageArgs::default()
                        .content(system_prompt)
                        .build()
                        .map_err(|e| CompletionError::Request(Box::new(e)))?
                        .into(),
                );
            }

            // Add chat history
            let chat_history = request
                .chat_history
                .into_iter()
                .map(|msg| {
                    let msgs: Vec<ChatCompletionRequestMessage> = msg.try_into()?;
                    Ok::<_, CompletionError>(msgs)
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();

            msgs.extend(chat_history);

            // Add the current prompt
            if request.prompt.rag_text().is_some() {
                let prompt: Vec<ChatCompletionRequestMessage> = request.prompt.try_into()?;
                msgs.extend(prompt);
            }

            // Build the request
            let mut request_body = serde_json::json!({
                "model": model,
                "messages": msgs,
            });

            // Add optional parameters
            if let Some(temp) = temperature {
                request_body["temperature"] = serde_json::json!(temp);
            }
            if let Some(max_tok) = max_tokens {
                request_body["max_tokens"] = serde_json::json!(max_tok);
            }

            // Add tools if provided
            if !request.tools.is_empty() {
                request_body["tools"] = serde_json::json!(
                    request
                        .tools
                        .into_iter()
                        .map(|tool| {
                            serde_json::json!({
                                "type": "function",
                                "function": {
                                    "name": tool.name,
                                    "description": tool.description,
                                    "parameters": tool.parameters,
                                }
                            })
                        })
                        .collect::<Vec<_>>()
                );
            }

            tracing::debug!(
                "OpenRouter Request: {}",
                serde_json::to_string_pretty(&request_body).unwrap_or_default()
            );

            // Make the request
            let response = client
                .post(format!("{}/chat/completions", base_url))
                .bearer_auth(&api_key)
                .header("HTTP-Referer", "https://swarms.world")
                .json(&request_body)
                .send()
                .await?;

            let status = response.status();
            let response_text = response.text().await?;

            tracing::debug!("OpenRouter Response Status: {}", status);
            tracing::debug!("OpenRouter Response: {}", response_text);

            if !status.is_success() {
                let error_msg = format!(
                    "OpenRouter API error ({}): {}",
                    status, response_text
                );
                return Err(CompletionError::Provider(error_msg));
            }

            let raw_response: serde_json::Value = serde_json::from_str(&response_text)?;

            // Parse the response
            let choices: Vec<AssistantContent> = raw_response
                .get("choices")
                .and_then(|c| c.as_array())
                .ok_or_else(|| {
                    CompletionError::Response("No choices in response".to_string())
                })?
                .iter()
                .filter_map(|choice| {
                    let message = choice.get("message")?;
                    let content = message.get("content");
                    let tool_calls = message.get("tool_calls");

                    if let Some(text) = content.and_then(|c| c.as_str()) {
                        Some(AssistantContent::Text(llm::completion::Text {
                            text: text.to_string(),
                        }))
                    } else if let Some(tools) = tool_calls.and_then(|t| t.as_array()) {
                        tools
                            .iter()
                            .find_map(|tool| {
                                let id = tool.get("id")?.as_str()?.to_string();
                                let function = tool.get("function")?;
                                let name = function.get("name")?.as_str()?.to_string();
                                let arguments = function.get("arguments")?.clone();

                                let arguments = if let serde_json::Value::String(s) = arguments {
                                    serde_json::from_str(&s).ok()?
                                } else {
                                    arguments
                                };

                                Some(AssistantContent::ToolCall(
                                    llm::completion::ToolCall {
                                        id,
                                        function: llm::completion::ToolFunction {
                                            name,
                                            arguments,
                                        },
                                    },
                                ))
                            })
                    } else {
                        None
                    }
                })
                .collect();

            if choices.is_empty() {
                return Err(CompletionError::Response(
                    "No valid choices parsed from response".to_string(),
                ));
            }

            Ok(CompletionResponse {
                choice: choices,
                raw_response,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openrouter_creation() {
        let model = OpenRouter::new("test-key");
        assert_eq!(model.model(), "openai/gpt-4o-mini");
        assert_eq!(model.base_url(), "https://openrouter.ai/api/v1");
    }

    #[test]
    fn test_openrouter_set_model() {
        let model = OpenRouter::new("test-key")
            .set_model("anthropic/claude-3.5-sonnet");
        assert_eq!(model.model(), "anthropic/claude-3.5-sonnet");
    }

    #[test]
    fn test_openrouter_custom_base_url() {
        let model = OpenRouter::new("test-key")
            .with_base_url("https://custom.api.com/v1");
        assert_eq!(model.base_url(), "https://custom.api.com/v1");
    }

    #[test]
    fn test_openrouter_set_temperature() {
        let model = OpenRouter::new("test-key").set_temperature(0.7);
        assert_eq!(model.temperature, Some(0.7));
    }

    #[test]
    fn test_openrouter_set_max_tokens() {
        let model = OpenRouter::new("test-key").set_max_tokens(2048);
        assert_eq!(model.max_tokens, Some(2048));
    }
}
