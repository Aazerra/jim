#!/bin/bash
# Quick test of save bug fix

echo "Creating test file..."
cat > /tmp/quick_test.json << 'EOF'
{
    "test": "original"
}
EOF

echo ""
echo "Original content:"
cat /tmp/quick_test.json
echo ""

echo "Now testing with jim..."
echo ""
echo "STEPS:"
echo "1. Opening /tmp/quick_test.json"
echo "2. You will see:"
echo '   {'
echo '       "test": "original"'
echo '   }'
echo ""
echo "3. Press 'G' to go to end"
echo "4. Press 'k' to go up one line"  
echo "5. Press 'A' to append at end of line"
echo "6. Type: ,"
echo '7. Press Enter and type:     "added": "field"'
echo "8. Press ESC"
echo "9. Type :w and press Enter (watch progress bar)"
echo "10. Type :q and press Enter"
echo ""
echo "Expected result in file:"
cat > /tmp/expected_quick.json << 'EOF'
{
    "test": "original",
    "added": "field"
}
EOF
cat /tmp/expected_quick.json
echo ""

read -p "Press Enter to start jim..."

./target/release/jim /tmp/quick_test.json

echo ""
echo "=== VERIFICATION ==="
echo ""
echo "Saved content:"
cat /tmp/quick_test.json
echo ""

if diff -q /tmp/quick_test.json /tmp/expected_quick.json >/dev/null 2>&1; then
    echo "✓✓✓ SUCCESS! Saved content matches what you typed! ✓✓✓"
    exit 0
else
    echo "Actual vs Expected:"
    diff /tmp/quick_test.json /tmp/expected_quick.json || true
    echo ""
    echo "❌ BUG: Saved content doesn't match!"
    exit 1
fi
