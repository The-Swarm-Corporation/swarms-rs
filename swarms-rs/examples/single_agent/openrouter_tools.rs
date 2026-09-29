//! Give an OpenRouter-hosted model tools with `#[tool]`. Any OpenRouter model that supports
//! tool calling works, so you can switch models without touching the tools.
//!
//! ```bash
//! export OPENROUTER_API_KEY="sk-or-..."
//! # Optional: defaults to anthropic/claude-opus-5.5
//! export OPENROUTER_MODEL="openai/gpt-5.5"
//! cargo run --example openrouter_tools
//! ```

use std::env;

use anyhow::Result;
use swarms_macro::tool;
use swarms_rs::llm::provider::openrouter::OpenRouter;
use swarms_rs::structs::agent::Agent;
use thiserror::Error;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    swarms_rs::logging::init_logger();

    let model =
        env::var("OPENROUTER_MODEL").unwrap_or_else(|_| "anthropic/claude-opus-5.5".to_string());

    let agent = OpenRouter::from_env_with_model(&model)
        .agent_builder()
        .agent_name("ToolAgent")
        .system_prompt(
            "You are a helpful assistant. Use the tools for the current time and for unit \
             conversions instead of guessing.",
        )
        .add_tool(CurrentUtcTime)
        .add_tool(ConvertTemperature)
        .max_loops(2)
        .build();

    let result = agent
        .run("What time is it in UTC right now, and what is 98.6°F in Celsius?".to_string())
        .await?;
    println!("[{model}]\n{result}");

    Ok(())
}

#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("unknown unit '{0}', expected 'celsius' or 'fahrenheit'")]
    UnknownUnit(String),
}

#[tool(description = "Get the current date and time in UTC (RFC 3339)")]
fn current_utc_time() -> Result<String, ConversionError> {
    Ok(chrono::Utc::now().to_rfc3339())
}

#[tool(
    description = "Convert a temperature between Celsius and Fahrenheit",
    arg(value, description = "The temperature to convert"),
    arg(to, description = "Target unit: 'celsius' or 'fahrenheit'")
)]
fn convert_temperature(value: f64, to: String) -> Result<f64, ConversionError> {
    match to.to_lowercase().as_str() {
        "celsius" => Ok((value - 32.0) * 5.0 / 9.0),
        "fahrenheit" => Ok(value * 9.0 / 5.0 + 32.0),
        other => Err(ConversionError::UnknownUnit(other.to_string())),
    }
}
