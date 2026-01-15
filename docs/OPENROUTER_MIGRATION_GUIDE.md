# OpenRouter Migration Guide

## Migrating from OpenAI to OpenRouter

If you're currently using OpenAI provider, switching to OpenRouter is straightforward.

### Before (OpenAI)

```rust
use swarms_rs::llm::provider::openai::OpenAI;

#[tokio::main]
async fn main() -> Result<()> {
    let model = OpenAI::from_env();

    let agent = model
        .agent_builder()
        .system_prompt("You are helpful")
        .build();

    let response = agent.run("Hello".to_string()).await?;
    Ok(())
}
```

### After (OpenRouter)

```rust
use swarms_rs::llm::provider::openrouter::OpenRouter;

#[tokio::main]
async fn main() -> Result<()> {
    let model = OpenRouter::from_env();

    let agent = model
        .agent_builder()
        .system_prompt("You are helpful")
        .build();

    let response = agent.run("Hello".to_string()).await?;
    Ok(())
}
```

The change is minimal - just replace the import and the initialization!

## Migrating from Anthropic to OpenRouter

### Before (Anthropic)

```rust
use swarms_rs::llm::provider::anthropic::Anthropic;

let model = Anthropic::from_env();
```

### After (OpenRouter - Using Claude)

```rust
use swarms_rs::llm::provider::openrouter::OpenRouter;

let model = OpenRouter::from_env()
    .set_model("anthropic/claude-3.5-sonnet");
```

## Environment Variable Changes

### OpenAI Setup

```bash
export OPENAI_API_KEY="sk-xxxxx"
export OPENAI_API_BASE="https://api.openai.com/v1"  # Optional
```

### Anthropic Setup

```bash
export ANTHROPIC_API_KEY="sk-ant-xxxxx"
```

### OpenRouter Setup

```bash
export OPENROUTER_API_KEY="sk-or-xxxxx"
export OPENROUTER_BASE_URL="https://openrouter.ai/api/v1"  # Optional
```

## Why Switch to OpenRouter?

### 1. **Cost Optimization**

- Compare prices across multiple providers
- Choose the most cost-effective model for your use case

### 2. **Flexibility**

- Switch models without code changes
- Test different models with the same codebase

### 3. **No Vendor Lock-in**

- Not dependent on a single provider
- Easy to migrate between models

### 4. **Access Multiple Providers**

```rust
// Use different providers with same API
let gpt4 = OpenRouter::from_env()
    .set_model("openai/gpt-4o");

let claude = OpenRouter::from_env()
    .set_model("anthropic/claude-3.5-sonnet");

let gemini = OpenRouter::from_env()
    .set_model("google/gemini-2-flash");
```

### 5. **Feature Parity**

- All features from original providers work with OpenRouter
- Tool calling
- Streaming (coming soon)
- Vision models
- Function calling

## Model Mapping

### OpenAI Models via OpenRouter

| Provider | Model                  |
| -------- | ---------------------- |
| OpenAI   | `openai/gpt-4o`        |
| OpenAI   | `openai/gpt-4-turbo`   |
| OpenAI   | `openai/gpt-3.5-turbo` |

### Anthropic Models via OpenRouter

| Provider  | Model                         |
| --------- | ----------------------------- |
| Anthropic | `anthropic/claude-3.5-sonnet` |
| Anthropic | `anthropic/claude-3-opus`     |
| Anthropic | `anthropic/claude-3-haiku`    |

### Google Models via OpenRouter

| Provider | Model                   |
| -------- | ----------------------- |
| Google   | `google/gemini-2-flash` |
| Google   | `google/gemini-2.5-pro` |

## Performance Comparison

### Speed (Fast to Slow)

1. GPT-3.5 Turbo
2. Gemini Flash
3. Claude Haiku
4. GPT-4 / Gemini Pro / Claude Sonnet
5. GPT-4 Turbo / Claude Opus

### Intelligence (Low to High)

1. GPT-3.5 Turbo
2. Gemini Flash
3. Claude Haiku
4. Gemini Pro
5. Claude Sonnet
6. GPT-4 / Claude Opus
7. GPT-4 Turbo

### Cost (Low to High)

1. Gemini Flash (Free tier available)
2. GPT-3.5 Turbo
3. Claude Haiku
4. Mistral Models
5. Gemini Pro
6. Claude Sonnet
7. GPT-4
8. Claude Opus / GPT-4 Turbo

## Configuration Changes

### Temperature Handling

```rust
// OpenAI
let model = OpenAI::new(key).set_temperature(0.5);

// OpenRouter (identical)
let model = OpenRouter::new(key).set_temperature(0.5);
```

### Max Tokens

```rust
// OpenAI
let model = OpenAI::new(key).set_max_tokens(2048);

// OpenRouter (identical)
let model = OpenRouter::new(key).set_max_tokens(2048);
```

### Model Selection

```rust
// OpenAI (single model provider, but:)
let model = OpenAI::new(key).set_model("gpt-4");

// OpenRouter (multiple models)
let model = OpenRouter::new(key)
    .set_model("openai/gpt-4o")
    .set_temperature(0.7)
    .set_max_tokens(2048);
```

## Common Patterns

### A/B Testing Different Models

