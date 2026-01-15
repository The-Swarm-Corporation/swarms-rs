use dotenv::dotenv;
use swarms_rs::agent::SwarmsAgentBuilder;
use swarms_rs::llm::provider::anthropic::Anthropic;
use swarms_rs::logging::init_logger;
use swarms_rs::structs::agent::Agent;
use swarms_macro::tool;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    // Initialize advanced logging system
    // Set SWARMS_LOG_LEVEL environment variable for different levels:
    // TRACE, DEBUG, INFO, WARN, ERROR, OFF
    init_logger();

    // Single-line change to select model across providers.
    // Set environment variable MODEL_NAME to switch models, e.g. "claude-3-opus-20240229".
    let model_name = std::env::var("MODEL_NAME").unwrap_or_else(|_| "claude-3-5-haiku-20241022".to_string());

    let model = Anthropic::from_env_with_model(model_name.clone());

    // Add a simple tool to demonstrate function-calling / tool execution
    #[tool(
        description = "Get weather information for a location",
        arg(location, description = "City and country e.g. Bogotá, Colombia", required = true)
    )]
    fn get_weather(location: String) -> Result<String> {
        tracing::info!("get_weather called with: {}", location);
        Ok(format!("Weather in {}: Sunny, 25°C", location))
    }

    let agent = model
        .agent_builder()
        .agent_name("ClaudeTestAgent")
        .system_prompt("You are Claude, a helpful AI assistant. Keep responses brief and clear.")
        .max_loops(1)
        .temperature(0.7)
        .verbose(true)
        .add_tool(get_weather)
        .build();

    let result = agent
        .run("Hello! Please respond with a brief greeting and confirm you're Claude.".to_string())
        .await?;

    println!("Result: {}", result.to_string());

    Ok(())
}
