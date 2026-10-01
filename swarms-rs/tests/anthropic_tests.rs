//! Integration tests for the Anthropic Claude provider
//!
//! This test suite covers:
//! - Client creation and configuration with real environment variables
//! - Public API behavior with actual API calls
//! - Error handling scenarios
//!
//! Note: These tests require valid ANTHROPIC_API_KEY environment variable to run.
//! Set the environment variable before running tests:
//! export ANTHROPIC_API_KEY="your-api-key-here"

use std::env;

use serde_json::json;

use swarms_rs::llm::Model;
use swarms_rs::llm::completion::{AssistantContent, Message};
use swarms_rs::llm::provider::anthropic::*;
use swarms_rs::llm::request::{CompletionRequest, ToolDefinition};

// ============================================================================
// Unit Tests (No API Calls Required)
// ============================================================================

#[cfg(test)]
mod unit_tests {
    use super::*;
    use swarms_rs::llm::provider::any::ModelNameError;

    /// Tests run in parallel threads, so the ones that set or remove
    /// `ANTHROPIC_API_KEY` must not interleave.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        // A should_panic test poisons the lock; the guarded state is still fine.
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn test_anthropic_new() {
        let client = Anthropic::new("test-api-key");
        assert_eq!(client.model(), "claude-opus-5-5");
    }

    #[test]
    fn test_anthropic_from_url() {
        let client = Anthropic::from_url("https://custom.anthropic.com", "test-key");
        assert_eq!(client.model(), "claude-opus-5-5");
    }

    #[test]
    #[should_panic(expected = "ANTHROPIC_API_KEY environment variable is not set")]
    fn test_anthropic_from_env_missing_key() {
        let _guard = env_lock();
        // Ensure API key is not set
        unsafe {
            env::remove_var("ANTHROPIC_API_KEY");
        }

        // This should panic with the expected message
        let _ = Anthropic::from_env();
    }

    #[test]
    fn test_anthropic_from_env_with_key() {
        let _guard = env_lock();
        // Set API key in a scoped way to avoid interference with other tests
        {
            unsafe {
                env::set_var("ANTHROPIC_API_KEY", "test-key-from-env");
            }

            let client = Anthropic::from_env();
            assert_eq!(client.model(), "claude-opus-5-5");
        }

        // Clean up
        unsafe {
            env::remove_var("ANTHROPIC_API_KEY");
        }
    }

    #[test]
    fn test_anthropic_from_env_with_model() {
        let _guard = env_lock();
        // Set API key in a scoped way to avoid interference with other tests
        {
            unsafe {
                env::set_var("ANTHROPIC_API_KEY", "test-key-from-env");
            }

            let client = Anthropic::from_env_with_model("claude-haiku-4-5");
            assert_eq!(client.model(), "claude-haiku-4-5");
        }

        // Clean up
        unsafe {
            env::remove_var("ANTHROPIC_API_KEY");
        }
    }

    #[test]
    fn test_anthropic_try_from_env_missing_key() {
        let _guard = env_lock();
        unsafe {
            env::remove_var("ANTHROPIC_API_KEY");
        }

        let error = Anthropic::try_from_env()
            .err()
            .expect("missing API key should return an error");

        assert_eq!(
            error,
            ModelNameError::MissingApiKey {
                model: "claude-opus-5-5".to_string(),
                var: "ANTHROPIC_API_KEY",
            }
        );
    }

    #[test]
    fn test_anthropic_try_from_env_empty_key() {
        let _guard = env_lock();
        unsafe {
            env::set_var("ANTHROPIC_API_KEY", "");
        }

        let result = Anthropic::try_from_env();
        unsafe {
            env::remove_var("ANTHROPIC_API_KEY");
        }

        assert_eq!(
            result.err().expect("empty API key should return an error"),
            ModelNameError::MissingApiKey {
                model: "claude-opus-5-5".to_string(),
                var: "ANTHROPIC_API_KEY",
            }
        );
    }