```rust
async fn compare_models(prompt: &str) -> Result<()> {
    let models = vec![
        ("openai/gpt-4o", "GPT-4 Optimized"),
        ("anthropic/claude-3.5-sonnet", "Claude Sonnet"),
        ("google/gemini-2-flash", "Gemini Flash"),
    ];

    for (model_name, label) in models {
        let response = OpenRouter::from_env()
            .set_model(model_name)
            .agent_builder()
            .build()
            .run(prompt.to_string())
            .await?;

        println!("{}: {}", label, response);
    }

    Ok(())
}
```

### Cost-Conscious Implementation

```rust
// Start with fast/cheap model
let model = OpenRouter::from_env()
    .set_model("openai/gpt-3.5-turbo");

// Fallback to better model if needed
async fn get_response_with_fallback(prompt: &str) -> Result<String> {
    let fast_model = OpenRouter::from_env()
        .set_model("openai/gpt-3.5-turbo");

    match fast_model.agent_builder().build().run(prompt.to_string()).await {
        Ok(response) => Ok(response),
        Err(_) => {
            // Use more powerful model
            let powerful_model = OpenRouter::from_env()
                .set_model("anthropic/claude-3.5-sonnet");

            powerful_model.agent_builder().build()
                .run(prompt.to_string())
                .await
        }
    }
}
```

### Testing Different Temperatures

```rust
let temperatures = [0.0, 0.3, 0.7, 1.0];

for temp in temperatures {
    let model = OpenRouter::from_env()
        .set_model("anthropic/claude-3.5-sonnet")
        .set_temperature(temp);

    let response = model.agent_builder().build()
        .run("Be creative!".to_string())
        .await?;

    println!("Temperature {}: {}", temp, response);
}
```

## Troubleshooting Migration

### Issue: "Model not found"

**Solution**: Check [model list](https://openrouter.ai/docs#models) for exact names

### Issue: Different behavior with same model

**Possible causes**:

- Temperature differences
- Max tokens settings
- System prompt changes
- Chat history

**Solution**: Explicitly set all parameters:

```rust
let model = OpenRouter::from_env()
    .set_model("openai/gpt-4o")
    .set_temperature(0.5)
    .set_max_tokens(2048);
```

### Issue: Higher costs than expected

**Solution**:

1. Check [OpenRouter pricing](https://openrouter.ai/docs#pricing)
2. Use cheaper models for simple tasks
3. Limit max_tokens
4. Monitor usage on OpenRouter dashboard

## Key Differences

| Feature            | OpenAI | Anthropic | OpenRouter |
| ------------------ | ------ | --------- | ---------- |
| Multiple Providers | ❌     | ❌        | ✅         |
| Cost Comparison    | ❌     | ❌        | ✅         |
| Fallback Support   | ❌     | ❌        | ✅         |
| Claude Access      | ❌     | ✅        | ✅         |
| GPT-4 Access       | ✅     | ❌        | ✅         |
| Gemini Access      | ❌     | ❌        | ✅         |
| Custom Endpoints   | ✅     | ✅        | ✅         |
| Tool Calling       | ✅     | ✅        | ✅         |

## Rollback Plan

If you need to revert to a previous provider:

### From OpenRouter to OpenAI

```rust
// Change this:
use swarms_rs::llm::provider::openrouter::OpenRouter;
let model = OpenRouter::from_env()
    .set_model("openai/gpt-4o");

// To this:
use swarms_rs::llm::provider::openai::OpenAI;
let model = OpenAI::from_env()
    .set_model("gpt-4");
```

## Best Practices for Migration

1. **Test First**

   - Set up OpenRouter account
   - Test with a small prompt
   - Verify cost expectations

2. **Gradual Migration**

   - Migrate one agent at a time
   - Compare results with old provider
   - Monitor costs

3. **Monitor Costs**

   - Check OpenRouter dashboard
   - Set budget alerts
   - Compare with original provider

4. **Performance Testing**

   - Test latency
   - Test accuracy
   - Test cost-performance ratio

5. **Documentation**
   - Document chosen models
   - Document cost assumptions
   - Document fallback strategies

## Support Resources

- [OpenRouter Documentation](https://openrouter.ai/docs)
- [OpenRouter Models](https://openrouter.ai/docs#models)
- [OpenRouter Pricing](https://openrouter.ai/docs#pricing)
- [Swarms Documentation](docs/OPENROUTER_README.md)
- [GitHub Issues](https://github.com/The-Swarm-Corporation/swarms-rs/issues)

## FAQ

**Q: Do I need to change my API key format?**
A: Yes, OpenRouter API keys start with `sk-or-`. Get one from their dashboard.

**Q: Can I use OpenRouter with existing OpenAI code?**
A: Almost - just change the import and remove model.set_model() if using GPT-4.

**Q: How do I compare costs?**
A: Check [OpenRouter pricing page](https://openrouter.ai/docs#pricing) and your usage dashboard.

**Q: What if OpenRouter is down?**
A: Implement fallback to original providers or use multiple providers in sequence.

**Q: Is OpenRouter HIPAA/SOC2 compliant?**
A: Check with OpenRouter team - they provide compliance information on their website.

## Next Steps

1. Create OpenRouter account
2. Set `OPENROUTER_API_KEY` environment variable
3. Update imports in your code
4. Test with `cargo run --example openrouter_agent`
5. Monitor costs and performance
6. Deploy confidently
