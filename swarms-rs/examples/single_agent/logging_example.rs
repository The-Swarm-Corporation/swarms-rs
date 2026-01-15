use dotenv::dotenv;
use std::env;
use swarms_rs::{
    Agent, agent::swarms_agent::SwarmsAgentBuilder, llm::provider::openai::OpenAI,
    logging::init_logger, structs::agent::AgentConfig,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the logger with environment variable support
    init_logger();

    dotenv().ok();

    // Set up the OpenAI model (you'll need to set OPENAI_API_KEY)
    let api_key = env::var("OPENAI_API_KEY").unwrap_or_else(|_| {
        println!("⚠️  OPENAI_API_KEY not set, using demo key (will show API errors)");
        "demo-key".to_string()
    });

    let model = OpenAI::new(api_key).set_model("");

    // Create agent configuration with logging-friendly settings
    let config = AgentConfig::builder()
        .agent_name("LoggingDemoAgent")
        .user_name("TestUser")
        .description("An agent to demonstrate comprehensive logging")
        .temperature(0.7)
        .max_loops(1)
        .max_tokens(100)
        .enable_plan(Some("Plan this task step by step".to_string()))
        .retry_attempts(1)
        .build();

    // Build the agent with comprehensive logging
    let agent = SwarmsAgentBuilder::new_with_model(model)
        .config((*config).clone())
        .system_prompt("You are a helpful assistant that demonstrates logging capabilities.")
        .build();

    // Demonstrate different log levels by setting environment variable
    println!("🚀 Starting logging demonstration...");
    println!("💡 Tip: Set SWARMS_LOG_LEVEL=DEBUG for more detailed output");
    println!("💡 Available levels: TRACE, DEBUG, INFO, WARN, ERROR, OFF");

    // Test a simple prompt first
    println!("\n📝 Testing simple prompt...");
    let prompt_result = agent.prompt("What is 2+2?").await;
    match prompt_result {
        Ok(response) => println!("✅ Prompt response received: {}", response),
        Err(e) => println!("❌ Prompt failed: {}", e),
    }

    // Test the full agent run
    println!("\n🎯 Testing full agent task execution...");
    let task = "Write a haiku about artificial intelligence";

    match agent.run(task.to_string()).await {
        Ok(result) => {
            println!("✅ Task completed successfully!");
            println!("📋 Result: {:?}", result);
        },
        Err(e) => {
            println!("❌ Task failed: {:?}", e);
        },
    }

    println!("\n🎉 Logging demonstration complete!");
    println!("📊 Check the logs above to see the comprehensive logging in action.");

    Ok(())
}
