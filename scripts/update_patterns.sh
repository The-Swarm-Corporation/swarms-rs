#!/bin/bash
# Autonomous Pattern Updater
# Keeps security patterns and configurations up to date

set -e

echo "Autonomous Pattern Updater"
echo "============================="

PATTERNS_URL="https://raw.githubusercontent.com/security-patterns/rust-patterns/main/patterns.json"
CONFIG_FILE="autonomous_test_config.json"
BACKUP_DIR="backups"

# Create backup directory
mkdir -p "$BACKUP_DIR"

# Function to backup current configuration
backup_config() {
    if [[ -f "$CONFIG_FILE" ]]; then
        local timestamp=$(date +"%Y%m%d_%H%M%S")
        cp "$CONFIG_FILE" "$BACKUP_DIR/config_backup_$timestamp.json"
        echo "Configuration backed up to $BACKUP_DIR/config_backup_$timestamp.json"
    fi
}

# Function to validate new patterns
validate_patterns() {
    local patterns_file=$1
    
    if ! jq empty "$patterns_file" 2>/dev/null; then
        echo " Invalid JSON format in patterns file"
        return 1
    fi
    
    # Check required fields
    if ! jq '.security_patterns | length' "$patterns_file" >/dev/null 2>&1; then
        echo " Missing security_patterns field"
        return 1
    fi
    
    echo "Pattern file validation passed"
    return 0
}

# Function to merge patterns safely
merge_patterns() {
    local new_patterns=$1
    local current_config=$2
    
    if [[ -f "$current_config" ]]; then
        # Merge new patterns with existing config
        jq -s '.[0] * .[1]' "$current_config" "$new_patterns" > "${current_config}.tmp"
        mv "${current_config}.tmp" "$current_config"
        echo " Patterns merged successfully"
    else
        # Use new patterns as base config
        cp "$new_patterns" "$current_config"
        echo "New configuration created"
    fi
}

# Function to update security patterns
update_security_patterns() {
    echo "🔍 Checking for security pattern updates..."
    
    # Download latest patterns
    if curl -s -f "$PATTERNS_URL" -o "latest_patterns.json"; then
        echo " Downloaded latest security patterns"
        
        if validate_patterns "latest_patterns.json"; then
            backup_config
            merge_patterns "latest_patterns.json" "$CONFIG_FILE"
            rm "latest_patterns.json"
            echo "Security patterns updated successfully"
        else
            echo " Pattern validation failed, keeping current patterns"
            rm "latest_patterns.json"
            return 1
        fi
    else
        echo "Could not download latest patterns (offline?), using current patterns"
    fi
}

# Function to update thresholds based on project size
auto_adjust_thresholds() {
    echo "🔧 Auto-adjusting thresholds based on project size..."
    
    # Count source files
    local src_files=$(find . -name "*.rs" -path "*/src/*" | wc -l)
    local test_files=$(find . -name "*.rs" -path "*/tests/*" | wc -l)
    
    echo "Project metrics:"
    echo "   - Source files: $src_files"
    echo "   - Test files: $test_files"
    
    # Adjust thresholds based on project size
    if [[ $src_files -gt 100 ]]; then
        # Large project - more lenient thresholds
        jq '.security_thresholds.max_unsafe_blocks = 100' "$CONFIG_FILE" > "${CONFIG_FILE}.tmp"
        mv "${CONFIG_FILE}.tmp" "$CONFIG_FILE"
        echo "Adjusted thresholds for large project"
    elif [[ $src_files -gt 50 ]]; then
        # Medium project
        jq '.security_thresholds.max_unsafe_blocks = 75' "$CONFIG_FILE" > "${CONFIG_FILE}.tmp"
        mv "${CONFIG_FILE}.tmp" "$CONFIG_FILE"
        echo "Adjusted thresholds for medium project"
    else
        # Small project - strict thresholds
        jq '.security_thresholds.max_unsafe_blocks = 25' "$CONFIG_FILE" > "${CONFIG_FILE}.tmp"
        mv "${CONFIG_FILE}.tmp" "$CONFIG_FILE"
        echo "Adjusted thresholds for small project"
    fi
}

# Function to clean old backups
cleanup_old_backups() {
    echo "🧹 Cleaning old backups..."
    
    # Keep only last 10 backups
    ls -t "$BACKUP_DIR"/config_backup_*.json 2>/dev/null | tail -n +11 | xargs rm -f
    
    local backup_count=$(ls "$BACKUP_DIR"/config_backup_*.json 2>/dev/null | wc -l)
    echo "Kept $backup_count recent backups"
}

# Function to validate current configuration
validate_current_config() {
    echo "🔍 Validating current configuration..."
    
    if [[ -f "$CONFIG_FILE" ]]; then
        if jq empty "$CONFIG_FILE" 2>/dev/null; then
            echo " Current configuration is valid"
        else
            echo " Current configuration has JSON errors"
            return 1
        fi
    else
        echo "No configuration file found, will create default"
    fi
}

# Main update process
main() {
    # Check dependencies
    if ! command -v jq &> /dev/null; then
        echo "jq is required but not installed"
        echo "Install with: brew install jq (macOS) or apt-get install jq (Ubuntu)"
        exit 1
    fi
    
    if ! command -v curl &> /dev/null; then
        echo "curl is required but not installed"
        exit 1
    fi
    
    validate_current_config
    update_security_patterns
    auto_adjust_thresholds
    cleanup_old_backups
    
    echo ""
    echo "Pattern update completed successfully!"
    echo "Next steps:"
    echo "   1. Run tests to verify new patterns: cargo test"
    echo "   2. Review configuration: cat $CONFIG_FILE"
    echo "   3. Commit updated configuration to git"
}

# Handle command line arguments
case "${1:-}" in
    "validate")
        validate_current_config
        ;;
    "backup")
        backup_config
        ;;
    "patterns")
        update_security_patterns
        ;;
    "thresholds")
        auto_adjust_thresholds
        ;;
    *)
        main
        ;;
esac
