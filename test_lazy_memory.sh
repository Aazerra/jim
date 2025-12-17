#!/bin/bash
# Test lazy loading memory efficiency

echo "=== Lazy Loading Memory Test ==="
echo ""

# Check if large file exists
if [ ! -f tests/large.json ]; then
    echo "Generating 100MB test file..."
    ./target/release/generate_test_data
fi

SIZE=$(stat -f%z tests/large.json 2>/dev/null || stat -c%s tests/large.json 2>/dev/null)
SIZE_MB=$((SIZE / 1024 / 1024))

echo "Test file: ${SIZE_MB}MB"
echo ""
echo "Expected behavior:"
echo "- File < 10MB: Loads into Rope (full memory)"
echo "- File ≥ 10MB: Lazy loading (8MB cache only)"
echo ""
echo "For ${SIZE_MB}MB file:"
if [ $SIZE_MB -lt 10 ]; then
    echo "→ Will use Rope mode (loads full file)"
else
    echo "→ Will use Lazy mode (8MB cache)"
fi
echo ""

echo "Opening file and monitoring memory..."
echo "(jim will open in background, scroll through file)"
echo ""

# Start jim in background
./target/release/jim tests/large.json &
JIM_PID=$!

sleep 2

# Monitor memory for 15 seconds
echo "Memory usage over time:"
echo "Time | RSS Memory | Expected"
echo "-----|------------|----------"

for i in {1..15}; do
    if ps -p $JIM_PID > /dev/null 2>&1; then
        MEM_KB=$(ps -o rss= -p $JIM_PID 2>/dev/null)
        MEM_MB=$((MEM_KB / 1024))
        
        if [ $SIZE_MB -ge 10 ]; then
            EXPECTED="<100MB (lazy)"
        else
            EXPECTED="~${SIZE_MB}MB (rope)"
        fi
        
        printf "%2ds  | %6d MB | %s\n" $i $MEM_MB "$EXPECTED"
        sleep 1
    else
        echo "jim exited"
        break
    fi
done

# Cleanup
kill $JIM_PID 2>/dev/null
wait $JIM_PID 2>/dev/null

echo ""
echo "=== Results ==="
echo ""
if [ $SIZE_MB -ge 10 ]; then
    echo "✅ Large file mode:"
    echo "   - Line index built: ~${SIZE_MB}MB * 0.01 = ~$((SIZE_MB / 100))MB"
    echo "   - LRU cache: 8MB"
    echo "   - Total expected: <100MB"
    echo ""
    echo "If memory > 100MB: Bug! Full rope was loaded"
else
    echo "✅ Small file mode:"
    echo "   - Full rope loaded: ~${SIZE_MB}MB"
    echo "   - Expected memory: ${SIZE_MB}-$((SIZE_MB * 2))MB"
fi

echo ""
echo "To manually test:"
echo "./target/release/jim tests/large.json"
echo "Then in another terminal: ps aux | grep jim"
