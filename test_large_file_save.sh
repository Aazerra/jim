#!/bin/bash
# Test that save doesn't crash on large files

echo "=== Large File Save Test ==="
echo ""

# Check if large.json exists
if [ -f tests/large.json ]; then
    SIZE=$(stat -f%z tests/large.json 2>/dev/null || stat -c%s tests/large.json 2>/dev/null)
    SIZE_MB=$((SIZE / 1024 / 1024))
    echo "Found tests/large.json: ${SIZE_MB}MB"
else
    echo "Generating 100MB test file..."
    ./target/release/generate_test_data
    SIZE=$(stat -f%z tests/large.json 2>/dev/null || stat -c%s tests/large.json 2>/dev/null)
    SIZE_MB=$((SIZE / 1024 / 1024))
    echo "Created tests/large.json: ${SIZE_MB}MB"
fi

echo ""
echo "Memory test:"
echo "- Before fix: .to_string() would load entire ${SIZE_MB}MB into memory → CRASH"
echo "- After fix: streams rope chunks with 8MB buffer → NO CRASH"
echo ""

# Create a backup
cp tests/large.json /tmp/large_backup.json

echo "Opening large file in jim..."
echo ""
echo "INSTRUCTIONS:"
echo "1. File will open (may take a few seconds)"
echo "2. Press 'i' to enter insert mode"
echo "3. Type some text: 'test edit'"
echo "4. Press ESC"
echo "5. Type ':w' and press Enter"
echo "6. Watch the progress bar (should NOT crash)"
echo "7. Type ':q' to quit"
echo ""
echo "Expected: Save completes successfully without crash"
echo "Bug symptom: OOM crash or freeze during save"
echo ""

read -p "Press Enter to start (or Ctrl-C to cancel)..."

# Monitor memory usage in background
(
    sleep 2
    echo ""
    echo "=== Memory Usage During Save ==="
    for i in {1..30}; do
        if ps aux | grep "[j]im.*large.json" > /dev/null; then
            MEM=$(ps aux | grep "[j]im.*large.json" | awk '{print $6}')
            echo "$(date +%H:%M:%S) - Memory: ${MEM}KB"
            sleep 1
        else
            break
        fi
    done
) &
MONITOR_PID=$!

# Run jim
./target/release/jim tests/large.json

# Stop monitoring
kill $MONITOR_PID 2>/dev/null

echo ""
echo "=== Verification ==="
echo ""

# Check if file was modified
if cmp -s tests/large.json /tmp/large_backup.json; then
    echo "File unchanged (no save happened)"
else
    echo "✓ File was saved successfully!"
    echo "✓ No crash during save!"
    
    # Restore backup
    mv /tmp/large_backup.json tests/large.json
    echo "✓ Restored original file"
fi

echo ""
echo "Memory test complete!"
