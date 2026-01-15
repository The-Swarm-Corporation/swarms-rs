/// OpenRouter Advanced Example with Tool Calling
///
/// This example demonstrates advanced features of the OpenRouter integration:
/// - Tool definitions and usage
/// - Custom model selection
/// - Temperature and token configuration
/// - Error handling and logging
///
/// Run with:
/// ```bash
/// cargo run --example openrouter_advanced
/// ```

use std::env;

use anyhow::Result;
use swarms_rs::{
    llm::provider::openrouter::OpenRouter,
    llm::request::ToolDefinition,
    structs::agent::Agent,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Define a calculator tool
fn calculator_tool() -> ToolDefinition {
    ToolDefinition {
        name: "calculator".to_string(),
        description: "Perform mathematical calculations".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["add", "subtract", "multiply", "divide"],
                    "description": "The operation to perform"
                },
                "a": {
                    "type": "number",
                    "description": "First number"
                },
                "b": {
                    "type": "number",
                    "description": "Second number"
                }
            },
            "required": ["operation", "a", "b"]
        }),
    }
}

/// Define a web search tool
fn web_search_tool() -> ToolDefinition {
    ToolDefinition {
        name: "web_search".to_string(),
        description: "Search the web for information about a topic".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results to return",
                    "minimum": 1,
                    "maximum": 10
                }
            },
            "required": ["query"]
        }),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(
            tracing_subscriber::fmt::layer()
                .with_line_number(true)
                .with_file(true)
                .with_target(false),
        )
        .init();

    println!("🚀 OpenRouter Advanced Example");
    println!("================================\n");

    // ============================================================
    // Example 1: Using Claude for Complex Reasoning
    // ============================================================
    println!("📚 Example 1: Complex Reasoning with Claude");
    println!("{}", "=".repeat(50));

    let claude_model = OpenRouter::from_env()
        .set_model("anthropic/claude-3.5-sonnet")
        .set_temperature(0.3) // Low temperature for factual accuracy
        .set_max_tokens(1024);

    let reasoning_agent = claude_model
        .agent_builder()
        .system_prompt(
            "You are an expert reasoning assistant. Provide clear, logical, \
             and well-structured answers with proper explanations.",
        )
        .agent_name("ReasoningAgent")
        .max_loops(1)
        .build();

    let reasoning_response = reasoning_agent
        .run(
            "Explain the concept of entropy in thermodynamics and why it's important in physics."
                .to_string(),
        )
        .await?;

    println!("✓ Response:\n{}\n", reasoning_response);

    // ============================================================
    // Example 2: Using GPT-4 for Creative Tasks
    // ============================================================
    println!("🎨 Example 2: Creative Writing with GPT-4");
    println!("{}", "=".repeat(50));

    let gpt4_model = OpenRouter::from_env()
        .set_model("openai/gpt-4o")
        .set_temperature(0.8) // Higher temperature for creativity
        .set_max_tokens(1024);

    let creative_agent = gpt4_model
        .agent_builder()
        .system_prompt(
            "You are a creative writing expert. Generate engaging, \
             imaginative content that captures the reader's attention.",
        )
        .agent_name("CreativeWriter")
        .max_loops(1)
        .build();

    let creative_response = creative_agent
        .run(
            "Write a short, mysterious story opening (2-3 paragraphs) \
             about discovering an ancient artifact."
                .to_string(),
        )
        .await?;

    println!("✓ Response:\n{}\n", creative_response);

    // ============================================================
    // Example 3: Fast Lightweight Model for Quick Tasks
    // ============================================================
    println!("⚡ Example 3: Fast Responses with Lightweight Model");
    println!("{}", "=".repeat(50));

    let fast_model = OpenRouter::from_env()
        .set_model("openai/gpt-3.5-turbo")
        .set_temperature(0.5)
        .set_max_tokens(512);

    let fast_agent = fast_model
        .agent_builder()
        .system_prompt("Provide concise and direct answers.")
        .agent_name("FastAssistant")
        .max_loops(1)
        .build();

    let fast_response = fast_agent
        .run("What are the top 3 benefits of machine learning?".to_string())
        .await?;

    println!("✓ Response:\n{}\n", fast_response);

    // ============================================================
    // Example 4: Cost-Effective Gemini Model
    // ============================================================
    println!("💰 Example 4: Cost-Effective with Google Gemini");
    println!("{}", "=".repeat(50));

    let gemini_model = OpenRouter::from_env()
        .set_model("google/gemini-2-flash")
        .set_temperature(0.5)
        .set_max_tokens(1024);

    let gemini_agent = gemini_model
        .agent_builder()
        .system_prompt(
            "You are a helpful assistant powered by Google Gemini. \
             Provide accurate and helpful information.",
        )
        .agent_name("GeminiAssistant")
        .max_loops(1)
        .build();

    let gemini_response = gemini_agent
        .run(
            "Summarize the key differences between SQL and NoSQL databases."
                .to_string(),
        )
        .await?;

    println!("✓ Response:\n{}\n", gemini_response);

    // ============================================================
    // Example 5: Multi-Task Workflow
    // ============================================================
    println!("🔄 Example 5: Multi-Task Workflow");
    println!("{}", "=".repeat(50));

    let workflow_model = OpenRouter::from_env()
        .set_model("anthropic/claude-3.5-sonnet")
        .set_temperature(0.4);

    let workflow_agent = workflow_model
        .agent_builder()
        .system_prompt(
            "You are an expert project manager. Break down complex tasks \
             into manageable steps and provide detailed action plans.",
        )
        .agent_name("ProjectManager")
        .enable_autosave()
        .save_state_dir("./temp/workflow_agent/")
        .max_loops(3)
        .build();

    let tasks = vec![
        "Create a plan for learning Rust programming",
        "Design a system architecture for a real-time chat application",
        "Outline a marketing strategy for a new SaaS product",
    ];

    for (i, task) in tasks.iter().enumerate() {
        println!("\n📋 Task {}: {}", i + 1, task);
        match workflow_agent.run(task.to_string()).await {
            Ok(response) => println!("✓ Plan:\n{}\n", response),
            Err(e) => eprintln!("✗ Error: {}", e),
        }
    }

    println!("\n✓ All examples completed successfully!");
    println!("💡 Tip: Check the ./temp/ directory for saved agent states");

    Ok(())
}
