use std::fs;
use std::path::{Path, PathBuf};
use tokio::fs as tokio_fs;
use tokio::io::AsyncWriteExt;
use tokio::task;
use std::sync::Arc;
use log::{info, error};

/// Creates a folder if it doesn't already exist.
///
/// # Arguments
///
/// * `folder_name` - The name of the folder to create.
///
/// # Example
///
/// ```rust
/// create_folder("my_folder").await;
/// ```
pub async fn create_folder(folder_name: &str) -> Result<(), std::io::Error> {
    let path = Path::new(folder_name);
    if !path.exists() {
        tokio_fs::create_dir_all(path).await?;
        info!("Folder '{}' created.", folder_name);
    } else {
        info!("Folder '{}' already exists.", folder_name);
    }
    Ok(())
}

/// Creates a file with the specified name, extension, and content within a folder.
///
/// # Arguments
///
/// * `folder_name` - The name of the folder where the file will be created.
/// * `file_name` - The name of the file, including the extension.
/// * `content` - The content to write to the file.
///
/// # Example
///
/// ```rust
/// create_file("my_folder", "my_file.txt", "Hello, world!").await;
/// ```
pub async fn create_file(
    folder_name: &str,
    file_name: &str,
    content: &str,
) -> Result<(), std::io::Error> {
    let folder_path = Path::new(folder_name);
    let file_path = folder_path.join(file_name);

    // Create the folder if it doesn't exist
    create_folder(folder_name).await?;

    // Write the content to the file
    let mut file = tokio_fs::File::create(file_path).await?;
    file.write_all(content.as_bytes()).await?;
    info!("File '{}/{}' created with content.", folder_name, file_name);

    Ok(())
}

/// Creates multiple files concurrently, each with its own content.
///
/// # Arguments
///
/// * `folder_name` - The name of the folder where the files will be created.
/// * `files` - A vector of tuples, each containing a file name and its content.
///
/// # Example
///
/// ```rust
/// create_multiple_files("my_folder", vec![("file1.txt", "Content 1"), ("file2.txt", "Content 2")]).await;
/// ```
pub async fn create_multiple_files(
    folder_name: &str,
    files: Vec<(&str, &str)>,
) -> Result<(), std::io::Error> {
    let folder_name = Arc::new(folder_name.to_string());

    let handles = files.into_iter().map(|(file_name, content)| {
        let folder_name = Arc::clone(&folder_name);
        task::spawn(async move {
            if let Err(e) = create_file(&folder_name, file_name, content).await {
                error!("Failed to create file '{}/{}': {}", folder_name, file_name, e);
            }
        })
    });

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    Ok(())
}

// Examples
// mod file_utils;

// use file_utils::{create_file, create_multiple_files};
// use log::info;
// use tokio::main;

// #[main]
// async fn main() {
//     env_logger::init();

//     // Example 1: Create a single file
//     if let Err(e) = create_file("my_folder", "my_file.txt", "Hello, world!").await {
//         eprintln!("Error creating file: {}", e);
//     }

//     // Example 2: Create multiple files concurrently
//     let files = vec![
//         ("file1.txt", "Content 1"),
//         ("file2.txt", "Content 2"),
//         ("file3.log", "Log content"),
//     ];

//     if let Err(e) = create_multiple_files("my_folder", files).await {
//         eprintln!("Error creating multiple files: {}", e);
//     }

//     info!("All files have been created.");
// }
