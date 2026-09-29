<h1 align="left">Swarms Rust</h1>

<div align="left">
  <a href="https://swarms.ai">
    <img src="https://raw.githubusercontent.com/The-Swarm-Corporation/swarms-rs/main/logo.svg" style="margin: 15px; max-width: 800px" width="80%" alt="Logo">
  </a>
</div>

<p align="left">
  <a href="https://crates.io/crates/swarms-rs"><img alt="Crates.io" src="https://img.shields.io/crates/v/swarms-rs?style=for-the-badge&logo=rust&color=orange" /></a>
  <a href="https://docs.rs/swarms-rs"><img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-swarms--rs-blue?style=for-the-badge&logo=rust" /></a>
  <a href="https://discord.gg/EamjgSaEQf"><img alt="Discord" src="https://img.shields.io/discord/1202327470812078080?label=Discord&logo=discord&style=for-the-badge&color=5865F2" /></a>
  <a href="https://github.com/The-Swarm-Corporation/swarms-rs/blob/main/LICENSE"><img alt="License" src="https://img.shields.io/github/license/The-Swarm-Corporation/swarms-rs?style=for-the-badge&color=success" /></a>
</p>

<a href="README.md">English</a> | <a href="docs/README.zh.md">中文</a> | <a href="docs/README.ja.md">日本語</a>


<p align="left">
  <em>The Enterprise-Grade Production-Ready Multi-Agent Orchestration Framework in Rust</em>
</p>


Swarms Rust is the first-ever enterprise-grade, production-ready multi-agent orchestration framework built in Rust, designed to handle the most demanding tasks with unparalleled speed and efficiency. By leveraging Rust's cutting-edge performance and safety features, `swarms-rs` provides a powerful and scalable solution for orchestrating complex multi-agent systems across various industries.



## Key Benefits

| Feature                        | Description                                                                                                                                                                                                 |
|--------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Extreme Performance            | Utilize the full potential of modern multi-core processors with Rust's zero-cost abstractions and fearless concurrency. `Swarms-rs` ensures that your agents run with minimal overhead, achieving maximum throughput and efficiency. |
| Bleeding-Edge Speed            | Written in Rust, `swarms-rs` delivers near-zero latency and lightning-fast execution, making it the ideal choice for high-frequency and real-time applications.                                              |
| Enterprise-Grade Reliability    | Rust's ownership model guarantees memory safety without the need for a garbage collector, ensuring that your multi-agent systems are free from data races and memory leaks.                                   |
| Production-Ready               | Designed for real-world deployment, `swarms-rs` is ready to handle mission-critical tasks with robustness and reliability that you can depend on.                                                           |
| Powerful Orchestration         | Seamlessly manage and coordinate thousands of agents, allowing them to communicate and collaborate efficiently to achieve complex goals.                                                                     |
| Extensible and Modular         | `Swarms-rs` is highly modular, allowing developers to easily extend and customize the framework to suit specific use cases.                                                                                 |
| Scalable and Efficient         | Whether you're orchestrating a handful of agents or scaling up to millions, `swarms-rs` is designed to grow with your needs, maintaining top-tier performance at every level.                               |
| Resource Efficiency             | Maximize the use of system resources with Rust's fine-grained control over memory and processing power, ensuring that your agents run optimally even under heavy loads.                                      |

## Getting Started

### Prerequisites

- Rust (latest stable version recommended)
- Cargo package manager
- An API key for your LLM provider (OpenAI, DeepSeek, Anthropic etc.)


----------

### Installation

```bash
# Add the latest version to your project
cargo add swarms-rs

# Used by the examples below
cargo add tokio --features full
cargo add anyhow

# Only needed if you define tools with #[tool]
cargo add swarms-macro serde --features serde/derive
cargo add serde_json thiserror schemars@0.8
```


-------

### Environment Setup

Create a `.env` file in your project root with your API credentials:

