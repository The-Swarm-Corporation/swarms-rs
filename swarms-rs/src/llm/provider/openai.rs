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
        ImageUrl, InputAudio, InputAudioFormat,
    },
};
use futures::future::BoxFuture;

use crate::{
    agent::SwarmsAgentBuilder, // Updated import path - now from crate::agent instead of crate::structs::agent
    llm::{
        self, CompletionError, Model,
        request::{CompletionRequest, CompletionResponse},
    },
};

#[derive(Clone)]
pub struct OpenAI {
    client: Client<OpenAIConfig>,
    model: String,
    system_prompt: Option<String>,
}

impl OpenAI {
    pub fn new<S: Into<String>>(api_key: S) -> Self {
        let config = OpenAIConfig::new().with_api_key(api_key);
        let http_client = reqwest::ClientBuilder::new()
            .user_agent("swamrs-rs")
            .build()
            .expect("TLS backend cannot be initialized");
        let client = Client::with_config(config).with_http_client(http_client);
        Self {
            client,
            model: "gpt-4o-mini".to_owned(),
            system_prompt: None,
        }
    }

    pub fn from_url<S: Into<String>>(base_url: S, api_key: S) -> Self {
        let config = OpenAIConfig::new()
            .with_api_base(base_url)
            .with_api_key(api_key);
        let http_client = reqwest::ClientBuilder::new()
            .user_agent("swamrs-rs")
            .build()
            .expect("TLS backend cannot be initialized");
        let client = Client::with_config(config).with_http_client(http_client);
        Self {
            client,
            model: "gpt-4o-mini".to_owned(),
            system_prompt: None,
        }
    }

    pub fn from_env() -> Self {
        let base_url =
            env::var("OPENAI_API_BASE").unwrap_or("https://api.openai.com/v1".to_owned());
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

impl Model for OpenAI {
    type RawCompletionResponse = async_openai::types::CreateChatCompletionResponse;

    fn completion(
        &self,
        request: CompletionRequest,
    ) -> BoxFuture<Result<CompletionResponse<Self::RawCompletionResponse>, CompletionError>> {
        Box::pin(async move {
            let mut msgs = Vec::new();

            if let Some(system_prompt) = request.system_prompt {
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

            if request.prompt.rag_text().is_some() {
                let prompt: Vec<ChatCompletionRequestMessage> = request.prompt.try_into()?;
                msgs.extend(prompt);
            }

            let mut create_request_builder = CreateChatCompletionRequestArgs::default();
            if let Some(max_tokens) = request.max_tokens {
                create_request_builder.max_tokens(max_tokens as u32);
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
                self.client.chat().create(create_request).await?.into();

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
            async_openai::error::OpenAIError::JSONDeserialize(e, _) => e.into(),
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

                    return Ok(results.into_iter().map(Into::into).collect());
                }

                match other_content.len().cmp(&1) {
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
                            _ => unimplemented!("Unsupported content type"),
                        })
                        .collect::<Result<Vec<ChatCompletionRequestUserMessageContentPart>, _>>()?;
                        Ok(vec![
                            ChatCompletionRequestUserMessageArgs::default()
                                .content(content_array)
                                .build()
                                .unwrap() // Safety: All required fields are set
                                .into(),
                        ])
                    },
                    Ordering::Equal => {
                        let content = match &other_content[0] {
                            llm::completion::UserContent::Text(text) => {
                                ChatCompletionRequestUserMessageArgs::default()
                                    .content(text.text.as_str())
                                    .build()
                                    .unwrap() // Safety: All required fields are set
                                    .into()
                            },
                            llm::completion::UserContent::Image(image) => {
                                let content_part = vec![
                                    ChatCompletionRequestMessageContentPartImage::from(image)
                                        .into(),
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
                                    || (audio.media_type
                                        != Some(llm::completion::AudioMediaType::WAV)
                                        && audio.media_type
                                            != Some(llm::completion::AudioMediaType::MP3))
                                {
                                    return Err(CompletionError::Request("Only support wav and mp3 for now, and must be base64 encoded".into()));
                                }
                                let content_part = vec![
                                    ChatCompletionRequestMessageContentPartAudio::from(
                                        audio.clone(),
                                    )
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
                        };

                        Ok(vec![content])
                    },
                    Ordering::Less => Err(CompletionError::Request(
                        "User message must have at least one content".into(),
                    )),
                }
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
                    (Some(_), Some(tool_calls)) | (None, Some(tool_calls)) => {
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
                    _ => unreachable!(),
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
        Self {
            image_url: ImageUrl {
                url: image.data,
                detail: None,
            },
        }
    }
}

impl From<&llm::completion::Image>
    for async_openai::types::ChatCompletionRequestMessageContentPartImage
{
    fn from(image: &llm::completion::Image) -> Self {
        Self {
            image_url: ImageUrl {
                url: image.data.clone(),
                detail: None,
            },
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

impl From<async_openai::types::CreateChatCompletionResponse>
    for llm::CompletionResponse<async_openai::types::CreateChatCompletionResponse>
{
    fn from(response: async_openai::types::CreateChatCompletionResponse) -> Self {
        let choices = response
            .choices
            .iter()
            .flat_map(|choice| {
                let content = choice.message.content.to_owned();
                let tool_calls = choice.message.tool_calls.to_owned();

                // Check if tool_calls has actual calls (some providers like DeepInfra return empty array instead of None)
                let has_tool_calls = tool_calls.as_ref().is_some_and(|tc| !tc.is_empty());

                if !has_tool_calls {
                    // Return text content if no tool calls
                    if let Some(content) = content {
                        vec![llm::completion::AssistantContent::text(content)]
                    } else {
                        vec![]
                    }
                } else {
                    let tool_calls = tool_calls.expect("We just checked that it is not empty");
                    tool_calls
                        .iter()
                        .map(|tool_call| {
                            llm::completion::AssistantContent::tool_call(
                                tool_call.id.clone(),
                                tool_call.function.name.clone(),
                                serde_json::from_str(&tool_call.function.arguments)
                                    .expect("OpenAI return invalid json"),
                            )
                        })
                        .collect::<Vec<_>>()
                }
            })
            .collect::<Vec<_>>();

        Self {
            choice: choices,
            raw_response: response,
        }
    }
}
