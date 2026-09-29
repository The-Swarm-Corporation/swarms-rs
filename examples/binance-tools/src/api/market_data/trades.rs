use anyhow::Result;
use reqwest::Client;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

const API_URL: &str = "/api/v3/trades";

/// Get recent trades.
pub async fn trades(
    client: &Client,
    base_url: &str,
    mut params: TradesRequest,
) -> Result<Vec<TradesResponse>> {
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
pub struct TradesRequest {
    pub symbol: String,
    #[validate(range(min = 1, max = 1000))]
    pub limit: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TradesResponse {
    pub id: i64,
    pub price: String,
    pub qty: String,
    /// Unix timestamp in milliseconds.
    pub time: i64,
    pub is_buyer_maker: bool,
    pub is_best_match: bool,
}
