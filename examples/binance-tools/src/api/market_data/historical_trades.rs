use anyhow::Result;
use reqwest::Client;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

const API_URL: &str = "/api/v3/historicalTrades";

/// Get older trades.
pub async fn historical_trades(
    client: &Client,
    base_url: &str,
    mut params: HistoricalTradesRequest,
) -> Result<Vec<HistoricalTradesResponse>> {
    params.validate()?;
    if params.limit.is_none() {
        params.limit = Some(500);
    }

    let url = format!("{}{}", base_url, API_URL);
    let response = client.get(&url).query(&params).send().await?;
    response.error_for_status_ref()?;

    Ok(response.json().await?)
}

#[derive(Debug, Validate, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalTradesRequest {
    pub symbol: String,
    #[validate(range(min = 1, max = 1000))]
    pub limit: Option<i32>,
    /// From which transaction ID to start returning. If not specified, the most recent transaction records will be returned by default.
    pub from_id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalTradesResponse {
    pub id: i64,
    pub price: String,
    pub qty: String,
    /// Unix timestamp in milliseconds.
    pub time: i64,
    pub is_buyer_maker: bool,
    pub is_best_match: bool,
}
