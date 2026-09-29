# Conversations and Memory

`swarms_rs::structs::conversation` holds the message history that agents and workflows build up while they work. You get these types back from every workflow, and you can create and edit them yourself.

| Type | What it is |
|------|------------|
| `AgentConversation` | One ordered conversation: a `Vec<Message>` plus an optional message limit |
| `AgentShortMemory` | A map from task text to `AgentConversation`; each `SwarmsAgent` keeps one |
| `SwarmConversation` | An append-only log of `{agent_name, task, response}` entries |
| `Message` | A `Role` and a `Content` |
| `Role` | `Role::User(name)` or `Role::Assistant(name)` |
| `Content` | `Content::Text(String)`, or `Content::ToolCalls { .. }` for an agent turn that called tools |

## Messages

`add` stores the text with a timestamp line in front of it, so every message's content looks like this:

```text
Timestamp(millis): 1790697587441 
Hello
```

Printing a conversation (`Display`, or `to_string()`) writes each message as `Name(Role): content`:

```text
Alice(User): Timestamp(millis): 1790697587441 
Hello
Bot(Assistant): Timestamp(millis): 1790697587442 
Hi there
```

## Working with a conversation

```rust
use swarms_rs::structs::conversation::{AgentConversation, Content, Role};

fn main() {
    let mut conversation = AgentConversation::new("support-bot".to_string());
    conversation.add(Role::User("Alice".to_string()), "My order is late".to_string());
    conversation.add(Role::Assistant("Bot".to_string()), "Let me check.".to_string());

    // Read by index, or search the text of every message.
    println!("{}", conversation.query(0).content);
    let hits = conversation.search("order");
    assert_eq!(hits.len(), 1);

    // Count messages per role. Keys are the displayed role, e.g. "Alice(User)".
    let counts = conversation.count_messages_by_role();
    assert_eq!(counts["Bot(Assistant)"], 1);

    // Replace or remove a message by index. `update` stores the content as given,
    // without adding a timestamp line.
    conversation.update(1, Role::Assistant("Bot".to_string()), Content::Text("Shipped today.".to_string()));
    conversation.delete(0);
    assert_eq!(conversation.history.len(), 1);

    // `history` is public, so you can also iterate it directly.
    for message in &conversation.history {
        println!("{} -> {}", message.role, message.content);
    }

    conversation.clear();
    assert!(conversation.history.is_empty());
}
```

`query`, `update` and `delete` panic if the index is out of range, like indexing a `Vec`. Check `history.len()` first when the index comes from outside your code. `search` matches against the full content, including the timestamp line, so searching for `"Timestamp"` matches every message.

## Limiting the number of messages

`AgentConversation::new` keeps up to 1,000,000 messages. Use `with_max_messages` to set your own limit; once it's reached, each `add` drops the oldest message:

```rust
use swarms_rs::structs::conversation::{AgentConversation, Role};

fn main() {
    let mut recent = AgentConversation::with_max_messages("chat".to_string(), Some(2));
    for i in 0..5 {
        recent.add(Role::User("user".to_string()), format!("message {i}"));
    }
    // Only "message 3" and "message 4" are left.
    assert_eq!(recent.history.len(), 2);

    // `None` means no limit.
    let _unbounded = AgentConversation::with_max_messages("log".to_string(), None);
}
```

A limit of `Some(0)` behaves like `Some(1)`: the newest message is always kept.

## Saving and loading

There are two formats.

**Text**, with `export_to_file` and `import_from_file`. The file is exactly what `Display` prints. Importing parses the `Name(User): ` and `Name(Assistant): ` headers back into messages, and multi-line messages survive the round trip:

```rust
use std::path::Path;
use swarms_rs::structs::conversation::{AgentConversation, Role};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut conversation = AgentConversation::new("notes".to_string());
    conversation.add(Role::User("Alice".to_string()), "Plan the launch".to_string());
    conversation.add(Role::Assistant("Planner".to_string()), "1. Draft\n2. Review".to_string());

    let path = Path::new("./temp/conversations/launch.txt");
    conversation.export_to_file(path).await?;

    let mut restored = AgentConversation::new("notes".to_string());
    restored.import_from_file(path).await?;
    assert_eq!(restored.to_string(), conversation.to_string());
    Ok(())
}
```

