# OpenRouter Quick Reference

## Installation

### 1. Get API Key

```bash
# Visit https://openrouter.ai and create account
# Copy your API key
```

### 2. Set Environment

```bash
export OPENROUTER_API_KEY="sk-or-xxxxx"
```

### 3. Add to .env (optional)

```bash
# .env
OPENROUTER_API_KEY=sk-or-xxxxx
OPENROUTER_BASE_URL=https://openrouter.ai/api/v1
```

## Basic Usage

### Create a Model

```rust
use swarms_rs::llm::provider::openrouter::OpenRouter;

let model = OpenRouter::from_env();
```

### Create an Agent

```rust
let agent = model
    .agent_builder()
    .system_prompt("You are helpful")
    .build();
```

### Run a Query

```rust
let response = agent.run("Hello!".to_string()).await?;
println!("{}", response);
```

## Configuration

### Set Model

```rust
let model = OpenRouter::from_env()
    .set_model("anthropic/claude-3.5-sonnet");
```

### Set Temperature

```rust
let model = OpenRouter::from_env()
    .set_temperature(0.5);
```

### Set Max Tokens

```rust
let model = OpenRouter::from_env()
    .set_max_tokens(2048);
```

### Custom Endpoint

```rust
let model = OpenRouter::new("api-key")
    .with_base_url("https://custom.api.com/v1");
```

## Popular Models

### Fastest (Best for Simple Tasks)

- `openai/gpt-3.5-turbo` - 1-3s response
- `google/gemini-2-flash` - 1-3s response

### Balanced (Best for Most Tasks)

- `anthropic/claude-3-haiku` - 2-5s response
- `openai/gpt-4o-mini` - 2-5s response

### Most Capable (Best for Complex Tasks)

- `anthropic/claude-3.5-sonnet` - 3-8s response
- `openai/gpt-4o` - 3-8s response
- `openai/gpt-4-turbo` - 5-15s response

### Most Economical

- `google/gemini-2-flash` - Fastest & cheapest
- `openai/gpt-3.5-turbo` - Good balance
- `anthropic/claude-3-haiku` - Capable & cheap

## Examples

### Run Included Examples

```bash
# Basic example
cargo run --example openrouter_agent

# Advanced examples
cargo run --example openrouter_advanced
```

### Complete Code Example

```rust
use swarms_rs::llm::provider::openrouter::OpenRouter;
use swarms_rs::structs::agent::Agent;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let model = OpenRouter::from_env()
        .set_model("anthropic/claude-3.5-sonnet")
        .set_temperature(0.5);

    let agent = model
        .agent_builder()
        .system_prompt("You are an expert assistant")
        .agent_name("Expert")
        .build();

    let response = agent.run("Explain machine learning".to_string()).await?;
    println!("{}", response);

    Ok(())
}
```

## Common Tasks

### Task Classification

```rust
let model = OpenRouter::from_env()
    .set_model("openai/gpt-3.5-turbo")  // Fast
    .set_temperature(0.1);               // Factual
```

### Creative Writing

```rust
let model = OpenRouter::from_env()
    .set_model("anthropic/claude-3.5-sonnet")  // Creative
    .set_temperature(0.8);                      // Creative
```

### Code Generation

```rust
let model = OpenRouter::from_env()
    .set_model("openai/gpt-4o")         // Powerful
    .set_temperature(0.2);              // Accurate
```

### Quick Summaries

```rust
let model = OpenRouter::from_env()
    .set_model("google/gemini-2-flash") // Fast
    .set_max_tokens(512);               // Short
```

### Research & Analysis

```rust
let model = OpenRouter::from_env()
    .set_model("anthropic/claude-3.5-sonnet")  // Powerful
    .set_max_tokens(4096);                      // Detailed
```

## Error Handling

### Handle Errors

```rust
match agent.run(prompt).await {
    Ok(response) => println!("{}", response),
    Err(e) => eprintln!("Error: {}", e),
}
```

### Check API Key

```bash
# Verify key is set
echo $OPENROUTER_API_KEY

# Update if needed
export OPENROUTER_API_KEY="sk-or-xxxxx"
```

## Troubleshooting

