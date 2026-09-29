use futures::future::BoxFuture;
use rmcp::{
    RoleClient,
    model::CallToolRequestParam,
    service::{DynService, RunningService},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{fmt::Display, future::Future, ops::Deref, sync::Arc};
use thiserror::Error;

use crate::llm::request::ToolDefinition;

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    /// Error returned by the tool
    #[error("ToolCallError: {0}")]
    ToolCallError(#[from] Box<dyn core::error::Error + Send + Sync>),

    #[error("JsonError: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Contains the complete information about a single tool execution.
///
/// When an agent executes a tool, this structure captures all the relevant
/// information about the call, including the tool name, arguments passed,
/// and the result returned. The agent keeps these in its conversation (see
/// [`Content::ToolCalls`](crate::structs::conversation::Content::ToolCalls)), so
/// workflows can read tool results back without parsing text.
///
/// # Examples
///
/// ```rust,no_run
/// use swarms_rs::agent::ToolCallOutput;
///
/// let tool_output = ToolCallOutput {
///     name: "calculator".to_string(),
///     args: r#"{"operation": "add", "a": 5, "b": 3}"#.to_string(),
///     result: "8".to_string(),
/// };
///
/// println!("Tool {} with args {} returned: {}",
///          tool_output.name, tool_output.args, tool_output.result);
/// assert_eq!(tool_output.result_as::<i64>().unwrap(), 8);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallOutput {
    /// The name of the tool that was executed.
    ///
    /// This corresponds to the tool's identifier as registered with the agent.
    pub name: String,

    /// The arguments passed to the tool as a JSON string.
    ///
    /// The arguments are serialized as JSON to provide a consistent format
    /// regardless of the tool's specific parameter structure.
    pub args: String,

    /// The result returned by the tool's execution as a string.
    ///
    /// All tool results are converted to strings for consistent handling,
    /// even if the tool internally works with other data types.
    pub result: String,
}

impl ToolCallOutput {
    /// Deserialize the result into `T`.
    ///
    /// Tools defined with `#[tool]` return their output serialized as JSON, so this recovers
    /// the typed value. Results that aren't JSON (tool errors, MCP text, sub-agent answers)
    /// return an error.
    pub fn result_as<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_str(&self.result)
    }
}

/// The text form the agent keeps in memory and sends back to the LLM.
impl Display for ToolCallOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[Tool name]: {}\n[Tool args]: {}\n[Tool result]: {}\n\n",
            self.name, self.args, self.result
        )
    }
}

pub trait Tool: Sized + Send + Sync {
    type Error: core::error::Error + Send + Sync + 'static;
    type Args: for<'a> Deserialize<'a> + Send + Sync;
    type Output: Serialize;

    const NAME: &'static str;

    // Required methods
    fn definition(&self) -> ToolDefinition;
    fn call(
        &self,
        args: Self::Args,
    ) -> impl Future<Output = Result<Self::Output, Self::Error>> + Send;

    // Provided method
    fn name(&self) -> String {
        Self::NAME.to_string()
    }
}

pub trait ToolDyn: Send + Sync {
    fn name(&self) -> String;

    fn definition(&self) -> ToolDefinition;

    fn call(&self, args: String) -> BoxFuture<Result<String, ToolError>>;
}

impl<T: Tool> ToolDyn for T {
    fn name(&self) -> String {
        self.name()
    }

    fn definition(&self) -> ToolDefinition {
        <Self as Tool>::definition(self)
    }

    fn call(&self, args: String) -> BoxFuture<Result<String, ToolError>> {
        Box::pin(async move {
            match serde_json::from_str(&args) {
                Ok(args) => <Self as Tool>::call(self, args)
                    .await
                    .map_err(|e| ToolError::ToolCallError(Box::new(e)))
                    .and_then(|output| {
                        serde_json::to_string(&output).map_err(ToolError::JsonError)
                    }),
                Err(e) => Err(ToolError::JsonError(e)),
            }
        })
    }
}

