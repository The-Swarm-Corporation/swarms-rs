# Using Anthropic Claude Models with Swarms-rs

Swarms-rs provides first-class support for Anthropic's Claude models, enabling you to build intelligent multi-agent systems with cutting-edge AI capabilities.

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Authentication](#authentication)
- [Available Models](#available-models)
- [Configuration](#configuration)
- [Usage Examples](#usage-examples)
  - [Basic Agent](#basic-agent)
  - [Advanced Configuration](#advanced-configuration)
  - [Tool Integration](#tool-integration)
  - [Multi-Agent Systems](#multi-agent-systems)
- [Performance Optimization](#performance-optimization)
- [Error Handling](#error-handling)
- [Best Practices](#best-practices)

## Installation

Anthropic support is already included in swarms-rs. Simply ensure you have the latest version:

```toml
[dependencies]
swarms-rs = "0.2.1"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

The simplest way to get started with Anthropic Claude:

```rust
use swarms_rs::agent::SwarmsAgentBuilder;
use swarms_rs::llm::provider::anthropic::Anthropic;
use swarms_rs::structs::agent::Agent;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create Anthropic client from environment variables
    let model = Anthropic::from_env();

    // Build agent with Claude
    let agent = SwarmsAgentBuilder::new_with_model(model)
        .agent_name("ClaudeAssistant")
        .system_prompt("You are a helpful AI assistant.")
        .build();

    // Run the agent
    let result = agent.run("Hello, Claude!".to_string()).await?;
    println!("Response: {}", result);

    Ok(())
}
```

## Authentication

### Environment Variables

Anthropic requires an API key for authentication. Set the following environment variable:

```bash
export ANTHROPIC_API_KEY="your-api-key-here"

# Optional: custom API endpoint (defaults to https://api.anthropic.com)
export ANTHROPIC_BASE_URL="https://api.anthropic.com"
```

### Methods of Creating a Client

#### From Environment Variables

```rust
use swarms_rs::llm::provider::anthropic::Anthropic;

// Uses default Claude 3.5 Sonnet model
let client = Anthropic::from_env();

// Uses specific model
let client = Anthropic::from_env_with_model("claude-3-5-haiku-20241022");
```

#### Direct Creation

```rust
use swarms_rs::llm::provider::anthropic::Anthropic;

// Create with API key directly
let client = Anthropic::new("your-api-key");

// Create with custom base URL
let client = Anthropic::from_url(
    "https://api.anthropic.com",
    "your-api-key"
);

// Set model after creation
let client = Anthropic::new("your-api-key")
    .set_model("claude-3-opus-20240229");
```

## Available Models

Anthropic regularly updates their model lineup. Here are the currently recommended models:

| Model                        | Release | Performance | Speed     | Use Case                            |
| ---------------------------- | ------- | ----------- | --------- | ----------------------------------- |
| `claude-3-5-sonnet-20241022` | Latest  | Highest     | Fast      | Complex analysis, reasoning, coding |
| `claude-3-5-haiku-20241022`  | Latest  | Good        | Very Fast | Quick responses, real-time apps     |
| `claude-3-opus-20240229`     | Legacy  | Maximum     | Slower    | Maximum intelligence required       |
| `claude-3-sonnet-20240229`   | Legacy  | High        | Balanced  | General purpose tasks               |
| `claude-3-haiku-20240307`    | Legacy  | Good        | Fastest   | High-throughput applications        |

**Note:** Always check the [Anthropic API documentation](https://docs.anthropic.com/en/api/models) for the latest available models.

## Configuration

### Basic Agent Configuration

```rust
use swarms_rs::agent::SwarmsAgentBuilder;
use swarms_rs::llm::provider::anthropic::Anthropic;

let agent = SwarmsAgentBuilder::new_with_model(Anthropic::from_env())
    .agent_name("MyAgent")
    .system_prompt("You are an expert in your field.")
    .max_loops(3)
    .temperature(0.7)
    .max_tokens(2048)
    .verbose(true)
    .build();
```

### Configuration Options

- **`agent_name`**: Name for the agent (used in logging)
- **`system_prompt`**: System instructions for the model
- **`max_loops`**: Maximum number of interaction loops (for agentic behavior)
- **`temperature`**: 0.0-1.0 (lower = more deterministic, higher = more creative)
- **`max_tokens`**: Maximum tokens to generate (Claude supports up to 4096)
- **`verbose`**: Enable detailed logging

## Usage Examples

### Basic Agent

A simple question-answering agent:

```rust
use swarms_rs::agent::SwarmsAgentBuilder;
use swarms_rs::llm::provider::anthropic::Anthropic;
use swarms_rs::structs::agent::Agent;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let agent = SwarmsAgentBuilder::new_with_model(
        Anthropic::from_env_with_model("claude-3-5-haiku-20241022")
    )
    .agent_name("SimpleAgent")
    .system_prompt("Answer questions concisely and accurately.")
    .build();

    let response = agent.run(
        "What are the three primary colors?".to_string()
    ).await?;

    println!("Agent Response: {}", response);
    Ok(())
}
```

### Advanced Configuration

Using Claude 3.5 Sonnet with custom parameters:

```rust
use swarms_rs::agent::SwarmsAgentBuilder;
use swarms_rs::llm::provider::anthropic::Anthropic;
use swarms_rs::structs::agent::Agent;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model = Anthropic::from_env_with_model("claude-3-5-sonnet-20241022");

    let agent = SwarmsAgentBuilder::new_with_model(model)
        .agent_name("AdvancedClaude")
        .system_prompt(
            "You are an expert AI assistant. Provide detailed, well-reasoned responses. \
             Always cite your sources when applicable."
        )
        .max_loops(5)
        .temperature(0.3)  // More deterministic for analysis
        .max_tokens(4096)
        .enable_plan(Some("Analyze the following: ".to_string()))
        .verbose(true)
        .build();

    let response = agent.run(
        "Analyze the impact of AI on software development.".to_string()
    ).await?;

    println!("Analysis: {}", response);
    Ok(())
}
```

### Tool Integration

Claude models excel at tool use. Here's how to integrate tools:

```rust
use swarms_rs::agent::SwarmsAgentBuilder;
use swarms_rs::llm::provider::anthropic::Anthropic;
use swarms_rs::structs::agent::Agent;
use swarms_rs::structs::tool::Tool;
use swarms_rs::llm::request::ToolDefinition;

// Define a custom tool
#[derive(Debug, Clone)]
struct WeatherTool;

#[derive(Debug, serde::Deserialize)]
struct WeatherArgs {
    location: String,
}

impl Tool for WeatherTool {
    type Error = std::io::Error;
    type Args = WeatherArgs;
    type Output = String;
    const NAME: &'static str = "get_weather";

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get current weather information for a specified location.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "The city and state/country for weather (e.g., 'San Francisco, CA')"
                    }
                },
                "required": ["location"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Simulate weather API call
        Ok(format!(
            "Weather in {}: Sunny, 72°F, light breeze",
            args.location
        ))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model = Anthropic::from_env();

    let agent = SwarmsAgentBuilder::new_with_model(model)
        .agent_name("WeatherAssistant")
        .system_prompt(
            "You are a helpful weather assistant. Use the weather tool to provide current \
             conditions for any location the user asks about."
        )
        .add_tool(WeatherTool)
        .build();

    let response = agent.run(
        "What's the weather in New York City right now?".to_string()
    ).await?;

    println!("Weather Info: {}", response);
    Ok(())
}
```

### Multi-Agent Systems

Combining multiple Claude agents:

```rust
use swarms_rs::agent::SwarmsAgentBuilder;
use swarms_rs::llm::provider::anthropic::Anthropic;
use swarms_rs::structs::agent::Agent;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Research agent
    let researcher = SwarmsAgentBuilder::new_with_model(
        Anthropic::from_env_with_model("claude-3-5-sonnet-20241022")
    )
    .agent_name("Researcher")
    .system_prompt("You are a research expert. Gather and analyze information thoroughly.")
    .temperature(0.5)
    .build();

    // Writer agent
    let writer = SwarmsAgentBuilder::new_with_model(
        Anthropic::from_env_with_model("claude-3-5-sonnet-20241022")
    )
    .agent_name("Writer")
    .system_prompt("You are a skilled writer. Create engaging, well-structured content.")
    .temperature(0.7)
    .build();

    // Get research
    let research = researcher.run(
        "What are the latest trends in machine learning?".to_string()
    ).await?;

    // Use research to write content
    let content = writer.run(
        format!("Write an article based on this research: {}", research)
    ).await?;

    println!("Final Article:\n{}", content);
    Ok(())
}
```

## Performance Optimization

### Connection Pooling

The Anthropic client uses hyper's connection pooling for optimal performance:

- **Pool size**: Up to 20 concurrent connections per host
- **Keep-alive**: 90 seconds (tuned for typical request patterns)
- **TLS reuse**: Connections are reused when possible

No special configuration is needed - connection pooling is automatic.

### Token Efficiency

Tips for optimal token usage:

1. **Be specific in system prompts**: Clear instructions reduce token consumption
2. **Use appropriate models**: Use Haiku for simple tasks, Sonnet for complex ones
3. **Set reasonable max_tokens**: Don't set unnecessarily high limits
4. **Cache repeated queries**: Consider implementing a response cache

### Streaming Responses

For real-time applications, consider implementing streaming:

```rust
// Streaming support can be added in future versions
// For now, use standard completion requests with reasonable max_tokens
let response = agent.run(
    "Stream this long response...".to_string()
).await?;
```

## Error Handling

The Anthropic client provides comprehensive error handling:

```rust
use swarms_rs::agent::SwarmsAgentBuilder;
use swarms_rs::llm::provider::anthropic::Anthropic;
use swarms_rs::structs::agent::Agent;
use swarms_rs::llm::CompletionError;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let agent = SwarmsAgentBuilder::new_with_model(Anthropic::from_env())
        .agent_name("ErrorHandlingAgent")
        .build();

    match agent.run("Hello".to_string()).await {
        Ok(response) => println!("Success: {}", response),
        Err(e) => {
            eprintln!("Error occurred: {}", e);
            // Handle specific error types
            match e.downcast_ref::<CompletionError>() {
                Some(CompletionError::Provider(msg)) => {
                    eprintln!("API Error: {}", msg);
                }
                Some(CompletionError::Http(http_err)) => {
                    eprintln!("Network Error: {}", http_err);
                }
                Some(CompletionError::Response(resp_err)) => {
                    eprintln!("Response Error: {}", resp_err);
                }
                _ => eprintln!("Other error: {:?}", e),
            }
        }
    }

    Ok(())
}
```

### Common Errors

| Error                       | Cause                        | Solution                            |
| --------------------------- | ---------------------------- | ----------------------------------- |
| `ANTHROPIC_API_KEY not set` | Missing environment variable | Set `ANTHROPIC_API_KEY`             |
| `Invalid API key format`    | Malformed API key            | Check key in Anthropic dashboard    |
| `Model not found`           | Invalid model ID             | Use valid model from available list |
| `Rate limit exceeded`       | Too many requests            | Implement exponential backoff       |
| `Empty response body`       | API connection issue         | Check network and API status        |

## Best Practices

### 1. System Prompts

Craft effective system prompts for better results:

```rust
// Good: Specific and detailed
let system_prompt = r#"
You are a data analysis expert. When analyzing data:
1. Start by understanding the dataset structure
2. Identify key patterns and outliers
3. Provide statistical summaries
4. Conclude with actionable insights
"#;

// Avoid: Vague or unclear
let system_prompt = "You are helpful";
```

### 2. Temperature Settings

Choose appropriate temperatures for your use case:

```rust
// Factual tasks (research, analysis)
.temperature(0.2)

// Balanced tasks (general Q&A)
.temperature(0.5)

// Creative tasks (writing, brainstorming)
.temperature(0.8)
```

### 3. Token Management

```rust
// For general Q&A
.max_tokens(1024)

// For detailed responses
.max_tokens(2048)

// For complex analysis
.max_tokens(4096)
```

### 4. Error Handling

Always handle errors gracefully:

```rust
match agent.run(input).await {
    Ok(response) => {
        // Process response
    }
    Err(e) => {
        // Log error with context
        eprintln!("Agent failed: {}", e);
        // Retry with fallback or notify user
    }
}
```

### 5. Rate Limiting

For high-throughput applications, implement rate limiting:

```rust
use std::time::Duration;
use tokio::time::sleep;

let mut backoff = Duration::from_millis(100);
let max_retries = 5;

for attempt in 0..max_retries {
    match agent.run(input.clone()).await {
        Ok(response) => return Ok(response),
        Err(e) => {
            if attempt < max_retries - 1 {
                sleep(backoff).await;
                backoff *= 2;
            } else {
                return Err(e);
            }
        }
    }
}
```

### 6. Logging

Enable detailed logging for debugging:

```bash
export SWARMS_LOG_LEVEL=DEBUG
export RUST_LOG=swarms_rs=debug
```

```rust
use swarms_rs::logging::init_logger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logger();
    // Your code here
    Ok(())
}
```

## Running Examples

The repository includes a complete example:

```bash
# Run the Anthropic example
cargo run --example anthropic_claude_agent
```

Make sure `ANTHROPIC_API_KEY` is set:

```bash
export ANTHROPIC_API_KEY="your-key-here"
cargo run --example anthropic_claude_agent
```

## Troubleshooting

### "ANTHROPIC_API_KEY is not set"

```bash
# Check if the variable is set
echo $ANTHROPIC_API_KEY

# Set it in your shell
export ANTHROPIC_API_KEY="sk-ant-..."

# Or add to .env file
echo "ANTHROPIC_API_KEY=sk-ant-..." > .env
```

### "HTTP request failed"

Check your internet connection and API endpoint:

```rust
// Test connectivity
let client = Anthropic::from_env();
// The client creation will validate API key format
```

### "Empty response body"

This may indicate an API issue. Check:

1. API key validity
2. Model availability
3. Network connectivity
4. API status page

### Performance Issues

If you experience slow responses:

1. Check model selection (use faster models like Haiku)
2. Verify network latency
3. Check Anthropic API status
4. Monitor token usage

## Resources

- [Anthropic API Documentation](https://docs.anthropic.com/)
- [Claude Model Documentation](https://docs.anthropic.com/en/api/models)
- [Swarms-rs Repository](https://github.com/The-Swarm-Corporation/swarms-rs)
- [Claude Prompt Engineering Guide](https://docs.anthropic.com/en/docs/build-a-claude-bot)

## Support

For issues or questions:

1. Check this documentation
2. Review the [examples](../examples/)
3. Open an issue on [GitHub](https://github.com/The-Swarm-Corporation/swarms-rs/issues)
4. Consult [Anthropic's support](https://support.anthropic.com/)
