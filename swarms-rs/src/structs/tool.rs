use futures::future::BoxFuture;
use rmcp::{
    RoleClient,
    model::CallToolRequestParam,
    service::{DynService, RunningService},
};
use serde::{Deserialize, Serialize};
use std::{future::Future, ops::Deref, sync::Arc};
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
    ) -> impl Future<Output = Result<Self::Output, Self::Error>> + Send + Sync;

    // Provided method
    fn name(&self) -> String {
        Self::NAME.to_string()
    }
}

pub trait ToolDyn: Send + Sync {
    fn name(&self) -> String;

    fn definition(&self) -> ToolDefinition;

    fn call(&self, args: String) -> BoxFuture<'_, Result<String, ToolError>>;
}

impl<T: Tool> ToolDyn for T {
    fn name(&self) -> String {
        self.name()
    }

    fn definition(&self) -> ToolDefinition {
        <Self as Tool>::definition(self)
    }

    fn call(&self, args: String) -> BoxFuture<'_, Result<String, ToolError>> {
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

        if result.is_error.unwrap_or(false) {
            return Err(ToolError::from(MCPToolError(format!(
                "MCP tool call failed, content: {:?}",
                result.content
            ))));
        }

        Ok(result
            .content
            .into_iter()
            .map(|content| match content.raw {
                rmcp::model::RawContent::Text(raw_text_content) => raw_text_content.text,
                rmcp::model::RawContent::Image(raw_image_content) => format!(
                    "data:{};base64,{}",
                    raw_image_content.mime_type, raw_image_content.data
                ),
                rmcp::model::RawContent::Resource(rmcp::model::RawEmbeddedResource {
                    resource,
                }) => match resource {
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
                        "[URI]:{}\n{}[BLOB]:{}",
                        uri,
                        mime_type.map_or("".to_owned(), |mime| format!("[MIME]:{mime}\n")),
                        blob
                    ),
                },
                // TODO: latest version should uncomment the following line, but now we use old version
                // rmcp::model::RawContent::Audio(annotated) => format!(
                //     "data:{};base64,{}",
                //     annotated.raw.mime_type, annotated.raw.data
                // ),
            })
            .collect::<Vec<_>>()
            .join(""))
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
