#!/bin/bash
# Quick test to identify where crash happens

echo "=== Crash Location Test ==="
echo ""

# Generate a moderately large file if needed
if [ ! -f tests/medium.json ]; then
    echo "Generating 50MB test file..."
    python3 << 'PYTHON'
import json
data = [{"id": i, "data": "x" * 100, "nested": {"value": i * 2}} for i in range(250000)]
with open("tests/medium.json", "w") as f:
    json.dump(data, f)
PYTHON
    echo "Created tests/medium.json"
fi

SIZE=$(stat -f%z tests/medium.json 2>/dev/null || stat -c%s tests/medium.json 2>/dev/null)
SIZE_MB=$((SIZE / 1024 / 1024))
echo "Test file: ${SIZE_MB}MB"
echo ""

echo "Testing crash location..."
echo ""

# Test 1: Can we even open the file?
echo "Test 1: Opening file..."
timeout 10 ./target/release/jim tests/medium.json << 'EOF' 2>&1 | head -5
:q
EOF

if [ $? -eq 124 ]; then
    echo "❌ CRASH during file open/load"
    echo ""
    echo "Problem: Rope::from_reader() loads entire file into memory"
    echo "Solution: Need to implement lazy rope loading"
    exit 1
elif [ $? -eq 0 ]; then
    echo "✓ File opens successfully"
else
    echo "⚠ Unknown error during open"
fi

echo ""

# Test 2: Can we save after a small edit?
echo "Test 2: Making edit and saving..."
timeout 30 ./target/release/jim tests/medium.json << 'EOF' 2>&1 | tail -10
i
test
:w
:q
EOF

if [ $? -eq 124 ]; then
    echo "❌ CRASH during save"
    echo ""
    echo "Problem: Save operation running out of memory"
    echo "Solution: Already fixed - streaming rope chunks"
    exit 1
elif [ $? -eq 0 ]; then
    echo "✓ Save completes successfully"
else
    echo "⚠ Unknown error during save"
fi

echo ""
echo "=== Diagnosis ==="
echo ""
echo "If Test 1 fails: Crash is during file LOAD (Rope::from_reader)"
echo "If Test 2 fails: Crash is during file SAVE (our recent fix)"
echo ""
echo "Current status: Testing..."
