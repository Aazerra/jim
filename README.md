# JSON Tool

**A streaming, structural, Vim-like JSON editor for gigabyte-scale files**

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

---

## Features

### Current (Phase 1 COMPLETE ✅)

✅ **Vim Operators & Motions (NEW - Week 9)**
- `d{motion}` - Delete: `dd`, `dw`, `diw`, `di"`
- `c{motion}` - Change: `cc`, `cw`, `ciw`, `ci"`
- `y{motion}` - Yank: `yy`, `yw`, `yiw`, `yi"`
- `p` / `P` - Paste after/before cursor
- `x` / `X` - Delete single character
- Word motions: `w`, `b`, `e`
- Text objects: `iw`, `aw`, `i"`, `a"`

✅ **Register System (NEW - Week 9)**
- Unnamed register for all operations
- Named registers `"a-"z` for explicit storage
- Yank register `"0` preserves last yank
- Small delete register `"-` for <1 line deletions

✅ **Full Text Editing**
- Rope-based buffer for O(log n) operations
- Real-time character insertion/deletion
- Multi-line editing with Enter
- Backspace/Delete works correctly
- Automatic cursor position tracking

✅ **Vim-like Modal Editing**
- Normal mode for navigation and commands
- Insert mode for text editing
- Mode indicator in status bar
- Smooth mode transitions (i, a, o, O, A, I, ESC)

✅ **Undo/Redo System**
- `u` to undo, `Ctrl-R` to redo
- Transaction-based edit grouping
- Automatic grouping in insert mode
- Up to 1000 undo levels
- Cursor position restoration

✅ **Lazy File Loading**
- Opens 100MB files in 0.23s
- Memory usage independent of file size (~45MB for 100MB file)
- Line-by-line on-demand reading with LRU cache

✅ **Structural Navigation**
- `]j` / `[j` - Jump between JSON siblings
- Cursor tracks current node type
- Navigate by structure, not just lines

✅ **Syntax Highlighting**
- Brackets/Braces: Blue
- Strings: Green
- Numbers: Yellow
- Booleans: Cyan
- Nulls: Gray

✅ **Performance Overlay**
- Press `F12` to toggle
- Shows FPS, frame times (avg/p99), node count
- Real-time performance metrics

✅ **60fps Scrolling**
- Smooth navigation with hjkl or arrow keys
- Page up/down with `Ctrl+d` / `Ctrl+u`
- No frame drops even on large files

---

## Installation

### From Source

```bash
git clone https://github.com/yourusername/json-tool.git
cd json-tool
cargo build --release
./target/release/json-tool sample.json
```

### Requirements

- Rust 1.70 or later
- Linux, macOS, or Windows with terminal support

---

## Usage

### Basic Commands

```bash
# Open a JSON file
json-tool data.json

# Open with performance overlay
json-tool large.json
# (Press F12 in the app to toggle performance view)
```

### Keybindings

#### Navigation
- `j` / `↓` - Scroll down one line
- `k` / `↑` - Scroll up one line
- `Ctrl+d` - Page down (half screen)
- `Ctrl+u` - Page up (half screen)

#### Structural Navigation
- `]j` - Jump to next sibling node
- `[j` - Jump to previous sibling node

#### System
- `F12` - Toggle performance overlay
- `q` - Quit
- `Ctrl+C` - Force quit

---

## Demo

```bash
# Generate test data
cargo run --bin generate_test_data

# Open medium test file (100MB)
cargo run --release -- tests/medium.json

# Navigate with j/k, try ]j to jump between siblings
# Press F12 to see performance metrics
```

### Status Bar

The status bar shows:
- **File name** and **size** (e.g., "data.json (2.45 MB)")
- **Current line / Total lines** (e.g., "1234:5678")
- **Cursor position** (e.g., "45:12" = line 45, column 12)
- **Current node type** (e.g., "Object", "Array", "String")
- **FPS** (frames per second)

Example:
```
 data.json (2.45 MB) | 1234:5678 | 45:12 | Object | FPS: 60.0 | F12: perf
```

---

## Architecture

JSON Tool uses a **lazy loading** architecture:

1. **Index on Open**: Scan file to build line offset index (O(n) one-time cost)
2. **Read on Demand**: Only load visible lines into memory
3. **Structural Index**: Parse visible portion to build JSON tree
4. **Cache**: Keep recently accessed lines in LRU cache (1000 lines)

Result: **Memory usage independent of file size**

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed design.

---

## Performance

### File Opening

