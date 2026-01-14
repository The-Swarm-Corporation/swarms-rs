use swarms_rs::llm::provider::openai::OpenAI;
use swarms_rs::structs::swarms_router::{SwarmRouter, SwarmRouterConfig, SwarmType};

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_line_number(true)
        .with_file(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let client = OpenAI::from_env().set_model("deepseek-chat");
    let portfolio_analyzer = client
        .agent_builder()
        .agent_name("Portfolio-Analysis-Specialist")
        .system_prompt(
            "You are an expert portfolio analyst specializing in fund analysis and selection. \
             Your core competencies include:
            - Comprehensive analysis of mutual funds, ETFs, and index funds
            - Evaluation of fund performance metrics (expense ratios, tracking error, Sharpe ratio)
            - Assessment of fund composition and strategy alignment
            - Risk-adjusted return analysis
            - Tax efficiency considerations
            
            For each portfolio analysis:
            1. Evaluate fund characteristics and performance metrics
            2. Analyze expense ratios and fee structures
            3. Assess historical performance and volatility
            4. Compare funds within same category
            5. Consider tax implications
            6. Review fund manager track record and strategy consistency
            
            Maintain focus on cost-efficiency and alignment with investment objectives.",
        )
        .user_name("investment_team")
        .max_loops(1)
        .retry_attempts(2)
        .max_tokens(2000)
        .save_state_dir("./temp/swarm_router/portfolio_analyzer")
        .build();

    let allocation_strategist = client
        .agent_builder()
        .agent_name("Asset-Allocation-Strategist")
        .system_prompt(
            "You are a specialized asset allocation strategist focused on portfolio construction \
             and optimization. Your expertise includes:
            - Strategic and tactical asset allocation
            - Risk tolerance assessment and portfolio matching
            - Geographic and sector diversification
            - Rebalancing strategy development
            - Portfolio optimization using modern portfolio theory
            
            For each allocation:
            1. Analyze investor risk tolerance and objectives
            2. Develop appropriate asset class weights
            3. Select optimal fund combinations
            4. Design rebalancing triggers and schedules
            5. Consider tax-efficient fund placement
            6. Account for correlation between assets
            
            Focus on creating well-diversified portfolios aligned with client goals and risk \
             tolerance.",
        )
        .user_name("investment_team")
        .max_loops(1)
        .retry_attempts(2)
        .max_tokens(2000)
        .save_state_dir("./temp/swarm_router/allocation_strategist")
        .build();

    let risk_manager = client
        .agent_builder()
        .agent_name("Risk-Management-Specialist")
        .system_prompt(
            "You are a risk management specialist focused on portfolio risk assessment and \
             mitigation. Your expertise covers:
             - Portfolio risk metrics analysis
             - Downside protection strategies
             - Correlation analysis between funds
             - Stress testing and scenario analysis
             - Market condition impact assessment
             
             For each portfolio:
             1. Calculate key risk metrics (Beta, Standard Deviation, etc.)
             2. Analyze correlation matrices
             3. Perform stress tests under various scenarios
             4. Evaluate liquidity risks
             5. Assess concentration risks
             6. Monitor factor exposures
             
             Focus on maintaining appropriate risk levels while maximizing risk-adjusted returns.",
        )
        .user_name("investment_team")
        .max_loops(1)
        .retry_attempts(2)
        .max_tokens(2000)
        .save_state_dir("./temp/swarm_router/risk_manager")
        .build();

    let implementation_specialist = client
        .agent_builder()
        .agent_name("Risk-Management-Specialist")
        .system_prompt(
            "
            You are a portfolio implementation specialist focused on efficient execution and \
             maintenance. Your responsibilities include:
            - Fund selection for specific asset class exposure
            - Tax-efficient implementation strategies
            - Portfolio rebalancing execution
            - Trading cost analysis
            - Cash flow management
            
            For each implementation:
            1. Select most efficient funds for desired exposure
            2. Plan tax-efficient transitions
            3. Design rebalancing schedule
            4. Optimize trade execution
            5. Manage cash positions
            6. Monitor tracking error
            
            Maintain focus on minimizing costs and maximizing tax efficiency during implementation.
            ",
        )
        .user_name("investment_team")
        .max_loops(1)
        .retry_attempts(2)
        .max_tokens(2000)
        .save_state_dir("./temp/swarm_router/implementation_specialist")
        .build();

    let monitoring_specialist = client
        .agent_builder()
        .agent_name("Portfolio-Monitoring-Specialist")
        .system_prompt(
            "You are a portfolio monitoring specialist focused on ongoing portfolio oversight and \
             optimization. Your expertise includes:
             - Regular portfolio performance review
             - Drift monitoring and rebalancing triggers
             - Fund changes and replacements
             - Tax loss harvesting opportunities
             - Performance attribution analysis
             
             For each review:
             1. Track portfolio drift from targets
             2. Monitor fund performance and changes
             3. Identify tax loss harvesting opportunities
             4. Analyze tracking error and expenses
             5. Review risk metrics evolution
             6. Generate performance attribution reports
             
             Ensure continuous alignment with investment objectives while maintaining optimal \
             portfolio efficiency.",
        )
        .user_name("investment_team")
        .max_loops(1)
        .retry_attempts(2)
        .max_tokens(2000)
        .save_state_dir("./temp/swarm_router/monitoring_specialist")
        .build();

    let portfolio_agents = vec![
        portfolio_analyzer,
        allocation_strategist,
        risk_manager,
        implementation_specialist,
        monitoring_specialist,
    ];

    let router = SwarmRouter::new_with_config(SwarmRouterConfig {
        name: "etf-portfolio-management-swarm".into(),
        description: "Creates and suggests an optimal portfolio".into(),
        agents: portfolio_agents,
        swarm_type: SwarmType::SequentialWorkflow,
        ..Default::default()
    })
    .expect("Error creating swarm router");

    let task = "I have 10,000$ and I want to create a porfolio based on energy, ai, and \
                datacenter companies. high growth.";
    let task_result = router.run(task).await.expect("Error running swarm router");

    println!("The result of the task is: {task_result}");
}
