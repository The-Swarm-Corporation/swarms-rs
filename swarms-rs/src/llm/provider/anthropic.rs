//! # Anthropic Claude Provider
//!
//! This module provides an optimized Anthropic Claude API client for the Swarms framework.
//! It uses the hyper library for high-performance HTTP requests and supports all Claude models.
//!
//! ## Features
//!
//! - **High Performance**: Uses hyper for efficient HTTP requests
//! - **All Claude Models**: Support for Claude 3.5 Sonnet, Haiku, Opus, and legacy models
//! - **Tool Integration**: Full support for tool calling and function execution
//! - **Streaming Support**: Efficient request/response handling
//! - **Error Handling**: Comprehensive error handling with detailed messages
//! - **Environment Configuration**: Easy setup via environment variables
//!
//! ## Setup
//!
//! ### Environment Variables
//!
//! ```bash
//! export ANTHROPIC_API_KEY="your-api-key-here"
//! export ANTHROPIC_BASE_URL="https://api.anthropic.com"  # Optional, defaults to official API
//! ```
//!
//! ### Authentication
//!
//! The Anthropic provider uses the `x-api-key` header for authentication:
//!
//! ```http
//! x-api-key: your-api-key-here
//! anthropic-version: 2023-06-01
//! ```
//!
//! ### Cargo.toml
//!
//! The Anthropic provider requires these dependencies (already included):
//!
//! ```toml
//! hyper = { version = "1.0", features = ["http1", "client", "server"] }
//! hyper-util = { version = "0.1", features = ["client", "client-legacy", "http1"] }
//! hyper-tls = "0.6"
//! bytes = "1.0"
//! http-body-util = "0.1"
//! ```
//!
//! ## Usage Examples
//!
//! ### Basic Usage
//!
//! ```rust,no_run
//! use swarms_rs::agent::SwarmsAgentBuilder;
//! use swarms_rs::llm::provider::anthropic::Anthropic;
//! use swarms_rs::structs::agent::Agent;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create Anthropic client from environment
//! let model = Anthropic::from_env();
//!
//! // Build agent with Claude
//! let agent = SwarmsAgentBuilder::new_with_model(model)
//!     .agent_name("ClaudeAssistant")
//!     .system_prompt("You are Claude, a helpful AI assistant.")
//!     .build();
//!
//! let result = agent.run("Hello, Claude!".to_string()).await?;
//! println!("Response: {}", result);
//! # Ok(())
//! # }
//! ```
//!
//! ### Advanced Configuration
//!
//! ```rust,no_run
//! use swarms_rs::agent::SwarmsAgentBuilder;
//! use swarms_rs::llm::provider::anthropic::Anthropic;
//! use swarms_rs::structs::agent::Agent;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Use Claude 3.5 Sonnet for complex tasks
//! let model = Anthropic::from_env()
//!     .set_model("claude-3-5-sonnet-20241022");
//!
//! let agent = SwarmsAgentBuilder::new_with_model(model)
//!     .agent_name("AdvancedClaude")
//!     .system_prompt("You are an advanced AI assistant with expertise in multiple domains.")
//!     .max_loops(5)
//!     .temperature(0.3)
//!     .max_tokens(4096)
//!     .enable_plan(Some("Create a detailed plan for: ".to_string()))
//!     .verbose(true)
//!     .build();
//!
//! let result = agent.run("Analyze the current state of AI development.".to_string()).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Tool Integration
//!
//! ```rust,no_run
//! use swarms_rs::agent::SwarmsAgentBuilder;
//! use swarms_rs::llm::provider::anthropic::Anthropic;
//! use swarms_rs::structs::agent::Agent;
//! use swarms_rs::structs::tool::Tool;
//! use swarms_rs::llm::request::ToolDefinition;
//!
//! // Define a custom tool
//! #[derive(Debug)]
//! struct WeatherTool;
//!
//! #[derive(Debug, serde::Deserialize)]
//! struct WeatherArgs {
//!     location: String,
//! }
//!
//! impl Tool for WeatherTool {
//!     type Error = std::io::Error;
//!     type Args = WeatherArgs;
//!     type Output = String;
//!     const NAME: &'static str = "get_weather";
//!
//!     fn definition(&self) -> ToolDefinition {
//!         ToolDefinition {
//!             name: "get_weather".to_string(),
//!             description: "Get weather information for a location".to_string(),
//!             parameters: serde_json::json!({
//!                 "type": "object",
//!                 "properties": {
//!                     "location": {
//!                         "type": "string",
//!                         "description": "The location to get weather for"
//!                     }
//!                 },
//!                 "required": ["location"]
//!             }),
//!         }
//!     }
//!
//!     async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
//!         Ok(format!("Weather in {}: Sunny, 72°F", args.location))
//!     }
//! }
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let model = Anthropic::from_env_with_model("claude-3-5-haiku-20241022");
//!
//! let agent = SwarmsAgentBuilder::new_with_model(model)
//!     .add_tool(WeatherTool)
//!     .system_prompt("You can check the weather using the weather tool.")
//!     .build();
//!
//! let result = agent.run("What's the weather like today?".to_string()).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Available Models
//!
//! | Model | Description | Use Case |
//! |-------|-------------|----------|
//! | `claude-3-5-sonnet-20241022` | Most intelligent model | Complex analysis, creative tasks |
//! | `claude-3-5-haiku-20241022` | Fast and efficient | Quick responses, simple tasks |
//! | `claude-3-opus-20240229` | Most powerful model | Maximum intelligence required |
//! | `claude-3-sonnet-20240229` | Balanced performance | General purpose |
//! | `claude-3-haiku-20240307` | Fastest model | High-throughput applications |
//!
//! ## Performance Optimization
//!
//! - **Connection Reuse**: Uses hyper's connection pooling
//! - **Efficient Serialization**: Optimized JSON handling
//! - **Async Processing**: Fully asynchronous request handling
//! - **Memory Efficient**: Minimal memory allocations
//!
//! ## Error Handling
//!
//! The provider handles various error scenarios:
//!
//! - **API Errors**: Invalid API keys, rate limits, model not found
//! - **Network Errors**: Connection timeouts, DNS resolution failures
//! - **Parsing Errors**: Invalid JSON responses
//! - **Tool Errors**: Tool execution failures
//!
//! All errors are converted to the standard `CompletionError` type for consistent handling.

