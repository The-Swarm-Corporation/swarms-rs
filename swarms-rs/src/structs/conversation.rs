use std::{
    collections::{HashMap, VecDeque},
    fmt::Display,
    path::{Path, PathBuf},
};

use chrono::Local;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::structs::{
    persistence::{self, PersistenceError},
    tool::ToolCallOutput,
};

#[derive(Debug, Error)]
pub enum ConversationError {
    #[error("Json error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("FilePersistence error: {0}")]
    FilePersistenceError(#[from] PersistenceError),
    #[error("Invalid conversation format: {0}")]
    InvalidFormat(String),
}

#[derive(Clone, Serialize)]
pub struct AgentShortMemory(pub DashMap<Task, AgentConversation>);
type Task = String;

impl AgentShortMemory {
    pub fn new() -> Self {
        Self(DashMap::new())
    }

    pub fn add(
        &self,
        task: impl Into<String>,
        conversation_owner: impl Into<String>,
        role: Role,
        message: impl Into<String>,
    ) {
        let mut conversation = self
            .0
            .entry(task.into())
            .or_insert(AgentConversation::new(conversation_owner.into()));
        conversation.add(role, message.into())
    }

    /// Record one turn of tool calls, keeping each call's typed output and any text the
    /// model wrote with them.
    pub fn add_tool_calls(
        &self,
        task: impl Into<String>,
        conversation_owner: impl Into<String>,
        role: Role,
        text: Option<String>,
        outputs: Vec<ToolCallOutput>,
    ) {
        let mut conversation = self
            .0
            .entry(task.into())
            .or_insert(AgentConversation::new(conversation_owner.into()));
        conversation.add_tool_calls(role, text, outputs)
    }
}

impl Default for AgentShortMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Serialize)]
pub struct AgentConversation {
    agent_name: String,
    save_filepath: Option<PathBuf>,
    pub history: Vec<Message>,
    max_messages: Option<usize>,
}

impl AgentConversation {
    pub fn new(agent_name: String) -> Self {
        Self {
            agent_name,
            save_filepath: None,
            history: Vec::new(),
            max_messages: Some(1_000_000), // Default maximum messages
        }
    }

    /// Create a new AgentConversation with a custom maximum message limit
    pub fn with_max_messages(agent_name: String, max_messages: Option<usize>) -> Self {
        Self {
            agent_name,
            save_filepath: None,
            history: Vec::new(),
            max_messages,
        }
    }

    /// Add a message to the conversation history.
    pub fn add(&mut self, role: Role, message: String) {
        let timestamp = Local::now().timestamp_millis();
        self.push(Message {
            role,
            content: Content::Text(format!("Timestamp(millis): {timestamp} \n{message}")),
        });
    }

    /// Add one turn of tool calls, keeping each call's typed output, along with any `text`
    /// the model wrote in the same reply. Its text form (used by `Display`,
    /// [`Self::export_to_file`] and the LLM history) matches what [`Self::add`] would store
    /// for the text followed by the formatted results.
    pub fn add_tool_calls(
        &mut self,
        role: Role,
        text: Option<String>,
        outputs: Vec<ToolCallOutput>,
    ) {
        self.push(Message {
            role,
            content: Content::ToolCalls {
                timestamp_millis: Local::now().timestamp_millis(),
                text: text.filter(|t| !t.trim().is_empty()),
                outputs,
            },
        });
    }

    /// Every tool call recorded in this conversation, oldest first.
    pub fn tool_outputs(&self) -> impl Iterator<Item = &ToolCallOutput> {
        self.history
            .iter()
            .flat_map(|message| message.content.tool_outputs())
    }

    fn push(&mut self, message: Message) {
        // Only check message limit if it's set
        if let Some(max) = self.max_messages
            && self.history.len() >= max
        {
            // Remove oldest messages to make room for new ones (bounded for max == 0)
            let excess = (self.history.len() + 1 - max).min(self.history.len());
            self.history.drain(0..excess);
        }

        self.history.push(message);

        if let Some(filepath) = &self.save_filepath {
            let filepath = filepath.clone();
            let history = self.history.clone();
            tokio::spawn(async move {
                let history = history;
                let _ = Self::save_as_json(&filepath, &history).await;
            });
        }
    }

    /// Delete a message from the conversation history.
    pub fn delete(&mut self, index: usize) {
        self.history.remove(index);
    }

    /// Update a message in the conversation history.
    pub fn update(&mut self, index: usize, role: Role, content: Content) {
        self.history[index] = Message { role, content };
    }

    /// Query a message in the conversation history.
    pub fn query(&self, index: usize) -> &Message {
        &self.history[index]
    }

    /// Search for a message in the conversation history.
    pub fn search(&self, keyword: &str) -> Vec<&Message> {
        self.history
            .iter()
            .filter(|message| message.content.to_string().contains(keyword))
            .collect()
    }

    // Clear the conversation history.
    pub fn clear(&mut self) {
        self.history.clear();
    }

    pub fn to_json(&self) -> Result<String, ConversationError> {
        Ok(serde_json::to_string(&self.history)?)
    }

    /// Replace the history with messages from JSON produced by [`Self::to_json`]. Unlike the
    /// text format, this round-trips any message content exactly.
    pub fn load_json(&mut self, json: &str) -> Result<(), ConversationError> {
        self.history = serde_json::from_str(json)?;
        Ok(())
    }

