#!/usr/bin/env bash

# 1. Pre-build the compiler to eliminate Cargo overhead in the loop
echo "Building compiler..."
cargo build -q || { echo -e "\n\e[31mBuild failed!\e[0m"; exit 1; }

passed=0
failed=0
unexpected_passed=0
expected_failed=0

fail_tests=()

echo "Running tests..."
echo "----------------"

for f in tests/*.chs; do
    # Guard against empty directories expanding to literal '*.chs'
    [ -e "$f" ] || { echo "No test files found."; break; }

    if target/debug/chs run "$f"; then
        echo -e "\e[32m[ PASS ]\e[0m $f"
        ((passed++))
    else
        echo -e "\e[31m[ FAIL ]\e[0m $f"
        fail_tests+=("Fail: $f")
        ((failed++))

        # 3. Properly pause on failure, allowing you to read the error
        # read -p "Press [Enter] to continue to the next test, or Ctrl+C to abort..."
        # exit 1
    fi
done

for f in tests/*.fail; do
    # Guard against empty directories expanding to literal '*.chs'
    [ -e "$f" ] || { echo "No test files found."; break; }

    if target/debug/chs run "$f"; then
        echo -e "\e[31m[ UNEXPECTED PASS ]\e[0m $f"
        ((unexpected_passed++))
        fail_tests+=("Unexpected pass: $f")
    else
        echo -e "\e[32m[ EXPECTED FAIL ]\e[0m $f"
        ((expected_failed++))

        # 3. Properly pause on failure, allowing you to read the error
        # read -p "Press [Enter] to continue to the next test, or Ctrl+C to abort..."
        # exit 1
    fi
done

echo "----------------"
echo "Results: $passed passed, $failed failed, $expected_failed expected to fail, $unexpected_passed unexpected to pass."

for item in "${fail_tests[@]}"; do
    echo "$item"
done
