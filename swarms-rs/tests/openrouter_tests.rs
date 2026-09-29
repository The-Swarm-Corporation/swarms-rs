//! Tests for the OpenRouter provider (and the OpenAI-compatible path it shares with the
//! OpenAI provider), run against a local mock HTTP server, so no API key is needed.

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use swarms_rs::llm::Model;
use swarms_rs::llm::completion::{AssistantContent, Message};
use swarms_rs::llm::provider::openrouter::{DEFAULT_MODEL, OpenRouter};
use swarms_rs::llm::request::{CompletionRequest, ToolDefinition};

/// A request the mock server received.
struct Captured {
    request_line: String,
    headers: Vec<(String, String)>,
    body: Value,
}

impl Captured {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Serve exactly one request with `status` and `body`, returning the base URL and the
/// captured request.
async fn mock_server(status: u16, body: String) -> (String, oneshot::Receiver<Captured>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        let header_end = loop {
            let n = socket.read(&mut chunk).await.unwrap();
            buf.extend_from_slice(&chunk[..n]);
            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break pos + 4;
            }
        };
        let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
        let mut lines = head.split("\r\n");
        let request_line = lines.next().unwrap().to_string();
        let headers: Vec<(String, String)> = lines
            .filter_map(|l| l.split_once(": "))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let content_length: usize = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
            .map(|(_, v)| v.parse().unwrap())
            .unwrap_or(0);
        while buf.len() < header_end + content_length {
            let n = socket.read(&mut chunk).await.unwrap();
            buf.extend_from_slice(&chunk[..n]);
        }
        let request_body = serde_json::from_slice(&buf[header_end..header_end + content_length])
            .unwrap_or(Value::Null);
        let response = format!(
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
        let _ = tx.send(Captured {
            request_line,
            headers,
            body: request_body,
        });
    });
    (format!("http://{addr}/api/v1"), rx)
}

fn chat_response(message: Value) -> String {
    json!({
        "id": "gen-1",
        "object": "chat.completion",
        "created": 1,
        "model": "openrouter/auto",
        "choices": [{"index": 0, "message": message, "finish_reason": "stop"}]
    })
    .to_string()
}

fn request(prompt: &str) -> CompletionRequest {
    CompletionRequest {
        prompt: Message::user(prompt),
        system_prompt: Some("Be brief.".to_string()),
        chat_history: vec![],
        tools: vec![],
        temperature: None,
        max_tokens: Some(256),
    }
}

#[tokio::test]
async fn sends_openrouter_request_and_parses_text() {
    // `"tool_calls": []` on a plain reply is common with OpenAI-compatible backends and used
    // to make the reply come back empty.
    let (base_url, captured) = mock_server(
        200,
        chat_response(json!({"role": "assistant", "content": "Hello!", "tool_calls": []})),
    )
    .await;

    let client = OpenRouter::from_url(base_url, "test-key".to_string())
        .with_app_url("https://example.com")
        .with_app_name("swarms-rs tests");
    let response = client.completion(request("Hi")).await.unwrap();

    assert_eq!(response.choice.len(), 1);
    assert!(matches!(&response.choice[0], AssistantContent::Text(t) if t.text == "Hello!"));

    let captured = captured.await.unwrap();
    assert_eq!(
        captured.request_line,
        "POST /api/v1/chat/completions HTTP/1.1"
    );
    assert_eq!(captured.header("authorization"), Some("Bearer test-key"));
    assert_eq!(captured.header("http-referer"), Some("https://example.com"));
    assert_eq!(
        captured.header("x-openrouter-title"),
        Some("swarms-rs tests")
    );
    assert_eq!(captured.body["model"], DEFAULT_MODEL);
    // OpenRouter documents `max_tokens`; `max_completion_tokens` is only for api.openai.com.
    assert_eq!(captured.body["max_tokens"], 256);
    assert!(captured.body.get("max_completion_tokens").is_none());
    assert!(captured.body.get("temperature").is_none());
    assert_eq!(captured.body["messages"][0]["role"], "system");
    assert_eq!(captured.body["messages"][1]["content"], "Hi");
}

#[tokio::test]
async fn keeps_text_and_tool_calls_and_accepts_empty_arguments() {
    let (base_url, captured) = mock_server(
        200,
        chat_response(json!({
            "role": "assistant",
            "content": "Let me check.",
            "tool_calls": [
                {"id": "call_1", "type": "function",
                 "function": {"name": "get_time", "arguments": ""}},
                {"id": "call_2", "type": "function",
                 "function": {"name": "get_weather", "arguments": "{\"city\":\"Paris\"}"}}
            ]
        })),
    )
    .await;

    let client = OpenRouter::from_url(base_url, "test-key".to_string()).set_model("openai/gpt-4o");
    let mut req = request("What time is it, and the weather in Paris?");
    req.tools = vec![ToolDefinition {
        name: "get_time".to_string(),
        description: "Current time".to_string(),
        parameters: json!({"type": "object", "properties": {}}),
    }];
    let response = client.completion(req).await.unwrap();

    assert_eq!(response.choice.len(), 3);
    assert!(matches!(&response.choice[0], AssistantContent::Text(t) if t.text == "Let me check."));
    let AssistantContent::ToolCall(call) = &response.choice[1] else {
        panic!("expected a tool call");
    };
    assert_eq!(call.function.name, "get_time");
    assert_eq!(call.function.arguments, json!({}));
    let AssistantContent::ToolCall(call) = &response.choice[2] else {
        panic!("expected a tool call");
    };
    assert_eq!(call.function.arguments, json!({"city": "Paris"}));

    let captured = captured.await.unwrap();
    assert_eq!(captured.body["model"], "openai/gpt-4o");
    assert_eq!(captured.body["tools"][0]["function"]["name"], "get_time");
}

#[tokio::test]
async fn invalid_tool_arguments_are_an_error_not_a_panic() {
    let (base_url, _captured) = mock_server(
        200,
        chat_response(json!({
            "role": "assistant",
            "content": null,
            "tool_calls": [{"id": "call_1", "type": "function",
                            "function": {"name": "search", "arguments": "{\"query\": \"trunc"}}]
        })),
    )
    .await;

    let client = OpenRouter::from_url(base_url, "test-key".to_string());
    let err = client.completion(request("search")).await.unwrap_err();
    assert!(
        err.to_string()
            .contains("Invalid JSON arguments for tool 'search'"),
        "{err}"
    );
}

#[tokio::test]
async fn refusal_is_reported() {
    let (base_url, _captured) = mock_server(
        200,
        chat_response(
            json!({"role": "assistant", "content": null, "refusal": "I can't help with that."}),
        ),
    )
    .await;

    let client = OpenRouter::from_url(base_url, "test-key".to_string());
    let err = client.completion(request("...")).await.unwrap_err();
    assert!(err.to_string().contains("I can't help with that."), "{err}");
}

#[tokio::test]
async fn non_json_error_body_is_surfaced() {
    let (base_url, _captured) =
        mock_server(400, "<html>Bad Request from proxy</html>".to_string()).await;

    let client = OpenRouter::from_url(base_url, "test-key".to_string());
    let err = client.completion(request("Hi")).await.unwrap_err();
    assert!(err.to_string().contains("Bad Request from proxy"), "{err}");
}

#[test]
fn defaults_to_auto_router() {
    assert_eq!(DEFAULT_MODEL, "openrouter/auto");
    // Constructing with an invalid header value logs and keeps the client usable.
    let _client = OpenRouter::new("test-key").with_app_name("bad\nname");
}
