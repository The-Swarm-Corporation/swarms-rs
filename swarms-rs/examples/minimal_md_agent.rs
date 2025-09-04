use anyhow::Result;
use swarms_rs::structs::hierarchical_swarm::HierarchicalSwarmBuilder;
use swarms_rs::structs::swarms_client::{AgentSpec, SwarmsClient, ClientConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // PUT YOUR API KEY HERE!
    // You have TWO options:
    
    // OPTION 1: Set environment variable (recommended)
    // export SWARMS_API_KEY="your-actual-api-key-here"
    // export OPENAI_API_KEY="your-openai-key-here"
    
    // OPTION 2: Hardcode it here (not recommended for production)
    let swarms_api_key = "your-swarms-api-key-here";  // ← PUT YOUR SWARMS API KEY HERE
    let openai_api_key = "your-openai-api-key-here";  // ← PUT YOUR OPENAI API KEY HERE
    
    // Create client with your API keys
    let client = SwarmsClient::builder()
        .unwrap()
        .api_key(swarms_api_key)
        .openai_api_key(openai_api_key)
        .enable_openai_fallback(true)  // Falls back to OpenAI if Swarms fails
        .timeout(std::time::Duration::from_secs(60))
        .max_retries(3)
        .build()
        .expect("Failed to create client");

    // Minimal agent - just 3 lines!
    let agent = AgentSpec {
        agent_name: "MinimalAgent".to_string(),
        description: Some("A minimal test agent that responds with markdown formatting".to_string()),
        system_prompt: Some("You are a minimal test agent. Respond with markdown formatting including headers, bullet points, and bold text.".to_string()),
        model_name: "gpt-4o-mini".to_string(),
        auto_generate_prompt: false,
        max_tokens: 500,
        temperature: 0.3,
        role: Some("worker".to_string()),
        max_loops: 1,
        tools_dictionary: None,
    };

    // Create director agent
    let director = AgentSpec {
        agent_name: "Director".to_string(),
        description: Some("A Director Agent that creates simple plans and assigns tasks".to_string()),
        system_prompt: Some("You are a Director Agent. Create a simple plan and assign one task to the MinimalAgent.".to_string()),
        model_name: "gpt-4o-mini".to_string(),
        auto_generate_prompt: true,
        max_tokens: 500,
        temperature: 0.3,
        role: Some("director".to_string()),
        max_loops: 1,
        tools_dictionary: None,
    };

    // HERE'S THE PROOF - .md(true) enables everything with ONE LINE!
    let swarm = HierarchicalSwarmBuilder::new()
        .name("Minimal MD Test")
        .director(director)
        .agent(agent)
        .max_loops(1)
        .md(true)  //  THIS ONE LINE ENABLES ALL MARKDOWN OUTPUT!
        .client(client)
        .build()?;

    println!(" Executing minimal swarm with .md(true) - watch the beautiful output!");
    
    // Execute - .md(true) automatically renders everything beautifully!
    let results = swarm.run("Create a simple markdown response with a header, bullet points, and bold text", None).await?;

    println!(" Minimal swarm with .md(true) completed!");
    println!(" The .md(true) automatically rendered:");
    println!("   - Director planning with beautiful borders");
    println!("   - Agent execution with bordered output");
    println!("   - All markdown formatting automatically!");
    println!(" Generated {} result(s)", results.len());
    
    Ok(())
} 
