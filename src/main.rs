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

/// General-purpose concurrent swarm executor.
///
/// This function takes any async callable (a function or closure), runs it concurrently `N` times,
/// and returns a vector of results.
///
/// # Arguments
///
/// * `callable` - An async function or closure that takes a `Client` and returns a `Future`.
/// * `n` - The number of times to execute the callable concurrently.
/// * `client` - The HTTP client instance used to make requests.
/// * `output_file` - The path to the file where the results will be logged as JSON.
///
/// # Returns
///
/// A `Vec<Result<T, reqwest::Error>>` where each `Result` contains either the successful output
/// or an error.
///
/// # Example
///
/// ```rust
/// let client = Client::new();
/// let results = concurrent_swarm(call_openai_api, 5, client, "responses.json").await;
/// ```
pub async fn concurrent_swarm<F, Fut, T>(
    callable: F,
    n: usize,
    client: Arc<Client>,
    output_file: &str,
) -> Vec<Result<T, reqwest::Error>>
where
    F: Fn(Arc<Client>) -> Fut + Send + Sync + 'static + Copy,
    Fut: Future<Output = Result<T, reqwest::Error>> + Send,
    T: Send + 'static + Debug + Serialize, // Ensure T implements Serialize
{
    let (tx, mut rx) = mpsc::channel(n);
    let mut results = Vec::with_capacity(n);

    let file = Arc::new(Mutex::new(File::create(output_file).unwrap()));

    // Spawn N tasks
    for i in 0..n {
        let callable = callable.clone();
        let client = Arc::clone(&client);
        let tx = tx.clone();
        let file = Arc::clone(&file);

        task::spawn(async move {
            let result = callable(client).await;
            let log_entry = match &result {
                Ok(response) => json!({
                    "task": i + 1,
                    "status": "success",
                    "response": response,
                }),
                Err(e) => json!({
                    "task": i + 1,
                    "status": "error",
                    "error": format!("{:?}", e),
                }),
            };

            // Log the entry after the async block completes to avoid holding the MutexGuard during await
            {
                let mut file = file.lock().unwrap();
                writeln!(file, "{}", log_entry.to_string()).unwrap();
            }

            if tx.send(result).await.is_err() {
                warn!("Failed to send result to the receiver");
            }
        });
    }

    drop(tx); // Close the sending side

    while let Some(result) = rx.recv().await {
        results.push(result);
    }

    results
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
async fn call_openai_api(
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

#[tokio::main]
async fn main() {
    env_logger::init(); // Initialize the logger

    let client = Arc::new(Client::new());

    info!("Starting concurrent OpenAI API requests");

    // Define the model, system prompt, and user task
    let model = "gpt-4o-mini";
    let system_prompt = "You are a helpful assistant.";
    let user_task = "Who won the world series in 2020?";

    // Output file for logging the responses
    let output_file = "responses.json";

    // Create a closure that wraps the call_openai_api function with the provided parameters
    let task = |client: Arc<Client>| call_openai_api(client, model, system_prompt, user_task);

    // Run the concurrent swarm
    let results = concurrent_swarm(task, 4, client, output_file).await;

    for (i, result) in results.into_iter().enumerate() {
        match result {
            Ok(response) => info!("Request {}: Success - {:?}", i + 1, response),
            Err(e) => error!("Request {}: Failed - {:?}", i + 1, e),
        }
    }

    info!("All tasks completed.");
}