| File Size | Open Time | Memory Usage |
|-----------|-----------|--------------|
| 1 MB      | 0.01s     | ~10 MB       |
| 100 MB    | 0.23s     | ~45 MB       |
| 2 GB      | ~5s*      | ~300 MB      |

*Estimated, not yet tested

### Scrolling

- **60fps sustained** (16ms frame budget)
- Frame time: 8-10ms average, <15ms p99
- Cache hit rate: >90% during typical scrolling

See [PERFORMANCE.md](PERFORMANCE.md) for detailed benchmarks.

---

## Roadmap

### Phase 1: Vim Core & Editing (Weeks 5-9)
- Insert mode (`i`, `a`, `o`)
- Undo/redo system
- Vim operators (`d`, `c`, `y`, `p`)
- Text objects (`iw`, `i{`, `i"`)

### Phase 2: JSON-Aware Features (Weeks 10-13)
- JSON text objects (`ik`, `iv`, `io`, `ia`)
- Parent/child navigation (`]k`, `[k`)
- Structural visual mode

### Phase 3: Streaming & Large Files (Weeks 14-17)
- Background parsing
- Progressive rendering
- Smart folding for large nodes

### Phase 4: Query Engine (Weeks 18-21)
- JSONPath queries
- Query as motion target
- Transform operations

See [IMPLEMENTATION_GUIDE.md](IMPLEMENTATION_GUIDE.md) for full roadmap.

---

## Development

### Build Commands

```bash
# Run in development
cargo run -- sample.json

# Run with optimizations
cargo run --release -- large.json

# Run tests
cargo test

# Run benchmarks
cargo bench

# Check code
cargo clippy -- -D warnings

# Format code
cargo fmt
```

### Generate Test Data

```bash
cargo run --bin generate_test_data
# Creates tests/small.json (1MB), medium.json (100MB), large.json (2GB)
```

### Project Structure

```
src/
  ├── main.rs              # Entry point, event loop
  ├── buffer/
  │   ├── mod.rs           # Lazy file loading, line cache
  │   └── cursor.rs        # Cursor position tracking
  ├── parser/
  │   ├── tokenizer.rs     # Streaming JSON lexer
  │   ├── structural_index.rs # JSON tree representation
  │   ├── node.rs          # Node types (Object, Array, etc.)
  │   └── token.rs         # Token types
  ├── ui/
  │   ├── mod.rs           # UI rendering
  │   └── viewport.rs      # Visible region management
  └── navigation/
      └── mod.rs           # Structural navigation (future)

tests/
  └── generate_test_data.rs # Test file generator

benches/
  └── scroll_bench.rs       # Performance benchmarks
```

---

## Contributing

Contributions welcome! Areas of interest:

1. **Performance**: Optimize hot paths (rendering, cache, parsing)
2. **Features**: Implement Phase 1+ features from roadmap
3. **Testing**: Add test cases, especially for edge cases
4. **Documentation**: Improve guides, add examples

Please open an issue to discuss major changes before submitting PRs.

---

## License

MIT License - see [LICENSE](LICENSE) for details

---

## Inspiration

- **Vim** - Modal editing, operator grammar
- **jq** - JSON query language
- **Helix** - Modern modal editor architecture
- **Xi Editor** - Rope data structure, async design

---

## FAQ

**Q: Why not just use jq/jless/fx?**  
A: Those are excellent tools! JSON Tool focuses on **interactive editing** of gigabyte-scale files with Vim-like navigation. It's complementary to jq (which is better for command-line processing).

**Q: Can it handle 10GB files?**  
A: Phase 0 can *open* them (~20s) and navigate smoothly. Editing is Phase 1+. Memory usage grows with file size but stays tractable (~1GB for 10GB file).

**Q: Is it production-ready?**  
A: **No, Phase 0 is alpha quality.** Use for exploration and experimentation. Don't rely on it for critical data yet. Backups recommended!

**Q: What about Windows support?**  
A: Should work (crossterm is cross-platform), but not thoroughly tested yet. Contributions welcome!

**Q: Can I use it as a library?**  
A: Not yet designed for that, but the `Buffer` and `StructuralIndex` components could be extracted. Open an issue if interested.

---

## Status

**Phase 0 Complete** (December 2025)
- Core lazy loading architecture ✅
- Structural navigation foundation ✅
- Syntax highlighting ✅
- Performance overlay ✅

**Next: Phase 1 - Vim Core & Editing** (Starting January 2026)

---

## Contact

- Issues: [GitHub Issues](https://github.com/yourusername/json-tool/issues)
- Discussions: [GitHub Discussions](https://github.com/yourusername/json-tool/discussions)

---

**Built with ❤️ and Rust**