```
RUST_LOG=debug

SWARMS_LOG_LEVEL=DEBUG 

OPENAI_API_KEY=your_openai_key_here
OPENAI_API_BASE=https://api.openai.com/v1

# Or for DeepSeek
DEEPSEEK_API_KEY="your_deepseek_key_here"
DEEPSEEK_BASE_URL="https://api.deepseek.com/v1"

ANTHROPIC_API_KEY=""

# Or for OpenRouter (one key for models from every major provider)
OPENROUTER_API_KEY=""
```

------------


## Quickstart


### Agents

An agent is an entity powered by an LLM equipped with tools and memory that can run autonomously to automate issues. Here's an example:

```rust
use std::env;

use anyhow::Result;
use swarms_rs::{llm::provider::openai::OpenAI, structs::agent::Agent};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(
            tracing_subscriber::fmt::layer()
                .with_line_number(true)
                .with_file(true),
        )
        .init();

    let base_url = env::var("DEEPSEEK_BASE_URL").unwrap();
    let api_key = env::var("DEEPSEEK_API_KEY").unwrap();
        let client = OpenAI::from_url(base_url, api_key).set_model("deepseek-chat");
    let agent = client
        .agent_builder()
        .system_prompt(
            "You are a sophisticated cryptocurrency analysis assistant specialized in:
            1. Technical analysis of crypto markets
            2. Fundamental analysis of blockchain projects
            3. Market sentiment analysis
            4. Risk assessment
            5. Trading patterns recognition
            
            When analyzing cryptocurrencies, always consider:
            - Market capitalization and volume
            - Historical price trends
            - Project fundamentals and technology
            - Recent news and developments
            - Market sentiment indicators
            - Potential risks and opportunities
            
            Provide clear, data-driven insights and always include relevant disclaimers about market volatility."
        )
        .agent_name("CryptoAnalyst")
        .user_name("Trader")
        .enable_autosave()
        .max_loops(3)  // Increased to allow for more thorough analysis
        .save_state_dir("./crypto_analysis/")
        .enable_plan("Break down the crypto analysis into systematic steps:
            1. Gather market data
            2. Analyze technical indicators
            3. Review fundamental factors
            4. Assess market sentiment
            5. Provide comprehensive insights".to_owned())
        .build();
    let response = agent
        .run("What is the meaning of life?".to_owned())
        .await
        .unwrap();
    println!("{response}");
    Ok(())
}

```

### Any provider by model name

`AnyModel` picks the provider from the model name, so switching providers is a one-string change. It reads the matching API key from the environment, and tools work the same way on every provider:

```rust
use swarms_rs::llm::provider::any::AnyModel;

let agent = AnyModel::from_model_name("anthropic/claude-opus-5-5")?  // or "openai/gpt-5.5",
    .agent_builder()                                                // "deepseek/deepseek-chat",
    .system_prompt("You are a helpful assistant.")                  // "google/gemini-3.8-flash", ...
    .build();
```

| Model name | Provider | API key |
|------------|----------|---------|
| `openai/...`, or bare `gpt-*`, `o1*`, `o3*`, `o4*` | OpenAI | `OPENAI_API_KEY` |
| `anthropic/...`, or bare `claude-*` | Anthropic | `ANTHROPIC_API_KEY` |
| `deepseek/...`, or bare `deepseek-*` | DeepSeek | `DEEPSEEK_API_KEY` |
| `openrouter/...`, or any other `vendor/model` (Google, Meta, Mistral, ...) | OpenRouter | `OPENROUTER_API_KEY` |

### OpenRouter

