//! Ask a panel of models from different providers the same question at the same time, using
//! one OpenRouter key and a `ConcurrentWorkflow`.
//!
//! ```bash
//! export OPENROUTER_API_KEY="sk-or-..."
//! # Optional: comma-separated model IDs from https://openrouter.ai/models
//! export OPENROUTER_PANEL_MODELS="anthropic/claude-opus-5.5,openai/gpt-5.5,google/gemini-3.8-flash"
//! cargo run --example openrouter_model_panel
//! ```

use std::env;

use anyhow::Result;
use swarms_rs::llm::provider::openrouter::OpenRouter;
use swarms_rs::structs::agent::Agent;
use swarms_rs::structs::concurrent_workflow::ConcurrentWorkflow;

const DEFAULT_PANEL: &[&str] = &[
    "anthropic/claude-opus-5.5",
    "openai/gpt-5.5",
    "google/gemini-3.8-flash",
    "deepseek/deepseek-v4-pro",
];

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    swarms_rs::logging::init_logger();

    let models: Vec<String> = match env::var("OPENROUTER_PANEL_MODELS") {
        Ok(list) => list
            .split(',')
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty())
            .collect(),
        Err(_) => DEFAULT_PANEL.iter().map(|m| m.to_string()).collect(),
    };

    // One client (one connection pool); each agent just picks a different model.
    let client = OpenRouter::from_env().with_app_name("swarms-rs model panel");
    let agents: Vec<Box<dyn Agent>> = models
        .iter()
        .map(|model| {
            Box::new(
                client
                    .clone()
                    .set_model(model)
                    .agent_builder()
                    .agent_name(model)
                    .system_prompt(
                        "You are one expert on a panel. Answer in at most three sentences, \
                         and commit to a clear position.",
                    )
                    .max_loops(1)
                    .build(),
            ) as Box<dyn Agent>
        })
        .collect();

    let workflow = ConcurrentWorkflow::builder()
        .name("ModelPanel")
        .description("The same question answered by models from different providers")
        .agents(agents)
        .build();

    let result = workflow
        .run("Should a new backend service in 2026 start as a monolith or as microservices?")
        .await?;

    for message in &result.history {
        println!("── {} ──\n{}\n", message.role, message.content);
    }

    Ok(())
}
