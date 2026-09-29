//! A coordinator that delegates to a sub-agent (and keeps control) and can hand the task
//! off to a specialist (who takes over and answers).
//!
//! ```bash
//! export OPENROUTER_API_KEY="sk-or-..."
//! cargo run --example sub_agents_and_handoffs
//! ```

use anyhow::Result;
use swarms_rs::llm::provider::openrouter::OpenRouter;
use swarms_rs::structs::agent::Agent;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    swarms_rs::logging::init_logger();

    let client = OpenRouter::from_env_with_model("anthropic/claude-opus-5.5");

    // Sub-agent: the coordinator calls it like a tool and uses its answer.
    let researcher = client
        .agent_builder()
        .agent_name("Researcher")
        .description("Looks up facts and returns a short, sourced summary")
        .system_prompt("You are a researcher. Answer with concise facts only.")
        .max_loops(1)
        .build();

    // Handoff target: once the coordinator transfers to it, it finishes the task.
    let writer = client
        .agent_builder()
        .agent_name("Writer")
        .description("Writes the final, polished answer for the user")
        .system_prompt(
            "You are a writer. Using the context and conversation you are given, write the \
             final answer for the user in two short paragraphs.",
        )
        .max_loops(1)
        .build();

    let coordinator = client
        .agent_builder()
        .agent_name("Coordinator")
        .system_prompt(
            "You coordinate a small team. Delegate fact-finding to the Researcher, then \
             transfer to the Writer with the facts as context so it can write the answer.",
        )
        .add_sub_agent(researcher)
        .add_handoff(writer)
        .max_loops(4)
        .build();

    let task = "Why did Rust adopt async/await instead of green threads?";
    let output = coordinator.run(task.to_string()).await?;
    println!("{output}");

    // Every tool call, including the delegation and the handoff, is kept with its result.
    if let Some(conversation) = coordinator.conversation(task) {
        for call in conversation.tool_outputs() {
            println!("-- {} returned {} bytes", call.name, call.result.len());
        }
    }

    Ok(())
}
