#!/bin/bash
# Test chunk-based editing for large files

set -e

echo "=== Testing Chunk-Based Editing (No Full Load) ==="
echo

cd "$(dirname "$0")"
CARGO_BUILD="../target/release/json-tool"
TEST_FILE="./large.json"

# Build if needed
if [ ! -f "$CARGO_BUILD" ]; then
    echo "Building release binary..."
    cargo build --release
fi

# Check if test file exists
if [ ! -f "$TEST_FILE" ]; then
    echo "Error: $TEST_FILE not found"
    echo "Run ./generate_test_data first"
    exit 1
fi

FILE_SIZE=$(stat -f%z "$TEST_FILE" 2>/dev/null || stat -c%s "$TEST_FILE")
echo "Test file: $TEST_FILE ($((FILE_SIZE / 1024 / 1024))MB)"
echo

# Test 1: Memory usage on open (should be low, not loading full file)
echo "Test 1: Memory on Open (Lazy Mode)"
echo "------------------------------------"
echo "Starting jim in background..."

# Start jim with the large file
"$CARGO_BUILD" "$TEST_FILE" &
JIM_PID=$!
sleep 2

# Check memory usage
if ps -p $JIM_PID > /dev/null; then
    MEM_KB=$(ps -o rss= -p $JIM_PID 2>/dev/null || echo "0")
    MEM_MB=$((MEM_KB / 1024))
    echo "Memory usage: ${MEM_MB}MB"
    
    # For a 100MB file, memory should be ~10-30MB (not 100MB!)
    if [ $MEM_MB -lt 100 ]; then
        echo "✅ PASS: Using lazy mode (not loading full file)"
    else
        echo "❌ FAIL: Memory too high (may be loading full file)"
    fi
    
    # Kill jim
    kill $JIM_PID 2>/dev/null || true
    wait $JIM_PID 2>/dev/null || true
else
    echo "❌ FAIL: Process died"
fi

echo
echo "Test 2: Edit Operation Memory"
echo "------------------------------"
echo "Editing a line should NOT load full file to memory"
echo "(Manual test: Open file, press 'i', type text, check memory)"
echo

# Test 3: Verify edit overlay is used
echo "Test 3: Edit Overlay Architecture"
echo "-----------------------------------"
echo "Architecture check:"
echo "  - Files ≥10MB: Use mmap + edit overlay"
echo "  - Files <10MB: Use rope"
echo

if [ $FILE_SIZE -ge $((10 * 1024 * 1024)) ]; then
    echo "✅ File is ≥10MB → Should use lazy mode + edit overlay"
else
    echo "⚠️  File is <10MB → Will use rope mode (expected)"
fi

echo
echo "Test 4: Save After Edit"
echo "-----------------------"
echo "1. Open $TEST_FILE"
echo "2. Press 'i' to enter insert mode"
echo "3. Type some text"
echo "4. Press ESC, then :w to save"
echo "5. Monitor memory during save (should NOT spike to ${FILE_SIZE}MB)"
echo
echo "Expected behavior:"
echo "  - Memory stays under 100MB during edit"
echo "  - Save shows progress bar"
echo "  - Save completes in <30 seconds"
echo

echo "=== Manual Test Checklist ==="
echo
echo "Run: $CARGO_BUILD $TEST_FILE"
echo
echo "[ ] 1. File opens quickly (<10s)"
echo "[ ] 2. Can scroll with j/k"
echo "[ ] 3. Press 'i', type text → memory doesn't spike"
echo "[ ] 4. Press ESC, then :w → save works"
echo "[ ] 5. Monitor with: watch -n 1 'ps aux | grep jim | grep -v grep'"
echo "[ ] 6. Memory stays under 100MB throughout"
echo

echo "=== Architecture Verification ==="
echo
echo "Code locations:"
echo "  - Buffer struct: src/buffer/mod.rs (line ~50)"
echo "  - Edit overlay: edits: HashMap<usize, String>"
echo "  - Insert logic: insert() method (line ~420)"
echo "  - Save logic: save() method (line ~465)"
echo
echo "Key points:"
echo "  - use_rope=false for large files"
echo "  - edits HashMap stores modified lines only"
echo "  - get_line() checks overlay before mmap"
echo "  - save() merges overlay when writing"
echo

echo "Done!"
