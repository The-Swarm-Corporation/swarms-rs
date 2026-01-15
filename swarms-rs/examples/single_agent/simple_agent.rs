use std::env;

use anyhow::{Context, Result};
use swarms_rs::{llm::provider::openai::OpenAI, structs::agent::Agent};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(
            tracing_subscriber::fmt::layer()
                .with_line_number(true)
                .with_file(true),
        )
        .init();

    // Check for required environment variables
    let api_key = env::var("OPENAI_API_KEY")
        .context("OPENAI_API_KEY environment variable not set. Please set it in your .env file or environment.")?;

    // Create OpenAI client with error handling
    let client = OpenAI::new(api_key).set_model("");

    // Build agent with error handling
    let agent = client
        .agent_builder()
        .system_prompt("You are a helpful assistant.")
        .agent_name("SwarmsAgent")
        .user_name("User")
        .enable_autosave()
        .max_loops(1)
        .save_state_dir("./temp/")
        .enable_plan("Split the task into subtasks.".to_owned())
        .build();

    // Run agent with error handling
    let response = agent
        .run("What is the meaning of life?".to_owned())
        .await
        .context("Failed to run agent")?;

    println!("{response}");

    Ok(())
}
