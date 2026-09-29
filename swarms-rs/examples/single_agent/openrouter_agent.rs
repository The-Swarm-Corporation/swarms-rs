//! Build an agent on OpenRouter: one API key for models from OpenAI, Anthropic, Google,
//! Meta, Mistral, DeepSeek and more.
//!
//! ```bash
//! export OPENROUTER_API_KEY="sk-or-..."
//! # Optional: any model ID from https://openrouter.ai/models (defaults to openrouter/auto)
//! export OPENROUTER_MODEL="anthropic/claude-opus-5.5"
//! cargo run --example openrouter_agent
//! ```

use std::env;

use anyhow::Result;
use swarms_rs::llm::provider::openrouter::{DEFAULT_MODEL, OpenRouter};
use swarms_rs::structs::agent::Agent;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    swarms_rs::logging::init_logger();

    let model = env::var("OPENROUTER_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
    let client = OpenRouter::from_env_with_model(&model)
        // Optional: credit your app on openrouter.ai rankings
        .with_app_name("swarms-rs example");

    let agent = client
        .agent_builder()
        .agent_name("OpenRouterAgent")
        .system_prompt("You are a concise research assistant. Answer in a short paragraph.")
        .max_loops(1)
        .build();

    let response = agent
        .run(
            "Explain what makes Rust's ownership model useful for concurrent programs.".to_string(),
        )
        .await?;
    println!("[{model}]\n{response}");

    Ok(())
}