    #[test]
    fn test_anthropic_try_from_env_with_key() {
        let _guard = env_lock();
        unsafe {
            env::set_var("ANTHROPIC_API_KEY", "test-key-from-env");
        }

        let result = Anthropic::try_from_env();
        unsafe {
            env::remove_var("ANTHROPIC_API_KEY");
        }

        let client = result.expect("supplied API key should allow construction");
        assert_eq!(client.model(), "claude-opus-5-5");
    }

    #[test]
    fn test_anthropic_try_from_env_with_model_missing_key() {
        let _guard = env_lock();
        unsafe {
            env::remove_var("ANTHROPIC_API_KEY");
        }

        let error = Anthropic::try_from_env_with_model("claude-haiku-4-5".to_string())
            .err()
            .expect("missing API key should return an error");

        assert_eq!(
            error,
            ModelNameError::MissingApiKey {
                model: "claude-haiku-4-5".to_string(),
                var: "ANTHROPIC_API_KEY",
            }
        );
    }

    #[test]
    fn test_anthropic_try_from_env_with_model_empty_key() {
        let _guard = env_lock();
        unsafe {
            env::set_var("ANTHROPIC_API_KEY", "");
        }

        let result = Anthropic::try_from_env_with_model("claude-haiku-4-5");
        unsafe {
            env::remove_var("ANTHROPIC_API_KEY");
        }

        assert_eq!(
            result.err().expect("empty API key should return an error"),
            ModelNameError::MissingApiKey {
                model: "claude-haiku-4-5".to_string(),
                var: "ANTHROPIC_API_KEY",
            }
        );
    }

    #[test]
    fn test_anthropic_try_from_env_with_model() {
        let _guard = env_lock();
        unsafe {
            env::set_var("ANTHROPIC_API_KEY", "test-key-from-env");
        }

        let result = Anthropic::try_from_env_with_model("claude-haiku-4-5");
        unsafe {
            env::remove_var("ANTHROPIC_API_KEY");
        }

        let client = result.expect("supplied API key should allow construction");
        assert_eq!(client.model(), "claude-haiku-4-5");
    }

    #[test]
    fn test_set_model() {
        let client = Anthropic::new("test-key").set_model("claude-opus-5-5");
        assert_eq!(client.model(), "claude-opus-5-5");
    }

    #[test]
    fn test_client_clone() {
        let client = Anthropic::new("test-key").set_model("custom-model");
        let cloned = client.clone();
        assert_eq!(cloned.model(), client.model());
    }

    #[test]
    fn test_client_implements_model_trait() {
        let client = Anthropic::new("test-key");
        // This tests that Anthropic implements the Model trait
        let _model: &dyn Model<RawCompletionResponse = _> = &client;
    }

    #[test]
    fn test_request_creation() {
        let request = CompletionRequest {
            prompt: Message::user("Hello!"),
            system_prompt: Some("You are helpful.".to_string()),
            chat_history: vec![],
            tools: vec![],
            temperature: Some(0.5),
            max_tokens: Some(100),
        };

        assert_eq!(request.system_prompt, Some("You are helpful.".to_string()));
        assert_eq!(request.temperature, Some(0.5));
        assert_eq!(request.max_tokens, Some(100));
        assert!(request.tools.is_empty());
        assert!(request.chat_history.is_empty());
    }

    #[test]
    fn test_tool_definition_creation() {
        let tool = ToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters: json!({"type": "object", "properties": {"param": {"type": "string"}}}),
        };

        assert_eq!(tool.name, "test_tool");
        assert_eq!(tool.description, "A test tool");
        assert!(tool.parameters["properties"]["param"]["type"].is_string());
    }
}

// ============================================================================
// Integration Tests (Require API Key)
// ============================================================================

#[cfg(test)]
mod integration_tests {
    use super::*;
    use tokio::test as async_test;

    /// Helper function to check if we have a valid API key for integration tests
    fn has_valid_api_key() -> bool {
        match env::var("ANTHROPIC_API_KEY") {
            Ok(key) => {
                !key.is_empty()
                    && key != "dummy-key"
                    && key != "test-key"
                    && key.starts_with("sk-ant-")
            },
            Err(_) => false,
        }
    }

