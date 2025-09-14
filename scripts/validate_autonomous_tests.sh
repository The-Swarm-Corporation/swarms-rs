#!/bin/bash
# Autonomous Test Validation Script
# This ensures all test files are properly maintained

set -e

echo "Validating Autonomous Testing Suite..."

# Check if all test files exist
required_tests=(
    "test_suite_health.rs"
    "test_coverage_analysis.rs" 
    "test_integration_scaffolding.rs"
    "test_security_safety.rs"
)

test_dir="swarms-rs/tests"

for test_file in "${required_tests[@]}"; do
    if [[ -f "$test_dir/$test_file" ]]; then
        echo "$test_file exists"
        # Validate syntax
        if cargo check --test "${test_file%.*}" --quiet; then
            echo "$test_file compiles correctly"
        else
            echo "$test_file has compilation errors"
            exit 1
        fi
    else
        echo "Missing required test file: $test_file"
        exit 1
    fi
done

# Check CI configuration
if grep -q "test_suite_health\|test_coverage_analysis\|test_integration_scaffolding\|test_security_safety" .github/workflows/ci.yml; then
    echo " CI workflow properly configured"
else
    echo " CI workflow missing autonomous test configuration"
    exit 1
fi

echo " All autonomous tests validated successfully!"
