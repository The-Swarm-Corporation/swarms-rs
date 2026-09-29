use std::{cmp::Ordering, env};

use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessageArgs,
        ChatCompletionRequestAssistantMessageContent,
        ChatCompletionRequestAssistantMessageContentPart, ChatCompletionRequestMessage,
        ChatCompletionRequestMessageContentPartAudio, ChatCompletionRequestMessageContentPartImage,
        ChatCompletionRequestMessageContentPartText, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestToolMessage, ChatCompletionRequestToolMessageContent,
        ChatCompletionRequestToolMessageContentPart, ChatCompletionRequestUserMessageArgs,
        ChatCompletionRequestUserMessageContentPart, ChatCompletionToolArgs,
        ChatCompletionToolType, CreateChatCompletionRequestArgs, FunctionCall, FunctionObjectArgs,
        ImageDetail, ImageUrl, InputAudio, InputAudioFormat,
    },
};
use futures::future::BoxFuture;
use reqwest::header::HeaderMap;

use crate::{
    agent::SwarmsAgentBuilder, // Updated import path - now from crate::agent instead of crate::structs::agent
    llm::{
        self, CompletionError, Model,
        completion::MimeType,
        request::{CompletionRequest, CompletionResponse},
    },
};

const OPENAI_API_BASE: &str = "https://api.openai.com/v1";

#[derive(Clone)]
pub struct OpenAI {
    client: Client<OpenAIConfig>,
    model: String,
    system_prompt: Option<String>,
    /// OpenAI's own endpoint rejects `max_tokens` on reasoning models and wants
    /// `max_completion_tokens`; most OpenAI-compatible servers only know `max_tokens`.
    use_max_completion_tokens: bool,
}

impl OpenAI {
    pub fn new<S: Into<String>>(api_key: S) -> Self {
        Self::from_url(OPENAI_API_BASE.to_owned(), api_key.into())
    }

    pub fn from_url<S: Into<String>>(base_url: S, api_key: S) -> Self {
        let base_url = base_url.into();
        let use_max_completion_tokens = base_url.contains("api.openai.com");
        let config = OpenAIConfig::new()
            .with_api_base(base_url.trim_end_matches('/'))
            .with_api_key(api_key);
        let client =
            Client::with_config(config).with_http_client(build_http_client(HeaderMap::new()));
        Self {
            client,
            model: "gpt-4o-mini".to_owned(),
            system_prompt: None,
            use_max_completion_tokens,
        }
    }

    /// Send these headers on every request (e.g. OpenRouter's app attribution headers).
    pub(crate) fn with_default_headers(mut self, headers: HeaderMap) -> Self {
        self.client = self.client.with_http_client(build_http_client(headers));
        self
    }

    pub fn from_env() -> Self {
        let base_url = env::var("OPENAI_API_BASE").unwrap_or(OPENAI_API_BASE.to_owned());
        let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY is not set");
        Self::from_url(base_url, api_key)
    }

    pub fn from_env_with_model<S: Into<String>>(model: S) -> Self {
        let openai = Self::from_env();
        openai.set_model(model)
    }

    pub fn set_model<S: Into<String>>(mut self, model: S) -> Self {
        self.model = model.into();
        self
    }

    pub fn set_system_prompt<S: Into<String>>(&mut self, prompt: S) {
        self.system_prompt = Some(prompt.into());
    }

    pub fn agent_builder(&self) -> SwarmsAgentBuilder<Self> {
        SwarmsAgentBuilder::new_with_model(self.clone())
    }
}

fn build_http_client(default_headers: HeaderMap) -> reqwest::Client {
    reqwest::ClientBuilder::new()
        .user_agent("swarms-rs")
        .default_headers(default_headers)
        .build()
        .expect("TLS backend cannot be initialized")
}

impl Model for OpenAI {
    type RawCompletionResponse = async_openai::types::CreateChatCompletionResponse;

