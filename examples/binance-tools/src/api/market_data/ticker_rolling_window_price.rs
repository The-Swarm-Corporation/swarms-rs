use anyhow::{Result, bail};
use reqwest::Client;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::ticker_trading_day::TickerType;

const API_URL: &str = "/api/v3/ticker";

/// Rolling window price change statistics
pub async fn ticker_rolling_window_price(
    client: &Client,
    base_url: &str,
    mut params: TickerRollingWindowPriceRequest,
) -> Result<TickerRollingWindowPriceResponse> {
    if params.symbol.is_none() && params.symbols.is_none() {
        bail!("Either symbol or symbols must be provided")
    }

    if params.window_size.is_none() {
        params.window_size = Some("1d".to_string());
    }

    let url = format!("{}{}", base_url, API_URL);
    let response = client.get(&url).query(&params).send().await?;
    response.error_for_status_ref()?;

    Ok(response.json().await?)
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
/// **Note**: This endpoint is different from the `GET /api/v3/ticker/24hr` endpoint.
///
/// The window used to compute statistics will be no more than 59999ms from the requested `windowSize`.
///
/// `openTime` for `/api/v3/ticker` always starts on a minute, while the `closeTime` is the current time of the request. As such, the effective window will be up to 59999ms wider than windowSize.
///
/// E.g. If the `closeTime` is 1641287867099 (January 04, 2022 09:17:47:099 UTC) , and the `windowSize` is `1d`. the `openTime` will be: 1641201420000 (January 3, 2022, 09:17:00)
///
/// **Weight**:
///
/// 4 for each requested symbol regardless of `windowSize`.
///
/// The weight for this request will cap at 200 once the number of symbols in the request is more than 50.
pub struct TickerRollingWindowPriceRequest {
    /// Parameter symbol and symbols cannot be used in combination.
    /// If neither parameter is sent, bookTickers for all symbols will be returned in an array.
    pub symbol: Option<String>,
    /// Examples of accepted format for the symbols parameter: ["BTCUSDT","BNBUSDT"]
    /// or
    /// %5B%22BTCUSDT%22,%22BNBUSDT%22%5D
    #[serde(serialize_with = "super::symbols_as_json")]
    pub symbols: Option<Vec<String>>,
    /// Defaults to 1d if no parameter provided
    /// Supported windowSize values:
    /// 1m,2m....59m for minutes
    /// 1h, 2h....23h - for hours
    /// 1d...7d - for days
    pub window_size: Option<String>,
    /// Supported values: FULL or MINI.
    /// If none provided, the default is FULL
    pub r#type: Option<TickerType>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
/// If symbol is provided, the response is a single object.
/// If symbols is provided, the response is an array of objects.
pub enum TickerRollingWindowPriceResponse {
    Single(Box<TickerRollingWindowPriceData>),
    List(Vec<TickerRollingWindowPriceData>),
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TickerRollingWindowPriceData {
    /// Symbol Name
    pub symbol: String,
    // --- Fields present in FULL only (marked as Option) ---
    /// Absolute price change
    pub price_change: Option<String>,
    /// Relative price change in percent
    pub price_change_percent: Option<String>,
    /// QuoteVolume / Volume
    pub weighted_avg_price: Option<String>,
    // --- Fields present in both FULL and MINI ---
    /// Opening price of the interval
    pub open_price: String,
    /// Highest price in the interval
    pub high_price: String,
    /// Lowest price in the interval
    pub low_price: String,
    /// Closing price of the interval
    pub last_price: String,
    /// Total trade volume (in base asset)
    pub volume: String,
    /// Total trade volume (in quote asset) - Sum of (price * volume) for all trades
    pub quote_volume: String,
    /// Open time for ticker window. Unix timestamp in milliseconds.
    pub open_time: i64,
    /// Close time for ticker window. Unix timestamp in milliseconds.
    pub close_time: i64,
    /// First tradeId considered
    pub first_id: i64,
    /// Last tradeId considered
    pub last_id: i64,
    /// Total trade count
    pub count: i64,
}