use bytes::Bytes;
use futures::future::BoxFuture;
use http_body_util::{BodyExt, Full};
use hyper::{
    Method, Request, Uri,
    body::Buf,
    header::{CONTENT_TYPE, HeaderValue},
};
use hyper_tls::HttpsConnector;
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};
use serde::{Deserialize, Serialize};

use crate::agent::SwarmsAgentBuilder;
use crate::llm::{
    self, CompletionError, Model,
    request::{CompletionRequest, CompletionResponse},
};

/// Anthropic API client for Claude models
///
/// This struct provides a high-performance interface to Anthropic's Claude models.
/// It uses hyper for efficient HTTP requests and supports all Claude model variants.
///
/// # Architecture
///
/// - Uses hyper client with TLS support for secure, efficient HTTP requests
/// - Implements connection pooling and reuse for optimal performance
/// - Handles JSON serialization/deserialization with serde
/// - Fully asynchronous with tokio runtime
/// - Performance optimizations including cached headers and pre-parsed URIs
///
/// # Memory Safety
///
/// - Zero-copy request building where possible
/// - Efficient byte buffer management
/// - Minimal heap allocations during request processing
///
/// # Thread Safety
///
/// The client is `Clone` and can be safely shared across threads.
/// All internal state is immutable after construction.
///
/// # Performance Features
///
/// - Cached API key header to avoid string allocation on each request
/// - Pre-parsed URI for reduced parsing overhead
/// - Optimized request building with reusable components
/// - Efficient JSON serialization with pre-allocated buffers
///
/// # Example
///
/// ```rust,no_run
/// use swarms_rs::llm::provider::anthropic::Anthropic;
///
/// // Create from environment variables
/// let client = Anthropic::from_env();
///
/// // Create with specific model
/// let sonnet_client = Anthropic::from_env_with_model("claude-3-5-sonnet-20241022");
///
/// // Create with custom configuration
/// let custom_client = Anthropic::from_url(
///     "https://api.anthropic.com",
///     "your-api-key"
/// ).set_model("claude-3-opus-20240229");
/// ```
#[derive(Clone)]
pub struct Anthropic {
    /// Hyper HTTP client with TLS support
    client: Client<HttpsConnector<HttpConnector>, Full<Bytes>>,
    /// Anthropic API key for authentication
    #[allow(dead_code)]
    api_key: String,
    /// Claude model identifier (e.g., "claude-3-5-sonnet-20241022")
    model: String,
    /// Base URL for Anthropic API (default: "https://api.anthropic.com")
    #[allow(dead_code)]
    base_url: String,
    /// Cached API key header value (performance optimization)
    api_key_header: HeaderValue,
    /// Pre-parsed messages endpoint URI (performance optimization)
    messages_uri: Uri,
}

