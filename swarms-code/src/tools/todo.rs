//! The `todo_write` tool: a checklist the agent keeps for multi-step tasks, shown to the user.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use super::CodingTool;
use crate::session::Session;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Todo {
    pub content: String,
    pub status: TodoStatus,
}

pub struct TodoWrite(pub Arc<Session>);

#[derive(Deserialize)]
pub struct TodoArgs {
    todos: Vec<Todo>,
}

impl CodingTool for TodoWrite {
    type Args = TodoArgs;

    const NAME: &'static str = "todo_write";
    const DESCRIPTION: &'static str = "Create or update your todo list for the current task; the \
        user sees it. Send the complete list each time, since it replaces the previous one. Use \
        it for tasks with three or more steps: keep exactly one item in_progress while you work \
        on it, and mark items completed as soon as they're done. Skip it for simple tasks.";

    fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "description": "The complete, updated todo list",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": { "type": "string", "description": "What needs doing, as a short imperative" },
                            "status": { "type": "string", "enum": ["pending", "in_progress", "completed"] }
                        },
                        "required": ["content", "status"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["todos"],
            "additionalProperties": false
        })
    }

    fn session(&self) -> &Session {
        &self.0
    }

    fn run(&self, args: TodoArgs) -> impl Future<Output = Result<String, String>> + Send {
        let session = &self.0;
        session.ui.tool_start("Update Todos", "");
        session.ui.todos(&args.todos);
        let done = args
            .todos
            .iter()
            .filter(|todo| todo.status == TodoStatus::Completed)
            .count();
        let summary = format!(
            "Todo list updated: {done} of {} completed.",
            args.todos.len()
        );
        session.set_todos(args.todos);
        std::future::ready(Ok(summary))
    }
}
