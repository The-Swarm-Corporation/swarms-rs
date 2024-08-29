use log::{info, error, warn, debug};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::error::Error;

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse {
    content: Vec<HashMap<String, String>>,
    id: String,
    model: String,
    role: String,
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
    r#type: String,
    usage: HashMap<String, u64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
    stop_sequences: Option<Vec<String>>,
    temperature: Option<f64>,
    top_k: Option<u32>,
    top_p: Option<f64>,
    stream: Option<bool>,
    metadata: Option<HashMap<String, String>>,
    tools: Option<Vec<HashMap<String, Value>>>,
    system: Option<String>,
    tool_choice: Option<Value>,
}

// Utility function to fetch the API key from the environment
fn get_api_key() -> Result<String, Box<dyn Error>> {
    match env::var("ANTHROPIC_API_KEY") {
        Ok(key) => {
            debug!("API key found in environment.");
            Ok(key)
        },
        Err(_) => {
            error!("API key not found in environment variable ANTHROPIC_API_KEY.");
            Err(Box::from("API key not found in environment variable ANTHROPIC_API_KEY."))
        }
    }
}

async fn call_anthropic_api(api_version: &str, request: ApiRequest) -> Result<ApiResponse, Box<dyn Error>> {
    // Fetch the API key
    let api_key = get_api_key()?;
    
    // Initialize the HTTP client
    let client = Client::new();
    
    // Prepare the headers
    let mut headers = HeaderMap::new();
    headers.insert("x-api-key", HeaderValue::from_str(&api_key)?);
    headers.insert("anthropic-version", HeaderValue::from_str(api_version)?);
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    // Convert the request to JSON
    let request_body = serde_json::to_string(&request)?;
    debug!("Request Body: {}", request_body);

    // Send the POST request
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .headers(headers)
        .body(request_body)
        .send()
        .await;

    // Handle potential errors
    let response = match response {
        Ok(res) => {
            info!("API call successful, status: {}", res.status());
            res
        },
        Err(err) => {
            error!("API call failed: {}", err);
            return Err(Box::new(err));
        }
    };

    // Parse the response
    let response_body = response.text().await?;
    debug!("Response Body: {}", response_body);

    let api_response: ApiResponse = match serde_json::from_str(&response_body) {
        Ok(res) => res,
        Err(err) => {
            error!("Failed to parse API response: {}", err);
            return Err(Box::new(err));
        }
    };

    // Log usage data
    if let Some(usage) = api_response.usage.get("input_tokens") {
        info!("Input Tokens Used: {}", usage);
    }
    if let Some(usage) = api_response.usage.get("output_tokens") {
        info!("Output Tokens Generated: {}", usage);
    }

    Ok(api_response)
}

// // Example usage
// #[tokio::main]
// async fn main() {
//     // Initialize logger (log to console for this example)
//     env_logger::init();

//     // Define your API version
//     let api_version = "2023-06-01";

//     // Create the API request
//     let request = ApiRequest {
//         model: "claude-3-5-sonnet-20240620".to_string(),
//         max_tokens: 1024,
//         messages: vec![
//             Message {
//                 role: "user".to_string(),
//                 content: serde_json::json!("Hello, Claude"),
//             },
//         ],
//         stop_sequences: None,
//         temperature: Some(0.7),
//         top_k: Some(50),
//         top_p: Some(0.95),
//         stream: Some(false),
//         metadata: None,
//         tools: None,
//         system: None,
//         tool_choice: None,
//     };

//     // Call the API
//     match call_anthropic_api(api_version, request).await {
//         Ok(response) => {
//             info!("Received response: {:?}", response);
//         },
//         Err(err) => {
//             error!("Failed to get response: {}", err);
//         }
//     }
// }
