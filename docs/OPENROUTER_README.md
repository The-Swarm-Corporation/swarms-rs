# OpenRouter LLM Integration Guide

## Overview

OpenRouter is a unified API gateway that provides access to multiple Large Language Models (LLMs) from different providers through a single, consistent interface. This guide covers the integration of OpenRouter with the Swarms framework.

## Table of Contents

1. [Getting Started](#getting-started)
2. [Installation & Setup](#installation--setup)
3. [Basic Usage](#basic-usage)
4. [Supported Models](#supported-models)
5. [Advanced Configuration](#advanced-configuration)
6. [Examples](#examples)
7. [Error Handling](#error-handling)
8. [Best Practices](#best-practices)

## Getting Started

### What is OpenRouter?

OpenRouter provides a unified interface to access various LLM providers:

- **OpenAI**: GPT-4, GPT-4 Turbo, GPT-3.5
- **Anthropic**: Claude 3.5 Sonnet, Claude 3 Opus, Claude 3 Haiku
- **Google**: Gemini Pro, Gemini 2.5
- **Meta**: Llama 2, Llama 3
- **Mistral**: Mistral Large, Mistral Medium
- **And many more...**

### Benefits

- **Cost Optimization**: Compare prices across providers and choose the most cost-effective option
- **Fallback Support**: Automatic fallback to alternative models if the primary is unavailable
- **Simplified Integration**: Single API for multiple providers
- **Provider Independence**: Switch providers without changing your code

## Installation & Setup

### 1. Get an API Key

1. Visit [https://openrouter.ai](https://openrouter.ai)
2. Create a free account
3. Navigate to your API keys dashboard
4. Generate a new API key
5. Copy and save your API key securely

### 2. Set Environment Variables

```bash
# Set your OpenRouter API key
export OPENROUTER_API_KEY="sk-or-xxxxxxxxxxxxx"

# Optional: Set custom base URL (defaults to https://openrouter.ai/api/v1)
export OPENROUTER_BASE_URL="https://openrouter.ai/api/v1"
```

### 3. Add to .env File

For local development, create a `.env` file:

```bash
# .env
OPENROUTER_API_KEY=sk-or-xxxxxxxxxxxxx
OPENROUTER_BASE_URL=https://openrouter.ai/api/v1
```

Then load it in your Rust code:

```rust
dotenv::dotenv().ok();
let model = OpenRouter::from_env();
```

## Basic Usage

### Simple Agent

```rust
use swarms_rs::llm::provider::openrouter::OpenRouter;
use swarms_rs::structs::agent::Agent;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    // Create OpenRouter client from environment
    let client = OpenRouter::from_env();

    // Build an agent
    let agent = client
        .agent_builder()
        .system_prompt("You are a helpful AI assistant.")
        .agent_name("MyAssistant")
        .max_loops(1)
        .build();

    // Run the agent
    let response = agent.run("Hello! What is 2+2?".to_string()).await?;
    println!("{}", response);

    Ok(())
}
```

### Direct Model Usage

```rust
use swarms_rs::llm::provider::openrouter::OpenRouter;
use swarms_rs::llm::Model;
use swarms_rs::llm::request::CompletionRequest;
use swarms_rs::llm::completion::{Message, UserContent, Text};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let model = OpenRouter::from_env();

    let request = CompletionRequest {
        prompt: Message::User {
            content: vec![UserContent::Text(Text {
                text: "What is machine learning?".to_string(),
            })],
        },
        system_prompt: Some("You are a helpful assistant.".to_string()),
        chat_history: vec![],
        tools: vec![],
        temperature: Some(0.7),
        max_tokens: Some(2048),
    };

    let response = model.completion(request).await?;
    println!("{:?}", response);

    Ok(())
}
```

## Supported Models

OpenRouter supports a wide range of models. Here are some popular ones:

### OpenAI Models

- `openai/gpt-4o` - Latest GPT-4 model
- `openai/gpt-4-turbo` - GPT-4 Turbo
- `openai/gpt-4` - Original GPT-4
- `openai/gpt-3.5-turbo` - Fast and economical

### Anthropic Claude Models

- `anthropic/claude-3.5-sonnet` - Latest Claude model
- `anthropic/claude-3-opus` - High-intelligence model
- `anthropic/claude-3-sonnet` - Fast model
- `anthropic/claude-3-haiku` - Lightweight model

### Google Gemini Models

- `google/gemini-2.5-pro` - Latest Gemini Pro
- `google/gemini-2.5-pro-exp` - Experimental Gemini Pro
- `google/gemini-2-flash` - Fast Gemini

### Meta Llama Models

- `meta-llama/llama-2-70b-chat` - Llama 2 70B Chat
- `meta-llama/llama-3-70b-instruct` - Llama 3 70B

### Mistral Models

- `mistralai/mistral-large` - Mistral Large
- `mistralai/mistral-medium` - Mistral Medium

For a complete list of supported models, visit the [OpenRouter models page](https://openrouter.ai/docs#models).

## Advanced Configuration

### Setting Model Parameters

```rust
let model = OpenRouter::from_env()
    .set_model("anthropic/claude-3.5-sonnet")
    .set_temperature(0.5)
    .set_max_tokens(4096);

let agent = model
    .agent_builder()
    .system_prompt("You are an expert AI assistant.")
    .build();
```

### Custom API Endpoint

```rust
let model = OpenRouter::new("your-api-key")
    .with_base_url("https://custom-proxy.example.com/v1")
    .set_model("openai/gpt-4o");
```

### Full Agent Configuration

```rust
use swarms_rs::llm::provider::openrouter::OpenRouter;

let model = OpenRouter::from_env()
    .set_model("anthropic/claude-3.5-sonnet")
    .set_temperature(0.3)
    .set_max_tokens(2048);

let agent = model
    .agent_builder()
    .agent_name("ResearchAssistant")
    .user_name("Researcher")
    .system_prompt(
        "You are an expert research assistant with deep knowledge in \
         scientific methodology and data analysis."
    )
    .max_loops(10)
    .temperature(0.3)
    .max_tokens(4096)
    .enable_autosave()
    .save_state_dir("./research_agent_state/")
    .enable_plan(Some("Create a detailed plan for the research task.".to_string()))
    .verbose(true)
    .build();
```

## Examples

### Example 1: Content Generation

```rust
use swarms_rs::llm::provider::openrouter::OpenRouter;
use swarms_rs::structs::agent::Agent;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let model = OpenRouter::from_env()
        .set_model("openai/gpt-4o")
        .set_temperature(0.7);

    let agent = model
        .agent_builder()
        .system_prompt(
            "You are a creative writing expert. Generate high-quality, \
             engaging content on requested topics."
        )
        .agent_name("ContentWriter")
        .build();

    let response = agent
        .run(
            "Write a compelling blog post introduction about the \
             future of artificial intelligence".to_string()
        )
        .await?;

    println!("{}", response);
    Ok(())
}
```

### Example 2: Multi-Provider Fallback

```rust
use swarms_rs::llm::provider::openrouter::OpenRouter;
use swarms_rs::structs::agent::Agent;

async fn get_response_with_fallback(query: &str) -> Result<String> {
    let models = vec![
        "anthropic/claude-3.5-sonnet",
        "openai/gpt-4o",
        "google/gemini-2-flash",
    ];

    for model_name in models {
        match OpenRouter::from_env().set_model(model_name).agent_builder()
            .system_prompt("Answer concisely and accurately.")
            .build()
            .run(query.to_string())
            .await
        {
            Ok(response) => return Ok(response),
            Err(e) => {
                eprintln!("Failed with {}: {}", model_name, e);
                continue;
            }
        }
    }

    Err(anyhow::anyhow!("All models failed"))
}
```

### Example 3: Tool-Enhanced Agent

```rust
use swarms_rs::llm::provider::openrouter::OpenRouter;
use swarms_rs::structs::agent::Agent;
use swarms_rs::llm::request::ToolDefinition;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let model = OpenRouter::from_env()
        .set_model("anthropic/claude-3.5-sonnet");

    let weather_tool = ToolDefinition {
        name: "get_weather".to_string(),
        description: "Get current weather information for a location".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "City and country"
                }
            },
            "required": ["location"]
        }),
    };

    let agent = model
        .agent_builder()
        .system_prompt("You can check the weather using the provided tool.")
        .agent_name("WeatherAssistant")
        .build();

    let response = agent
        .run("What's the weather in London?".to_string())
        .await?;

    println!("{}", response);
    Ok(())
}
```

## Error Handling

### Common Errors

```rust
use swarms_rs::llm::{Model, CompletionError};
use swarms_rs::llm::provider::openrouter::OpenRouter;
use swarms_rs::llm::request::CompletionRequest;

#[tokio::main]
async fn main() -> Result<()> {
    let model = OpenRouter::from_env();
    let request = CompletionRequest { /* ... */ };

    match model.completion(request).await {
        Ok(response) => {
            println!("Success: {:?}", response.choice);
        }
        Err(CompletionError::Http(e)) => {
            eprintln!("Network error: {}", e);
        }
        Err(CompletionError::Provider(e)) => {
            eprintln!("OpenRouter API error: {}", e);
        }
        Err(CompletionError::Json(e)) => {
            eprintln!("JSON parsing error: {}", e);
        }
        Err(e) => {
            eprintln!("Other error: {}", e);
        }
    }

    Ok(())
}
```

### Handling Missing API Key

```rust
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    match env::var("OPENROUTER_API_KEY") {
        Ok(_) => {
            let model = OpenRouter::from_env();
            // Use model...
        }
        Err(_) => {
            eprintln!("Error: OPENROUTER_API_KEY environment variable not set");
            eprintln!("Please get an API key from https://openrouter.ai");
            std::process::exit(1);
        }
    }

    Ok(())
}
```

## Best Practices

### 1. Environment Variable Management

```rust
// ✓ Good: Use environment variables
let model = OpenRouter::from_env();

// ✗ Bad: Hardcode API keys
let model = OpenRouter::new("sk-or-xxxxx");
```

### 2. Model Selection

```rust
// ✓ Good: Use fast models for simple tasks
let simple_model = OpenRouter::from_env()
    .set_model("openai/gpt-3.5-turbo");

// ✓ Good: Use powerful models for complex tasks
let complex_model = OpenRouter::from_env()
    .set_model("anthropic/claude-3.5-sonnet");
```

### 3. Error Handling

```rust
// ✓ Good: Handle errors explicitly
match agent.run(prompt).await {
    Ok(response) => println!("{}", response),
    Err(e) => eprintln!("Error: {}", e),
}

// ✗ Bad: Unwrap without handling
let response = agent.run(prompt).await.unwrap();
```

### 4. Token Limits

```rust
// ✓ Good: Set appropriate token limits
let model = OpenRouter::from_env()
    .set_max_tokens(2048);

// ✗ Bad: No limits
let model = OpenRouter::from_env();
```

### 5. Temperature Settings

```rust
// ✓ Good: Use low temperature for factual tasks
let factual_model = OpenRouter::from_env()
    .set_temperature(0.1);

// ✓ Good: Use high temperature for creative tasks
let creative_model = OpenRouter::from_env()
    .set_temperature(0.8);
```

## Pricing & Cost Optimization

OpenRouter provides transparent pricing for each model. Here are some cost optimization tips:

1. **Use Cheaper Models**: For simple tasks, use models like GPT-3.5 Turbo or Llama 2
2. **Batch Requests**: Group multiple requests together when possible
3. **Set Max Tokens**: Limit output to necessary length
4. **Monitor Usage**: Check your OpenRouter dashboard regularly

## Troubleshooting

### Issue: "OPENROUTER_API_KEY is not set"

**Solution**: Set the environment variable before running:

```bash
export OPENROUTER_API_KEY="sk-or-xxxxx"
```

### Issue: "Authentication failed"

**Solution**: Verify your API key is correct:

```bash
echo $OPENROUTER_API_KEY
```

### Issue: "Model not found"

**Solution**: Check the [OpenRouter models page](https://openrouter.ai/docs#models) for valid model names.

### Issue: "Rate limit exceeded"

**Solution**: Implement retry logic with exponential backoff:

```rust
use std::time::Duration;

async fn retry_with_backoff<F, T>(mut f: F) -> Result<T>
where
    F: FnMut() -> impl std::future::Future<Output = Result<T>>,
{
    let mut backoff = Duration::from_secs(1);
    loop {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) if e.to_string().contains("rate limit") => {
                tokio::time::sleep(backoff).await;
                backoff *= 2;
            }
            Err(e) => return Err(e),
        }
    }
}
```

## References

- [OpenRouter Official Website](https://openrouter.ai)
- [OpenRouter API Documentation](https://openrouter.ai/docs)
- [OpenRouter Models](https://openrouter.ai/docs#models)
- [OpenRouter Pricing](https://openrouter.ai/docs#pricing)

## Support

For issues or questions:

1. Check the [troubleshooting section](#troubleshooting)
2. Visit [OpenRouter Discord](https://discord.gg/openrouter)
3. Open an issue on [GitHub](https://github.com/The-Swarm-Corporation/swarms-rs)
