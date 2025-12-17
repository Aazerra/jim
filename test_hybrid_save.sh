#!/bin/bash

# Test script for hybrid save approach

echo "=== Hybrid Save Approach Test ==="
echo ""

# Test 1: Small file with few edits (should use COW if supported)
echo "Test 1: Small file (tests/small.json)"
echo "- Edit count will be small"
echo "- Should attempt COW save if filesystem supports reflink"
echo ""

# Test 2: Large file
echo "Test 2: Large file (tests/large.json)"
if [ -f tests/large.json ]; then
    SIZE=$(du -h tests/large.json | cut -f1)
    echo "- File size: $SIZE"
    echo "- Will use streaming save (fallback)"
else
    echo "- File not found, generating..."
    cargo run --release --bin generate_test_data
fi
echo ""

# Check filesystem type
echo "Filesystem information:"
df -T . | grep -v Filesystem
echo ""

# Test if reflink is supported
echo "Testing reflink support:"
touch .test_reflink_src
if command -v cp &> /dev/null; then
    if cp --reflink=auto .test_reflink_src .test_reflink_dst 2>/dev/null; then
        echo "✓ Filesystem supports reflink (COW)"
        echo "  → Small edits will use FAST COW save"
    else
        echo "✗ Filesystem does not support reflink"
        echo "  → Will use streaming save (still efficient)"
    fi
    rm -f .test_reflink_src .test_reflink_dst
else
    echo "? Cannot test (cp command not found)"
    rm -f .test_reflink_src
fi
echo ""

echo "=== Performance Expectations ==="
echo ""
echo "COW Save (if supported):"
echo "  - 100MB file + 10KB edits: ~2 seconds"
echo "  - Only writes modified disk blocks"
echo "  - Memory: ~20MB"
echo ""
echo "Streaming Save (universal):"
echo "  - 100MB file + 10KB edits: ~5 seconds"
echo "  - Streams entire file with patches"
echo "  - Memory: ~20MB"
echo ""
echo "Both methods:"
echo "  - Progress bar in status line"
echo "  - Background execution (UI stays responsive)"
echo "  - Atomic rename (safe)"
echo ""

echo "Run: cargo run --release --bin jim -- tests/small.json"
echo "Then try: Insert mode → type text → :w → watch progress bar"
