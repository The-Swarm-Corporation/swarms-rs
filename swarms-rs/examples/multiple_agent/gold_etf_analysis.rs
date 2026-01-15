use std::env;

use anyhow::Result;
use bytes::Bytes;
use reqwest::Client;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use swarms_rs::agent::SwarmsAgentBuilder;
use swarms_rs::llm::provider::anthropic::Anthropic;
use swarms_rs::logging::init_logger;
use swarms_rs::structs::concurrent_workflow::ConcurrentWorkflow;
use tokio::sync::OnceCell;
use tokio::time;

static POLYGON_CLIENT: OnceCell<PolygonClient> = OnceCell::const_new();

#[derive(Debug)]
struct PolygonClient {
    api_key: String,
    client: Client,
}

impl PolygonClient {
    fn new(api_key: String) -> Self {
        let client = Client::new();

        Self { api_key, client }
    }

    async fn make_request_with_retry(
        &self,
        url: &str,
        max_retries: u32,
    ) -> Result<Bytes, PolygonApiError> {
        let mut attempts = 0;
        loop {
            match self.make_request(url).await {
                Ok(data) => return Ok(data),
                Err(e) => {
                    attempts += 1;
                    if attempts >= max_retries {
                        return Err(e);
                    }
                    // Wait before retrying
                    time::sleep(time::Duration::from_millis(1000u64 * attempts as u64)).await;
                },
            }
        }
    }

    async fn make_request(&self, url: &str) -> Result<Bytes, PolygonApiError> {
        // Send request with authorization header
        let response = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("User-Agent", "swarms-rs/1.0")
            .send()
            .await
            .map_err(|e| PolygonApiError::HttpError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(PolygonApiError::HttpError(format!(
                "API returned status: {}",
                response.status()
            )));
        }

        // Read response body
        let body = response
            .bytes()
            .await
            .map_err(|e| PolygonApiError::HttpError(format!("Failed to read response: {}", e)))?;

        Ok(body)
    }

