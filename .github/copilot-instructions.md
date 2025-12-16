# Jim - JSON Interactive Manager - AI Coding Assistant Guide

## Project Overview

Jim: Streaming Vim-like JSON editor in Rust. Handles multi-gigabyte JSON files through lazy parsing and structural navigation. **Phase 0 status**: Early implementation, most code not yet written.

## Architecture

Three-component design with thread separation:

- **UI Thread**: ratatui rendering + crossterm input (main.rs, ui/)
- **Shared State**: Rope buffer (ropey) + StructuralIndex via Arc<RwLock<T>>
- **Parser Thread**: Background tokenizer → index builder, communicates via crossbeam channels

Key files (when implemented):

- `src/buffer/mod.rs` - Rope wrapper with O(log n) edits
- `src/parser/tokenizer.rs` - Streaming state machine lexer
- `src/parser/structural_index.rs` - Interval tree for JSON structure
- `src/navigation/mod.rs` - Structural motions (siblings, parent)

## Critical Design Decisions

- **Never load full JSON into String**: Use Rope + streaming tokenizer
- **Never block UI thread**: Parse in background tokio task
- **Never panic on invalid JSON**: Maintain partial structural index with error recovery
- **Lazy parsing**: Only fully parse visible/edited regions; elsewhere use placeholders

## Development Patterns

### State Management

```rust
// Share state between threads
let buffer = Arc::new(RwLock::new(Buffer::new()));
let buffer_ui = Arc::clone(&buffer);

// UI thread: short-lived read locks only
let text = buffer_ui.read().unwrap().slice(viewport);
```

### Message Passing

Use crossbeam channels for parser communication:

- `ParserMessage::Parse(Range)` → trigger region parsing
- `ParserResponse::IndexUpdate` → send back structural nodes

### Error Handling

Use `anyhow::Result` for functions, `thiserror::Error` for domain errors. Example:

```rust
#[derive(Error, Debug)]
pub enum JsonToolError {
    #[error("Parse error at offset {offset}: {message}")]
    ParseError { offset: usize, message: String },
}
```

## Performance Requirements (Phase 0)

- Open 2GB file: <3s (first render visible)
- Memory footprint: <100MB initially
- Scroll: 60fps sustained (p99 <16ms frame time)
- Index query: <1ms (binary search)

**Always benchmark in --release mode**: `cargo run --release -- large.json`

## Testing Strategy

- **Unit tests**: Token streams, rope operations (see test_tokenize_simple_object pattern)
- **Property tests**: Use proptest for rope invariants
- **Benchmarks**: Criterion in benches/ directory (scroll_bench.rs template)
- **Test data**: Generate large JSON with `tests/generate_test_data.rs` script

## Common Pitfalls

1. Holding write locks during rendering → deadlock (copy viewport data first)
2. Re-parsing on every edit → use incremental updates only
3. String clones in hot paths → use `Cow<str>` or slice references
4. Forgetting terminal restore → use panic hook in main.rs

## Phase 0 Implementation Order

1. **Days 1-7**: Basic TUI (event loop, crossterm setup, viewport rendering)
2. **Days 8-14**: Buffer (rope, cursor, file loading, scroll with hjkl)
3. **Days 15-21**: Tokenizer (state machine, background thread, error recovery)
4. **Days 22-30**: Structural index (interval tree, sibling navigation, syntax coloring)

## Key Commands (Reference)

```bash
cargo run --release -- file.json  # Test with optimizations
cargo test                        # Unit tests
cargo bench                       # Criterion benchmarks
cargo clippy -- -D warnings       # Linting
cargo flamegraph --root -- file   # CPU profiling
```

## When Implementing New Features

- Check [IMPLEMENTATION_GUIDE.md](../docs/IMPLEMENTATION_GUIDE.md) for detailed specs
- Follow module structure: src/{ui,buffer,parser,navigation}
- Always implement error recovery for parser-related code
- Profile before optimizing (use flamegraph)
- Add benchmark if touching hot paths (scroll, render, parse)

## When documenting code

- Always create doc in docs directory

## Out of Scope for Phase 0

- Editing (insert mode) → Phase 1
- Undo/redo → Phase 1
- Query engine → Phase 4
- Multi-cursor → Phase 2
