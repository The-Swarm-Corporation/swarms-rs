use swarms_rs::structs::persistence;
use tempfile::tempdir;

#[tokio::test]
async fn test_save_and_load_file() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let file_path = dir.path().join("test.txt");

    let test_data = b"Hello, World!";
    persistence::save_to_file(test_data, &file_path).await?;

    let loaded_data = persistence::load_from_file(&file_path).await?;
    assert_eq!(loaded_data, test_data);

    Ok(())
}

#[tokio::test]
async fn test_append_to_file() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let file_path = dir.path().join("append_test.txt");

    persistence::append_to_file(b"First line\n", &file_path).await?;
    persistence::append_to_file(b"Second line\n", &file_path).await?;

    let content = persistence::load_from_file(&file_path).await?;
    let content_str = String::from_utf8(content)?;

    assert!(content_str.contains("First line"));
    assert!(content_str.contains("Second line"));

    Ok(())
}

#[tokio::test]
async fn test_compression() -> Result<(), Box<dyn std::error::Error>> {
    // Create a larger, repetitive string that will compress well
    let test_data = b"This is a test string that will be repeated multiple times to ensure compression works. \
                      This is a test string that will be repeated multiple times to ensure compression works. \
                      This is a test string that will be repeated multiple times to ensure compression works.";

    let compressed = persistence::compress(test_data)?;
    let decompressed = persistence::decompress(&compressed)?;

    assert_eq!(&decompressed, test_data);
    assert!(
        compressed.len() < test_data.len(),
        "Compressed size ({}) should be less than original size ({})",
        compressed.len(),
        test_data.len()
    );

    Ok(())
}

#[tokio::test]
async fn test_log_to_file() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let log_path = dir.path().join("test.log");

    persistence::log_to_file("Test log message", &log_path).await?;

    let content = persistence::load_from_file(&log_path).await?;
    let content_str = String::from_utf8(content)?;

    assert!(content_str.contains("Test log message"));
    assert!(content_str.contains("-")); // Check for timestamp separator

    Ok(())
}

#[tokio::test]
async fn test_log_to_file_creates_parent_dir_and_separates_entries()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let log_path = dir.path().join("logs").join("nested").join("agent.log");

    persistence::log_to_file("first", &log_path).await?;
    persistence::log_to_file("second", &log_path).await?;

    let content = String::from_utf8(persistence::load_from_file(&log_path).await?)?;
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2, "{content:?}");
    assert!(lines[0].ends_with("first"));
    assert!(lines[1].ends_with("second"));

    Ok(())
}
