use std::env;

use anyhow::Result;
use swarms_rs::llm::provider::openai::OpenAI;
use swarms_rs::structs::concurrent_workflow::ConcurrentWorkflow;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_line_number(true)
        .with_file(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let base_url = env::var("DEEPSEEK_BASE_URL").unwrap();
    let api_key = env::var("DEEPSEEK_API_KEY").unwrap();
    let client = OpenAI::from_url(base_url, api_key).set_model("");

    // Create specialized trading agents with independent roles
    let market_analysis_agent = client
        .agent_builder()
        .agent_name("Market Analysis Agent")
        .system_prompt(
            "You are a market analysis specialist for trading. Analyze the provided market data \
       and identify key trends, patterns, and technical indicators. Your task is to provide \
       a comprehensive market analysis including support/resistance levels, volume analysis, \
       and overall market sentiment. Focus only on analyzing current market conditions \
       without making specific trading recommendations. End your analysis with <DONE>.",
        )
        .user_name("Trader")
        .max_loops(1)
        .temperature(0.2) // Lower temperature for precise technical analysis
        .enable_autosave()
        .save_state_dir("./temp/concurrent_workflow/trading")
        .add_stop_word("<DONE>")
        .build();

    let trade_strategy_agent = client
        .agent_builder()
        .agent_name("Trade Strategy Agent")
        .system_prompt(
            "You are a trading strategy specialist. Based on the provided market scenario, \
       develop a comprehensive trading strategy. Your task is to analyze the given market \
       information and create a strategy that includes potential entry and exit points, \
       position sizing recommendations, and order types. Focus solely on strategy development \
       without performing risk assessment. End your strategy with <DONE>.",
        )
        .user_name("Trader")
        .max_loops(1)
        .temperature(0.3)
        .enable_autosave()
        .save_state_dir("./temp/concurrent_workflow/trading")
        .add_stop_word("<DONE>")
        .build();

    let risk_assessment_agent = client
        .agent_builder()
        .agent_name("Risk Assessment Agent")
        .system_prompt(
            "You are a risk assessment specialist for trading. Your role is to evaluate \
       potential risks in the provided market scenario. Calculate appropriate risk metrics \
       such as volatility, maximum drawdown, and risk-reward ratios based solely on the \
       market information provided. Provide an independent risk assessment without \
       considering specific trading strategies. End your assessment with <DONE>.",
        )
        .user_name("Trader")
        .max_loops(1)
        .temperature(0.2)
        .enable_autosave()
        .save_state_dir("./temp/concurrent_workflow/trading")
        .add_stop_word("<DONE>")
        .build();

    // Create a concurrent workflow with all trading agents
    let workflow = ConcurrentWorkflow::builder()
        .name("Trading Strategy Workflow")
        .metadata_output_dir("./temp/concurrent_workflow/trading/workflow/metadata")
        .description("A workflow for analyzing market data with independent specialized agents.")
        .agents(vec![
            Box::new(market_analysis_agent),
            Box::new(trade_strategy_agent),
            Box::new(risk_assessment_agent),
        ])
        .build();

    let result = workflow
        .run(
            "BTC/USD is approaching a key resistance level at $50,000 with increasing volume. \
             RSI is at 68 and MACD shows bullish momentum. Develop a trading strategy for a \
             potential breakout scenario.",
        )
        .await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
