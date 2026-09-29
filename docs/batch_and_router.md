# Batch Execution and the Swarm Router

Two entry points run agents at scale without wiring a workflow by hand:

- `AgentBatchExecutor` (`swarms_rs::structs::execute_agent_batch`) sends many tasks to a set of agents at once.
- `SwarmRouter` (`swarms_rs::structs::swarms_router`) picks a multi-agent structure (sequential, concurrent or rearrange) from a config, so you can switch structures without changing your code.

## AgentBatchExecutor

Every task is sent to every agent. Tasks run concurrently; within one task, the agents run one after another, and each agent gets the original task (agents don't see each other's answers). Use a [`SequentialWorkflow`](../README.md#multi-agent-architectures) if you want agents to build on each other.

```rust
use swarms_rs::llm::provider::openrouter::OpenRouter;
use swarms_rs::structs::agent::Agent;
use swarms_rs::structs::execute_agent_batch::{AgentBatchExecutor, BatchConfigBuilder};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = OpenRouter::from_env();
    let make_agent = |name: &str, model: &str, prompt: &str| -> Box<dyn Agent> {
        Box::new(
            client
                .clone()
                .set_model(model)
                .agent_builder()
                .agent_name(name)
                .system_prompt(prompt)
                .build(),
        )
    };

    let executor = AgentBatchExecutor::builder()
        .add_agent(make_agent("Summarizer", "anthropic/claude-opus-5.5", "Summarize in one sentence."))
        .add_agent(make_agent("Critic", "openai/gpt-5.5", "Give one weakness of the idea."))
        .config(BatchConfigBuilder::default().max_concurrent_tasks(4).build())
        .build();

    let tasks = vec![
        "A browser extension that summarizes long emails".to_string(),
        "A CLI that explains failing CI logs".to_string(),
    ];
    let results = executor.execute_batch(tasks).await?;

    // DashMap<task, AgentConversation>: one conversation per task,
    // with one Assistant message per agent that succeeded.
    for entry in results.iter() {
        println!("## {}", entry.key());
        for message in &entry.value().history {
            println!("{} said:\n{}\n", message.role, message.content);
        }
    }
    Ok(())
}
```

### Configuration

`BatchConfig` controls how many tasks are in flight at once. Build it with `BatchConfigBuilder::default()`, or use `BatchConfig::default()`.

| Field | Default | Meaning |
|-------|---------|---------|
| `max_concurrent_tasks` | `None` | How many tasks run at the same time |
| `worker_threads` | `None` | Used as the limit when `max_concurrent_tasks` is not set. It doesn't start any threads. |
| `auto_cpu_optimization` | `true` | When neither of the above is set, use the number of CPU cores; if `false`, use 4 |

The limit is chosen in that order, and a value of 0 is treated as 1.

### Results and errors

`execute_batch(tasks)` returns `Result<DashMap<String, AgentConversation>, BatchExecutionError>`:

- The key is the task text. The conversation has one `Role::Assistant(agent name)` message per agent that succeeded, in agent order. Each message's content is that agent's full output: for a `SwarmsAgent`, the transcript of its run.
- If an agent fails on a task, the error is logged and that agent is left out. A task where every agent failed is missing from the map, so compare the map's keys with your task list to spot failures. If nothing succeeded at all, the call returns `Err(BatchExecutionError::AgentError(..))` with the last error.
- Repeating a task in the input runs it again and adds the new answers to the same conversation.
- `BatchExecutionError::NoAgents` and `NoTasks` are returned for an executor without agents or an empty task list.

### Testing offline with a stub model

Any type that implements `Model` can power an agent, which makes it easy to exercise batch code without an API key:

```rust
use futures::future::BoxFuture;
use swarms_rs::agent::SwarmsAgentBuilder;
use swarms_rs::llm::completion::AssistantContent;
use swarms_rs::llm::request::{CompletionRequest, CompletionResponse};
use swarms_rs::llm::{CompletionError, Model};
use swarms_rs::structs::agent::Agent;
use swarms_rs::structs::execute_agent_batch::AgentBatchExecutor;

/// Always answers "ok".
#[derive(Clone)]
struct StubModel;

impl Model for StubModel {
    type RawCompletionResponse = ();

    fn completion(
        &self,
        _request: CompletionRequest,
    ) -> BoxFuture<'_, Result<CompletionResponse<()>, CompletionError>> {
        Box::pin(async {
            Ok(CompletionResponse {
                choice: vec![AssistantContent::text("ok")],
                raw_response: (),
            })
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let agent = |name: &str| -> Box<dyn Agent> {
        Box::new(SwarmsAgentBuilder::new_with_model(StubModel).agent_name(name).build())
    };
    let executor = AgentBatchExecutor::builder()
        .add_agent(agent("A"))
        .add_agent(agent("B"))
        .build();

    let results = executor.execute_batch(vec!["t1".to_string(), "t2".to_string()]).await?;
    assert_eq!(results.len(), 2);
    assert_eq!(results.get("t1").unwrap().history.len(), 2); // one message per agent
    Ok(())
}
```

## SwarmRouter

`SwarmRouter` builds one of three structures from a `SwarmRouterConfig` and runs tasks on it.

| `SwarmType` | Runs as | Agents |
|-------------|---------|--------|
| `SequentialWorkflow` (default) | `SequentialWorkflow` | One after another, each getting the previous agent's output |
| `ConcurrentWorkflow` | `ConcurrentWorkflow` | All at once on the same task |
| `AgentRearrange` | `AgentRearrange` | Following `flow`, e.g. `"Researcher -> Writer, Reviewer"` |

`SwarmType` implements `Deserialize`, so it can come from a config file: `"ConcurrentWorkflow"` in JSON parses to `SwarmType::ConcurrentWorkflow`.

```rust
use swarms_rs::llm::provider::openai::OpenAI;
use swarms_rs::structs::swarms_router::{SwarmRouter, SwarmRouterConfig, SwarmType};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Reads OPENAI_API_KEY (and OPENAI_API_BASE for OpenAI-compatible servers).
    let client = OpenAI::from_env().set_model("gpt-4o-mini");
    let researcher = client
        .agent_builder()
        .agent_name("Researcher")
        .system_prompt("List the key facts about the topic.")
        .build();
    let writer = client
        .agent_builder()
        .agent_name("Writer")
        .system_prompt("Write a short paragraph from the facts you are given.")
        .build();

    let router = SwarmRouter::new_with_config(SwarmRouterConfig {
        name: "research-team".to_string(),
        swarm_type: SwarmType::AgentRearrange,
        agents: vec![researcher, writer],
        flow: Some("Researcher -> Writer".to_string()),
        rules: Some("Cite a source for every claim.".to_string()),
        ..Default::default()
    })?;

    // One task: returns an AgentConversation.
    let conversation = router.run("The history of the Rust language").await?;
    for message in &conversation.history {
        println!("{}: {}", message.role, message.content);
    }

    // Several tasks: returns DashMap<task, AgentConversation>.
    let batch = router
        .batch_run(vec!["Topic A".to_string(), "Topic B".to_string()])
        .await?;
    println!("{} tasks finished", batch.len());
    Ok(())
}
```

For a one-off run, `swarm_router(task, config)` creates the router and runs a single task in one call.

### Configuration

| Field | Default | Meaning |
|-------|---------|---------|
| `name` | `"swarm-router"` | Name of the underlying workflow |
| `description` | `"Routes your task to the desired swarm"` | Description stored with the workflow |
| `swarm_type` | `SequentialWorkflow` | Which structure to build |
| `agents` | empty | The agents; at least one is required |
| `rules` | `None` | Text appended to every agent's system prompt under `### SWARM RULES ###` |
| `multi_agent_collab_prompt` | `true` | Appends the built-in multi-agent collaboration prompt to every agent's system prompt |
| `flow` | `None` | Required for `AgentRearrange`; uses agent names |
| `max_loops` | `None` | Loops for `AgentRearrange` (defaults to 1) |

### What you get back

- `run(task)` returns `Result<AgentConversation, SwarmRouterError>`. For the sequential and concurrent types, the conversation starts with the task as a `User` message, followed by one `Assistant` message per agent. For the concurrent type, those messages are in the order the agents finished. `AgentRearrange` conversations start with the task under the name `System`.
- `batch_run(tasks)` returns `Result<DashMap<String, AgentConversation>, SwarmRouterError>`, keyed by task. The concurrent type runs the tasks at the same time and leaves failed tasks out of the map. The sequential and rearrange types run tasks one at a time and stop with an error at the first failure.

### Errors

| Variant | When |
|---------|------|
| `ValidationError` | `new_with_config` got no agents |
| `SequentialWorkflowError` / `ConcurrentWorkflowError` / `AgentRearrangeError` | The workflow failed, e.g. an `AgentRearrange` without a `flow` fails with "Flow cannot be empty" |

### Limitations

- All agents in one router use the same model type. `SwarmRouterConfig::default()` is for OpenAI agents; for any other model (Anthropic, OpenRouter, `AnyModel`, your own `Model`), start from `SwarmRouterConfig::with_agents(agents)`. To mix providers in one router, use `AnyModel` for every agent.
- The sequential type writes a metadata file per run to `./temp/sequential_workflow/metadata`, relative to the current directory. See [Persistence](persistence.md#where-the-framework-saves-files).