    #[async_test]
    async fn test_basic_completion_with_env() {
        if !has_valid_api_key() {
            println!("Skipping test: ANTHROPIC_API_KEY not set");
            return;
        }

        let client = Anthropic::from_env();
        let request = CompletionRequest {
            prompt: Message::user("Say hello in exactly 3 words."),
            system_prompt: Some("You are a helpful assistant.".to_string()),
            chat_history: vec![],
            tools: vec![],
            temperature: None,
            max_tokens: Some(1024),
        };

        let result = client.completion(request).await;
        assert!(
            result.is_ok(),
            "Completion should succeed with valid API key"
        );

        let response = result.unwrap();
        assert!(
            !response.choice.is_empty(),
            "Should have at least one response choice"
        );

        match &response.choice[0] {
            AssistantContent::Text(text) => {
                assert!(!text.text.is_empty(), "Response text should not be empty");
                println!("Response: {}", text.text);
            },
            _ => panic!("Expected text response"),
        }
    }

    #[async_test]
    async fn test_completion_with_different_models() {
        if !has_valid_api_key() {
            println!("Skipping test: ANTHROPIC_API_KEY not set");
            return;
        }

        let models = vec!["claude-haiku-4-5"];

        for model_name in models {
            let client = Anthropic::from_env().set_model(model_name);
            let request = CompletionRequest {
                prompt: Message::user("What is 2+2? Answer with just the number."),
                system_prompt: None,
                chat_history: vec![],
                tools: vec![],
                temperature: Some(0.0),
                max_tokens: Some(10),
            };

            let result = client.completion(request).await;
            assert!(
                result.is_ok(),
                "Completion should succeed for model {}",
                model_name
            );

            let response = result.unwrap();
            assert!(
                !response.choice.is_empty(),
                "Should have response for model {}",
                model_name
            );

            match &response.choice[0] {
                AssistantContent::Text(text) => {
                    assert!(
                        text.text.trim() == "4",
                        "Should contain correct answer '4' for model {}, got: '{}'",
                        model_name,
                        text.text
                    );
                },
                _ => panic!("Expected text response for model {}", model_name),
            }
        }
    }

    #[async_test]
    async fn test_completion_with_chat_history() {
        if !has_valid_api_key() {
            println!("Skipping test: ANTHROPIC_API_KEY not set");
            return;
        }

        let client = Anthropic::from_env();
        let chat_history = vec![
            Message::user("My name is Alice."),
            Message::assistant("Hello Alice! Nice to meet you."),
        ];

        let request = CompletionRequest {
            prompt: Message::user("What's my name?"),
            system_prompt: None,
            chat_history,
            tools: vec![],
            temperature: None,
            max_tokens: Some(1024),
        };

        let result = client.completion(request).await;
        assert!(
            result.is_ok(),
            "Completion should succeed with chat history"
        );

        let response = result.unwrap();
        assert!(!response.choice.is_empty(), "Should have response");

        match &response.choice[0] {
            AssistantContent::Text(text) => {
                assert!(!text.text.is_empty(), "Response should not be empty");
                // The response should mention "Alice" since that's in the chat history
                assert!(
                    text.text.to_lowercase().contains("alice"),
                    "Response should remember the name from chat history: {}",
                    text.text
                );
            },
            _ => panic!("Expected text response"),
        }
    }

    #[async_test]
    async fn test_completion_error_handling() {
        // Test with invalid API key
        let client = Anthropic::new("invalid-api-key");
        let request = CompletionRequest {
            prompt: Message::user("Hello"),
            system_prompt: None,
            chat_history: vec![],
            tools: vec![],
            temperature: None,
            max_tokens: Some(10),
        };

        let result = client.completion(request).await;
        assert!(result.is_err(), "Should fail with invalid API key");

        let error = result.unwrap_err();
        match error {
            swarms_rs::llm::CompletionError::Provider(msg) => {
                assert!(
                    msg.contains("error")
                        || msg.contains("invalid")
                        || msg.contains("unauthorized"),
                    "Error message should indicate authentication issue: {}",
                    msg
                );
            },
            _ => panic!("Expected provider error for invalid API key"),
        }
    }

