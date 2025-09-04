use anyhow::Result;
use swarms_rs::{llm::provider::openai::OpenAI, structs::agent::Agent};
use swarms_rs::agent::SwarmsAgentBuilder;
use swarms_rs::llm::request::ToolDefinition;
use swarms_rs::agent::SwarmsAgent;


#[tokio::test]
async fn test_basic_agent_functionality() -> Result<()> {
    // Skip test if environment variables are not set
    if std::env::var("DEEPSEEK_BASE_URL").is_err() || std::env::var("DEEPSEEK_API_KEY").is_err() {
        println!("Skipping test_basic_agent_functionality: Required environment variables not set");
        return Ok(());
    }

    // Load environment variables from .env file
    dotenv::dotenv().ok();

    // Set up the OpenAI client with DeepSeek configuration
    let base_url = std::env::var("DEEPSEEK_BASE_URL").unwrap();
    let api_key = std::env::var("DEEPSEEK_API_KEY").unwrap();
    let client = OpenAI::from_url(base_url, api_key).set_model("deepseek-chat");

    // Create the agent
    let agent = client
        .agent_builder()
        .system_prompt("You are a helpful assistant.")
        .agent_name("TestAgent")
        .user_name("TestUser")
        .enable_autosave()
        .max_loops(1)
        .save_state_dir("./temp/test_agent_state.json")
        .enable_plan("Split the task into subtasks.".to_owned())
        .build();

    // Test the agent with a simple query
    let response = agent.run("What is 2+2?".to_owned()).await?;

    // Basic assertions
    assert!(!response.is_empty(), "Response should not be empty");
    assert!(
        response.len() > 10,
        "Response should be reasonably detailed"
    );
    assert!(
        response.to_lowercase().contains("4"),
        "Response should contain the answer '4'"
    );

    Ok(())
}

// Helper function to create a mock agent for testing
#[cfg(test)]
fn create_mock_agent() -> impl Agent {
    let client = OpenAI::from_url("https://mock-url.com".to_string(), "mock-key".to_string())
        .set_model("mock-model");

    client
        .agent_builder()
        .system_prompt("You are a test assistant.")
        .agent_name("MockAgent")
        .user_name("TestUser")
        .build()
}

#[tokio::test]
async fn test_agent_creation() -> Result<()> {
    let agent = create_mock_agent();
    assert_eq!(agent.name(), "MockAgent");
    Ok(())
}

#[tokio::test]
async fn test_lazy_initialized_for_SwarmsAgentBuilder() {
    let mut builder = SwarmsAgentBuilder::new_with_model(OpenAI::new("mock-api-key".to_string()));

    // Initially, tools_impl should not be initialized
    assert!(
        builder.tools_impl().is_empty(),
        "tools_impl should be empty before first access"
    );

    {
        // First borrow
        let tools_map = builder.tools_impl();
        assert!(
            tools_map.is_empty(),
            "tools_impl should be empty after first access"
        );
    } // tools_map goes out of scope here

    // Now safe to re-borrow
    let tools_map2 = builder.tools_impl();
    assert!(
        tools_map2.is_empty(),
        "tools_impl should still be empty when no tools are added"
    );
}