| Issue                        | Solution                                              |
| ---------------------------- | ----------------------------------------------------- |
| "OPENROUTER_API_KEY not set" | `export OPENROUTER_API_KEY="sk-or-xxxxx"`             |
| "Authentication failed"      | Check API key is correct                              |
| "Model not found"            | Check [model list](https://openrouter.ai/docs#models) |
| "Rate limit exceeded"        | Implement retry with backoff                          |
| "High costs"                 | Use cheaper models like GPT-3.5 or Gemini Flash       |

## Agent Configuration Options

```rust
let agent = model
    .agent_builder()
    // System configuration
    .agent_name("AgentName")
    .user_name("UserName")
    .system_prompt("System prompt text")

    // Execution control
    .max_loops(10)
    .temperature(0.5)
    .max_tokens(2048)

    // Features
    .enable_autosave()
    .save_state_dir("./state/")
    .enable_plan(Some("Create a plan".to_string()))

    // Logging
    .verbose(true)

    .build();
```

## Cost Optimization Tips

1. **Start with cheap models**: GPT-3.5, Gemini Flash
2. **Use temperature 0 for facts**: Faster, cheaper
3. **Limit max_tokens**: Don't request unnecessary output
4. **Cache responses**: Don't repeat queries
5. **Monitor usage**: Check OpenRouter dashboard

## Model Comparison Chart

```
Speed        ████░░░░░░ Gemini Flash
             ████░░░░░░ GPT-3.5
             ███░░░░░░░ Claude Haiku
             ██░░░░░░░░ GPT-4

Cost         ████░░░░░░ Gemini Flash
             ███░░░░░░░ GPT-3.5
             ██░░░░░░░░ Claude Haiku
             ░░░░░░░░░░ Claude Opus

Capability   ░░░░░░░░░░ GPT-3.5
             ██░░░░░░░░ Claude Haiku
             ███░░░░░░░ Claude Sonnet
             ████░░░░░░ GPT-4
             █████░░░░░ Claude Opus
```

## Migration from OpenAI

### Before

```rust
use swarms_rs::llm::provider::openai::OpenAI;
let model = OpenAI::from_env();
```

### After

```rust
use swarms_rs::llm::provider::openrouter::OpenRouter;
let model = OpenRouter::from_env()
    .set_model("openai/gpt-4o");
```

## Migration from Anthropic

### Before

```rust
use swarms_rs::llm::provider::anthropic::Anthropic;
let model = Anthropic::from_env();
```

### After

```rust
use swarms_rs::llm::provider::openrouter::OpenRouter;
let model = OpenRouter::from_env()
    .set_model("anthropic/claude-3.5-sonnet");
```

## Full Model List

### OpenAI

- `openai/gpt-4o` - Latest & greatest
- `openai/gpt-4-turbo` - Powerful
- `openai/gpt-3.5-turbo` - Fast & cheap

### Anthropic

- `anthropic/claude-3.5-sonnet` - Latest
- `anthropic/claude-3-opus` - Most capable
- `anthropic/claude-3-sonnet` - Balanced
- `anthropic/claude-3-haiku` - Lightweight

### Google

- `google/gemini-2-flash` - Fast
- `google/gemini-2.5-pro` - Powerful
- `google/gemini-2-pro` - Balanced

### Meta

- `meta-llama/llama-3-70b-instruct` - Latest
- `meta-llama/llama-2-70b-chat` - Previous

### Mistral

- `mistralai/mistral-large` - Powerful
- `mistralai/mistral-medium` - Balanced

[Full list at openrouter.ai/docs#models](https://openrouter.ai/docs#models)

## Documentation Links

- **Full Guide**: [docs/OPENROUTER_README.md](docs/OPENROUTER_README.md)
- **Migration**: [docs/OPENROUTER_MIGRATION_GUIDE.md](docs/OPENROUTER_MIGRATION_GUIDE.md)
- **Implementation**: [OPENROUTER_INTEGRATION_SUMMARY.md](OPENROUTER_INTEGRATION_SUMMARY.md)
- **OpenRouter Docs**: https://openrouter.ai/docs

## Quick Test

```bash
# 1. Set up environment
export OPENROUTER_API_KEY="sk-or-xxxxx"

# 2. Run example
cargo run --example openrouter_agent

# 3. Run advanced example
cargo run --example openrouter_advanced
```

## Support

- Check [troubleshooting](docs/OPENROUTER_README.md#troubleshooting)
- Visit [OpenRouter Discord](https://discord.gg/openrouter)
- Open [GitHub issue](https://github.com/The-Swarm-Corporation/swarms-rs/issues)

---

**Last Updated**: January 15, 2026  
**Status**: ✅ Production Ready
