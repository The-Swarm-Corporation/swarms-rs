//! The coding tools the agent can call.
//!
//! Each tool implements [`CodingTool`] and is registered with the swarms-rs agent through
//! [`Wrapped`], which implements swarms-rs's [`ToolDyn`] directly. Going through `ToolDyn`
//! rather than swarms-rs's typed `Tool` trait lets results reach the model as plain text:
//! `Tool` serializes its output as JSON, which would turn file contents and command output
//! into one long escaped string.

pub mod bash;
pub mod fs;
pub mod search;
pub mod todo;

use std::{future::Future, sync::Arc};

use futures::future::BoxFuture;
use serde::de::DeserializeOwned;
use serde_json::Value;
use swarms_rs::{
    llm::request::ToolDefinition,
    structs::tool::{ToolDyn, ToolError},
};

use crate::session::Session;

/// Sent back to the model when the user declines a tool call; the turn then ends.
pub const DENIED: &str =
    "The user declined this action. Stop here and wait for their next message.";

/// Sent for calls in the same step after one was declined.
const SKIPPED: &str = "Not run: the user declined an earlier action in this step.";

pub trait CodingTool: Send + Sync + 'static {
    type Args: DeserializeOwned + Send;
    const NAME: &'static str;
    const DESCRIPTION: &'static str;

    /// JSON Schema for [`Self::Args`].
    fn schema() -> Value;

    fn session(&self) -> &Session;

    /// Run the tool. `Err` is shown to the user and reported to the model as a failed call.
    fn run(&self, args: Self::Args) -> impl Future<Output = Result<String, String>> + Send;
}

/// Adapts a [`CodingTool`] to the agent's tool interface.
pub struct Wrapped<T>(pub T);

impl<T: CodingTool> ToolDyn for Wrapped<T> {
    fn name(&self) -> String {
        T::NAME.to_owned()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: T::NAME.to_owned(),
            description: T::DESCRIPTION.to_owned(),
            parameters: T::schema(),
        }
    }

    fn call(&self, args: String) -> BoxFuture<'_, Result<String, ToolError>> {
        Box::pin(async move {
            // After the user declines one call, don't run (or ask about) the rest of the step.
            if self.0.session().halted() {
                return Ok(SKIPPED.to_owned());
            }
            let args = match serde_json::from_str::<T::Args>(&args) {
                Ok(args) => args,
                Err(e) => {
                    let message = format!("Invalid arguments for {}: {e}", T::NAME);
                    self.0.session().ui.tool_error(&message);
                    return Ok(format!("Error: {message}"));
                },
            };
            Ok(match self.0.run(args).await {
                Ok(output) => output,
                Err(message) => {
                    self.0.session().ui.tool_error(&message);
                    format!("Error: {message}")
                },
            })
        })
    }
}

/// Every tool, in the order the model sees them.
pub fn all(session: &Arc<Session>) -> Vec<Box<dyn ToolDyn>> {
    let s = || Arc::clone(session);
    vec![
        Box::new(Wrapped(fs::ReadFile(s()))),
        Box::new(Wrapped(fs::WriteFile(s()))),
        Box::new(Wrapped(fs::EditFile(s()))),
        Box::new(Wrapped(fs::ListDir(s()))),
        Box::new(Wrapped(search::Glob(s()))),
        Box::new(Wrapped(search::Grep(s()))),
        Box::new(Wrapped(bash::Bash(s()))),
        Box::new(Wrapped(todo::TodoWrite(s()))),
    ]
}

/// Lets a boxed tool be registered with `SwarmsAgent::tool`, which takes a concrete type.
pub struct Boxed(pub Box<dyn ToolDyn>);

impl ToolDyn for Boxed {
    fn name(&self) -> String {
        self.0.name()
    }

    fn definition(&self) -> ToolDefinition {
        self.0.definition()
    }

    fn call(&self, args: String) -> BoxFuture<'_, Result<String, ToolError>> {
        self.0.call(args)
    }
}

/// Keep long output readable for the model: the start and end, with a note in between.
pub fn truncate_middle(text: &str, max_chars: usize) -> String {
    let total = text.chars().count();
    if total <= max_chars {
        return text.to_owned();
    }
    let head: String = text.chars().take(max_chars * 3 / 5).collect();
    let tail_len = max_chars * 2 / 5;
    let tail: String = text.chars().skip(total - tail_len).collect();
    format!(
        "{head}\n\n… [{} characters omitted] …\n\n{tail}",
        total - head.chars().count() - tail_len
    )
}

/// Treat data with NUL bytes near the start as binary.
pub fn looks_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8192).any(|&b| b == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_the_middle_of_long_text() {
        assert_eq!(truncate_middle("short", 10), "short");
        let long = "a".repeat(60) + &"b".repeat(40);
        let cut = truncate_middle(&long, 50);
        assert!(cut.starts_with(&"a".repeat(30)));
        assert!(cut.ends_with(&"b".repeat(20)));
        assert!(cut.contains("[50 characters omitted]"));
    }
}
