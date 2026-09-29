//! A research → write → edit pipeline where every stage runs on a different provider's
//! model, all through one OpenRouter key and a `SequentialWorkflow`.
//!
//! ```bash
//! export OPENROUTER_API_KEY="sk-or-..."
//! # Optional overrides for each stage:
//! export OPENROUTER_RESEARCH_MODEL="google/gemini-3.8-flash"
//! export OPENROUTER_WRITER_MODEL="anthropic/claude-opus-5.5"
//! export OPENROUTER_EDITOR_MODEL="openai/gpt-5.5"
//! cargo run --example openrouter_pipeline
//! ```

use std::env;

use anyhow::Result;
use swarms_rs::llm::provider::openrouter::OpenRouter;
use swarms_rs::structs::agent::Agent;
use swarms_rs::structs::sequential_workflow::SequentialWorkflow;

fn model_from_env(var: &str, default: &str) -> String {
    env::var(var).unwrap_or_else(|_| default.to_string())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    swarms_rs::logging::init_logger();

    let client = OpenRouter::from_env().with_app_name("swarms-rs pipeline");

    // Fast, long-context model gathers the facts.
    let researcher = client
        .clone()
        .set_model(model_from_env(
            "OPENROUTER_RESEARCH_MODEL",
            "google/gemini-3.8-flash",
        ))
        .agent_builder()
        .agent_name("Researcher")
        .system_prompt(
            "You are a researcher. List the key facts, trade-offs and common misconceptions \
             about the topic as concise bullet points. Do not write prose.",
        )
        .max_loops(1)
        .build();

    // Strong writer turns the notes into an article.
    let writer = client
        .clone()
        .set_model(model_from_env(
            "OPENROUTER_WRITER_MODEL",
            "anthropic/claude-opus-5.5",
        ))
        .agent_builder()
        .agent_name("Writer")
        .system_prompt(
            "You are a technical writer. Turn the researcher's notes into a clear, engaging \
             article of about 300 words for experienced developers.",
        )
        .max_loops(1)
        .build();

    // A different model family reviews the draft, which catches different mistakes.
    let editor = client
        .set_model(model_from_env("OPENROUTER_EDITOR_MODEL", "openai/gpt-5.5"))
        .agent_builder()
        .agent_name("Editor")
        .system_prompt(
            "You are an editor. Fix factual errors and tighten the prose of the draft, then \
             return only the final article.",
        )
        .max_loops(1)
        .build();

    let agents: Vec<Box<dyn Agent>> =
        vec![Box::new(researcher), Box::new(writer), Box::new(editor)];
    let workflow = SequentialWorkflow::builder()
        .name("OpenRouterPipeline")
        .description("Research, write and edit with a different model at each stage")
        .metadata_output_dir("./temp/openrouter_pipeline/metadata")
        .agents(agents)
        .build();

    let result = workflow
        .run("How Rust's borrow checker prevents data races")
        .await?;

    // The last message is the editor's final article.
    if let Some(article) = result.history.last() {
        println!("{}", article.content);
    }

    Ok(())
}
