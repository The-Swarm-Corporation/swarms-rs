use anyhow::{Result, bail};
use reqwest::Client;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const API_URL: &str = "/api/v3/ticker/price";

/// Latest price for a symbol or symbols.
pub async fn ticker_price(
    client: &Client,
    base_url: &str,
    params: TickerPriceRequest,
) -> Result<TickerPriceResponse> {
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
/// Request latest price for a symbol or symbols.
///
/// **Weight**:
///
/// | Parameter | Symbols Provided | Weight |
/// |-----------|------------------|--------|
/// | symbol    | 1                | 2      |
/// |     | symbol parameter is omitted | 4 |
/// | symbols   | Any              | 4      |
pub struct TickerPriceRequest {
    /// Parameter symbol and symbols cannot be used in combination.
    /// If neither parameter is sent, prices for all symbols will be returned in an array.
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
pub enum TickerPriceResponse {
    Single(TickerPriceDayData),
    List(Vec<TickerPriceDayData>),
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TickerPriceDayData {
    pub symbol: String,
    pub price: String,
}