pub struct MCPTool {
    tool: rmcp::model::Tool,
    client: Arc<RunningService<RoleClient, Box<dyn DynService<RoleClient>>>>,
}

impl MCPTool {
    pub fn from_server(
        tool: rmcp::model::Tool,
        client: Arc<RunningService<RoleClient, Box<dyn DynService<RoleClient>>>>,
    ) -> Self {
        Self { tool, client }
    }
}

impl Tool for MCPTool {
    type Error = ToolError;

    type Args = serde_json::Map<String, serde_json::Value>;

    type Output = String;

    // We don't need NAME for MCPTool
    const NAME: &'static str = "";

    fn name(&self) -> String {
        self.tool.name.to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::from(&self.tool)
    }

    async fn call(
        &self,
        args: serde_json::Map<String, serde_json::Value>,
    ) -> Result<Self::Output, Self::Error> {
        let result = self
            .client
            .call_tool(CallToolRequestParam {
                name: Tool::name(self).into(),
                arguments: Some(args),
            })
            .await
            .map_err(|e| MCPToolError(format!("MCP tool call failed: {e}")))?;

        let text = result
            .content
            .into_iter()
            .map(|content| mcp_content_to_text(content.raw))
            .collect::<Vec<_>>()
            .join("\n");

        if result.is_error.unwrap_or(false) {
            return Err(ToolError::from(MCPToolError(format!(
                "MCP tool returned an error: {text}"
            ))));
        }
        Ok(text)
    }
}

/// Render one MCP result block as text for the model. Binary data (images, blobs) is
/// summarized: a model can't read base64 as text, and it would be re-sent on every loop.
fn mcp_content_to_text(content: rmcp::model::RawContent) -> String {
    match content {
        rmcp::model::RawContent::Text(raw_text_content) => raw_text_content.text,
        rmcp::model::RawContent::Image(image) => format!(
            "[image: {}, {} bytes of base64 data]",
            image.mime_type,
            image.data.len()
        ),
        rmcp::model::RawContent::Resource(rmcp::model::RawEmbeddedResource { resource }) => {
            match resource {
                rmcp::model::ResourceContents::TextResourceContents {
                    uri,
                    mime_type,
                    text,
                } => format!(
                    "[URI]:{}\n{}[TEXT]:{}",
                    uri,
                    mime_type.map_or("".to_owned(), |m| format!("[MIME]:{}\n", m)),
                    text
                ),
                rmcp::model::ResourceContents::BlobResourceContents {
                    uri,
                    mime_type,
                    blob,
                } => format!(
                    "[URI]:{}\n{}[BLOB]: {} bytes of base64 data",
                    uri,
                    mime_type.map_or("".to_owned(), |mime| format!("[MIME]:{mime}\n")),
                    blob.len()
                ),
            }
        },
    }
}

#[derive(Debug, Error)]
#[error("MCPToolError: {0}")]
pub struct MCPToolError(String);

impl From<&rmcp::model::Tool> for ToolDefinition {
    fn from(value: &rmcp::model::Tool) -> Self {
        let name = value.name.to_string();
        let description = value
            .to_owned()
            .description
            // TODO: latest version should uncomment the following line, but now we use old version
            // .unwrap_or(name.clone().into())
            .to_string();
        let parameters = serde_json::Value::Object(value.input_schema.deref().to_owned());

        Self {
            name,
            description,
            parameters,
        }
    }
}

impl From<MCPToolError> for ToolError {
    fn from(value: MCPToolError) -> Self {
        Self::ToolCallError(Box::new(value))
    }
}

#[cfg(test)]
mod mcp_tests {
    use super::*;

    #[test]
    fn mcp_content_is_readable_text() {
        let text = rmcp::model::RawContent::text("hello");
        assert_eq!(mcp_content_to_text(text), "hello");

        let image = rmcp::model::RawContent::image("aGVsbG8=", "image/png");
        assert_eq!(
            mcp_content_to_text(image),
            "[image: image/png, 8 bytes of base64 data]"
        );
    }
}