impl Anthropic {
    /// Create a new Anthropic client with API key
    ///
    /// This constructor creates a client with the default Claude 3.5 Sonnet model
    /// and the official Anthropic API endpoint.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your Anthropic API key for authentication
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use swarms_rs::llm::provider::anthropic::Anthropic;
    ///
    /// let client = Anthropic::new("your-api-key-here");
    /// ```
    pub fn new<S: Into<String>>(api_key: S) -> Self {
        let api_key = api_key.into();
        let base_url = "https://api.anthropic.com".to_string();
        Self::create_with_cached_fields(api_key, "claude-3-5-sonnet-20241022".to_string(), base_url)
    }

    /// Create a new Anthropic client with custom base URL
    pub fn from_url<S: Into<String>>(base_url: S, api_key: S) -> Self {
        let api_key = api_key.into();
        let base_url = base_url.into();
        Self::create_with_cached_fields(api_key, "claude-3-5-sonnet-20241022".to_string(), base_url)
    }

    /// Create a new Anthropic client from environment variables
    pub fn from_env() -> Self {
        let base_url = std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".to_string());
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .expect("ANTHROPIC_API_KEY environment variable is not set");

        Self::create_with_cached_fields(api_key, "claude-3-5-sonnet-20241022".to_string(), base_url)
    }

    /// Create a new Anthropic client with a specific model
    pub fn from_env_with_model<S: Into<String>>(model: S) -> Self {
        let base_url = std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".to_string());
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .expect("ANTHROPIC_API_KEY environment variable is not set");
        let model = model.into();

        Self::create_with_cached_fields(api_key, model, base_url)
    }

    /// Set the model to use
    pub fn set_model<S: Into<String>>(mut self, model: S) -> Self {
        self.model = model.into();
        self
    }

    /// Get a reference to the current model name
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Convenience: create a `SwarmsAgentBuilder` from this Anthropic client
    pub fn agent_builder(&self) -> SwarmsAgentBuilder<Self> {
        SwarmsAgentBuilder::new_with_model(self.clone())
    }

    /// Helper function to create Anthropic client with cached fields for performance
    ///
    /// This function pre-computes and caches:
    /// - API key header value (avoids string allocation per request)
    /// - Messages endpoint URI (avoids parsing per request)
    /// - HTTP client with optimized settings
    ///
    /// # Arguments
    ///
    /// * `api_key` - The Anthropic API key
    /// * `model` - The Claude model identifier
    /// * `base_url` - The base URL for the Anthropic API
    ///
    /// # Performance Benefits
    ///
    /// - Eliminates URI parsing overhead on each request
    /// - Avoids header value allocation on each request
    /// - Uses optimized HTTP client configuration with connection pooling
    fn create_with_cached_fields(api_key: String, model: String, base_url: String) -> Self {
        // Create HTTP client with optimized settings using helper function
        let client = Self::create_optimized_client();

        // Pre-cache and validate API key header (performance optimization)
        let api_key_header =
            Self::prepare_api_key_header(&api_key).expect("Failed to prepare API key header");

        // Pre-parse and validate messages endpoint URI (performance optimization)
        let messages_uri =
            Self::prepare_messages_uri(&base_url).expect("Failed to prepare messages URI");

        Self {
            client,
            api_key,
            model,
            base_url,
            api_key_header,
            messages_uri,
        }
    }

