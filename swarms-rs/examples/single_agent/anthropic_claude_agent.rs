use dotenv::dotenv;
use swarms_rs::agent::SwarmsAgentBuilder;
use swarms_rs::llm::provider::anthropic::Anthropic;
use swarms_rs::logging::init_logger;
use swarms_rs::structs::agent::Agent;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    // Initialize advanced logging system
    // Set SWARMS_LOG_LEVEL environment variable for different levels:
    // TRACE, DEBUG, INFO, WARN, ERROR, OFF
    init_logger();

    let model = Anthropic::from_env_with_model("claude-3-5-haiku-20241022");

    let agent = model
        .agent_builder()
        .agent_name("ClaudeTestAgent")
    .system_prompt("You are Claude, a helpful AI assistant. Keep responses brief and clear.")
    .max_loops(1)
    .temperature(0.7)
    .verbose(true)
    .build();

    let result = agent
        .run("Hello! Please respond with a brief greeting and confirm you're Claude.".to_string())
        .await?;

    println!("Result: {}", result.to_string());

    Ok(())
}
