use std::env;

use anyhow::Result;
use swarms_rs::{
    llm::provider::openai::OpenAI,
    structs::swarms_router::{SwarmRouter, SwarmRouterConfig, SwarmType},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    dotenv::dotenv().ok();
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(
            tracing_subscriber::fmt::layer()
                .with_line_number(true)
                .with_file(true),
        )
        .init();

    // Initialize OpenAI client
    let api_key = env::var("OPENAI_API_KEY").unwrap();
    let client = OpenAI::from_url("https://api.openai.com/v1", &api_key).set_model("");

    // Example 1: Sequential Workflow for Research Analysis
    println!("\n=== Running Sequential Workflow Example ===\n");
    let research_agent = client
        .agent_builder()
        .agent_name("Research Agent")
        .system_prompt(
            "You are a research specialist. Your role is to gather and analyze information \
            about the given topic. Focus on finding key facts, statistics, and relevant \
            information. Present your findings in a clear, structured format.",
        )
        .user_name("Analyst")
        .max_loops(1)
        .temperature(0.2)
        .build();

    let analysis_agent = client
        .agent_builder()
        .agent_name("Analysis Agent")
        .system_prompt(
            "You are an analysis specialist. Your role is to take the research findings \
            and provide deep insights, identify patterns, and draw meaningful conclusions. \
            Focus on interpreting the data and providing actionable insights.",
        )
        .user_name("Analyst")
        .max_loops(1)
        .temperature(0.3)
        .build();

    let report_agent = client
        .agent_builder()
        .agent_name("Report Agent")
        .system_prompt(
            "You are a report writing specialist. Your role is to take the analysis \
            and create a comprehensive, well-structured report. Focus on clear communication, \
            proper formatting, and actionable recommendations.",
        )
        .user_name("Analyst")
        .max_loops(1)
        .temperature(0.4)
        .build();

    // Create sequential workflow configuration
    let sequential_config = SwarmRouterConfig {
        name: "Research Analysis Workflow".to_string(),
        description: "A sequential workflow for conducting research and analysis".to_string(),
        swarm_type: SwarmType::SequentialWorkflow,
        agents: vec![research_agent, analysis_agent, report_agent],
        rules: Some(
            "1. Each agent must maintain professional tone\n\
             2. All findings must be backed by data\n\
             3. Reports must be clear and actionable"
                .to_string(),
        ),
        multi_agent_collab_prompt: true,
        flow: None,
        max_loops: None,
    };

    // Run sequential workflow
    let sequential_result = SwarmRouter::new_with_config(sequential_config)?
        .run("Analyze the impact of artificial intelligence on healthcare in the next decade")
        .await?;

    println!("Sequential Workflow Result:\n{}", sequential_result);

    // Example 2: Concurrent Workflow for Market Analysis
    println!("\n=== Running Concurrent Workflow Example ===\n");
    let market_analysis_agent = client
        .agent_builder()
        .agent_name("Market Analysis Agent")
        .system_prompt(
            "You are a market analysis specialist. Your role is to analyze market trends, \
            identify key patterns, and provide insights about market conditions. Focus on \
            technical analysis and market indicators.",
        )
        .user_name("Trader")
        .max_loops(1)
        .temperature(0.2)
        .build();

    let sentiment_analysis_agent = client
        .agent_builder()
        .agent_name("Sentiment Analysis Agent")
        .system_prompt(
            "You are a sentiment analysis specialist. Your role is to analyze market sentiment, \
            news impact, and social media trends. Focus on understanding market psychology \
            and emotional factors affecting the market.",
        )
        .user_name("Trader")
        .max_loops(1)
        .temperature(0.3)
        .build();

    let risk_assessment_agent = client
        .agent_builder()
        .agent_name("Risk Assessment Agent")
        .system_prompt(
            "You are a risk assessment specialist. Your role is to evaluate potential risks, \
            calculate risk metrics, and provide risk management recommendations. Focus on \
            identifying and quantifying various risk factors.",
        )
        .user_name("Trader")
        .max_loops(1)
        .temperature(0.2)
        .build();

    // Create concurrent workflow configuration
    let concurrent_config = SwarmRouterConfig {
        name: "Market Analysis Workflow".to_string(),
        description: "A concurrent workflow for comprehensive market analysis".to_string(),
        swarm_type: SwarmType::ConcurrentWorkflow,
        agents: vec![
            market_analysis_agent,
            sentiment_analysis_agent,
            risk_assessment_agent,
        ],
        rules: Some(
            "1. All analysis must be data-driven\n\
             2. Include specific metrics and numbers\n\
             3. Provide clear risk levels and confidence scores"
                .to_string(),
        ),
        multi_agent_collab_prompt: true,
        flow: None,
        max_loops: None,
    };

    // Run concurrent workflow
    let concurrent_result = SwarmRouter::new_with_config(concurrent_config)?
        .run("Analyze the current state of the cryptocurrency market, focusing on Bitcoin and Ethereum")
        .await?;

    println!("Concurrent Workflow Result:\n{}", concurrent_result);

    Ok(())
}
