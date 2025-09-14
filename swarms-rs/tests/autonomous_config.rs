// Autonomous Test Configuration System
// Handles whitelisting, thresholds, and custom patterns

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct SecurityConfig {
    pub whitelisted_files: HashSet<String>,
    pub whitelisted_patterns: HashSet<String>,
    pub security_thresholds: SecurityThresholds,
    pub custom_patterns: Vec<CustomPattern>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SecurityThresholds {
    pub max_unsafe_blocks: usize,
    pub max_raw_pointers: usize,
    pub max_high_risk_patterns: usize,
    pub max_medium_risk_patterns: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CustomPattern {
    pub name: String,
    pub pattern: String,
    pub risk_level: String,
    pub description: String,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        let mut whitelisted_files = HashSet::new();
        whitelisted_files.insert("test_security_safety.rs".to_string());
        whitelisted_files.insert("debug_autonomous_tests.sh".to_string());
        whitelisted_files.insert("validate_autonomous_tests.sh".to_string());
        
        let mut whitelisted_patterns = HashSet::new();
        whitelisted_patterns.insert("// Test pattern for malware detection".to_string());
        whitelisted_patterns.insert("// Security test - not actual threat".to_string());
        whitelisted_patterns.insert("fn test_malicious_code_detection".to_string());
        
        SecurityConfig {
            whitelisted_files,
            whitelisted_patterns,
            security_thresholds: SecurityThresholds {
                max_unsafe_blocks: 50,
                max_raw_pointers: 20,
                max_high_risk_patterns: 0,
                max_medium_risk_patterns: 5,
            },
            custom_patterns: vec![
                CustomPattern {
                    name: "SQL Injection Attempt".to_string(),
                    pattern: r#"(?i)(union\s+select|drop\s+table|delete\s+from.*where|insert\s+into.*values)"#.to_string(),
                    risk_level: "HIGH".to_string(),
                    description: "Potential SQL injection patterns".to_string(),
                },
                CustomPattern {
                    name: "Command Injection".to_string(),
                    pattern: r#"(?i)(system\(|exec\(|eval\(|`.*`|\$\(.*\))"#.to_string(),
                    risk_level: "HIGH".to_string(),
                    description: "Potential command injection patterns".to_string(),
                },
            ],
        }
    }
}

impl SecurityConfig {
    pub fn load_or_create() -> Self {
        let config_path = "autonomous_test_config.json";
        
        if std::path::Path::new(config_path).exists() {
            match std::fs::read_to_string(config_path) {
                Ok(content) => {
                    match serde_json::from_str(&content) {
                        Ok(config) => return config,
                        Err(e) => {
                            eprintln!("⚠️  Failed to parse config: {}. Using defaults.", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("⚠️  Failed to read config: {}. Using defaults.", e);
                }
            }
        }
        
        let default_config = SecurityConfig::default();
        default_config.save();
        default_config
    }
    
    pub fn save(&self) {
        let config_json = serde_json::to_string_pretty(self).unwrap();
        std::fs::write("autonomous_test_config.json", config_json).unwrap_or_else(|e| {
            eprintln!("⚠️  Failed to save config: {}", e);
        });
    }
    
    pub fn is_file_whitelisted(&self, file_path: &str) -> bool {
        self.whitelisted_files.iter().any(|pattern| file_path.contains(pattern))
    }
    
    pub fn is_pattern_whitelisted(&self, content: &str) -> bool {
        self.whitelisted_patterns.iter().any(|pattern| content.contains(pattern))
    }
}

// Coverage Configuration
#[derive(Serialize, Deserialize, Debug)]
pub struct CoverageConfig {
    pub minimum_coverage_percentage: f64,
    pub excluded_files: HashSet<String>,
    pub coverage_targets: HashMap<String, f64>,
}

impl Default for CoverageConfig {
    fn default() -> Self {
        let mut excluded_files = HashSet::new();
        excluded_files.insert("tests/".to_string());
        excluded_files.insert("benches/".to_string());
        excluded_files.insert("examples/".to_string());
        
        let mut coverage_targets = HashMap::new();
        coverage_targets.insert("src/lib.rs".to_string(), 80.0);
        coverage_targets.insert("src/agent/".to_string(), 70.0);
        coverage_targets.insert("src/structs/".to_string(), 75.0);
        
        CoverageConfig {
            minimum_coverage_percentage: 60.0,
            excluded_files,
            coverage_targets,
        }
    }
}

// Configuration Manager
pub struct ConfigManager {
    pub security: SecurityConfig,
    pub coverage: CoverageConfig,
}

impl ConfigManager {
    pub fn new() -> Self {
        ConfigManager {
            security: SecurityConfig::load_or_create(),
            coverage: CoverageConfig::default(),
        }
    }
    
    pub fn update_security_threshold(&mut self, threshold_type: &str, value: usize) {
        match threshold_type {
            "unsafe_blocks" => self.security.security_thresholds.max_unsafe_blocks = value,
            "raw_pointers" => self.security.security_thresholds.max_raw_pointers = value,
            "high_risk" => self.security.security_thresholds.max_high_risk_patterns = value,
            "medium_risk" => self.security.security_thresholds.max_medium_risk_patterns = value,
            _ => eprintln!("⚠️  Unknown threshold type: {}", threshold_type),
        }
        self.security.save();
    }
    
    pub fn add_whitelist_file(&mut self, file_pattern: String) {
        self.security.whitelisted_files.insert(file_pattern);
        self.security.save();
    }
    
    pub fn add_custom_security_pattern(&mut self, pattern: CustomPattern) {
        self.security.custom_patterns.push(pattern);
        self.security.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_loading() {
        let config = SecurityConfig::load_or_create();
        assert!(!config.whitelisted_files.is_empty());
        assert!(config.security_thresholds.max_unsafe_blocks > 0);
    }

    #[test]
    fn test_whitelist_functionality() {
        let config = SecurityConfig::default();
        assert!(config.is_file_whitelisted("test_security_safety.rs"));
        assert!(!config.is_file_whitelisted("some_random_file.rs"));
    }

    #[test]
    fn test_config_manager() {
        let mut manager = ConfigManager::new();
        manager.update_security_threshold("unsafe_blocks", 100);
        assert_eq!(manager.security.security_thresholds.max_unsafe_blocks, 100);
    }
}
