# Anthropic Claude Provider for Swarms-RS

This document provides comprehensive guidance for using Anthropic's Claude models with the Swarms-RS framework.

## 🚀 Quick Start

### 1. Environment Setup

```bash
# Set your Anthropic API key
export ANTHROPIC_API_KEY="your-api-key-here"

# Optional: Set custom base URL (defaults to https://api.anthropic.com)
export ANTHROPIC_BASE_URL="https://api.anthropic.com"
```

### 2. Basic Usage

```rust
use swarms_rs::llm::provider::anthropic::Anthropic;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create Anthropic client from environment
    let model = Anthropic::from_env();

    // Build agent with Claude (convenience builder)
    let agent = model
        .agent_builder()
        .agent_name("ClaudeAssistant")
        .system_prompt("You are Claude, a helpful AI assistant created by Anthropic.")
        .max_loops(3)
        .verbose(true)
        .build();

    // Execute a task
    let result = agent.run("Explain quantum computing in simple terms.".to_string()).await?;
    println!("Response: {}", result);

    Ok(())
}
```

### 3. Run the Example

```bash
# Run the comprehensive example
cargo run --example anthropic_claude_agent

# Run specific example functions
cargo run --example anthropic_claude_agent -- --help
```

## 🧠 Available Claude Models

| Model                        | Context Window | Description            | Best For                         |
| ---------------------------- | -------------- | ---------------------- | -------------------------------- |
| `claude-3-5-sonnet-20241022` | 200K           | Most intelligent model | Complex analysis, creative tasks |
| `claude-3-5-haiku-20241022`  | 200K           | Fast and efficient     | Quick responses, simple tasks    |
| `claude-3-opus-20240229`     | 200K           | Most powerful model    | Maximum intelligence required    |
| `claude-3-sonnet-20240229`   | 200K           | Balanced performance   | General purpose                  |
| `claude-3-haiku-20240307`    | 200K           | Fastest model          | High-throughput applications     |

### Model Selection Examples

```rust
// For complex analytical tasks
let sonnet_model = Anthropic::from_env_with_model("claude-3-5-sonnet-20241022");

// For fast, simple responses
let haiku_model = Anthropic::from_env_with_model("claude-3-5-haiku-20241022");

// For maximum intelligence
let opus_model = Anthropic::from_env_with_model("claude-3-opus-20240229");
```

## 🛠️ Advanced Configuration

### Custom Agent Configuration

```rust
use swarms_rs::llm::provider::anthropic::Anthropic;

let model = Anthropic::from_env();

let agent = model
    .agent_builder()
    .agent_name("AdvancedClaude")
    .system_prompt("You are an advanced AI assistant with expertise in multiple domains.")
    .max_loops(5)
    .temperature(0.3)        // Lower temperature for more focused responses
    .max_tokens(4096)        // Increase token limit for detailed responses
    .enable_plan(Some("Create a detailed plan for solving: ".to_string()))
    .enable_autosave()
    .save_state_dir("./claude_agent_states")
    .retry_attempts(3)
    .verbose(true)
    .build();
```

### Tool Integration

```rust
use swarms_rs::agent::SwarmsAgentBuilder;
use swarms_rs::llm::provider::anthropic::Anthropic;
use swarms_rs::structs::tool::Tool;

// Define a custom tool
struct CalculatorTool;

impl Tool for CalculatorTool {
    fn name(&self) -> &str { "calculator" }
    fn definition(&self) -> swarms_rs::llm::request::ToolDefinition {
        serde_json::json!({
            "name": "calculator",
            "description": "Perform mathematical calculations",
            "parameters": {
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "Mathematical expression to evaluate"
                    }
                },
                "required": ["expression"]
            }
        }).into()
    }
}

impl swarms_rs::structs::tool::ToolDyn for CalculatorTool {
    fn call(&self, args: String) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, swarms_rs::structs::tool::ToolError>> + Send + '_>> {
        Box::pin(async move {
            // Implementation here
            Ok("42".to_string())
        })
    }
}

// Use with agent
let agent = Anthropic::from_env()
    .agent_builder()
    .add_tool(CalculatorTool)
    .system_prompt("You can use the calculator tool for mathematical computations.")
    .build();
```

