use anyhow::Result;
use swarms_rs::{llm::provider::openai::OpenAI, structs::agent::Agent};

/// Test DeepInfra provider compatibility
///
/// This test verifies that the agent works correctly with DeepInfra,
/// which returns empty `tool_calls: []` arrays instead of `None`.
#[tokio::test]
async fn test_deepinfra_agent() -> Result<()> {
    // Skip test if environment variables are not set
    if std::env::var("DEEPINFRA_BASE_URL").is_err() || std::env::var("DEEPINFRA_API_KEY").is_err() {
        println!("Skipping test_deepinfra_agent: DEEPINFRA env vars not set");
        return Ok(());
    }

    dotenv::dotenv().ok();

    let base_url = std::env::var("DEEPINFRA_BASE_URL").unwrap();
    let api_key = std::env::var("DEEPINFRA_API_KEY").unwrap();

    let client = OpenAI::from_url(base_url, api_key).set_model("meta-llama/Llama-3.3-70B-Instruct");

    let agent = client
        .agent_builder()
        .system_prompt("You are a helpful assistant.")
        .agent_name("DeepInfraAgent")
        .user_name("TestUser")
        .max_loops(1)
        .disable_task_complete_tool()
        .build();

    let response = agent.run("What is 2+2? Answer with just the number.".to_owned()).await?;

    assert!(!response.is_empty(), "Response should not be empty");
    assert!(
        response.contains("4"),
        "Response should contain the answer '4'"
    );

    Ok(())
}

/// Test OpenAI provider compatibility
#[tokio::test]
async fn test_openai_agent() -> Result<()> {
    // Skip test if environment variables are not set
    if std::env::var("OPENAI_BASE_URL").is_err() || std::env::var("OPENAI_API_KEY").is_err() {
        println!("Skipping test_openai_agent: OPENAI env vars not set");
        return Ok(());
    }

    dotenv::dotenv().ok();

    let base_url = std::env::var("OPENAI_BASE_URL").unwrap();
    let api_key = std::env::var("OPENAI_API_KEY").unwrap();

    let client = OpenAI::from_url(base_url, api_key).set_model("gpt-4o-mini");

    let agent = client
        .agent_builder()
        .system_prompt("You are a helpful assistant.")
        .agent_name("OpenAIAgent")
        .user_name("TestUser")
        .max_loops(1)
        .disable_task_complete_tool()
        .build();

    let response = agent.run("What is 2+2? Answer with just the number.".to_owned()).await?;

    assert!(!response.is_empty(), "Response should not be empty");
    assert!(
        response.contains("4"),
        "Response should contain the answer '4'"
    );

    Ok(())
}