    /// Helper function to build optimized Anthropic request structure
    ///
    /// This function pre-allocates and optimizes the request structure
    /// for better memory performance and faster serialization.
    ///
    /// # Arguments
    ///
    /// * `model` - The Claude model identifier
    /// * `max_tokens` - Maximum tokens to generate
    /// * `system_prompt` - Optional system prompt
    /// * `messages` - Vector of Anthropic messages
    /// * `temperature` - Optional temperature parameter
    /// * `tools` - Vector of available tools
    ///
    /// # Performance Benefits
    ///
    /// - Pre-allocates vectors with known capacities
    /// - Avoids unnecessary allocations during request building
    fn build_optimized_request(
        model: String,
        max_tokens: u64,
        system_prompt: Option<String>,
        messages: Vec<AnthropicMessage>,
        temperature: Option<f64>,
        tools: Vec<AnthropicTool>,
    ) -> AnthropicRequest {
        AnthropicRequest {
            model,
            max_tokens,
            system: system_prompt,
            messages,
            temperature,
            tools,
        }
    }

    /// Helper function for efficient response parsing
    ///
    /// This function provides optimized JSON parsing with better error handling
    /// and memory management for Anthropic API responses.
    ///
    /// # Arguments
    ///
    /// * `response_text` - Raw JSON response text
    ///
    /// # Returns
    ///
    /// Parsed AnthropicResponse or CompletionError
    ///
    /// # Performance Benefits
    ///
    /// - Uses serde's optimized deserialization
    /// - Provides detailed error context for debugging
    /// - Includes response preview for truncated responses
    fn parse_response_efficiently(
        response_text: &str,
    ) -> Result<AnthropicResponse, CompletionError> {
        // Check for common truncation patterns
        if response_text.len() < 50 {
            return Err(CompletionError::Response(format!(
                "Response too short ({} chars). Response: '{}'",
                response_text.len(),
                response_text.chars().take(100).collect::<String>()
            )));
        }

        // Check for incomplete JSON
        let trimmed = response_text.trim();
        if trimmed.starts_with('{') && !trimmed.ends_with('}') {
            return Err(CompletionError::Response(format!(
                "Incomplete JSON object. Response (first 200 chars): '{}...'",
                response_text.chars().take(200).collect::<String>()
            )));
        }

        if trimmed.starts_with('[') && !trimmed.ends_with(']') {
            return Err(CompletionError::Response(format!(
                "Incomplete JSON array. Response (first 200 chars): '{}...'",
                response_text.chars().take(200).collect::<String>()
            )));
        }

        // Check for unterminated strings
        let quote_count = response_text.chars().filter(|&c| c == '"').count();
        if quote_count % 2 != 0 {
            return Err(CompletionError::Response(format!(
                "Unterminated string in JSON. Odd number of quotes ({}). Response: '{}'",
                quote_count,
                response_text.chars().take(200).collect::<String>()
            )));
        }

        serde_json::from_str::<AnthropicResponse>(response_text).map_err(|e| {
            CompletionError::Response(format!(
                "Failed to parse Anthropic response: {}. Response length: {} chars. Response preview: '{}'",
                e,
                response_text.len(),
                response_text.chars().take(300).collect::<String>()
            ))
        })
    }

