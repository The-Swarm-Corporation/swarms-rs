use std::env;

use anyhow::Result;
use swarms_rs::structs::hierarchical_swarm::{HierarchicalSwarmBuilder, AgentSpec, SwarmsClient};
use colored::*;
use regex::Regex;

/// Simple markdown formatter for console output
fn format_markdown(text: &str) -> String {
    let mut formatted = text.to_string();
    
    // Headers - need to be more careful with replacement order
    formatted = formatted.replace("#### ", &format!("{}", "#### ".bright_cyan().bold()));
    formatted = formatted.replace("### ", &format!("{}", "### ".bright_cyan().bold()));
    formatted = formatted.replace("## ", &format!("{}", "## ".bright_blue().bold()));
    formatted = formatted.replace("# ", &format!("{}", "# ".bright_magenta().bold()));
    
    // Bold text - handle **text** pattern
    let bold_pattern = Regex::new(r"\*\*(.*?)\*\*").unwrap();
    formatted = bold_pattern.replace_all(&formatted, |caps: &regex::Captures| {
        format!("{}", caps.get(1).unwrap().as_str().bold())
    }).to_string();
    
    // Italic text - handle *text* pattern (simple approach)
    let italic_pattern = Regex::new(r"\*([^*]+)\*").unwrap();
    formatted = italic_pattern.replace_all(&formatted, |caps: &regex::Captures| {
        format!("{}", caps.get(1).unwrap().as_str().italic())
    }).to_string();
    
    // Code blocks
    formatted = formatted.replace("```", &format!("{}", "```".bright_yellow()));
    
    // Inline code - handle `text` pattern
    let code_pattern = Regex::new(r"`([^`]+)`").unwrap();
    formatted = code_pattern.replace_all(&formatted, |caps: &regex::Captures| {
        format!("{}", caps.get(1).unwrap().as_str().bright_green())
    }).to_string();
    
    // Lists
    formatted = formatted.replace("- ", &format!("{}", "• ".bright_white()));
    
    // Horizontal rules
    formatted = formatted.replace("---", &format!("{}", "─".repeat(50).bright_black()));
    
    formatted
}