    #[async_test]
    async fn test_completion_with_tools() {
        if !has_valid_api_key() {
            println!("Skipping test: ANTHROPIC_API_KEY not set");
            return;
        }

        let tools = vec![ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get weather information for a location".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "The city or location to get weather for"
                    }
                },
                "required": ["location"]
            }),
        }];

        let client = Anthropic::from_env();
        let request = CompletionRequest {
            prompt: Message::user("What's the weather like in San Francisco?"),
            system_prompt: Some("You are a helpful assistant with access to tools. Use tools when appropriate to answer questions.".to_string()),
            chat_history: vec![],
            tools,
            temperature: None,
            max_tokens: Some(1024),
        };

        let result = client.completion(request).await;
        assert!(result.is_ok(), "Completion should succeed with tools");

        let response = result.unwrap();
        assert!(!response.choice.is_empty(), "Should have response");

        // The response might be text or a tool call - both are valid
        match &response.choice[0] {
            AssistantContent::Text(text) => {
                assert!(!text.text.is_empty(), "Text response should not be empty");
                println!("Tool response (text): {}", text.text);
            },
            AssistantContent::ToolCall(tool_call) => {
                assert_eq!(
                    tool_call.function.name, "get_weather",
                    "Should call weather tool"
                );
                assert!(
                    tool_call.function.arguments["location"].is_string(),
                    "Should have location parameter"
                );
                println!("Tool response (tool call): {}", tool_call.function.name);
            },
        }
    }
}

// ============================================================================
// Response handling against a local mock server (no API key needed)
// ============================================================================

mod mock_server_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Answer one request with `body` (status 200) and return the base URL.
    async fn serve_once(body: serde_json::Value) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = socket.read(&mut chunk).await.unwrap();
                buf.extend_from_slice(&chunk[..n]);
                let head_end = buf.windows(4).position(|w| w == b"\r\n\r\n");
                if let Some(end) = head_end {
                    let head = String::from_utf8_lossy(&buf[..end]).to_lowercase();
                    let len: usize = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length: "))
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(0);
                    if buf.len() >= end + 4 + len {
                        break;
                    }
                }
            }
            let body = body.to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        format!("http://{addr}")
    }

    fn message(content: serde_json::Value, stop_reason: &str) -> serde_json::Value {
        json!({
            "id": "msg_1", "type": "message", "role": "assistant", "model": "claude-opus-5-5",
            "content": content, "stop_reason": stop_reason, "stop_sequence": null,
            "usage": {"input_tokens": 1, "output_tokens": 1}
        })
    }

    fn request() -> CompletionRequest {
        CompletionRequest {
            prompt: Message::user("Hi"),
            system_prompt: None,
            chat_history: vec![],
            tools: vec![],
            temperature: None,
            max_tokens: Some(1024),
        }
    }

    #[tokio::test]
    async fn thinking_blocks_are_skipped_and_text_is_kept() {
        let base = serve_once(message(
            json!([
                {"type": "thinking", "thinking": "", "signature": "sig"},
                {"type": "text", "text": "The TV is 55\" wide."}
            ]),
            "end_turn",
        ))
        .await;
        let response = Anthropic::from_url(base, "key".to_string())
            .completion(request())
            .await
            .unwrap();
        assert_eq!(response.choice.len(), 1);
        assert!(
            matches!(&response.choice[0], AssistantContent::Text(t) if t.text == "The TV is 55\" wide.")
        );
    }

    #[tokio::test]
    async fn refusal_is_an_error_with_details() {
        let mut body = message(json!([]), "refusal");
        body["stop_details"] = json!({"type": "refusal", "category": "cyber"});
        let base = serve_once(body).await;
        let err = Anthropic::from_url(base, "key".to_string())
            .completion(request())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("refusal"), "{err}");
        assert!(err.to_string().contains("cyber"), "{err}");
    }

    #[tokio::test]
    async fn truncated_tool_call_is_not_returned() {
        let base = serve_once(message(
            json!([{"type": "tool_use", "id": "t1", "name": "search", "input": {"q": "par"}}]),
            "max_tokens",
        ))
        .await;
        let err = Anthropic::from_url(base, "key".to_string())
            .completion(request())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("max_tokens"), "{err}");
    }
}
