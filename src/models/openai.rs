use reqwest::Client;
use std::future::Future;
use std::sync::{Arc, Mutex};
use tokio::task;
use tokio::sync::mpsc;
use log::{info, warn, error};
use serde::Serialize;
use serde_json::json;
use std::fs::File;
use std::io::Write;
use std::env;
use std::fmt::Debug;


// Utility function to fetch the API key from the environment
fn get_api_key() -> Result<String, Box<dyn Error>> {
    match env::var("OPENAI_API_KEY") {
        Ok(key) => {
            debug!("API key found in environment.");
            Ok(key)
        },
        Err(_) => {
            error!("API key not found in environment variable OPENAI_API_KEY.");
            Err(Box::from("API key not found in environment variable OPENAI_API_KEY."))
        }
    }
}



/// Function to call the OpenAI API
///
/// This function is an example of how you can define a callable function to be used with
/// `concurrent_swarm`. It allows passing the model name, system prompt, and task (user message)
/// dynamically.
///
/// # Arguments
///
/// * `client` - The HTTP client instance.
/// * `model` - The name of the OpenAI model to use (e.g., "gpt-4o-mini").
/// * `system_prompt` - The system prompt to set the behavior of the assistant.
/// * `user_task` - The task or question you want to ask the assistant.
///
/// # Returns
///
/// A `Result<String, reqwest::Error>` with the response text or an error.
pub async fn call_openai_api(
    client: Arc<Client>,
    model: &str,
    system_prompt: &str,
    user_task: &str,
) -> Result<String, reqwest::Error> {
    let url = "https://api.openai.com/v1/chat/completions";
    let api_key = env::var("OPENAI_API_KEY").expect("API key not found in environment variables");

    let request_body = format!(
        r#"{{
            "model": "{}",
            "messages": [
                {{"role": "system", "content": "{}"}},
                {{"role": "user", "content": "{}"}}
            ]
        }}"#,
        model, system_prompt, user_task
    );

    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(request_body)
        .send()
        .await?;

    let text = response.text().await?;
    Ok(text)
}
