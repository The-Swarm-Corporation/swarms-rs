//! # Mixture of Agents example
//!
//! Demonstrates the layered `MixtureOfAgents` ensemble: several worker agents
//! generate proposals in parallel across multiple refinement layers, and an
//! aggregator agent synthesises a final answer from the full transcript.
//!
//! Run with `cargo run --example mixture_of_agents` after setting
//! `OPENAI_API_KEY` in your environment.

use std::env;

use anyhow::Result;
use swarms_rs::{
    llm::provider::openai::OpenAI,
    structs::mixture_of_agents::{
        MixtureOfAgents, MixtureOfAgentsBuilder, DEFAULT_AGGREGATOR_SYSTEM_PROMPT,
    },
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
    let client = OpenAI::from_url("https://api.openai.com/v1", &api_key).set_model("gpt-4o-mini");

    // Two specialised worker agents that propose answers in parallel.
    let researcher = client
        .agent_builder()
        .agent_name("Researcher")
        .system_prompt(
            "You are a meticulous researcher. Provide a fact-rich, well-structured answer \
             that surfaces the key evidence and considerations for the given task.",
        )
        .max_loops(1)
        .temperature(0.7)
        .build();

    let critic = client
        .agent_builder()
        .agent_name("Critic")
        .system_prompt(
            "You are a sharp critical thinker. Provide an answer that stress-tests common \
             assumptions, highlights risks, and offers a balanced perspective.",
        )
        .max_loops(1)
        .temperature(0.7)
        .build();

    // Aggregator synthesises the final answer from every worker response.
    let aggregator = client
        .agent_builder()
        .agent_name("Aggregator")
        .system_prompt(DEFAULT_AGGREGATOR_SYSTEM_PROMPT)
        .max_loops(1)
        .temperature(0.3)
        .build();

    // Two refinement layers, then aggregation.
    let moa: MixtureOfAgents = MixtureOfAgentsBuilder::default()
        .name("ResearchAndCritiqueMoA")
        .description("A researcher and a critic propose, an aggregator synthesises.")
        .agents(vec![Box::new(researcher), Box::new(critic)])
        .aggregator_agent(Box::new(aggregator))
        .layers(2)
        .build()?;

    let task = "What are the trade-offs of using multi-agent systems for production workloads?";
    let answer = moa.execute(task).await?;

    println!("\n=== Mixture of Agents final answer ===\n{answer}\n");
    Ok(())
}
