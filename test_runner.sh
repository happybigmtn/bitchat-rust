#!/bin/bash

# Test runner to identify hanging tests

echo "Building test binary..."
cargo test --lib --no-run 2>/dev/null

# Find the test binary
TEST_BIN=$(find target/debug/deps -name "bitcraps-*" -type f -executable | grep -v "\.d$" | head -1)

if [ -z "$TEST_BIN" ]; then
    echo "Could not find test binary"
    exit 1
fi

echo "Test binary: $TEST_BIN"
echo "Listing tests..."

# Try to list tests with timeout
timeout 2 $TEST_BIN --list 2>&1

if [ $? -eq 124 ]; then
    echo "Test listing timed out - there's likely a static initialization issue"
    
    echo -e "\nChecking for potential issues..."
    
    # Check for lazy_static or once_cell initialization
    echo -e "\nLazy static usage:"
    grep -r "lazy_static!" src --include="*.rs" | head -5
    
    echo -e "\nOnce cell usage:"
    grep -r "Lazy::new\|OnceCell::new" src --include="*.rs" | head -5
    
    # Check for global state
    echo -e "\nStatic mutable usage:"
    grep -r "static mut" src --include="*.rs" | head -5
else
    echo "Tests listed successfully"
fi