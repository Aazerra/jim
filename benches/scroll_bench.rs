use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use json_tool::buffer::Buffer;
use std::time::Duration;

fn scroll_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("scroll");
    group.measurement_time(Duration::from_secs(10));
    
    // Create test buffer with sample data
    let mut buffer = Buffer::new();
    let _ = buffer.load_file("sample.json");
    
    // Benchmark getting single line
    group.bench_function("get_single_line", |b| {
        b.iter(|| {
            black_box(buffer.get_line(5))
        })
    });
    
    // Benchmark getting visible lines (viewport)
    for size in [10, 20, 40, 80].iter() {
        group.bench_with_input(
            BenchmarkId::new("get_visible_lines", size),
            size,
            |b, &size| {
                b.iter(|| {
                    black_box(buffer.get_visible_lines(0, size))
                })
            },
        );
    }
    
    group.finish();
}

fn tokenizer_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("tokenizer");
    group.measurement_time(Duration::from_secs(10));
    
    let small_json = r#"{"key": "value", "number": 123, "bool": true}"#;
    let medium_json = r#"{
        "users": [
            {"id": 1, "name": "Alice", "email": "alice@example.com"},
            {"id": 2, "name": "Bob", "email": "bob@example.com"},
            {"id": 3, "name": "Charlie", "email": "charlie@example.com"}
        ],
        "metadata": {
            "count": 3,
            "timestamp": 1234567890
        }
    }"#;
    
    group.bench_function("tokenize_small", |b| {
        b.iter(|| {
            let mut tokenizer = json_tool::parser::Tokenizer::new(small_json.to_string());
            black_box(tokenizer.tokenize_all())
        })
    });
    
    group.bench_function("tokenize_medium", |b| {
        b.iter(|| {
            let mut tokenizer = json_tool::parser::Tokenizer::new(medium_json.to_string());
            black_box(tokenizer.tokenize_all())
        })
    });
    
    group.finish();
}

criterion_group!(benches, scroll_benchmark, tokenizer_benchmark);
criterion_main!(benches);