    /// Save the conversation history to a JSON file.
    async fn save_as_json(filepath: &Path, data: &[Message]) -> Result<(), ConversationError> {
        let json_data = serde_json::to_string_pretty(data)?;
        persistence::save_to_file(json_data.as_bytes(), filepath).await?;
        Ok(())
    }

    // TODO: We don't need this function now
    // Load the conversation history from a JSON file.
    // async fn load_from_json(&self, filepath: &Path) -> Result<Vec<Message>, ConversationError> {
    //     let data = persistence::load_from_file(filepath).await?;
    //     let history = serde_json::from_slice(&data)?;
    //     Ok(history)
    // }

    /// Export the conversation history to a file
    pub async fn export_to_file(&self, filepath: &Path) -> Result<(), ConversationError> {
        let data = self.to_string();
        persistence::save_to_file(data.as_bytes(), filepath).await?;
        Ok(())
    }

    /// Import the conversation history from a file written by [`Self::export_to_file`], or
    /// from a JSON file holding the output of [`Self::to_json`].
    pub async fn import_from_file(&mut self, filepath: &Path) -> Result<(), ConversationError> {
        let data = persistence::load_from_file(filepath).await?;
        if data.trim_ascii_start().starts_with(b"[")
            && let Ok(history) = serde_json::from_slice::<Vec<Message>>(&data)
        {
            self.history = history;
            return Ok(());
        }
        let text = String::from_utf8_lossy(&data);
        // Each message starts with a `Name(User): ` or `Name(Assistant): ` header; message
        // bodies (which include the timestamp line) can span several lines.
        let mut history: Vec<Message> = Vec::new();
        for line in text.lines() {
            let header = line.split_once(": ").and_then(|(role, content)| {
                if let Some(name) = role.strip_suffix("(User)") {
                    Some((Role::User(name.to_string()), content))
                } else {
                    role.strip_suffix("(Assistant)")
                        .map(|name| (Role::Assistant(name.to_string()), content))
                }
            });
            match (header, history.last_mut()) {
                (Some((role, content)), _) => history.push(Message {
                    role,
                    content: Content::Text(content.to_string()),
                }),
                // Imported messages are always text (the text export doesn't keep tool-call
                // structure; `to_json` does).
                (None, Some(message)) => {
                    if let Content::Text(text) = &mut message.content {
                        text.push('\n');
                        text.push_str(line);
                    }
                },
                (None, None) if line.is_empty() => {},
                (None, None) => {
                    return Err(ConversationError::InvalidFormat(format!(
                        "expected a `Name(User): ` or `Name(Assistant): ` header, found: {line}"
                    )));
                },
            }
        }
        self.history = history;
        Ok(())
    }

    /// Count the number of messages by role
    pub fn count_messages_by_role(&self) -> HashMap<String, usize> {
        let mut count = HashMap::new();
        for message in &self.history {
            *count.entry(message.role.to_string()).or_insert(0) += 1;
        }
        count
    }
}

impl Display for AgentConversation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for message in &self.history {
            writeln!(f, "{}: {}", message.role, message.content)?;
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Content,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    User(String),
    Assistant(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Content {
    Text(String),
    /// The tool calls an agent made in one turn, kept structured so workflows can read the
    /// results back without parsing. Displays as the same text the agent sends to the LLM:
    /// any text the model wrote with the calls, then one block per call.
    ToolCalls {
        timestamp_millis: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        outputs: Vec<ToolCallOutput>,
    },
}

impl Content {
    /// The tool calls in this message (empty for text).
    pub fn tool_outputs(&self) -> &[ToolCallOutput] {
        match self {
            Content::Text(_) => &[],
            Content::ToolCalls { outputs, .. } => outputs,
        }
    }
}

impl Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::User(name) => write!(f, "{}(User)", name),
            Role::Assistant(name) => write!(f, "{}(Assistant)", name),
        }
    }
}

impl Display for Content {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Content::Text(text) => f.pad(text),
            Content::ToolCalls {
                timestamp_millis,
                text: reply,
                outputs,
            } => {
                let mut text = format!("Timestamp(millis): {timestamp_millis} \n");
                if let Some(reply) = reply {
                    text.push_str(reply);
                    text.push_str("\n\n");
                }
                for output in outputs {
                    text.push_str(&output.to_string());
                }
                f.pad(&text)
            },
        }
    }
}

#[derive(Serialize)]
#[serde(rename = "history")]
pub struct SwarmConversation {
    pub logs: VecDeque<AgentLog>,
}

impl SwarmConversation {
    pub fn new() -> Self {
        Self {
            logs: VecDeque::new(),
        }
    }

    pub fn add_log(&mut self, agent_name: String, task: String, response: String) {
        tracing::info!("Agent: {agent_name} | Task: {task} | Response: {response}");
        let log = AgentLog {
            agent_name,
            task,
            response,
        };
        self.logs.push_back(log);
    }
}

impl Default for SwarmConversation {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize)]
pub struct AgentLog {
    pub agent_name: String,
    pub task: String,
    pub response: String,
}

impl From<&AgentConversation> for Vec<crate::llm::completion::Message> {
    fn from(conv: &AgentConversation) -> Self {
        conv.history
            .iter()
            .map(|msg| match &msg.role {
                Role::User(name) => {
                    crate::llm::completion::Message::user(format!("{}: {}", name, msg.content))
                },
                Role::Assistant(name) => {
                    crate::llm::completion::Message::assistant(format!("{}: {}", name, msg.content))
                },
            })
            .collect()
    }
}
