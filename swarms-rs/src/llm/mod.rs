use futures::future::BoxFuture;
use request::{CompletionRequest, CompletionResponse};
use thiserror::Error;

pub mod completion;
pub mod provider;
pub mod request;

pub trait Model {
    type RawCompletionResponse;

    fn completion(
        &self,
        request: CompletionRequest,
    ) -> BoxFuture<'_, Result<CompletionResponse<Self::RawCompletionResponse>, CompletionError>>;
}

// Errors
#[derive(Debug, Error)]
pub enum CompletionError {
    /// Http error (e.g.: connection error, timeout, etc.)
    #[error("HttpError: {0}")]
    Http(#[from] reqwest::Error),

    /// Json error (e.g.: serialization, deserialization)
    #[error("JsonError: {0}")]
    Json(#[from] serde_json::Error),

    /// Error building the completion request
    #[error("RequestError: {0}")]
    Request(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),

    /// Error parsing the completion response
    #[error("ResponseError: {0}")]
    Response(String),

    /// Error returned by the completion model provider
    #[error("ProviderError: {0}")]
    Provider(String),

    /// Other error
    #[error("OtherError: {0}")]
    Other(String),
}
