/// Example demonstrating Anthropic Claude agent with tool integration
/// 
/// This example shows how to build an intelligent agent that can use tools
/// to perform various tasks. The agent can reason about when to use which tool
/// and can chain multiple tool calls together.
/// 
/// Run with: cargo run --example anthropic_tools_example
/// 
/// Make sure to set ANTHROPIC_API_KEY environment variable first:
/// export ANTHROPIC_API_KEY="your-api-key"

use dotenv::dotenv;
use swarms_rs::agent::SwarmsAgentBuilder;
use swarms_rs::llm::provider::anthropic::Anthropic;
use swarms_rs::logging::init_logger;
use swarms_rs::structs::agent::Agent;
use swarms_rs::structs::tool::Tool;
use swarms_rs::llm::request::ToolDefinition;
use serde::{Deserialize, Serialize};

/// Calculator tool for mathematical operations
#[derive(Debug, Clone)]
struct CalculatorTool;

#[derive(Debug, Deserialize, Serialize)]
struct CalculatorArgs {
    operation: String,
    a: f64,
    b: f64,
}

impl Tool for CalculatorTool {
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Args = CalculatorArgs;
    type Output = String;
    const NAME: &'static str = "calculator";

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "calculator".to_string(),
            description: "Perform mathematical calculations with two numbers".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "The operation to perform: 'add', 'subtract', 'multiply', 'divide'"
                    },
                    "a": {
                        "type": "number",
                        "description": "The first number"
                    },
                    "b": {
                        "type": "number",
                        "description": "The second number"
                    }
                },
                "required": ["operation", "a", "b"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let result = match args.operation.to_lowercase().as_str() {
            "add" => args.a + args.b,
            "subtract" => args.a - args.b,
            "multiply" => args.a * args.b,
            "divide" => {
                if args.b == 0.0 {
                    return Err("Division by zero".into());
                }
                args.a / args.b
            }
            _ => return Err(format!("Unknown operation: {}", args.operation).into()),
        };

        Ok(format!(
            "{} {} {} = {}",
            args.a, args.operation, args.b, result
        ))
    }
}

/// Weather tool simulation
#[derive(Debug, Clone)]
struct WeatherTool;

#[derive(Debug, Deserialize, Serialize)]
struct WeatherArgs {
    location: String,
}

impl Tool for WeatherTool {
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Args = WeatherArgs;
    type Output = String;
    const NAME: &'static str = "get_weather";

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get current weather information for a specific location".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "The city name or location to get weather for (e.g., 'San Francisco', 'New York')"
                    }
                },
                "required": ["location"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Simulate weather API responses for demonstration
        let weather = match args.location.to_lowercase().as_str() {
            "san francisco" | "sf" => "Sunny, 72°F, light breeze from the west",
            "new york" | "nyc" => "Cloudy, 65°F, 20% chance of rain",
            "london" => "Rainy, 55°F, moderate wind",
            "tokyo" => "Clear, 78°F, humidity 65%",
            _ => "Partly cloudy, temperature unknown, check local weather service",
        };

        Ok(format!("Weather in {}: {}", args.location, weather))
    }
}

/// Knowledge base lookup tool
#[derive(Debug, Clone)]
struct KnowledgeBaseTool;

#[derive(Debug, Deserialize, Serialize)]
struct KnowledgeArgs {
    query: String,
}

impl Tool for KnowledgeBaseTool {
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Args = KnowledgeArgs;
    type Output = String;
    const NAME: &'static str = "search_knowledge_base";

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_knowledge_base".to_string(),
            description: "Search the knowledge base for information about a topic".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The topic or question to search for"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Simulate knowledge base responses
        let response = match args.query.to_lowercase().as_str() {
            q if q.contains("rust") => {
                "Rust is a systems programming language that runs blazingly fast, prevents \
                 segfaults, and guarantees thread safety."
            }
            q if q.contains("ai") => {
                "Artificial Intelligence (AI) is the simulation of human intelligence by machines, \
                 especially computer systems, including learning and reasoning."
            }
            q if q.contains("machine learning") => {
                "Machine Learning is a subset of AI that focuses on developing algorithms and models \
                 that can learn from data and make predictions or decisions."
            }
            q if q.contains("swarms") => {
                "Swarms-rs is a Rust framework for building multi-agent systems and orchestrating \
                 AI agents to work together toward common goals."
            }
            _ => "No information found in knowledge base for this query.",
        };

        Ok(response.to_string())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    
    // Initialize logging
    init_logger();

    println!("═══════════════════════════════════════════════════════════════");
    println!("  Anthropic Claude Agent with Tool Integration Example");
    println!("═══════════════════════════════════════════════════════════════\n");

    // Create an intelligent agent with multiple tools
    let agent = SwarmsAgentBuilder::new_with_model(
        Anthropic::from_env_with_model("claude-3-5-sonnet-20241022")
    )
    .agent_name("SmartAssistant")
    .system_prompt(
        "You are a helpful AI assistant with access to multiple tools. \
         Use the appropriate tool when needed to answer questions and complete tasks. \
         When asked to do calculations, use the calculator tool. \
         When asked about weather, use the weather tool. \
         When asked about topics, search the knowledge base. \
         Always explain your reasoning and be helpful."
    )
    .max_loops(3)
    .temperature(0.7)
    .max_tokens(2048)
    .verbose(true)
    .add_tool(CalculatorTool)
    .add_tool(WeatherTool)
    .add_tool(KnowledgeBaseTool)
    .build();

    // Example 1: Mathematical calculation
    println!("📊 Example 1: Mathematical Calculation");
    println!("─────────────────────────────────────");
    let math_query = "What is 156 multiplied by 7? Then add 89 to the result.";
    println!("User: {}", math_query);
    
    match agent.run(math_query.to_string()).await {
        Ok(response) => println!("Agent: {}\n", response),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 2: Weather inquiry
    println!("🌤️  Example 2: Weather Inquiry");
    println!("─────────────────────────────────────");
    let weather_query = "What's the weather like in San Francisco and New York? Which one is better?";
    println!("User: {}", weather_query);
    
    match agent.run(weather_query.to_string()).await {
        Ok(response) => println!("Agent: {}\n", response),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 3: Knowledge base query
    println!("🧠 Example 3: Knowledge Base Query");
    println!("─────────────────────────────────────");
    let knowledge_query = "Tell me about Rust programming and how it relates to building AI systems.";
    println!("User: {}", knowledge_query);
    
    match agent.run(knowledge_query.to_string()).await {
        Ok(response) => println!("Agent: {}\n", response),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 4: Complex multi-step task
    println!("🎯 Example 4: Complex Multi-Step Task");
    println!("─────────────────────────────────────");
    let complex_query = "If I travel to Tokyo and it costs $1500, and then to London which costs $800, \
                         how much have I spent in total? Also, what's the weather like in both places?";
    println!("User: {}", complex_query);
    
    match agent.run(complex_query.to_string()).await {
        Ok(response) => println!("Agent: {}\n", response),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 5: Knowledge integration
    println!("🔗 Example 5: Knowledge Integration");
    println!("─────────────────────────────────────");
    let integration_query = "What is swarms-rs and how can I use it to build AI systems with Rust?";
    println!("User: {}", integration_query);
    
    match agent.run(integration_query.to_string()).await {
        Ok(response) => println!("Agent: {}\n", response),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    println!("═══════════════════════════════════════════════════════════════");
    println!("  Example completed successfully!");
    println!("═══════════════════════════════════════════════════════════════");

    Ok(())
}