/// **HIERARCHICAL SWARM USAGE EXAMPLE WITH MARKDOWN**
///
/// Enable beautiful markdown output with just one line:
/// ```rust
/// let swarm = HierarchicalSwarmBuilder::new()
///     .name("My Swarm")
///     .director(director_agent)
///     .agent(worker_agent_1)
///     .agent(worker_agent_2)
///     .md(true)  //  Enable beautiful markdown output
///     .build()?;
///
/// // Execute - everything renders automatically with beautiful markdown!
/// let results = swarm.run("My task", None).await?;
/// ```
///
/// **What happens automatically when .md(true):**
/// - Director planning gets beautiful borders and markdown
/// - Each worker agent gets bordered output with markdown
/// - Feedback and evaluation get professional formatting
/// - Workflow completion gets summary with markdown
/// - No manual formatter management needed!
/// - Everything renders across the whole swarm automatically!

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_line_number(true)
        .with_file(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Set up API key - use Swarms API key (not OpenAI)
    let api_key = env::var("SWARMS_API_KEY").unwrap_or_else(|_| {
        "API_KEY_HERE".to_string()
    });

    // Create Swarms client using the self-contained implementation
    // Note: This uses the Swarms API (api.swarms.world), not OpenAI directly
    let client = SwarmsClient::builder()
        .unwrap()
        .api_key(&api_key)
        .timeout(std::time::Duration::from_secs(60))
        .max_retries(3)
        .build()
        .expect("Failed to create Swarms client");

    // Step 1: Create the Director Agent (coordinates everything according to architecture)
    let director_agent = AgentSpec {
        agent_name: "Director".to_string(),
        description: Some("Director Agent that coordinates and orchestrates the entire corporate restructuring process".to_string()),
        system_prompt: Some(r#"You are a Director Agent responsible for coordinating the restructuring of a classic 2008 corporation. Your role is to:

1. ANALYZE the current corporate structure and market conditions
2. CREATE a comprehensive plan for corporate restructuring and modernization
3. GENERATE specific orders for specialized business agents
4. EVALUATE results from agents and provide feedback
5. DECIDE if more iterations are needed for refinement

Always respond with a JSON structure containing:
- "plan": A detailed step-by-step plan for corporate restructuring
- "orders": Array of orders for specific worker agents with clear tasks

Example format:
{
  "plan": "Step 1: Analyze current market position... Step 2: Design new organizational structure...",
  "orders": [
    {"agent_name": "Financial Analyst", "task": "Analyze current financial position and create restructuring budget..."},
    {"agent_name": "HR Specialist", "task": "Plan organizational changes and workforce optimization..."}
  ]
}"#.to_string()),
        model_name: "gpt-4o-mini".to_string(),
        auto_generate_prompt: false,
        max_tokens: 2000,
        temperature: 0.3,
        role: Some("director".to_string()),
        max_loops: 1,
        tools_dictionary: None,
        markdown: false,
    };

    // Step 2: Create specialized Worker Agents (execute specific tasks)
    let financial_analyst = AgentSpec {
        agent_name: "Financial Analyst".to_string(),
        description: Some("Worker Agent analyzing financial position and creating restructuring budgets".to_string()),
        system_prompt: Some("You are a Financial Analyst Worker Agent. Analyze the current financial position of the corporation, identify cost-saving opportunities, create restructuring budgets, and develop financial strategies for modernization. Focus on profitability, cost optimization, and sustainable growth. Execute the task assigned by the Director Agent.".to_string()),
        model_name: "gpt-4o-mini".to_string(),
        auto_generate_prompt: false,
        max_tokens: 2000,
        temperature: 0.4,
        role: Some("worker".to_string()),
        max_loops: 1,
        tools_dictionary: None,
        markdown: false,
    };

    let hr_specialist = AgentSpec {
        agent_name: "HR Specialist".to_string(),
        description: Some("Worker Agent planning organizational changes and workforce optimization".to_string()),
        system_prompt: Some("You are an HR Specialist Worker Agent. Plan organizational changes, workforce optimization, employee retention strategies, and modern HR practices. Focus on talent management, employee engagement, and creating efficient organizational structures. Execute the task assigned by the Director Agent.".to_string()),
        model_name: "gpt-4o-mini".to_string(),
        auto_generate_prompt: false,
        max_tokens: 2000,
        temperature: 0.3,
        role: Some("worker".to_string()),
        max_loops: 1,
        tools_dictionary: None,
        markdown: false,
    };

    let technology_consultant = AgentSpec {
        agent_name: "Technology Consultant".to_string(),
        description: Some("Worker Agent planning technology modernization and digital transformation".to_string()),
        system_prompt: Some("You are a Technology Consultant Worker Agent. Plan technology modernization, digital transformation, IT infrastructure upgrades, and automation strategies. Focus on improving efficiency, reducing costs, and staying competitive in the digital age. Execute the task assigned by the Director Agent.".to_string()),
        model_name: "gpt-4o-mini".to_string(),
        auto_generate_prompt: false,
        max_tokens: 2000,
        temperature: 0.4,
        role: Some("worker".to_string()),
        max_loops: 1,
        tools_dictionary: None,
        markdown: false,
    };

    let operations_manager = AgentSpec {
        agent_name: "Operations Manager".to_string(),
        description: Some("Worker Agent optimizing business processes and operational efficiency".to_string()),
        system_prompt: Some("You are an Operations Manager Worker Agent. Optimize business processes, improve operational efficiency, streamline workflows, and implement best practices. Focus on reducing waste, improving quality, and increasing productivity. Execute the task assigned by the Director Agent.".to_string()),
        model_name: "gpt-4o-mini".to_string(),
        auto_generate_prompt: false,
        max_tokens: 2000,
        temperature: 0.5,
        role: Some("worker".to_string()),
        max_loops: 1,
        tools_dictionary: None,
        markdown: false,
    };

    // Step 3: Build the Hierarchical Swarm following the architecture
    let swarm = HierarchicalSwarmBuilder::new()
        .name("Classic 2008 Corporation Restructuring Swarm")
        .description("A hierarchical swarm that restructures a classic 2008 corporation")
        .director(director_agent)
        .agent(financial_analyst)
        .agent(hr_specialist)
        .agent(technology_consultant)
        .agent(operations_manager)
        .max_loops(2)
        .sequential_execution(true) // Use sequential execution with memory for better results
        .md(true) //  Enable beautiful markdown output
        .client(client)
        .build()
        .expect("Failed to create hierarchical swarm");

    // Define the task
    let task = "Restructure a classic 2008 corporation that has been struggling with outdated business practices, inefficient operations, and declining market share. The company needs comprehensive modernization to compete in today's digital economy. Plan for financial restructuring, organizational optimization, technology upgrades, and operational efficiency improvements. Provide detailed implementation strategies for transforming this traditional corporation into a modern, competitive, and profitable business.";

    // Execute the hierarchical swarm - everything renders automatically with beautiful markdown!
    match swarm.run(task, None).await {
        Ok(outputs) => {
            // Beautiful markdown output was automatically rendered for:
            // - Director planning and coordination
            // - Each worker agent execution
            // - Feedback and evaluation
            // - Workflow completion
            println!("{}", format_markdown(" Corporate restructuring process completed successfully!"));
            println!("{}", format_markdown(&format!(" Generated {} comprehensive restructuring documents", outputs.len())));
            
            // Print the actual results with markdown formatting
            for (i, output) in outputs.iter().enumerate() {
                println!("\n{}", format_markdown(&format!("=== Document {} ===", i + 1)));
                println!("{}", format_markdown(output));
                println!("{}", format_markdown("==================\n"));
            }
        }
        Err(e) => {
            println!(" Corporate restructuring failed: {:?}", e);
        }
    }

    println!(" Classic 2008 corporation restructuring demonstration completed!");
    println!(" Tip: Use .md(true) to enable beautiful markdown output for any swarm!");
    Ok(())
} 