`import_from_file` replaces the current history rather than appending to it. It returns `ConversationError::InvalidFormat` if the file doesn't start with a message header. Because headers are recognized by their text, a message line that itself starts with something like `Bob(User): ` is read back as a new message.

**JSON**, with `to_json` and `load_json`. `to_json` returns the history as a JSON array, and `load_json` replaces the history with it. `import_from_file` also accepts a file containing that JSON:

```rust
use swarms_rs::structs::conversation::{AgentConversation, Role};

fn main() -> anyhow::Result<()> {
    let mut conversation = AgentConversation::new("notes".to_string());
    conversation.add(Role::User("Alice".to_string()), "Hello".to_string());

    // [{"role":{"User":"Alice"},"content":{"Text":"Timestamp(millis): ... \nHello"}}]
    let json = conversation.to_json()?;

    let mut restored = AgentConversation::new("notes".to_string());
    restored.load_json(&json)?;
    assert_eq!(restored.to_string(), conversation.to_string());
    Ok(())
}
```

JSON round-trips any content exactly, so prefer it when message text may contain lines that look like headers (agent outputs in workflows often do). `AgentConversation` itself implements `Serialize` but not `Deserialize`; use `to_json` and `load_json`, which carry the `history` field.

## Agent memory: `AgentShortMemory`

A `SwarmsAgent` stores its working memory as an `AgentShortMemory`: one `AgentConversation` per task, keyed by the task text. You can use it the same way:

```rust
use swarms_rs::structs::conversation::{AgentShortMemory, Role};

fn main() {
    let memory = AgentShortMemory::new();
    memory.add("summarize report", "analyst", Role::User("user".to_string()), "Here is the report...");
    memory.add("summarize report", "analyst", Role::Assistant("analyst".to_string()), "Summary: ...");
    memory.add("draft email", "analyst", Role::User("user".to_string()), "Write to the team");

    // The inner DashMap is public: one conversation per task.
    assert_eq!(memory.0.len(), 2);
    let report = memory.0.get("summarize report").unwrap();
    assert_eq!(report.history.len(), 2);
}
```

Because memory is keyed by task text, running the same task string twice on the same agent continues the earlier conversation instead of starting a new one. Use distinct task strings, or a new agent, when you want a fresh start.

To send a conversation to a model yourself, convert it: `let messages: Vec<swarms_rs::llm::completion::Message> = (&conversation).into();`. Each message becomes a user or assistant message whose text is `name: content`.

## Typed tool results

When an agent calls tools, the turn is stored as `Content::ToolCalls`, which keeps each call's name, arguments and JSON result as a `ToolCallOutput`. Its text form is unchanged (`[Tool name]: ...`), so `Display` and the history sent to the model look the same as before. Read the results back without parsing text:

```rust,ignore
// `agent` is a SwarmsAgent that has run `task`
if let Some(conversation) = agent.conversation(&task) {
    for output in conversation.tool_outputs() {
        let value: f64 = output.result_as()?; // the tool's return type
        println!("{} returned {value}", output.name);
    }
}
```

## Workflow logs: `SwarmConversation`

`SwarmConversation` is a simple log for multi-agent runs. `add_log(agent_name, task, response)` appends an entry (and also logs it with `tracing`), and the struct serializes to JSON:

```rust
use swarms_rs::structs::conversation::SwarmConversation;

fn main() -> anyhow::Result<()> {
    let mut log = SwarmConversation::new();
    log.add_log("Researcher".to_string(), "find sources".to_string(), "Found 3 papers".to_string());
    // {"logs":[{"agent_name":"Researcher","task":"find sources","response":"Found 3 papers"}]}
    println!("{}", serde_json::to_string(&log)?);
    Ok(())
}
```

It can't be deserialized, and it has no size limit.

## Known limitations

- `query`, `update` and `delete` panic on an out-of-range index.
- Content always includes the timestamp line added by `add`; strip the first line if you need the bare text.
- The text format can misread message lines that look like headers; prefer JSON for arbitrary text.
- A whole `AgentConversation` can't be deserialized; `to_json` and `load_json` carry the history only.
- Agent memory is keyed by task text, so a repeated task reuses its old conversation.
- A conversation can't be set to save itself on every `add`; call `export_to_file` or `to_json` when you want a copy on disk. For saving an agent's memory automatically, see [autosave](persistence.md#where-the-framework-saves-files).