## ⚡ Performance Optimization

### Connection Management

The Anthropic provider uses hyper's connection pooling for optimal performance:

- **Connection Reuse**: Automatically reuses connections to reduce latency
- **TLS Optimization**: Efficient TLS handshake handling
- **Concurrent Requests**: Supports multiple simultaneous requests

### Memory Efficiency

- **Zero-copy Request Building**: Minimizes memory allocations
- **Efficient Serialization**: Optimized JSON handling with serde
- **Streaming Responses**: Handles large responses efficiently

### Benchmarking Performance

```rust
use std::time::Instant;

// Compare model performance
let models = vec![
    ("claude-3-5-haiku-20241022", "Fast"),
    ("claude-3-5-sonnet-20241022", "Balanced"),
    ("claude-3-opus-20240229", "Powerful"),
];

for (model_name, description) in models {
    let start = Instant::now();
    let model = Anthropic::from_env().set_model(model_name);

    let agent = model
        .agent_builder()
        .max_loops(1)
        .verbose(false)
        .build();

    let result = agent.run("Hello, Claude!".to_string()).await?;
    let duration = start.elapsed();

    println!("{} ({}): {:.2}s", description, model_name, duration.as_secs_f64());
}
```

## 🛡️ Error Handling

The provider includes comprehensive error handling:

### API Errors

```rust
match agent.run(task).await {
    Ok(response) => println!("Success: {}", response),
    Err(e) => match e {
        swarms_rs::structs::agent::AgentError::ProviderError(msg) => {
            println!("Anthropic API error: {}", msg);
        }
        swarms_rs::structs::agent::AgentError::HttpError(_) => {
            println!("Network error - check your connection");
        }
        _ => println!("Other error: {}", e),
    }
}
```

### Common Issues

1. **API Key Not Set**: Ensure `ANTHROPIC_API_KEY` environment variable is set
2. **Rate Limits**: Claude has rate limits; implement exponential backoff
3. **Context Window**: Respect the 200K token context window limit
4. **Invalid Model**: Use only supported model identifiers

## 🔧 Development and Testing

### Running Tests

```bash
# Run Anthropic provider tests
cargo test anthropic

# Run with verbose output
cargo test anthropic -- --nocapture
```

### Integration Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_claude_basic_completion() {
        let model = Anthropic::new("test-key");
        // Mock or integration test here
    }
}
```

## 📊 Monitoring and Logging

### Enable Verbose Logging

```rust
let agent = Anthropic::from_env()
    .agent_builder()
    .verbose(true)  // Enable detailed logging
    .build();
```

### Custom Logging

```rust
use swarms_rs::logging;

let agent = Anthropic::from_env()
    .agent_builder()
    .verbose(true)
    .build();

// Logs will include:
// - Task initialization
// - API request/response details
// - Tool execution
// - Performance metrics
// - Error information
```

## 🌐 Production Deployment

### Environment Variables

```bash
# Production configuration
export ANTHROPIC_API_KEY="sk-ant-prod-..."
export ANTHROPIC_BASE_URL="https://api.anthropic.com"
export RUST_LOG="swarms_rs=info"
```

### Health Checks

```rust
async fn health_check() -> Result<(), Box<dyn std::error::Error>> {
    let model = Anthropic::from_env();
    let agent = SwarmsAgentBuilder::new_with_model(model)
        .max_loops(1)
        .verbose(false)
        .build();

    // Simple health check
    agent.run("Hello".to_string()).await?;
    Ok(())
}
```

## 🔗 Additional Resources

- [Anthropic API Documentation](https://docs.anthropic.com/)
- [Claude Model Comparison](https://docs.anthropic.com/claude/docs/models-overview)
- [Swarms-RS Documentation](../README.md)
- [Tool Integration Guide](./TOOL_INTEGRATION.md)

## 🤝 Contributing

When contributing to the Anthropic provider:

1. Follow Rust best practices
2. Add comprehensive documentation
3. Include unit and integration tests
4. Update this README for new features
5. Ensure performance optimizations are maintained

## 📝 License

This Anthropic provider is part of the Swarms-RS framework and follows the same Apache 2.0 license.
