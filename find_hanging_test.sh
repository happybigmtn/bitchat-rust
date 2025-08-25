#!/bin/bash

echo "Testing each module separately..."

modules="cache config crypto database discovery keystore logging mesh monitoring optimization persistence platform protocol resilience session token transport ui validation"

for module in $modules; do
    echo -n "Testing $module... "
    if timeout 5 cargo test --lib ${module}:: 2>&1 | grep -q "test result:"; then
        echo "OK"
    else
        echo "HANGING or ERROR"
    fi
done