    async fn get_quote(&self, symbol: &str) -> Result<PolygonQuoteResponse, PolygonApiError> {
        // Use previous day close as quote data since real-time quotes may be restricted
        let url = format!(
            "https://api.polygon.io/v2/aggs/ticker/{}/prev?apiKey={}",
            symbol, self.api_key
        );
        let body = self.make_request_with_retry(&url, 3).await?;
        let api_response: serde_json::Value = serde_json::from_slice(&body)?;

        if let Some(results) = api_response.get("results") {
            if let Some(result_array) = results.as_array() {
                if let Some(first_result) = result_array.first() {
                    Ok(PolygonQuoteResponse {
                        symbol: symbol.to_string(),
                        bid_price: 0.0, // Not available in prev day data
                        ask_price: 0.0,
                        last_trade_price: first_result
                            .get("c")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0),
                        last_trade_size: 0, // Not available
                        last_trade_time: first_result
                            .get("t")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0),
                        volume: first_result
                            .get("v")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0) as u64,
                        updated: first_result.get("t").and_then(|v| v.as_i64()).unwrap_or(0),
                    })
                } else {
                    Err(PolygonApiError::SymbolNotFound(symbol.to_string()))
                }
            } else {
                Err(PolygonApiError::SymbolNotFound(symbol.to_string()))
            }
        } else {
            Err(PolygonApiError::SymbolNotFound(symbol.to_string()))
        }
    }

    async fn get_aggregates(
        &self,
        symbol: &str,
        timespan: Option<&str>,
        limit: Option<u32>,
    ) -> Result<PolygonAggregatesResponse, PolygonApiError> {
        let timespan = timespan.unwrap_or("day");
        let limit = limit.unwrap_or(30);

        // Get current date for the 'to' parameter
        let to_date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let from_date = (chrono::Utc::now() - chrono::Duration::days(limit as i64))
            .format("%Y-%m-%d")
            .to_string();

        let url = format!(
            "https://api.polygon.io/v2/aggs/ticker/{}/range/1/{}/{}/{}?apiKey={}",
            symbol, timespan, from_date, to_date, self.api_key
        );

        let body = self.make_request_with_retry(&url, 3).await?;
        let api_response: serde_json::Value = serde_json::from_slice(&body)?;

        if let Some(results) = api_response.get("results") {
            if let Some(bars_array) = results.as_array() {
                let bars: Vec<PolygonBar> = bars_array
                    .iter()
                    .filter_map(|bar| {
                        let obj = bar.as_object()?;
                        Some(PolygonBar {
                            timestamp: obj.get("t")?.as_i64()?,
                            open: obj.get("o")?.as_f64()?,
                            high: obj.get("h")?.as_f64()?,
                            low: obj.get("l")?.as_f64()?,
                            close: obj.get("c")?.as_f64()?,
                            volume: obj.get("v")?.as_f64()? as u64,
                            vwap: obj.get("vw")?.as_f64()?,
                            transactions: obj.get("n")?.as_f64()? as u32,
                        })
                    })
                    .collect();

                let bars_count = bars.len() as u32;
                Ok(PolygonAggregatesResponse {
                    symbol: symbol.to_string(),
                    bars,
                    timespan: timespan.to_string(),
                    query_count: bars_count,
                    results_count: bars_count,
                })
            } else {
                Ok(PolygonAggregatesResponse {
                    symbol: symbol.to_string(),
                    bars: Vec::new(),
                    timespan: timespan.to_string(),
                    query_count: 0,
                    results_count: 0,
                })
            }
        } else {
            Err(PolygonApiError::SymbolNotFound(symbol.to_string()))
        }
    }

    async fn get_company_info(
        &self,
        symbol: &str,
    ) -> Result<PolygonCompanyResponse, PolygonApiError> {
        // Try v3 endpoint first
        let url = format!(
            "https://api.polygon.io/v3/reference/tickers/{}?apiKey={}",
            symbol, self.api_key
        );
        match self.make_request_with_retry(&url, 3).await {
            Ok(body) => {
                let api_response: serde_json::Value = serde_json::from_slice(&body)?;

                if let Some(results) = api_response.get("results") {
                    let obj = results.as_object().ok_or_else(|| {
                        PolygonApiError::HttpError("Invalid response format".to_string())
                    })?;

                    // For ETFs, some fields might be different
                    let name = obj
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown ETF");
                    let description = obj
                        .get("description")
                        .or_else(|| obj.get("sic_description"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("ETF description not available");

                    Ok(PolygonCompanyResponse {
                        symbol: symbol.to_string(),
                        name: name.to_string(),
                        description: description.to_string(),
                        industry: "Exchange-Traded Fund".to_string(),
                        sector: obj
                            .get("sector")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Financial Services")
                            .to_string(),
                        market_cap: obj.get("market_cap").and_then(|v| v.as_f64()),
                        employees: None,
                        ceo: None,
                        hq_country: obj
                            .get("locale")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                    })
                } else {
                    // Fallback: create basic ETF info if API doesn't return data
                    Ok(PolygonCompanyResponse {
                        symbol: symbol.to_string(),
                        name: format!("{} ETF", symbol),
                        description: format!("Exchange-traded fund tracking {}", symbol),
                        industry: "Exchange-Traded Fund".to_string(),
                        sector: "Financial Services".to_string(),
                        market_cap: None,
                        employees: None,
                        ceo: None,
                        hq_country: Some("United States".to_string()),
                    })
                }
            },
            Err(_) => {
                // Fallback: create basic ETF info if API call fails
                Ok(PolygonCompanyResponse {
                    symbol: symbol.to_string(),
                    name: format!("{} ETF", symbol),
                    description: format!("Exchange-traded fund tracking {}", symbol),
                    industry: "Exchange-Traded Fund".to_string(),
                    sector: "Financial Services".to_string(),
                    market_cap: None,
                    employees: None,
                    ceo: None,
                    hq_country: Some("United States".to_string()),
                })
            },
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    // Initialize advanced logging system
    init_logger();

    // Get API keys from environment
    let polygon_api_key =
        env::var("POLYGON_API_KEY").expect("POLYGON_API_KEY must be set in environment variables");
    let _anthropic_api_key = env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY must be set in environment variables");

    // Create Polygon API client
    let polygon_client = PolygonClient::new(polygon_api_key.clone());
    POLYGON_CLIENT
        .set(polygon_client)
        .expect("Failed to set Polygon client");

    // Test API connectivity
    println!("🔗 Testing Polygon API connectivity...");
    let test_client = POLYGON_CLIENT
        .get()
        .expect("Polygon client not initialized");
    match test_client
        .make_request_with_retry(
            "https://api.polygon.io/v2/aggs/ticker/AAPL/prev?apiKey=test",
            2,
        )
        .await
    {
        Ok(_) => println!("✅ API connection successful"),
        Err(e) => println!(
            "⚠️  API connection issue: {}. This may indicate authentication problems.",
            e
        ),
    }

    // Create Anthropic client for agents
    let anthropic_client = Anthropic::from_env_with_model("claude-3-5-haiku-20241022");

    // Create specialized financial analysis agents (without tools)
    let technical_analysis_agent = anthropic_client.clone().agent_builder()
        .agent_name("Technical Analysis Agent")
        .system_prompt(
            "You are a technical analysis specialist for ETFs and stocks. Your role is to analyze \
            the provided market data including price movements, technical indicators, chart patterns, \
            and trading volumes. Focus on identifying key support/resistance levels, trend analysis, \
            momentum indicators, and volume patterns. Provide detailed technical insights based on \
            the data provided. End your analysis with <DONE>.",
        )
        .user_name("Financial Analyst")
        .max_loops(1)
        .temperature(0.2)
        .verbose(true)
        .add_stop_word("<DONE>")
        .build();

    let fundamental_analysis_agent = anthropic_client.clone().agent_builder()
        .agent_name("Fundamental Analysis Agent")
        .system_prompt(
            "You are a fundamental analysis specialist for ETFs and stocks. Your role is to analyze \
            the provided company and market data including fundamentals, market position, sector \
            performance, and economic indicators. Focus on evaluating financial health, competitive \
            positioning, industry trends, and macroeconomic factors. Provide detailed fundamental \
            insights based on the data provided. End your analysis with <DONE>.",
        )
        .user_name("Financial Analyst")
        .max_loops(1)
        .temperature(0.2)
        .verbose(true)
        .add_stop_word("<DONE>")
        .build();

    let market_sentiment_agent = anthropic_client.clone().agent_builder()
        .agent_name("Market Sentiment Agent")
        .system_prompt(
            "You are a market sentiment specialist for ETFs and stocks. Your role is to analyze \
            the provided market data to understand market psychology, investor behavior, and overall \
            market mood. Focus on identifying bullish/bearish signals, market volatility, investor \
            confidence levels based on the data provided. End your analysis with <DONE>.",
        )
        .user_name("Financial Analyst")
        .max_loops(1)
        .temperature(0.3)
        .verbose(true)
        .add_stop_word("<DONE>")
        .build();

    // Create concurrent workflow for analyzing multiple gold ETFs
    let workflow = ConcurrentWorkflow::builder()
        .name("Gold ETF Analysis Workflow")
        .metadata_output_dir("./temp/gold_etf_analysis/workflow/metadata")
        .description("Concurrent analysis of gold ETFs using specialized financial agents with Polygon API data.")
        .agents(vec![
            Box::new(technical_analysis_agent),
            Box::new(fundamental_analysis_agent),
            Box::new(market_sentiment_agent),
        ])
        .build();

    // Gold ETF symbols to analyze
    let gold_etf_symbols = vec![
        "GLD",  // SPDR Gold Shares
        "IAU",  // iShares Gold Trust
        "SGOL", // Aberdeen Standard Physical Gold Shares ETF
    ];

    // Fetch data for all gold ETFs
    println!(
        "📊 Fetching market data for {} gold ETFs...",
        gold_etf_symbols.len()
    );

    let mut analysis_tasks = Vec::new();

    for symbol in &gold_etf_symbols {
        // Get the Polygon client
        let client = POLYGON_CLIENT
            .get()
            .expect("Polygon client not initialized");

        println!("  📊 Fetching quote data for {}...", symbol);
        let quote_data = match client.get_quote(symbol).await {
            Ok(data) => {
                println!("  ✅ Quote data received for {}", symbol);
                format!(
                    "Quote Data: Last Price: ${:.2}, Volume: {}",
                    data.last_trade_price, data.volume
                )
            },
            Err(e) => {
                println!("  ❌ Quote data failed for {}: {}", symbol, e);
                format!("Quote Data: Error fetching - {}", e)
            },
        };

        // Add delay to avoid rate limiting
        time::sleep(time::Duration::from_millis(200)).await;

        println!("  📈 Fetching price history for {}...", symbol);
        let aggregate_data = match client.get_aggregates(symbol, Some("day"), Some(5)).await {
            Ok(data) => {
                println!(
                    "  ✅ Price history received for {} ({} bars)",
                    symbol,
                    data.bars.len()
                );
                let bars_text = data
                    .bars
                    .iter()
                    .take(3) // Show only last 3 bars to keep prompt manageable
                    .map(|bar| {
                        // Convert milliseconds to seconds for chrono
                        let timestamp_seconds = bar.timestamp / 1000;
                        let date = if timestamp_seconds > 0 {
                            chrono::DateTime::from_timestamp(timestamp_seconds, 0)
                                .unwrap_or_else(|| {
                                    chrono::DateTime::from_timestamp(
                                        chrono::Utc::now().timestamp(),
                                        0,
                                    )
                                    .unwrap()
                                })
                                .format("%Y-%m-%d")
                                .to_string()
                        } else {
                            "Invalid Date".to_string()
                        };
                        format!(
                            "Date: {}, O: ${:.2}, H: ${:.2}, L: ${:.2}, C: ${:.2}, Vol: {}",
                            date, bar.open, bar.high, bar.low, bar.close, bar.volume
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                format!("Recent Price History (last 3 days): {}", bars_text)
            },
            Err(e) => {
                println!("  ❌ Price history failed for {}: {}", symbol, e);
                format!("Price History: Error fetching - {}", e)
            },
        };

        // Add delay to avoid rate limiting
        time::sleep(time::Duration::from_millis(200)).await;

        println!("  🏢 Fetching company info for {}...", symbol);
        let company_data = match client.get_company_info(symbol).await {
            Ok(data) => {
                println!("  ✅ Company info received for {}", symbol);
                format!(
                    "Company Info: {}, Sector: {}, Description: {}",
                    data.name, data.sector, data.description
                )
            },
            Err(e) => {
                println!("  ❌ Company info failed for {}: {}", symbol, e);
                format!("Company Info: Error fetching - {}", e)
            },
        };

        // Combine all data into a comprehensive string
        let full_data = format!("{}\n{}\n{}", quote_data, aggregate_data, company_data);

        // Add delay between symbols to avoid rate limiting
        time::sleep(time::Duration::from_millis(500)).await;

        // Create analysis task with embedded data
        let task = format!(
            "Analyze the gold ETF {} based on the following market data:\n\n{}\n\n\
            As a specialist in your field, provide comprehensive insights about this ETF. \
            Focus on your area of expertise and provide detailed analysis based on the data provided above. \
            Consider current market conditions, trends, and factors that could impact this investment.",
            symbol, full_data
        );

        analysis_tasks.push(task);
    }

    println!(
        "🚀 Starting concurrent analysis of {} gold ETFs with {} specialized agents...",
        gold_etf_symbols.len(),
        3
    );

    // Run the concurrent workflow to analyze all gold ETFs simultaneously
    let results = workflow.run_batch(analysis_tasks).await?;

    println!("✅ Analysis complete! Results:");
    println!("{}", serde_json::to_string_pretty(&results)?);

    Ok(())
}

// Removed tool functions - data is now embedded directly in agent prompts

// Data structures for Polygon API responses

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PolygonQuoteResponse {
    symbol: String,
    bid_price: f64,
    ask_price: f64,
    last_trade_price: f64,
    last_trade_size: u32,
    last_trade_time: i64,
    volume: u64,
    updated: i64,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PolygonBar {
    timestamp: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: u64,
    vwap: f64,
    transactions: u32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PolygonAggregatesResponse {
    symbol: String,
    bars: Vec<PolygonBar>,
    timespan: String,
    query_count: u32,
    results_count: u32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PolygonCompanyResponse {
    symbol: String,
    name: String,
    description: String,
    industry: String,
    sector: String,
    market_cap: Option<f64>,
    employees: Option<u32>,
    ceo: Option<String>,
    hq_country: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PolygonApiError {
    #[error("HTTP request failed: {0}")]
    HttpError(String),

    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("API rate limit exceeded")]
    RateLimitError,

    #[error("Invalid API key")]
    AuthError,

    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),
}
