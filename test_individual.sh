#!/bin/bash

echo "Finding hanging test..."

# Get list of tests
TESTS=$(target/debug/deps/bitcraps-8e0ce6c4f6c7539d --list 2>&1 | grep ": test$" | cut -d: -f1)

for test in $TESTS; do
    echo -n "Testing: $test ... "
    if timeout 2 target/debug/deps/bitcraps-8e0ce6c4f6c7539d "$test" --nocapture 2>&1 | grep -q "test result: ok"; then
        echo "OK"
    else
        echo "FAILED or TIMEOUT"
        echo "  Found problematic test: $test"
        break
    fi
done