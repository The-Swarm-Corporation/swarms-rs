/// OpenRouter Example
///
/// This example demonstrates how to use the OpenRouter LLM provider with the Swarms framework.
/// OpenRouter provides access to multiple LLM providers (Claude, GPT-4, Gemini, etc.) through a single API.
///
/// ## Setup
///
/// 1. Get your OpenRouter API key from https://openrouter.ai
/// 2. Set the environment variable:
///    ```bash
///    export OPENROUTER_API_KEY="sk-or-xxxxxxxxxxxx"
///    ```
/// 3. Run the example:
///    ```bash
///    cargo run --example openrouter_agent
///    ```

use std::env;

use anyhow::{Context, Result};
use swarms_rs::{llm::provider::openrouter::OpenRouter, structs::agent::Agent};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    // Initialize tracing subscriber for logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(
            tracing_subscriber::fmt::layer()
                .with_line_number(true)
                .with_file(true),
        )
        .init();

    println!("🚀 OpenRouter Agent Example");
    println!("============================\n");

    // Create OpenRouter client from environment
    // Expects OPENROUTER_API_KEY to be set
    let client = OpenRouter::from_env();

    println!("✓ OpenRouter client created");
    println!("  Model: {}", client.model());
    println!("  Base URL: {}\n", client.base_url());

    // Build agent with OpenRouter
    let agent = client
        .agent_builder()
        .system_prompt(
            "You are a helpful and knowledgeable AI assistant. \
             Provide clear, concise, and accurate responses.",
        )
        .agent_name("OpenRouterAssistant")
        .user_name("User")
        .max_loops(1)
        .enable_autosave()
        .save_state_dir("./temp/openrouter_agent/")
        .build();

    // Example queries to demonstrate the agent
    let queries = vec![
        "What is machine learning and how does it work?",
        "Explain the concept of backpropagation in neural networks.",
        "What are the main differences between supervised and unsupervised learning?",
    ];

    for (i, query) in queries.iter().enumerate() {
        println!("📝 Query {}: {}", i + 1, query);
        println!("{:-<60}", "");

        match agent.run(query.to_string()).await {
            Ok(response) => {
                println!("✓ Response:");
                println!("{}\n", response);
            }
            Err(e) => {
                eprintln!("✗ Error: {}", e);
            }
        }
    }

    println!("✓ Example completed successfully!");

    Ok(())
}
