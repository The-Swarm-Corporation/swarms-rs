use anyhow::Result;
use swarms_rs::structs::swarms_client::{AgentSpec, SwarmsClient, SwarmType};

#[tokio::main]
async fn main() -> Result<()> {
    // PUT YOUR API KEY HERE!
    let swarms_api_key = "SWARMS_API_KEY_HERE";  // ← PUT YOUR SWARMS API KEY HERE
    
    // Create Swarms API client
    let client = SwarmsClient::builder()
        .unwrap()
        .api_key(swarms_api_key)
        .timeout(std::time::Duration::from_secs(60))
        .max_retries(3)
        .build()
        .expect("Failed to create client");

    println!(" Creating Swarms API sequential workflow with per-agent markdown control!");

    // Step 1: Research Agent - NO MARKDOWN (plain text output)
    let researcher = AgentSpec {
        agent_name: "Researcher".to_string(),
        description: Some("Researches and gathers information on renewable energy sources".to_string()),
        system_prompt: Some("You are a Research Agent. Gather comprehensive information on renewable energy sources. Keep output simple and factual without markdown formatting.".to_string()),
        model_name: "gpt-4o-mini".to_string(),
        auto_generate_prompt: false,
        max_tokens: 300,
        temperature: 0.3,
        role: Some("worker".to_string()),
        max_loops: 1,
        tools_dictionary: None,
        markdown: false,
    };

    // Step 2: Analyzer Agent - NO MARKDOWN (plain text output)
    let analyzer = AgentSpec {
        agent_name: "Analyzer".to_string(),
        description: Some("Analyzes research findings and provides insights".to_string()),
        system_prompt: Some("You are an Analysis Agent. Analyze the research provided and extract key insights. Keep output concise and clear without markdown formatting.".to_string()),
        model_name: "gpt-4o-mini".to_string(),
        auto_generate_prompt: false,
        max_tokens: 300,
        temperature: 0.3,
        role: Some("worker".to_string()),
        max_loops: 1,
        tools_dictionary: None,
        markdown: false,
    };

    // Step 3: Summarizer Agent - WITH MARKDOWN (beautiful formatted output)
    let summarizer = AgentSpec {
        agent_name: "Summarizer".to_string(),
        description: Some("Creates a final summary of all findings with beautiful markdown formatting".to_string()),
        system_prompt: Some("You are a Summary Agent. Create a concise, well-formatted summary of all the research and analysis provided. Use markdown formatting effectively with headers, bullet points, and bold text. Make it visually appealing and well-structured.".to_string()),
        model_name: "gpt-4o-mini".to_string(),
        auto_generate_prompt: false,
        max_tokens: 500,
        temperature: 0.3,
        role: Some("worker".to_string()),
        max_loops: 1,
        tools_dictionary: None,
        markdown: true,
    };

    println!("   - Researcher: Plain text output (no markdown)");
    println!("   - Analyzer: Plain text output (no markdown)");
    println!("   - Summarizer: Beautiful markdown output");
    
    // Create a sequential workflow swarm using the Swarms API
    let response = client.swarm()
        .completion()
        .name("Per-Agent Markdown Demo")
        .description("A sequential workflow demonstrating per-agent markdown control")
        .swarm_type(SwarmType::SequentialWorkflow)
        .task("Explain the benefits of renewable energy sources. Research the topic, analyze the findings, and provide a comprehensive summary with beautiful formatting.")
        .agent(|agent| {
            agent
                .name("Researcher")
                .description("Researches and gathers information on renewable energy sources")
                .model("gpt-4o-mini")
                .system_prompt("You are a Research Agent. Gather comprehensive information on renewable energy sources. Keep output simple and factual without markdown formatting.")
                .md(false)
        })
        .agent(|agent| {
            agent
                .name("Analyzer")
                .description("Analyzes research findings and provides insights")
                .model("gpt-4o-mini")
                .system_prompt("You are an Analysis Agent. Analyze the research provided and extract key insights. Keep output concise and clear without markdown formatting.")
                .md(false)
        })
        .agent(|agent| {
            agent
                .name("Summarizer")
                .description("Creates a final summary with beautiful markdown formatting")
                .model("gpt-4o-mini")
                .system_prompt("You are a Summary Agent. Create a concise, well-formatted summary of all the research and analysis provided. Use markdown formatting effectively with headers, bullet points, and bold text. Make it visually appealing and well-structured.")
                .md(true)
        })
        .max_loops(1)
        .send()
        .await?;

    println!(" Swarms API sequential workflow completed!");
    println!(" Response:");
    println!("   - Job ID: {}", response.job_id);
    println!("   - Status: {}", response.status);
    println!("   - Swarm Name: {:?}", response.swarm_name);
    println!("   - Number of Agents: {}", response.number_of_agents);
    println!("   - Service Tier: {}", response.service_tier);
    
    println!("\n Output:");
    response.render_output();
    
    Ok(())
} 
