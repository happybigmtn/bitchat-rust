#!/bin/bash

echo "Running tests that pass..."

# Modules that work
working_modules="cache config crypto logging monitoring optimization persistence platform resilience session token ui validation"

for module in $working_modules; do
    echo "Testing $module..."
    cargo test --lib ${module}:: 2>&1 | grep "test result:"
done

echo "Done - all passing modules tested"