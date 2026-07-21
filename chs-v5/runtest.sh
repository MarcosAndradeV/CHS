#!/usr/bin/env bash

# 1. Pre-build the compiler to eliminate Cargo overhead in the loop
echo "Building compiler..."
cargo build -q || { echo -e "\n\e[31mBuild failed!\e[0m"; exit 1; }

passed=0
failed=0

echo "Running tests..."
echo "----------------"

for f in tests/*.chs; do
    # Guard against empty directories expanding to literal '*.chs'
    [ -e "$f" ] || { echo "No test files found."; break; }

    # 2. Run the test (we use 'cargo run' here, but executing the binary directly
    # from target/debug/ is even faster if you know its exact name)
    if cargo run -q -- "$f"; then
        echo -e "\e[32m[ PASS ]\e[0m $f"
        ((passed++))
    else
        echo -e "\e[31m[ FAIL ]\e[0m $f"
        ((failed++))

        # 3. Properly pause on failure, allowing you to read the error
        # read -p "Press [Enter] to continue to the next test, or Ctrl+C to abort..."
        exit 1
    fi
done

echo "----------------"
echo "Results: $passed passed, $failed failed."
