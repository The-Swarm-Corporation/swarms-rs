//! Other agents exposed to the model as tools.
//!
//! - A **sub-agent** is delegation: the model calls `delegate_to_<name>` with a subtask, the
//!   sub-agent runs it, and its answer comes back as the tool result. The caller keeps control.
//! - A **handoff** is a transfer of control: when the model calls `transfer_to_<name>`, the
//!   run loop gives the target agent the task, the context the model wrote, and the
//!   conversation so far, then ends with the target's answer (see `SwarmsAgent::run_handoff`).

use std::sync::Arc;

use futures::future::BoxFuture;
use serde::Deserialize;
use serde_json::json;

use crate::{
    llm::request::ToolDefinition,
    structs::{
        agent::Agent,
        tool::{ToolDyn, ToolError},
    },
};

/// OpenAI and Anthropic both require tool names to match `^[a-zA-Z0-9_-]{1,64}$`.
const MAX_TOOL_NAME_LEN: usize = 64;

/// Build a valid tool name `<prefix><agent name>` that `is_taken` doesn't already use.
///
/// Characters outside `[a-zA-Z0-9_-]` become `_`, and the name is shortened to fit the
/// 64-character limit, leaving room for a `_2`, `_3`, ... suffix when needed.
pub(crate) fn tool_name_for(
    prefix: &str,
    agent_name: &str,
    is_taken: impl Fn(&str) -> bool,
) -> String {
    let mut body: String = agent_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if body.trim_matches('_').is_empty() {
        body = "agent".to_string();
    }

    // `body` is ASCII now, so byte lengths are character counts.
    let fit = |body: &str, suffix: &str| {
        let room = MAX_TOOL_NAME_LEN - prefix.len() - suffix.len();
        format!("{prefix}{}{suffix}", &body[..body.len().min(room)])
    };
    let mut name = fit(&body, "");
    let mut n = 2;
    while is_taken(&name) {
        name = fit(&body, &format!("_{n}"));
        n += 1;
    }
    name
}

#[derive(Deserialize)]
struct DelegateArgs {
    task: String,
}

/// Arguments of a handoff tool call.
#[derive(Deserialize)]
pub(crate) struct HandoffArgs {
    pub task: String,
    #[serde(default)]
    pub context: Option<String>,
}

/// Runs a sub-agent on the subtask the model gives it and returns the sub-agent's answer.
pub(crate) struct SubAgentTool {
    tool_name: String,
    agent: Arc<dyn Agent>,
}

impl SubAgentTool {
    pub(crate) fn new(tool_name: String, agent: Arc<dyn Agent>) -> Self {
        Self { tool_name, agent }
    }
}

impl ToolDyn for SubAgentTool {
    fn name(&self) -> String {
        self.tool_name.clone()
    }

    fn definition(&self) -> ToolDefinition {
        let name = self.agent.name();
        let description = self.agent.description();
        ToolDefinition {
            name: self.tool_name.clone(),
            description: if description.is_empty() {
                format!("Delegate a subtask to the '{name}' agent and get its answer back.")
            } else {
                format!(
                    "Delegate a subtask to the '{name}' agent and get its answer back. \
                     {description}"
                )
            },
            parameters: json!({
                "type": "object",
                "properties": {
                    "task": {
                        "type": "string",
                        "description": "The subtask for the agent, with everything it needs to know"
                    }
                },
                "required": ["task"]
            }),
        }
    }

    fn call(&self, args: String) -> BoxFuture<'_, Result<String, ToolError>> {
        Box::pin(async move {
            let args: DelegateArgs = serde_json::from_str(&args)?;
            self.agent.run(args.task).await.map_err(|e| {
                ToolError::ToolCallError(
                    format!("sub-agent '{}' failed: {e}", self.agent.name()).into(),
                )
            })
        })
    }
}

/// The tool the model calls to hand off. It only acknowledges the call; the run loop runs
/// the target agent, because only the loop has the conversation to pass along.
pub(crate) struct HandoffTool {
    tool_name: String,
    target_name: String,
    target_description: String,
}

impl HandoffTool {
    pub(crate) fn new(tool_name: String, target: &dyn Agent) -> Self {
        Self {
            tool_name,
            target_name: target.name(),
            target_description: target.description(),
        }
    }
}

impl ToolDyn for HandoffTool {
    fn name(&self) -> String {
        self.tool_name.clone()
    }

    fn definition(&self) -> ToolDefinition {
        let mut description = format!(
            "Transfer the task to the '{}' agent, which takes over and gives the final answer. \
             Use it when that agent is better suited to finish the task.",
            self.target_name
        );
        if !self.target_description.is_empty() {
            description.push(' ');
            description.push_str(&self.target_description);
        }
        ToolDefinition {
            name: self.tool_name.clone(),
            description,
            parameters: json!({
                "type": "object",
                "properties": {
                    "task": {
                        "type": "string",
                        "description": "The task the next agent should complete"
                    },
                    "context": {
                        "type": "string",
                        "description": "What the next agent needs to know: findings so far, constraints, the user's preferences"
                    }
                },
                "required": ["task"]
            }),
        }
    }

    fn call(&self, args: String) -> BoxFuture<'_, Result<String, ToolError>> {
        Box::pin(async move {
            serde_json::from_str::<HandoffArgs>(&args)?;
            Ok(format!("Handing off to '{}'", self.target_name))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_names_are_valid_and_unique() {
        let valid = |name: &str| {
            !name.is_empty()
                && name.len() <= MAX_TOOL_NAME_LEN
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        };

        let name = tool_name_for("delegate_to_", "Research Agent v2.1 🚀", |_| false);
        assert_eq!(name, "delegate_to_Research_Agent_v2_1__");
        assert!(valid(&name));

        assert_eq!(
            tool_name_for("delegate_to_", "", |_| false),
            "delegate_to_agent"
        );
        assert_eq!(
            tool_name_for("delegate_to_", "日本語", |_| false),
            "delegate_to_agent"
        );

        let long = "x".repeat(200);
        let name = tool_name_for("transfer_to_", &long, |_| false);
        assert_eq!(name.len(), MAX_TOOL_NAME_LEN);
        assert!(valid(&name));

        let taken = [name.clone()];
        let second = tool_name_for("transfer_to_", &long, |n| taken.contains(&n.to_string()));
        assert_ne!(second, name);
        assert!(second.ends_with("_2"));
        assert!(valid(&second));
    }
}
