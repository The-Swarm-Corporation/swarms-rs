#!/bin/bash
# Autonomous Test Debugger
# Helps diagnose test failures quickly

set -e

echo "Autonomous Test Debugger"
echo "=========================="

# Function to test individual components
debug_test() {
    local test_name=$1
    echo "🔍 Debugging: $test_name"
    
    if cargo test --test "$test_name" --verbose 2>&1 | tee "/tmp/${test_name}_debug.log"; then
        echo " $test_name passed"
    else
        echo " $test_name failed"
        echo "Debug log saved to: /tmp/${test_name}_debug.log"
        echo "Last 10 lines of error:"
        tail -10 "/tmp/${test_name}_debug.log"
    fi
    echo ""
}

# Function to check dependencies
check_dependencies() {
    echo "🔍 Checking Dependencies..."
    
    if cargo check --tests --quiet; then
        echo "All dependencies available"
    else
        echo " Dependency issues found"
        echo " Try: cargo update"
    fi
    echo ""
}

# Function to check environment
check_environment() {
    echo "Checking Environment..."
    
    echo "Rust version: $(rustc --version)"
    echo "Cargo version: $(cargo --version)"
    echo "Workspace: $(pwd)"
    
    if [[ -f "Cargo.toml" ]]; then
        echo " Cargo.toml found"
    else
        echo " Not in a Rust workspace"
    fi
    echo ""
}

# Main debugging flow
main() {
    check_environment
    check_dependencies
    
    echo "Testing individual components..."
    debug_test "test_suite_health"
    debug_test "test_coverage_analysis"
    debug_test "test_integration_scaffolding"
    debug_test "test_security_safety"
    
    echo " Quick fixes for common issues:"
    echo "1. Dependency issues: cargo update"
    echo "2. Permission issues: chmod +x scripts/*.sh"
    echo "3. Path issues: ensure you're in project root"
    echo "4. CI issues: check .github/workflows/ci.yml syntax"
}

# Run with specific test if provided
if [[ $# -eq 1 ]]; then
    debug_test "$1"
else
    main
fi
