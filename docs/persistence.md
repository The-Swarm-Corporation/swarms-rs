# Persistence

`swarms_rs::structs::persistence` is a small set of file helpers. The framework uses them for agent autosave, workflow metadata and conversation export, and you can use them directly.

| Function | Async | What it does |
|----------|-------|--------------|
| `save_to_file(data, path)` | yes | Writes `data` to `path`, creating missing parent directories. Overwrites an existing file. |
| `append_to_file(data, path)` | yes | Appends `data` to `path`, creating the file and its parent directories if needed. |
| `load_from_file(path)` | yes | Reads the whole file and returns `Vec<u8>`. |
| `log_to_file(message, path)` | yes | Appends one line: `YYYY-MM-DD HH:MM:SS - message` (local time), creating the file if needed. |
| `compress(data)` | no | Compresses bytes with zstd at the default level. |
| `decompress(data)` | no | Decompresses zstd bytes. |

`data` can be anything that implements `AsRef<[u8]>` (`&str`, `String`, `Vec<u8>`, `&[u8]`), and `path` anything that implements `AsRef<Path>`. The async functions use `tokio::fs`, so call them from inside a Tokio runtime.

All of them return `Result<_, PersistenceError>`:

| Variant | When |
|---------|------|
| `IoError` | Any filesystem error, such as a missing file in `load_from_file`, or corrupt input to `decompress` |
| `JsonError` | JSON failures in code that uses this error type |
| `MissingParent` | `save_to_file` was given a path with no parent, such as `/` |

## Saving and loading JSON

```rust
use serde::{Deserialize, Serialize};
use swarms_rs::structs::persistence;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Settings {
    model: String,
    max_loops: u32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings { model: "openrouter/auto".to_string(), max_loops: 3 };

    // Parent directories are created for you.
    let path = "./temp/state/settings.json";
    persistence::save_to_file(serde_json::to_vec_pretty(&settings)?, path).await?;

    let bytes = persistence::load_from_file(path).await?;
    let loaded: Settings = serde_json::from_slice(&bytes)?;
    assert_eq!(loaded, settings);
    Ok(())
}
```

## Compressed files

`compress` and `decompress` work on bytes in memory; pair them with `save_to_file` and `load_from_file`:

```rust
use swarms_rs::structs::persistence;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let transcript = "The agent said hello. ".repeat(1_000);

    let compressed = persistence::compress(&transcript)?;
    println!("{} bytes -> {} bytes", transcript.len(), compressed.len());
    persistence::save_to_file(&compressed, "./temp/state/transcript.zst").await?;

    let bytes = persistence::load_from_file("./temp/state/transcript.zst").await?;
    let restored = String::from_utf8(persistence::decompress(&bytes)?)?;
    assert_eq!(restored, transcript);
    Ok(())
}
```

The output is a standard zstd frame, so `zstd -d transcript.zst` can read it too. Both functions are synchronous and CPU-bound; for large inputs, run them inside `tokio::task::spawn_blocking` so they don't stall other tasks.

## Appending and logging

```rust
use swarms_rs::structs::persistence;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Each call appends; include your own newline with append_to_file.
    persistence::append_to_file("first entry\n", "./temp/logs/events.log").await?;
    persistence::append_to_file("second entry\n", "./temp/logs/events.log").await?;

    // log_to_file adds a timestamp and the newline:
    // 2026-09-29 11:42:07 - agent started
    persistence::log_to_file("agent started", "./temp/logs/agent.log").await?;
    Ok(())
}
```

## Where the framework saves files

- **Agent autosave.** Call both `.enable_autosave()` and `.save_state_dir(dir)` on the agent builder. The agent then writes `<dir>/<agent name>_<task hash>.json` during and after each run. The file is the task's conversation as JSON, with its messages under `"history"`. `enable_autosave()` without `save_state_dir` saves nothing and gives no warning.
- **Workflow metadata.** `SequentialWorkflow`, `ConcurrentWorkflow` and `AgentRearrange` write metadata only when you set `metadata_output_dir`. `SequentialWorkflow` writes one `<task hash>.json` per run; if the write fails, it logs a warning and still returns its result, while the other two return an error.
- **Conversation export.** `AgentConversation::export_to_file` uses `save_to_file`; see [Conversations and Memory](conversation.md#saving-and-loading).

To read an autosaved conversation back:

```rust
use swarms_rs::structs::conversation::Message;
use swarms_rs::structs::persistence;

async fn load_autosave(path: &str) -> anyhow::Result<Vec<Message>> {
    let bytes = persistence::load_from_file(path).await?;
    let saved: serde_json::Value = serde_json::from_slice(&bytes)?;
    Ok(serde_json::from_value(saved["history"].clone())?)
}
```

## Path pitfalls

- **Relative paths** resolve against the process's current directory, not your crate's directory. `cargo run` from the workspace root and from a crate folder writes to different places. Build absolute paths (for example from `std::env::current_dir()` or a config value) when it matters.
- **`save_to_file` overwrites** without asking. Use `append_to_file` to add to a file.
- **Bytes, not text.** `load_from_file` returns `Vec<u8>`; convert with `String::from_utf8` or `serde_json::from_slice`.
- **Missing files** give `IoError` ("No such file or directory"), not an empty result. Check `Path::exists()` first if a missing file is normal for you.