    fn completion(
        &self,
        request: CompletionRequest,
    ) -> BoxFuture<Result<CompletionResponse<Self::RawCompletionResponse>, CompletionError>> {
        Box::pin(async move {
            let mut msgs = Vec::new();

            if let Some(system_prompt) = request.system_prompt.or(self.system_prompt.clone()) {
                msgs.push(
                    ChatCompletionRequestSystemMessageArgs::default()
                        .content(system_prompt)
                        .build()?
                        .into(),
                );
            }

            let chat_history = request
                .chat_history
                .into_iter()
                .map(|msg| {
                    let msgs: Vec<ChatCompletionRequestMessage> = msg.try_into()?;
                    Ok::<_, CompletionError>(msgs)
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();

            msgs.extend(chat_history);

            // Send the whole prompt (tool results, images, ...), not only when it has text.
            if !is_empty_message(&request.prompt) {
                let prompt: Vec<ChatCompletionRequestMessage> = request.prompt.try_into()?;
                msgs.extend(prompt);
            }

            let mut create_request_builder = CreateChatCompletionRequestArgs::default();
            if let Some(max_tokens) = request.max_tokens {
                if self.use_max_completion_tokens {
                    create_request_builder.max_completion_tokens(max_tokens as u32);
                } else {
                    create_request_builder.max_tokens(max_tokens as u32);
                }
            }
            if let Some(temperature) = request.temperature {
                create_request_builder.temperature(temperature as f32);
            }
            if !request.tools.is_empty() {
                create_request_builder.tools(
                    request
                        .tools
                        .into_iter()
                        .map(|tool| {
                            ChatCompletionToolArgs::default()
                                .r#type(ChatCompletionToolType::Function)
                                .function(
                                    FunctionObjectArgs::default()
                                        .name(tool.name)
                                        .description(tool.description)
                                        .parameters(tool.parameters)
                                        .build()
                                        .expect("All field provided"),
                                )
                                .build()
                                .expect("All field provided")
                        })
                        .collect::<Vec<_>>(),
                );
            }
            let create_request = create_request_builder
                .model(self.model.clone())
                .messages(msgs)
                .build()?;

            tracing::debug!(
                "OpenAI Create Request: {}",
                serde_json::to_string_pretty(&create_request).unwrap()
            );

            let response: CompletionResponse<async_openai::types::CreateChatCompletionResponse> =
                self.client
                    .chat()
                    .create(create_request)
                    .await?
                    .try_into()?;

            tracing::debug!(
                "OpenAI response: {}",
                serde_json::to_string_pretty(&response.raw_response).unwrap()
            );

            Ok(response)
        })
    }
}

impl From<async_openai::error::OpenAIError> for CompletionError {
    fn from(error: async_openai::error::OpenAIError) -> Self {
        match error {
            async_openai::error::OpenAIError::Reqwest(e) => e.into(),
            async_openai::error::OpenAIError::ApiError(api_error) => {
                CompletionError::Provider(api_error.to_string())
            },
            // Non-JSON error bodies (proxies, OpenAI-compatible servers) land here; keep
            // the body so the caller can see what actually went wrong.
            async_openai::error::OpenAIError::JSONDeserialize(e, body) => {
                CompletionError::Response(format!("{e}: {body}"))
            },
            async_openai::error::OpenAIError::FileSaveError(e) => CompletionError::Other(e),
            async_openai::error::OpenAIError::FileReadError(e) => CompletionError::Other(e),
            async_openai::error::OpenAIError::StreamError(e) => {
                CompletionError::Other(e.to_string())
            },
            async_openai::error::OpenAIError::InvalidArgument(e) => {
                CompletionError::Request(e.into())
            },
        }
    }
}

impl TryFrom<llm::completion::Message> for Vec<ChatCompletionRequestMessage> {
    type Error = CompletionError;

    fn try_from(message: llm::completion::Message) -> Result<Self, Self::Error> {
        match message {
            llm::completion::Message::User { content } => {
                let (tool_results, other_content): (Vec<_>, Vec<_>) =
                    content.into_iter().partition(|content| {
                        matches!(content, llm::completion::UserContent::ToolResult(_))
                    });
                let mut messages: Vec<ChatCompletionRequestMessage> = Vec::new();
                if !tool_results.is_empty() {
                    let results = tool_results
                        .into_iter()
                        .map(|content| {
                            let llm::completion::UserContent::ToolResult(tool_result) = content
                            else {
                                unreachable!();
                            };

                            let content = tool_result
                                .content
                                .into_iter()
                                .map(|content| match content {
                                    llm::completion::ToolResultContent::Text(text) => {
                                        Ok(ChatCompletionRequestMessageContentPartText::from(text))
                                    },
                                    _ => Err(CompletionError::Request(
                                        "OpenAI only supports text for now".into(),
                                    )),
                                })
                                .collect::<Result<Vec<_>, _>>()?;

                            let content = match content.len() {
                                0 => Err(CompletionError::Request(
                                    "Tool result content cannot be empty".into(),
                                ))?,
                                1 => ChatCompletionRequestToolMessageContent::Text(
                                    content[0].text.clone(),
                                ),
                                _ => ChatCompletionRequestToolMessageContent::Array(
                                    content
                                        .into_iter()
                                        .map(ChatCompletionRequestToolMessageContentPart::Text)
                                        .collect(),
                                ),
                            };

                            Ok::<_, CompletionError>(ChatCompletionRequestToolMessage {
                                tool_call_id: tool_result.id,
                                content,
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;

                    messages.extend(results.into_iter().map(Into::into));
                    // Text or images sent alongside the tool results follow as a user message.
                    if other_content.is_empty() {
                        return Ok(messages);
                    }
                }

                let user_message = match other_content.len().cmp(&1) {
                    Ordering::Greater => {
                        let content_array = other_content
                        .into_iter()
                        .map(|content| match content {
                            llm::completion::UserContent::Text(text) => Ok(ChatCompletionRequestMessageContentPartText::from(text).into()),
                            llm::completion::UserContent::Image(image) => Ok(ChatCompletionRequestMessageContentPartImage::from(image).into()),
                            llm::completion::UserContent::Audio(audio) => {
                                if audio.format != Some(llm::completion::ContentFormat::Base64)
                                    || (audio.media_type
                                        != Some(llm::completion::AudioMediaType::WAV)
                                        && audio.media_type
                                            != Some(llm::completion::AudioMediaType::MP3))
                                {
                                    return Err(CompletionError::Request("Only support wav and mp3 for now, and must be base64 encoded".into()))
                                }

                                Ok(ChatCompletionRequestMessageContentPartAudio::from(audio).into())
                            }
                            _ => Err(CompletionError::Request("Unsupported content type".into())),
                        })
                        .collect::<Result<Vec<ChatCompletionRequestUserMessageContentPart>, _>>()?;
                        ChatCompletionRequestUserMessageArgs::default()
                            .content(content_array)
                            .build()
                            .unwrap() // Safety: All required fields are set
                            .into()
                    },
                    Ordering::Equal => match &other_content[0] {
                        llm::completion::UserContent::Text(text) => {
                            ChatCompletionRequestUserMessageArgs::default()
                                .content(text.text.as_str())
                                .build()
                                .unwrap() // Safety: All required fields are set
                                .into()
                        },
                        llm::completion::UserContent::Image(image) => {
                            let content_part = vec![
                                ChatCompletionRequestMessageContentPartImage::from(image).into(),
                            ];

                            ChatCompletionRequestUserMessageArgs::default()
                                .content(content_part)
                                .build()
                                .unwrap() // Safety: All required fields are set
                                .into()
                        },
                        llm::completion::UserContent::Audio(audio) => {
                            // Only support wav and mp3 for now, and must be base64 encoded
                            if audio.format != Some(llm::completion::ContentFormat::Base64)
                                || (audio.media_type != Some(llm::completion::AudioMediaType::WAV)
                                    && audio.media_type
                                        != Some(llm::completion::AudioMediaType::MP3))
                            {
                                return Err(CompletionError::Request(
                                    "Only support wav and mp3 for now, and must be base64 encoded"
                                        .into(),
                                ));
                            }
                            let content_part = vec![
                                ChatCompletionRequestMessageContentPartAudio::from(audio.clone())
                                    .into(),
                            ];
                            ChatCompletionRequestUserMessageArgs::default()
                                .content(content_part)
                                .build()
                                .unwrap()
                                .into()
                        },
                        _ => {
                            return Err(CompletionError::Request(
                                "Unsupported content type".into(),
                            ));
                        },
                    },
                    Ordering::Less => {
                        return Err(CompletionError::Request(
                            "User message must have at least one content".into(),
                        ));
                    },
                };
                messages.push(user_message);
                Ok(messages)
            },
            llm::completion::Message::Assistant { content } => {
                let (text_content, tool_calls) = content.into_iter().fold(
                    (Vec::new(), Vec::new()),
                    |(mut texts, mut tools), content| {
                        match content {
                            llm::completion::AssistantContent::Text(text) => texts.push(text),
                            llm::completion::AssistantContent::ToolCall(tool_call) => {
                                tools.push(tool_call)
                            },
                        }
                        (texts, tools)
                    },
                );

                let mut message_builder = ChatCompletionRequestAssistantMessageArgs::default();
                let text_content = (!text_content.is_empty()).then_some(text_content);
                let tool_calls = (!tool_calls.is_empty()).then_some(tool_calls);

                let message_builder = match (text_content, tool_calls) {
                    (text_content, Some(tool_calls)) => {
                        if let Some(text_content) = text_content {
                            let text = text_content
                                .into_iter()
                                .map(|text| text.text)
                                .collect::<Vec<_>>()
                                .join("\n");
                            message_builder.content(text);
                        }
                        let tool_calls = tool_calls
                            .into_iter()
                            .map(|tool_call| ChatCompletionMessageToolCall {
                                id: tool_call.id,
                                r#type: ChatCompletionToolType::Function,
                                function: FunctionCall {
                                    name: tool_call.function.name,
                                    arguments: tool_call.function.arguments.to_string(),
                                },
                            })
                            .collect::<Vec<_>>();
                        message_builder.tool_calls(tool_calls)
                    },
                    (Some(text_content), None) => {
                        let text_content = text_content
                            .into_iter()
                            .map(|text| {
                                ChatCompletionRequestAssistantMessageContentPart::Text(text.into())
                            })
                            .collect::<Vec<_>>();
                        let text_content = match text_content.len().cmp(&1) {
                            Ordering::Greater => {
                                ChatCompletionRequestAssistantMessageContent::Array(text_content)
                            },
                            Ordering::Equal => {
                                if let ChatCompletionRequestAssistantMessageContentPart::Text(
                                    content,
                                ) = &text_content[0]
                                {
                                    ChatCompletionRequestAssistantMessageContent::Text(
                                        content.text.clone(),
                                    )
                                } else {
                                    return Err(CompletionError::Request(
                                        "Unsupported content type".into(),
                                    ));
                                }
                            },
                            _ => unreachable!(),
                        };
                        message_builder.content(text_content)
                    },
                    (None, None) => {
                        return Err(CompletionError::Request(
                            "Assistant message must have at least one content".into(),
                        ));
                    },
                };

                Ok(vec![message_builder.build().unwrap().into()])
            },
        }
    }
}

impl From<llm::completion::Text>
    for async_openai::types::ChatCompletionRequestMessageContentPartText
{
    fn from(text: llm::completion::Text) -> Self {
        Self { text: text.text }
    }
}

impl From<llm::completion::Image>
    for async_openai::types::ChatCompletionRequestMessageContentPartImage
{
    fn from(image: llm::completion::Image) -> Self {
        Self::from(&image)
    }
}

impl From<&llm::completion::Image>
    for async_openai::types::ChatCompletionRequestMessageContentPartImage
{
    fn from(image: &llm::completion::Image) -> Self {
        // OpenAI takes a URL; raw base64 has to be wrapped in a data URL.
        let url = match (&image.format, &image.media_type) {
            (Some(llm::completion::ContentFormat::Base64), media_type)
                if !image.data.starts_with("data:") =>
            {
                let mime = media_type
                    .as_ref()
                    .map(|m| m.to_mime_type())
                    .unwrap_or("image/png");
                format!("data:{mime};base64,{}", image.data)
            },
            _ => image.data.clone(),
        };
        let detail = image.detail.as_ref().map(|detail| match detail {
            llm::completion::ImageDetail::Low => ImageDetail::Low,
            llm::completion::ImageDetail::High => ImageDetail::High,
            llm::completion::ImageDetail::Auto => ImageDetail::Auto,
        });
        Self {
            image_url: ImageUrl { url, detail },
        }
    }
}

impl From<llm::completion::Audio>
    for async_openai::types::ChatCompletionRequestMessageContentPartAudio
{
    fn from(audio: llm::completion::Audio) -> Self {
        let audio_type = match audio.media_type {
            Some(llm::completion::AudioMediaType::WAV) => InputAudioFormat::Wav,
            Some(llm::completion::AudioMediaType::MP3) => InputAudioFormat::Mp3,
            _ => unimplemented!("Unsupported audio type"),
        };

        Self {
            input_audio: InputAudio {
                data: audio.data,
                format: audio_type,
            },
        }
    }
}

impl TryFrom<async_openai::types::CreateChatCompletionResponse>
    for llm::CompletionResponse<async_openai::types::CreateChatCompletionResponse>
{
    type Error = CompletionError;

    fn try_from(
        response: async_openai::types::CreateChatCompletionResponse,
    ) -> Result<Self, Self::Error> {
        let mut choices = Vec::new();
        for choice in &response.choices {
            let message = &choice.message;
            if let Some(refusal) = message.refusal.as_ref().filter(|r| !r.is_empty()) {
                return Err(CompletionError::Response(format!(
                    "Model refused the request: {refusal}"
                )));
            }
            // Text and tool calls can arrive together, and some OpenAI-compatible
            // servers send `"tool_calls": []` on plain replies.
            if let Some(content) = message.content.as_ref().filter(|c| !c.is_empty()) {
                choices.push(llm::completion::AssistantContent::text(content));
            }
            for tool_call in message.tool_calls.iter().flatten() {
                let raw = tool_call.function.arguments.trim();
                // Zero-argument tools often come back with empty arguments.
                let arguments = if raw.is_empty() {
                    serde_json::json!({})
                } else {
                    serde_json::from_str(raw).map_err(|e| {
                        CompletionError::Response(format!(
                            "Invalid JSON arguments for tool '{}': {e}. Arguments: {raw}",
                            tool_call.function.name
                        ))
                    })?
                };
                choices.push(llm::completion::AssistantContent::tool_call(
                    tool_call.id.clone(),
                    tool_call.function.name.clone(),
                    arguments,
                ));
            }
        }

        Ok(Self {
            choice: choices,
            raw_response: response,
        })
    }
}

/// True when a message carries nothing worth sending (no parts, or only empty text).
fn is_empty_message(message: &llm::completion::Message) -> bool {
    match message {
        llm::completion::Message::User { content } => content
            .iter()
            .all(|c| matches!(c, llm::completion::UserContent::Text(text) if text.text.is_empty())),
        llm::completion::Message::Assistant { content } => content.is_empty(),
    }
}