[OpenRouter](https://openrouter.ai) gives you one API key and one API for models from Anthropic, OpenAI, Google, Meta, Mistral, DeepSeek, xAI and more. `OpenRouter` implements the same `Model` trait as the other providers, so it works with tools, MCP servers and every multi-agent structure. Pick any model ID from [openrouter.ai/models](https://openrouter.ai/models), or keep the default `openrouter/auto` and let OpenRouter choose a model for each prompt.

| Variable | Required | Purpose |
|----------|----------|---------|
| `OPENROUTER_API_KEY` | Yes | Your OpenRouter key |
| `OPENROUTER_API_BASE` | No | Override the API base (default `https://openrouter.ai/api/v1`) |
| `OPENROUTER_APP_URL` / `OPENROUTER_APP_NAME` | No | Credit your app on openrouter.ai rankings |

#### A single agent

```rust
use swarms_rs::llm::provider::openrouter::OpenRouter;
use swarms_rs::structs::agent::Agent;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let agent = OpenRouter::from_env_with_model("anthropic/claude-opus-5.5")
        .agent_builder()
        .agent_name("Researcher")
        .system_prompt("You are a concise research assistant.")
        .build();

    println!("{}", agent.run("What is a vector database?".to_string()).await?);
    Ok(())
}
```

#### An agent with tools

Tools defined with `#[tool]` work with any OpenRouter model that supports tool calling, so switching models doesn't touch the tools:

```rust
use swarms_macro::tool;
use swarms_rs::llm::provider::openrouter::OpenRouter;
use swarms_rs::structs::agent::Agent;

#[derive(Debug, thiserror::Error)]
#[error("unknown unit '{0}'")]
pub struct UnknownUnit(String);

#[tool(
    description = "Convert a temperature between Celsius and Fahrenheit",
    arg(value, description = "The temperature to convert"),
    arg(to, description = "Target unit: 'celsius' or 'fahrenheit'")
)]
fn convert_temperature(value: f64, to: String) -> Result<f64, UnknownUnit> {
    match to.as_str() {
        "celsius" => Ok((value - 32.0) * 5.0 / 9.0),
        "fahrenheit" => Ok(value * 9.0 / 5.0 + 32.0),
        other => Err(UnknownUnit(other.to_string())),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let agent = OpenRouter::from_env_with_model("openai/gpt-5.5")
        .agent_builder()
        .system_prompt("Use the tools for unit conversions instead of guessing.")
        .add_tool(ConvertTemperature)
        .max_loops(2)
        .build();

    println!("{}", agent.run("What is 98.6°F in Celsius?".to_string()).await?);
    Ok(())
}
```

#### A panel of models from different providers

One client, one key, and a different model per agent. A `ConcurrentWorkflow` asks them all the same question at once:

```rust
use swarms_rs::llm::provider::openrouter::OpenRouter;
use swarms_rs::structs::agent::Agent;
use swarms_rs::structs::concurrent_workflow::ConcurrentWorkflow;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = OpenRouter::from_env();
    let models = ["anthropic/claude-opus-5.5", "openai/gpt-5.5", "google/gemini-3.8-flash"];

    let agents: Vec<Box<dyn Agent>> = models
        .iter()
        .map(|model| {
            Box::new(
                client
                    .clone()
                    .set_model(*model)
                    .agent_builder()
                    .agent_name(*model)
                    .system_prompt("Answer in at most three sentences and commit to a position.")
                    .build(),
            ) as Box<dyn Agent>
        })
        .collect();

    let workflow = ConcurrentWorkflow::builder()
        .name("ModelPanel")
        .agents(agents)
        .build();

    let result = workflow
        .run("Should a new backend service start as a monolith or as microservices?")
        .await?;
    for message in &result.history {
        println!("── {} ──\n{}\n", message.role, message.content);
    }
    Ok(())
}
```

#### A multi-model pipeline

Each stage of a `SequentialWorkflow` can run on the model best suited to it, such as a fast long-context model for research, a strong writer, and a different model family as the editor:

```rust
use swarms_rs::llm::provider::openrouter::OpenRouter;
use swarms_rs::structs::agent::Agent;
use swarms_rs::structs::sequential_workflow::SequentialWorkflow;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = OpenRouter::from_env();
    let stage = |model: &str, name: &str, prompt: &str| -> Box<dyn Agent> {
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

    let workflow = SequentialWorkflow::builder()
        .name("OpenRouterPipeline")
        .agents(vec![
            stage("google/gemini-3.8-flash", "Researcher", "List the key facts as bullet points."),
            stage("anthropic/claude-opus-5.5", "Writer", "Turn the notes into a 300-word article."),
            stage("openai/gpt-5.5", "Editor", "Fix errors and return only the final article."),
        ])
        .build();

    let result = workflow.run("How Rust's borrow checker prevents data races").await?;
    if let Some(article) = result.history.last() {
        println!("{}", article.content);
    }
    Ok(())
}
```

#### Run the OpenRouter examples

```bash
export OPENROUTER_API_KEY="sk-or-..."

cargo run --example openrouter_agent        # a single agent (set OPENROUTER_MODEL to pick a model)
cargo run --example openrouter_tools        # an agent with #[tool] functions
cargo run --example openrouter_model_panel  # several providers' models answer concurrently
cargo run --example openrouter_pipeline     # research -> write -> edit, a different model per stage
```

The full sources are in [`examples/single_agent`](swarms-rs/examples/single_agent) and [`examples/multiple_agent`](swarms-rs/examples/multiple_agent).

--------

### MCP Tool Support

`swarms-rs` supports the Model Context Protocol (MCP), enabling agents to interact with external tools through standardized interfaces. This powerful feature allows your agents to access real-world data and perform actions beyond their language capabilities.

### Supported MCP Server Types

- **STDIO MCP Servers**: Connect to command-line tools that implement the MCP protocol
- **SSE MCP Servers**: Connect to web-based MCP servers using Server-Sent Events

### Example Usage

```rust
// Add a STDIO MCP server
.add_stdio_mcp_server("uvx", ["mcp-hn"])
.await

// Add an SSE MCP server
.add_sse_mcp_server("example-sse-mcp-server", "http://127.0.0.1:8000/sse")
.await
```

----

### Full MCP Agent Example

```rust
use std::env;

use anyhow::Result;
use swarms_rs::{llm::provider::openai::OpenAI, structs::agent::Agent};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(
            tracing_subscriber::fmt::layer()
                .with_line_number(true)
                .with_file(true),
        )
        .init();

    let base_url = env::var("DEEPSEEK_BASE_URL").unwrap();
    let api_key = env::var("DEEPSEEK_API_KEY").unwrap();
    let client = OpenAI::from_url(base_url, api_key).set_model("deepseek-chat");
    let agent = client
        .agent_builder()
        .system_prompt("You are a helpful assistant.")
        .agent_name("SwarmsAgent")
        .user_name("User")
        // How to install uv: https://github.com/astral-sh/uv#installation
        // mcp stdio server, any other stdio mcp server can be used
        .add_stdio_mcp_server("uvx", ["mcp-hn"])
        .await
        // mcp sse server, we can use mcp-proxy to proxy the stdio mcp server(which does not support sse mode) to sse server
        // run in console: uvx mcp-proxy --sse-port=8000 -- npx -y @modelcontextprotocol/server-filesystem ~
        // this will start a sse server on port 8000, and ~ will be the only allowed directory to access
        .add_sse_mcp_server("example-sse-mcp-server", "http://127.0.0.1:8000/sse")
        .await
        .retry_attempts(1)
        .max_loops(1)
        .build();

    let response = agent
        .run("Get the top 3 stories of today".to_owned())
        .await
        .unwrap();
    // mcp-hn stdio server is called and give us the response
    println!("STDIO MCP RESPONSE:\n{response}");

    let response = agent.run("List ~ directory".to_owned()).await.unwrap();
    // example-sse-mcp-server is called and give us the response
    println!("SSE MCP RESPONSE:\n{response}");

    Ok(())
}
```


See the [mcp_tool.rs](swarms-rs/examples/mcp_tool.rs) example for a complete implementation.

-----------


## Multi-Agent Architectures

### ConcurrentWorkflow

This is an example of utilizing the `ConcurrentWorkflow` to concurrently execute multiple agents at the same time


```rust
use std::env;

use anyhow::Result;
use swarms_rs::llm::provider::openai::OpenAI;
use swarms_rs::structs::concurrent_workflow::ConcurrentWorkflow;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_line_number(true)
        .with_file(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let base_url = env::var("DEEPSEEK_BASE_URL").unwrap();
    let api_key = env::var("DEEPSEEK_API_KEY").unwrap();
    let client = OpenAI::from_url(base_url, api_key).set_model("deepseek-chat");

    // Create specialized trading agents with independent roles
    let market_analysis_agent = client
        .agent_builder()
        .agent_name("Market Analysis Agent")
        .system_prompt(
            "You are a market analysis specialist for trading. Analyze the provided market data \
       and identify key trends, patterns, and technical indicators. Your task is to provide \
       a comprehensive market analysis including support/resistance levels, volume analysis, \
       and overall market sentiment. Focus only on analyzing current market conditions \
       without making specific trading recommendations. End your analysis with <DONE>.",
        )
        .user_name("Trader")
        .max_loops(1)
        .temperature(0.2) // Lower temperature for precise technical analysis
        .enable_autosave()
        .save_state_dir("./temp/concurrent_workflow/trading")
        .add_stop_word("<DONE>")
        .build();

    let trade_strategy_agent = client
        .agent_builder()
        .agent_name("Trade Strategy Agent")
        .system_prompt(
            "You are a trading strategy specialist. Based on the provided market scenario, \
       develop a comprehensive trading strategy. Your task is to analyze the given market \
       information and create a strategy that includes potential entry and exit points, \
       position sizing recommendations, and order types. Focus solely on strategy development \
       without performing risk assessment. End your strategy with <DONE>.",
        )
        .user_name("Trader")
        .max_loops(1)
        .temperature(0.3)
        .enable_autosave()
        .save_state_dir("./temp/concurrent_workflow/trading")
        .add_stop_word("<DONE>")
        .build();

    let risk_assessment_agent = client
        .agent_builder()
        .agent_name("Risk Assessment Agent")
        .system_prompt(
            "You are a risk assessment specialist for trading. Your role is to evaluate \
       potential risks in the provided market scenario. Calculate appropriate risk metrics \
       such as volatility, maximum drawdown, and risk-reward ratios based solely on the \
       market information provided. Provide an independent risk assessment without \
       considering specific trading strategies. End your assessment with <DONE>.",
        )
        .user_name("Trader")
        .max_loops(1)
        .temperature(0.2)
        .enable_autosave()
        .save_state_dir("./temp/concurrent_workflow/trading")
        .add_stop_word("<DONE>")
        .build();

    // Create a concurrent workflow with all trading agents
    let workflow = ConcurrentWorkflow::builder()
        .name("Trading Strategy Workflow")
        .metadata_output_dir("./temp/concurrent_workflow/trading/workflow/metadata")
        .description("A workflow for analyzing market data with independent specialized agents.")
        .agents(vec![
            Box::new(market_analysis_agent),
            Box::new(trade_strategy_agent),
            Box::new(risk_assessment_agent),
        ])
        .build();

    let result = workflow
        .run(
            "BTC/USD is approaching a key resistance level at $50,000 with increasing volume. \
             RSI is at 68 and MACD shows bullish momentum. Develop a trading strategy for a \
             potential breakout scenario.",
        )
        .await?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
```

### Sub-agents and handoffs

An agent can work with other agents in two ways:

- **Sub-agents** (`add_sub_agent`): the model gets a `delegate_to_<name>` tool. Calling it runs the sub-agent on a subtask and returns its answer, and the calling agent carries on.
- **Handoffs** (`add_handoff`): the model gets a `transfer_to_<name>` tool. Calling it passes the task, a note on context, and the conversation so far to the other agent, which takes over; the calling agent stops and its output ends with that agent's answer.

```rust
use swarms_rs::llm::provider::openrouter::OpenRouter;
use swarms_rs::structs::agent::Agent;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = OpenRouter::from_env_with_model("anthropic/claude-opus-5.5");

    let researcher = client
        .agent_builder()
        .agent_name("Researcher")
        .description("Looks up facts and returns a short summary")
        .build();
    let writer = client
        .agent_builder()
        .agent_name("Writer")
        .description("Writes the final answer for the user")
        .build();

    let coordinator = client
        .agent_builder()
        .agent_name("Coordinator")
        .system_prompt("Delegate research to the Researcher, then transfer to the Writer.")
        .add_sub_agent(researcher) // delegate_to_Researcher
        .add_handoff(writer) // transfer_to_Writer
        .max_loops(4)
        .build();

    let task = "Why did Rust adopt async/await instead of green threads?";
    println!("{}", coordinator.run(task.to_string()).await?);

    // Tool calls stay typed in the agent's conversation.
    for call in coordinator.conversation(task).unwrap().tool_outputs() {
        println!("{} -> {}", call.name, call.result);
    }
    Ok(())
}
```

Run it with `cargo run --example sub_agents_and_handoffs`. Tool results are kept as `ToolCallOutput` values in the conversation (`Content::ToolCalls`), and `ToolCallOutput::result_as::<T>()` recovers the typed output of a `#[tool]` function.


-----------



## Run Examples

In [swarms-rs/examples](swarms-rs/examples/) there is our sample code, which can provide a considerable degree of reference:

To run the graph workflow example:

```bash
cargo run --example graph_workflow
```

Most examples read the `DEEPSEEK_API_KEY` and `DEEPSEEK_BASE_URL` environment variables; the `openrouter_*` examples read `OPENROUTER_API_KEY` (see [OpenRouter](#openrouter)).

----


## Framework Architecture

In swarms-rs, we modularize the framework into three primary architectural stages, each building upon the previous to create increasingly sophisticated agent systems:


```text
swarms-rs/
├── swarms-rs/                        # The framework crate
│   ├── src/
│   │   │   ── 1. Agent Layer ──
│   │   ├── agent/
│   │   │   └── swarms_agent.rs       # SwarmsAgent: the run loop, tool calls, planning, autosave
│   │   ├── llm/                      # LLM integration
│   │   │   ├── completion.rs         # Message and content types shared by every provider
│   │   │   ├── request.rs            # CompletionRequest, CompletionResponse, ToolDefinition
│   │   │   └── provider/
│   │   │       ├── openai.rs         # OpenAI and OpenAI-compatible APIs (DeepSeek, vLLM, ...)
│   │   │       ├── anthropic.rs      # Anthropic Claude
│   │   │       └── openrouter.rs     # OpenRouter: one key for models from every major provider
│   │   ├── structs/
│   │   │   ├── agent.rs              # Agent trait and AgentConfig
│   │   │   ├── tool.rs               # Tool traits and MCP tools
│   │   │   ├── conversation.rs       # Conversation memory
│   │   │   ├── persistence.rs        # Saving state and logs to disk
│   │   │   │
│   │   │   │   ── 2. Multi-Agent Structures ──
│   │   │   ├── sequential_workflow.rs    # Agents in a chain, each building on the last
│   │   │   ├── concurrent_workflow.rs    # Agents working on the same task in parallel
│   │   │   ├── graph_workflow.rs         # A DAG of agents with conditional edges
│   │   │   ├── rearrange.rs              # AgentRearrange: flows such as "a -> b, c"
│   │   │   ├── execute_agent_batch.rs    # Run many tasks across many agents
│   │   │   │
│   │   │   │   ── 3. Cascading Systems ──
│   │   │   ├── swarms_router.rs      # Choose a swarm type at runtime
│   │   │   ├── swarm.rs              # Swarm trait and run metadata shared by all structures
│   │   │   └── utils.rs
│   │   ├── prompts/                  # Built-in multi-agent collaboration prompts
│   │   └── logging.rs
│   ├── examples/
│   │   ├── single_agent/             # Agents, tools, MCP, Anthropic, OpenRouter
│   │   └── multiple_agent/           # Workflows, routers, multi-model pipelines
│   └── tests/
├── swarms-macro/                     # The #[tool] procedural macro
├── examples/                         # Standalone example crates (MCP servers, Binance agent, ...)
└── docs/                             # Translated READMEs and provider guides
```

# Features

| Feature | What it does |
|---------|--------------|
| **Agents** | LLM-powered agents with tools, memory, planning, retries and autosave |
| **LLM providers** | OpenAI and compatible APIs (DeepSeek, vLLM, ...), Anthropic Claude, and OpenRouter |
| **Tools** | Turn any Rust function into a tool with `#[tool]`, or connect MCP servers |
| **Sequential workflows** | Agents run in a chain, each building on the previous agent's output |
| **Concurrent workflows** | Several agents work on the same task in parallel |
| **Graph workflows** | Connect agents in a graph with conditional edges |
| **Agent rearrange** | Describe a flow as a string, such as `"researcher -> writer, editor"` |
| **Swarm router** | Choose the multi-agent structure at runtime |
| **Batch execution** | Run many tasks across many agents at once |
| **Persistence** | Save agent state and conversations to disk |

## Architecture

`swarms-rs` is built with a modular architecture that allows for easy extension and customization:

| Layer/Component         | Description                                                                                      |
|------------------------|--------------------------------------------------------------------------------------------------|
| **Agent Layer**        | Core agent implementation with memory management and tool integration                            |
| **LLM Provider Layer** | Abstraction for different LLM providers (OpenAI, Anthropic, OpenRouter, DeepSeek, etc.)           |
| **Tool System**        | Extensible tool framework for adding capabilities to agents                                       |
| **MCP Integration**    | Support for Model Context Protocol tools via STDIO and SSE interfaces                            |
| **Swarm Orchestration**| Coordination of multiple agents for complex workflows                                            |
| **Persistence Layer**  | State management and recovery mechanisms                                                         |

## Documentation

- [Conversations and Memory](docs/conversation.md): `AgentConversation`, agent memory, import and export
- [Persistence](docs/persistence.md): saving, loading, compressing and logging files, and where the framework writes them
- [Batch Execution and the Swarm Router](docs/batch_and_router.md): `AgentBatchExecutor`, `SwarmRouter` and their configs
- [Anthropic Claude](docs/ANTHROPIC_README.md): using Claude models

---------


### Development Setup

1. Clone the repository:

   ```bash
   git clone https://github.com/The-Swarm-Corporation/swarms-rs
   cd swarms-rs
   ```

2. Install development dependencies:

   ```bash
   cargo install cargo-nextest
   ```

3. Run tests:

   ```bash
   cargo nextest run
   ```

4. Run benchmarks:

   ```bash
   cargo bench
   ```

----------------


## Community

Join our growing community around the world for real-time support, ideas, and discussions on Swarms 😊

### Connect With Us

| Platform | Link | Description |
|----------|------|-------------|
| 📚 Documentation | [docs.swarms.world](https://docs.swarms.world) | Official documentation and guides |
| 📝 Blog | [Medium](https://medium.com/@kyeg) | Latest updates and technical articles |
| 💬 Discord | [Join Discord](https://discord.gg/EamjgSaEQf) | Live chat and community support |
| 🐦 Twitter | [@kyegomez](https://twitter.com/kyegomez) | Latest news and announcements |
| 👥 LinkedIn | [The Swarm Corporation](https://www.linkedin.com/company/the-swarm-corporation) | Professional network and updates |
| 📺 YouTube | [Swarms Channel](https://www.youtube.com/channel/UC9yXyitkbU_WSy7bd_41SqQ) | Tutorials and demos |
| 🎫 Events | [Sign up here](https://lu.ma/5p2jnc2v) | Join our community events |

### Contributing

We welcome contributions from the community! Whether you're fixing bugs, improving documentation, or adding new features, your help is valuable. Here's how you can contribute:

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

For more details, please read our [Contributing Guidelines](CONTRIBUTING.md).

### Join Our Discord

Join our [Discord community](https://discord.gg/EamjgSaEQf) to:

- Get real-time support
- Share your ideas and feedback
- Connect with other developers
- Stay updated on the latest features
- Participate in community events

We're excited to have you join our growing community! 🌟

-----

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

--------

## Contact

For questions, suggestions, or feedback, please open an issue or contact us at [kye@swarms.world](mailto:kye@swarms.world).
