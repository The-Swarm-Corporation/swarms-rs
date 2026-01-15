use std::env;

use anyhow::Result;
use swarms_rs::llm::provider::openai::OpenAI;
use swarms_rs::structs::sequential_workflow::SequentialWorkflow;

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

    // Market Analysis Agent - Analyzes market trends and conditions
    let market_analyst = client
        .agent_builder()
        .agent_name("Market Analyst")
        .system_prompt(
            "You are a Market Analyst specializing in financial markets. Your task is to analyze current market conditions, \
            identify trends, and provide a comprehensive market overview. Focus on major indices, sector performance, \
            economic indicators, and global market influences. Your analysis will be used by the Risk Assessor to evaluate \
            potential investment risks.",
        )
        .user_name("Financial Advisor")
        .max_loops(1)
        .temperature(0.3)
        .enable_autosave()
        .save_state_dir("./temp")
        .build();

    // Risk Assessment Agent - Evaluates risks based on market analysis
    let risk_assessor = client
        .agent_builder()
        .agent_name("Risk Assessor")
        .system_prompt(
            "You are a Risk Assessor in the financial domain. Based on the market analysis provided, \
            your job is to identify and quantify potential risks associated with different investment options. \
            Consider volatility, liquidity risks, geopolitical factors, and economic uncertainties. \
            Provide a risk rating (Low, Medium, High) for each major market sector. \
            Your assessment will be used by the Investment Strategist to formulate trading recommendations."
        )
        .user_name("Financial Advisor")
        .max_loops(1)
        .temperature(0.3)
        .enable_autosave()
        .save_state_dir("./temp")
        .build();

    // Investment Strategy Agent - Provides investment recommendations
    let investment_strategist = client
        .agent_builder()
        .agent_name("Investment Strategist")
        .system_prompt(
            "You are an Investment Strategist who creates actionable trading and investment recommendations. \
            Using the market analysis and risk assessment provided, develop a comprehensive investment strategy. \
            Include specific asset allocation recommendations (stocks, bonds, commodities, etc.), \
            entry/exit points, position sizing based on risk tolerance, and timeframes for investments. \
            Your recommendations should be practical, considering both short-term opportunities and long-term portfolio stability."
        )
        .user_name("Financial Advisor")
        .max_loops(1)
        .temperature(0.3)
        .enable_autosave()
        .save_state_dir("./temp")
        .build();

    let agents = vec![market_analyst, risk_assessor, investment_strategist]
        .into_iter()
        .map(|a| Box::new(a) as _)
        .collect::<Vec<_>>();

    let workflow = SequentialWorkflow::builder()
        .name("FinancialAnalysisWorkflow")
        .metadata_output_dir("./temp/financial_workflow/metadata")
        .description("A sequential workflow for financial market analysis, risk assessment, and investment recommendations")
        .agents(agents)
        .build();

    // Example query for financial analysis
    let result = workflow.run("Analyze the current technology sector and provide investment recommendations considering recent market volatility.").await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
