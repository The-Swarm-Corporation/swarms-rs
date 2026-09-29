use anyhow::{Result, bail};
use reqwest::Client;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const API_URL: &str = "/api/v3/ticker/24hr";

/// 24 hour rolling window price change statistics. Careful when accessing this with no symbol.
pub async fn ticker_24hr(
    client: &Client,
    base_url: &str,
    mut params: Ticker24HrRequest,
) -> Result<Ticker24HrResponse> {
    if params.r#type.is_none() {
        params.r#type = Some(Ticker24HrType::Full);
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
/// Please note that if the symbol parameter is not carried, data of all trading pairs will be returned. The data is not only huge, but also has a very high weight.
pub struct Ticker24HrRequest {
    /// Parameter symbol and symbols cannot be used in combination.
    /// If neither parameter is sent, tickers for all symbols will be returned in an array.
    ///
    /// | Symbols Provided | Weight |
    /// | --- | --- |
    /// | 1 | 2 |
    /// | symbol parameter is omitted | 80 |
    pub symbol: Option<String>,
    /// Examples of accepted format for the symbols parameter: ["BTCUSDT","BNBUSDT"]
    /// or
    /// %5B%22BTCUSDT%22,%22BNBUSDT%22%5D
    ///
    /// | Symbols Provided | Weight |
    /// | --- | --- |
    /// | 1-20 | 2 |
    /// | 21-100 | 40 |
    /// | 101 or more | 80 |
    /// | symbols parameter is omitted | 80 |
    #[serde(serialize_with = "super::symbols_as_json")]
    pub symbols: Option<Vec<String>>,
    /// Supported values: FULL or MINI.
    /// If none provided, the default is FULL
    pub r#type: Option<Ticker24HrType>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum Ticker24HrType {
    Full,
    Mini,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
/// If symbol is provided, the response is a single object.
/// If symbols is provided, the response is an array of objects.
pub enum Ticker24HrResponse {
    Single(Box<Ticker24HrData>),
    List(Vec<Ticker24HrData>),
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Ticker24HrData {
    /// Symbol Name
    pub symbol: String,
    // --- Fields present in FULL only (marked as Option) ---
    pub price_change: Option<String>,
    pub price_change_percent: Option<String>,
    pub weighted_avg_price: Option<String>,
    pub prev_close_price: Option<String>,
    pub last_qty: Option<String>,
    pub bid_price: Option<String>,
    pub bid_qty: Option<String>,
    pub ask_price: Option<String>,
    pub ask_qty: Option<String>,
    // --- Fields present in both FULL and MINI ---
    /// Closing price of the interval
    pub last_price: String,
    /// Opening price of the Interval
    pub open_price: String,
    /// Highest price in the interval
    pub high_price: String,
    /// Lowest price in the interval
    pub low_price: String,
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