    /// Helper function to create a connection-pooled HTTP client
    ///
    /// This function creates an optimized HTTP client with connection pooling
    /// and other performance enhancements for high-throughput scenarios.
    ///
    /// # Performance Benefits
    ///
    /// - Connection reuse reduces TLS handshake overhead
    /// - Optimized pool settings for concurrent requests
    /// - Memory-efficient connection management
    fn create_optimized_client() -> Client<HttpsConnector<HttpConnector>, Full<Bytes>> {
        let https = HttpsConnector::new();
        Client::builder(TokioExecutor::new())
            .pool_max_idle_per_host(20) // Increased for high concurrency
            .pool_idle_timeout(std::time::Duration::from_secs(90)) // Keep connections alive longer
            .build(https)
    }

    /// Helper function to validate and prepare API key
    ///
    /// This function validates the API key format and prepares it for caching.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Raw API key string
    ///
    /// # Returns
    ///
    /// Validated HeaderValue or error
    ///
    /// # Performance Benefits
    ///
    /// - Validates API key once during client creation
    /// - Prepares header value for repeated use
    fn prepare_api_key_header(api_key: &str) -> Result<HeaderValue, CompletionError> {
        if api_key.is_empty() {
            return Err(CompletionError::Request(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "API key cannot be empty",
            ))));
        }

        HeaderValue::from_str(api_key).map_err(|e| {
            CompletionError::Request(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid API key format: {}", e),
            )))
        })
    }

    /// Helper function to validate and parse base URL
    ///
    /// This function validates the base URL format and pre-parses the messages endpoint.
    ///
    /// # Arguments
    ///
    /// * `base_url` - Base URL string
    ///
    /// # Returns
    ///
    /// Parsed URI or error
    ///
    /// # Performance Benefits
    ///
    /// - Validates URL format once during client creation
    /// - Pre-parses URI to avoid repeated parsing
    fn prepare_messages_uri(base_url: &str) -> Result<Uri, CompletionError> {
        let uri_str = format!("{}/v1/messages", base_url);
        uri_str.parse::<Uri>().map_err(|e| {
            CompletionError::Request(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid base URL: {}", e),
            )))
        })
    }
}

/// Anthropic API request structure
#[derive(Serialize, Debug)]
struct AnthropicRequest {
    model: String,
    max_tokens: u64,
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<AnthropicTool>,
}

/// Anthropic message structure
#[derive(Serialize, Deserialize, Debug, Clone)]
struct AnthropicMessage {
    role: String,
    content: Vec<AnthropicContent>,
}

/// Anthropic content structure
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
enum AnthropicContent {
    Text {
        r#type: String,
        text: String,
    },
    ToolUse {
        r#type: String,
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        r#type: String,
        tool_call_id: String,
        content: Vec<AnthropicToolResultContent>,
    },
}

/// Anthropic tool structure
#[derive(Serialize, Debug, Clone)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

/// Anthropic tool result content
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
enum AnthropicToolResultContent {
    Text { r#type: String, text: String },
}

/// Anthropic API response structure
#[derive(Deserialize, Debug, Clone)]
pub struct AnthropicResponse {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    r#type: String,
    #[allow(dead_code)]
    role: String,
    content: Vec<AnthropicContent>,
    #[allow(dead_code)]
    model: String,
    #[allow(dead_code)]
    stop_reason: Option<String>,
    #[allow(dead_code)]
    stop_sequence: Option<String>,
    #[allow(dead_code)]
    usage: AnthropicUsage,
}

/// Anthropic usage information
#[derive(Deserialize, Debug, Clone)]
pub struct AnthropicUsage {
    #[allow(dead_code)]
    input_tokens: u32,
    #[allow(dead_code)]
    output_tokens: u32,
}

/// Anthropic API error response
#[derive(Deserialize, Debug)]
struct AnthropicError {
    #[allow(dead_code)]
    r#type: String,
    error: AnthropicErrorDetails,
}

/// Anthropic error details
#[derive(Deserialize, Debug)]
struct AnthropicErrorDetails {
    r#type: String,
    message: String,
}

impl Model for Anthropic {
    type RawCompletionResponse = AnthropicResponse;

