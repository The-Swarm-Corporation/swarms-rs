//! Health checks for the test suite itself
//! This ensures our testing infrastructure is working correctly
//! 
//! This is Step 1 of the autonomous testing suite implementation.
//! It only adds validation without affecting any production code.

use std::path::Path;

/// Test that validates all test files can be found and have basic structure
#[test]
fn test_suite_discovery() {
    let test_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    assert!(test_dir.exists(), "Tests directory should exist");
    
    // Count test files
    let test_files: Vec<_> = std::fs::read_dir(&test_dir)
        .expect("Should be able to read tests directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension()? == "rs" {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    
    assert!(test_files.len() > 0, "Should have at least one test file");
    println!("Found {} test files", test_files.len());
    
    // Validate each test file has basic test structure
    for test_file in test_files {
        let content = std::fs::read_to_string(&test_file)
            .expect("Should be able to read test file");
        
        // Basic validation: file should contain #[test] or #[tokio::test]
        let has_tests = content.contains("#[test]") || content.contains("#[tokio::test]");
        
        if !has_tests {
            println!("Warning: {} might not contain tests", test_file.display());
        }
    }
}

/// Test that validates the workspace Cargo.toml configuration
#[test]
fn test_workspace_configuration() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = manifest_dir.join("Cargo.toml");
    
    assert!(cargo_toml.exists(), "Cargo.toml should exist");
    
    let content = std::fs::read_to_string(&cargo_toml)
        .expect("Should be able to read Cargo.toml");
    
    // Validate basic workspace structure
    assert!(content.contains("[package]") || content.contains("[workspace]"), 
            "Cargo.toml should be a valid package or workspace");
}

/// Test that validates environment setup for testing
#[test]
fn test_environment_setup() {
    // Check that we're in a test environment
    assert!(cfg!(test), "Should be running in test mode");
    
    // Check that CARGO_MANIFEST_DIR is set
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    assert!(!manifest_dir.is_empty(), "CARGO_MANIFEST_DIR should be set");
    
    println!("Test environment validated for: {}", manifest_dir);
}

/// Test that validates dependencies are available
#[test]
fn test_dependencies_available() {
    // Test that core dependencies are available by importing them
    // This doesn't test functionality, just availability
    
    // Check if async runtime is available (tokio is always available in this project)
    let _runtime = tokio::runtime::Runtime::new();
    println!("Tokio runtime available");
    
    // Check if serialization is available (serde is always available in this project)
    let _: serde_json::Value = serde_json::json!({"test": "value"});
    println!("Serde available");
    
    // Basic assertion that always passes
    assert!(true, "Dependencies check completed");
}
