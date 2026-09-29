use anyhow::{Result, bail};
use reqwest::Client;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const API_URL: &str = "/api/v3/ticker/bookTicker";

/// Best price/qty on the order book for a symbol or symbols.
pub async fn ticker_book_ticker(
    client: &Client,
    base_url: &str,
    params: TickerBookTickerRequest,
) -> Result<TickerBookTickerResponse> {
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
/// Request best price/qty on the order book for a symbol or symbols.
///
/// **Weight**:
///
/// | Parameter | Symbols Provided | Weight |
/// |-----------|------------------|--------|
/// | symbol    | 1                | 2      |
/// |     | symbol parameter is omitted | 4 |
/// | symbols   | Any              | 4      |
pub struct TickerBookTickerRequest {
    /// Parameter symbol and symbols cannot be used in combination.
    /// If neither parameter is sent, bookTickers for all symbols will be returned in an array.
    pub symbol: Option<String>,
    /// Examples of accepted format for the symbols parameter: ["BTCUSDT","BNBUSDT"]
    /// or
    /// %5B%22BTCUSDT%22,%22BNBUSDT%22%5D
    #[serde(serialize_with = "super::symbols_as_json")]
    pub symbols: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
/// If symbol is provided, the response is a single object.
/// If symbols is provided, the response is an array of objects.
pub enum TickerBookTickerResponse {
    Single(TickerBookTickerData),
    List(Vec<TickerBookTickerData>),
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TickerBookTickerData {
    pub symbol: String,
    pub bid_price: String,
    pub bid_qty: String,
    pub ask_price: String,
    pub ask_qty: String,
}