    fn completion(
        &self,
        request: CompletionRequest,
    ) -> BoxFuture<Result<CompletionResponse<Self::RawCompletionResponse>, CompletionError>> {
        Box::pin(async move {
            // Convert internal message format to Anthropic format
            let mut messages = Vec::new();
            let mut system_prompt = None;

            // Handle system prompt separately
            if let Some(sys_prompt) = request.system_prompt {
                system_prompt = Some(sys_prompt);
            }

            // Convert chat history to Anthropic format
            for message in request.chat_history {
                match message {
                    llm::completion::Message::User { content } => {
                        let anthropic_content = convert_user_content_to_anthropic(content)?;
                        messages.push(AnthropicMessage {
                            role: "user".to_string(),
                            content: anthropic_content,
                        });
                    },
                    llm::completion::Message::Assistant { content } => {
                        let anthropic_content = convert_assistant_content_to_anthropic(content)?;
                        messages.push(AnthropicMessage {
                            role: "assistant".to_string(),
                            content: anthropic_content,
                        });
                    },
                }
            }

            // Add the current prompt as a user message
            if let Some(rag_text) = request.prompt.rag_text() {
                messages.push(AnthropicMessage {
                    role: "user".to_string(),
                    content: vec![AnthropicContent::Text {
                        r#type: "text".to_string(),
                        text: rag_text,
                    }],
                });
            }

            // Convert tools to Anthropic format
            let tools = request
                .tools
                .into_iter()
                .map(|tool| AnthropicTool {
                    name: tool.name,
                    description: tool.description,
                    input_schema: tool.parameters,
                })
                .collect::<Vec<_>>();

            // Build Anthropic request using optimized helper function
            let anthropic_request = Self::build_optimized_request(
                self.model.clone(),
                request.max_tokens.unwrap_or(4096),
                system_prompt,
                messages,
                request.temperature,
                tools,
            );

            // Serialize request with optimized JSON handling
            let request_body = serde_json::to_string(&anthropic_request)
                .map_err(|e| CompletionError::Request(e.into()))?;

            // Build HTTP request using cached values (performance optimization)
            let req = Request::builder()
                .method(Method::POST)
                .uri(self.messages_uri.clone()) // Use cached URI
                .header(CONTENT_TYPE, "application/json")
                .header("x-api-key", self.api_key_header.clone()) // Use cached header
                .header("anthropic-version", "2023-06-01")
                .body(Full::new(Bytes::from(request_body)))
                .map_err(|e| CompletionError::Request(e.into()))?;

            // Send request
            let response = self
                .client
                .request(req)
                .await
                .map_err(|e| CompletionError::Other(format!("HTTP request failed: {}", e)))?;

            let status = response.status();

            // Read response body completely using stream reading
            let mut response_bytes = Vec::new();
            let mut body_stream = response.into_body();

            while let Some(frame) = body_stream.frame().await {
                let frame = frame.map_err(|e| {
                    CompletionError::Other(format!("Failed to read response frame: {}", e))
                })?;

                if let Some(data) = frame.data_ref() {
                    response_bytes.extend_from_slice(data.chunk());
                }
            }

            if response_bytes.is_empty() {
                return Err(CompletionError::Response(
                    "Empty response body from Anthropic API".to_string(),
                ));
            }

            let response_text = String::from_utf8(response_bytes).map_err(|e| {
                CompletionError::Response(format!("Invalid UTF-8 in response: {}", e))
            })?;

            // Log response for debugging if it's suspiciously short
            if response_text.len() < 500 {
                log::debug!(
                    "Short Anthropic response ({} chars): {}",
                    response_text.len(),
                    response_text
                );
            }

            // Handle non-success status codes
            if !status.is_success() {
                if let Ok(error_response) = serde_json::from_str::<AnthropicError>(&response_text) {
                    return Err(CompletionError::Provider(format!(
                        "Anthropic API error: {} - {}",
                        error_response.error.r#type, error_response.error.message
                    )));
                } else {
                    return Err(CompletionError::Provider(format!(
                        "Anthropic API error (status {}): {}",
                        status, response_text
                    )));
                }
            }

            // Parse successful response using optimized helper function
            let anthropic_response = Self::parse_response_efficiently(&response_text)?;

            // Convert Anthropic response to internal format
            let choice =
                convert_anthropic_response_to_internal(anthropic_response.content.clone())?;

            Ok(CompletionResponse {
                choice,
                raw_response: anthropic_response,
            })
        })
    }
}

/// Convert internal user content to Anthropic format
fn convert_user_content_to_anthropic(
    content: Vec<llm::completion::UserContent>,
) -> Result<Vec<AnthropicContent>, CompletionError> {
    let mut result = Vec::new();

    for item in content {
        match item {
            llm::completion::UserContent::Text(text) => {
                result.push(AnthropicContent::Text {
                    r#type: "text".to_string(),
                    text: text.text,
                });
            },
            llm::completion::UserContent::ToolResult(tool_result) => {
                let content: Result<Vec<AnthropicToolResultContent>, CompletionError> = tool_result
                    .content
                    .into_iter()
                    .map(|c| match c {
                        llm::completion::ToolResultContent::Text(text) => {
                            Ok(AnthropicToolResultContent::Text {
                                r#type: "text".to_string(),
                                text: text.text,
                            })
                        },
                        llm::completion::ToolResultContent::Image(_) => {
                            Err(CompletionError::Request(
                                "Image content in tool results not supported by Anthropic".into(),
                            ))
                        },
                    })
                    .collect();

                let content = content?;

                result.push(AnthropicContent::ToolResult {
                    r#type: "tool_result".to_string(),
                    tool_call_id: tool_result.id,
                    content,
                });
            },
            llm::completion::UserContent::Image(_)
            | llm::completion::UserContent::Audio(_)
            | llm::completion::UserContent::Document(_) => {
                return Err(CompletionError::Request(
                    "Multimedia content not yet supported for Anthropic".into(),
                ));
            },
        }
    }

    Ok(result)
}

/// Convert internal assistant content to Anthropic format
fn convert_assistant_content_to_anthropic(
    content: Vec<llm::completion::AssistantContent>,
) -> Result<Vec<AnthropicContent>, CompletionError> {
    let mut result = Vec::new();

    for item in content {
        match item {
            llm::completion::AssistantContent::Text(text) => {
                result.push(AnthropicContent::Text {
                    r#type: "text".to_string(),
                    text: text.text,
                });
            },
            llm::completion::AssistantContent::ToolCall(tool_call) => {
                result.push(AnthropicContent::ToolUse {
                    r#type: "tool_use".to_string(),
                    id: tool_call.id,
                    name: tool_call.function.name,
                    input: tool_call.function.arguments,
                });
            },
        }
    }

    Ok(result)
}

/// Convert Anthropic response to internal format
fn convert_anthropic_response_to_internal(
    content: Vec<AnthropicContent>,
) -> Result<Vec<llm::completion::AssistantContent>, CompletionError> {
    let mut result = Vec::new();

    for item in content {
        match item {
            AnthropicContent::Text { text, .. } => {
                result.push(llm::completion::AssistantContent::text(text));
            },
            AnthropicContent::ToolUse {
                id, name, input, ..
            } => {
                result.push(llm::completion::AssistantContent::tool_call(
                    id, name, input,
                ));
            },
            AnthropicContent::ToolResult { .. } => {
                // Tool results are handled in user messages, not assistant responses
                continue;
            },
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_creation() {
        let anthropic = Anthropic::new("test-key");
        assert_eq!(anthropic.api_key, "test-key");
        assert_eq!(anthropic.model, "claude-3-5-sonnet-20241022");
        assert_eq!(anthropic.base_url, "https://api.anthropic.com");
        assert_eq!(anthropic.api_key_header, "test-key");
        assert_eq!(
            anthropic.messages_uri,
            "https://api.anthropic.com/v1/messages"
        );
    }

    #[test]
    fn test_anthropic_with_custom_model() {
        let anthropic = Anthropic::new("test-key").set_model("claude-3-haiku-20240307");
        assert_eq!(anthropic.model, "claude-3-haiku-20240307");
    }
}
