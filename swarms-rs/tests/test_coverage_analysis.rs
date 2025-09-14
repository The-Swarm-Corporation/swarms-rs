//! Test Coverage Analysis and Reporting
//! 
//! This is Step 2 of the autonomous testing suite implementation.
//! It analyzes test coverage without affecting any production code.
//! 
//! Safety: This module only reads and analyzes code, making no changes.

use std::path::Path;
use std::collections::{HashMap, HashSet};

/// Analyzes test coverage by examining which modules have tests
#[test]
fn test_coverage_analysis() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let tests_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    
    // Find all Rust source files
    let src_files = find_rust_files(&src_dir);
    let test_files = find_rust_files(&tests_dir);
    
    println!("TEST COVERAGE ANALYSIS");
    println!("========================");
    println!("Source files found: {}", src_files.len());
    println!("Test files found: {}", test_files.len());
    
    // Analyze which modules are tested
    let mut coverage_report = HashMap::new();
    
    for src_file in &src_files {
        let module_name = extract_module_name(src_file, &src_dir);
        let has_tests = has_module_tests(&module_name, &test_files);
        coverage_report.insert(module_name.clone(), has_tests);
        
        if has_tests {
            println!(" {} - HAS TESTS", module_name);
        } else {
            println!(" {} - NEEDS TESTS", module_name);
        }
    }
    
    let tested_modules = coverage_report.values().filter(|&&has_tests| has_tests).count();
    let total_modules = coverage_report.len();
    let coverage_percentage = (tested_modules as f64 / total_modules as f64) * 100.0;
    
    println!("\n COVERAGE SUMMARY");
    println!("==================");
    println!("Modules with tests: {}/{}", tested_modules, total_modules);
    println!("Coverage percentage: {:.1}%", coverage_percentage);
    
    // This test always passes - it's just for analysis
    assert!(true, "Coverage analysis completed successfully");
}

/// Test that validates we can identify integration test gaps
#[test]
fn test_integration_coverage_analysis() {
    println!("\n INTEGRATION TEST ANALYSIS");
    println!("============================");
    
    // Check for integration between major components
    let integration_areas = vec![
        ("llm::provider", "agent"),
        ("agent", "structs::workflow"),
        ("structs::conversation", "persistence"),
        ("structs::tool", "agent"),
        ("llm", "structs::completion"),
    ];
    
    for (component_a, component_b) in integration_areas {
        let has_integration_test = check_integration_test_exists(component_a, component_b);
        if has_integration_test {
            println!("{}<->{} - Integration tests found", component_a, component_b);
        } else {
            println!(" {}<->{} - Integration tests recommended", component_a, component_b);
        }
    }
    
    assert!(true, "Integration test analysis completed");
}

/// Analyzes test quality and patterns
#[test]
fn test_quality_analysis() {
    println!("\n TEST QUALITY ANALYSIS");
    println!("========================");
    
    let tests_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let test_files = find_rust_files(&tests_dir);
    
    let mut total_tests = 0;
    let mut async_tests = 0;
    let mut has_setup_teardown = 0;
    let mut has_edge_cases = 0;
    
    for test_file in &test_files {
        if let Ok(content) = std::fs::read_to_string(test_file) {
            let file_tests = content.matches("#[test]").count() + content.matches("#[tokio::test]").count();
            total_tests += file_tests;
            
            if content.contains("#[tokio::test]") || content.contains("async fn") {
                async_tests += 1;
            }
            
            if content.contains("setup") || content.contains("cleanup") || content.contains("teardown") {
                has_setup_teardown += 1;
            }
            
            if content.contains("edge") || content.contains("boundary") || content.contains("error") {
                has_edge_cases += 1;
            }
        }
    }
    
    println!("Total test functions: {}", total_tests);
    println!("Files with async tests: {}", async_tests);
    println!("Files with setup/teardown: {}", has_setup_teardown);
    println!("Files testing edge cases: {}", has_edge_cases);
    
    // Quality metrics
    let async_coverage = (async_tests as f64 / test_files.len() as f64) * 100.0;
    let setup_coverage = (has_setup_teardown as f64 / test_files.len() as f64) * 100.0;
    let edge_case_coverage = (has_edge_cases as f64 / test_files.len() as f64) * 100.0;
    
    println!("\n QUALITY METRICS");
    println!("=================");
    println!("Async test coverage: {:.1}%", async_coverage);
    println!("Setup/teardown coverage: {:.1}%", setup_coverage);
    println!("Edge case coverage: {:.1}%", edge_case_coverage);
    
    assert!(true, "Test quality analysis completed");
}

/// Test that identifies potential test gaps in error handling
#[test]
fn test_error_handling_coverage() {
    println!("\n ERROR HANDLING COVERAGE");
    println!("==========================");
    
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let src_files = find_rust_files(&src_dir);
    
    let mut error_types_found = HashSet::new();
    let mut error_handling_patterns = HashMap::new();
    
    for src_file in &src_files {
        if let Ok(content) = std::fs::read_to_string(src_file) {
            // Look for error types
            if content.contains("Error") {
                let module_name = extract_module_name(src_file, &src_dir);
                error_types_found.insert(module_name.clone());
                
                // Count error handling patterns
                let result_count = content.matches("Result<").count();
                let option_count = content.matches("Option<").count();
                let unwrap_count = content.matches(".unwrap()").count();
                let expect_count = content.matches(".expect(").count();
                
                error_handling_patterns.insert(module_name, (result_count, option_count, unwrap_count, expect_count));
            }
        }
    }
    
    println!("Modules with error types: {}", error_types_found.len());
    
    for module in &error_types_found {
        if let Some((results, options, unwraps, expects)) = error_handling_patterns.get(module) {
            println!(" {} - Results: {}, Options: {}, Unwraps: {}, Expects: {}", 
                    module, results, options, unwraps, expects);
            
            if *unwraps > 5 {
                println!("     High unwrap usage - consider more error tests");
            }
        }
    }
    
    assert!(true, "Error handling analysis completed");
}

// Helper functions (safe utilities)

fn find_rust_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "rs") {
                files.push(path);
            } else if path.is_dir() && !path.file_name().unwrap().to_str().unwrap().starts_with('.') {
                files.extend(find_rust_files(&path));
            }
        }
    }
    files
}

fn extract_module_name(file_path: &Path, base_dir: &Path) -> String {
    file_path
        .strip_prefix(base_dir)
        .unwrap_or(file_path)
        .with_extension("")
        .to_string_lossy()
        .replace('/', "::")
        .replace('\\', "::")
}

fn has_module_tests(module_name: &str, test_files: &[std::path::PathBuf]) -> bool {
    for test_file in test_files {
        if let Ok(content) = std::fs::read_to_string(test_file) {
            // Check if test file mentions this module
            if content.contains(module_name) || 
               content.contains(&module_name.replace("::", "_")) ||
               test_file.file_name().unwrap().to_str().unwrap().contains(&module_name.replace("::", "_")) {
                return true;
            }
        }
    }
    false
}

fn check_integration_test_exists(component_a: &str, component_b: &str) -> bool {
    let tests_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let test_files = find_rust_files(&tests_dir);
    
    for test_file in &test_files {
        if let Ok(content) = std::fs::read_to_string(test_file) {
            if content.contains(component_a) && content.contains(component_b) {
                return true;
            }
        }
    }
    false
}
