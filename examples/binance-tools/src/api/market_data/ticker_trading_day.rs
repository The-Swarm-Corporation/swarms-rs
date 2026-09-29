use anyhow::{Result, bail};
use reqwest::Client;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const API_URL: &str = "/api/v3/ticker/tradingDay";

/// Price change statistics for a trading day.
pub async fn ticker_trading_day(
    client: &Client,
    base_url: &str,
    mut params: TickerTradingDayRequest,
) -> Result<TickerTradingDayResponse> {
    if params.r#type.is_none() {
        params.r#type = Some(TickerType::Full);
    }

    if params.time_zone.is_none() {
        params.time_zone = Some("0".to_string());
    }

    if params.symbol.is_none() && params.symbols.is_none() {
        bail!("Either symbol or symbols must be provided")
    }

    let url = format!("{}{}", base_url, API_URL);
    let response = client.get(&url).query(&params).send().await?;
    response.error_for_status_ref()?;

    Ok(response.json().await?)
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
/// **Notes**:
/// - Supported values for `timeZone`:
///   - Hours and minutes (e.g. `-1:00`, `05:45`)
///   - Only hours (e.g. `0`, `8`, `4`)
///
/// **Weight**:
///
/// 4 for each requested symbol.
///
/// The weight for this request will cap at 200 once the number of symbols in the request is more than 50.
///
/// **Symbol and Symbols**:
///
/// Either symbol or symbols must be provided
///
/// Examples of accepted format for the symbols parameter:
/// ["BTCUSDT","BNBUSDT"]
/// or
/// %5B%22BTCUSDT%22,%22BNBUSDT%22%5D
///
/// The maximum number of symbols allowed in a request is 100.
pub struct TickerTradingDayRequest {
    pub symbol: Option<String>,
    /// The maximum number of symbols allowed in a request is 100.
    #[serde(serialize_with = "super::symbols_as_json")]
    pub symbols: Option<Vec<String>>,
    /// Default: 0 (UTC)
    pub time_zone: Option<String>,
    /// Supported values: FULL or MINI.
    /// If none provided, the default is FULL
    pub r#type: Option<TickerType>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum TickerType {
    Full,
    Mini,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
/// If symbol is provided, the response is a single object.
/// If symbols is provided, the response is an array of objects.
pub enum TickerTradingDayResponse {
    Single(Box<TickerTradingDayData>),
    List(Vec<TickerTradingDayData>),
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TickerTradingDayData {
    /// Symbol Name
    pub symbol: String,
    // --- Fields present in FULL only (marked as Option) ---
    /// Absolute price change
    pub price_change: Option<String>,
    /// Relative price change in percent
    pub price_change_percent: Option<String>,
    /// quoteVolume / volume
    pub weighted_avg_price: Option<String>,
    // --- Fields present in both FULL and MINI ---
    /// Opening price of the Interval
    pub open_price: String,
    /// Highest price in the interval
    pub high_price: String,
    /// Lowest price in the interval
    pub low_price: String,
    /// Closing price of the interval
    pub last_price: String,
    /// Total trade volume (in base asset)
    pub volume: String,
    /// Total trade volume (in quote asset)
    pub quote_volume: String,
    /// Start of the ticker interval. Unix timestamp in milliseconds.
    pub open_time: i64,
    /// End of the ticker interval. Unix timestamp in milliseconds.
    pub close_time: i64,
    /// First tradeId considered
    pub first_id: i64,
    /// Last tradeId considered
    pub last_id: i64,
    /// Total trade count
    pub count: i64,
}
