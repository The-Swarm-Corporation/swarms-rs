use std::path::Path;

use chrono::Local;
use thiserror::Error;
use tokio::{fs, io::AsyncWriteExt};

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Missing directory: {0}")]
    MissingParent(String),
}

/// Save the data to a file, if the file exists, it will be overwritten
pub async fn save_to_file(
    data: impl AsRef<[u8]>,
    path: impl AsRef<Path>,
) -> Result<(), PersistenceError> {
    match path.as_ref().parent() {
        Some(parent) => fs::create_dir_all(parent).await?,
        None => {
            return Err(PersistenceError::MissingParent(
                path.as_ref().to_string_lossy().to_string(),
            ));
        },
    };
    fs::write(path, data).await.map_err(|e| e.into())
}

/// Append the data to a file, if the file doesn't exist, it will be created
pub async fn append_to_file(
    data: impl AsRef<[u8]>,
    path: impl AsRef<Path>,
) -> Result<(), PersistenceError> {
    // create the parent directory if it doesn't exist
    if let Some(parent) = path.as_ref().parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).await?;
    }

    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .await?;
    file.write_all(data.as_ref()).await?;
    // tokio finishes file writes in the background; without flushing, a read (or the next
    // append) right after this call can miss the data.
    file.flush().await?;
    Ok(())
}

/// Load the data from a file
pub async fn load_from_file(path: impl AsRef<Path>) -> Result<Vec<u8>, PersistenceError> {
    fs::read(path).await.map_err(|e| e.into())
}

/// Compress data, defaults to zstd
pub fn compress(data: impl AsRef<[u8]>) -> Result<Vec<u8>, PersistenceError> {
    use zstd::stream::encode_all;
    // 0 is the default compression level
    encode_all(data.as_ref(), 0).map_err(|e| e.into())
}

/// Decompress data, defaults to zstd
pub fn decompress(data: impl AsRef<[u8]>) -> Result<Vec<u8>, PersistenceError> {
    use zstd::stream::decode_all;
    decode_all(data.as_ref()).map_err(|e| e.into())
}

pub async fn log_to_file(
    message: impl AsRef<str>,
    path: impl AsRef<Path>,
) -> Result<(), PersistenceError> {
    let message = message.as_ref();
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let log_message = format!("{timestamp} - {message}\n");
    append_to_file(log_message.as_bytes(), path).await
}
