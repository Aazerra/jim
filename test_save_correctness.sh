#!/bin/bash
# Test that saved content matches viewport

set -e

echo "=== Save Correctness Test ==="
echo ""

# Create a test JSON file
cat > /tmp/test_save.json << 'EOF'
{
    "name": "test",
    "value": 123
}
EOF

echo "Original file:"
cat /tmp/test_save.json
echo ""

# Expected content after edit
cat > /tmp/expected.json << 'EOF'
{
    "name": "test",
    "value": 123,
    "new_field": "hello"
}
EOF

echo "Expected after edit:"
cat /tmp/expected.json
echo ""

# Instructions
echo "=== MANUAL TEST STEPS ==="
echo ""
echo "1. Run: ./target/release/jim /tmp/test_save.json"
echo ""
echo "2. Navigate to end of file (G)"
echo ""
echo "3. Go to insert mode before the closing brace:"
echo "   - Position cursor on }"
echo "   - Press 'i' to enter insert mode"
echo ""
echo "4. Add this text:"
echo '   ,'
echo '   "new_field": "hello"'
echo ""
echo "5. Press ESC to return to normal mode"
echo ""
echo "6. Save with :w"
echo ""
echo "7. Quit with :q"
echo ""
echo "8. Verify saved content:"
echo "   cat /tmp/test_save.json"
echo ""
echo "Expected result: File should contain the new_field you added"
echo "Bug symptom: File is corrupted or missing your edits"
echo ""
echo "=== AUTO-VERIFY (after manual test) ==="
echo ""
echo "Run this command to check if save worked:"
echo 'diff /tmp/test_save.json /tmp/expected.json && echo "✓ SAVE CORRECT!" || echo "✗ SAVE BUG - content mismatch"'
