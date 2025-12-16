use std::fs::File;
use std::io::{BufWriter, Write};

fn generate_json_array(path: &str, num_items: usize, item_size: usize) -> std::io::Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    
    writer.write_all(b"[\n")?;
    
    for i in 0..num_items {
        let data_padding = "x".repeat(item_size);
        let nested_object = format!(
            r#"  {{
    "id": {},
    "name": "item_{}",
    "data": "{}",
    "timestamp": {},
    "nested": {{
      "level": 1,
      "value": {},
      "tags": ["tag1", "tag2", "tag3"]
    }}
  }}"#,
            i, i, data_padding, i * 1000, i * 2
        );
        
        writer.write_all(nested_object.as_bytes())?;
        
        if i < num_items - 1 {
            writer.write_all(b",\n")?;
        } else {
            writer.write_all(b"\n")?;
        }
        
        // Flush periodically to avoid memory buildup
        if i % 1000 == 0 {
            writer.flush()?;
            print!("\rGenerated {} items...", i);
            std::io::stdout().flush()?;
        }
    }
    
    writer.write_all(b"]\n")?;
    writer.flush()?;
    println!("\rGenerated {} items successfully!", num_items);
    
    Ok(())
}

fn main() {
    println!("JSON Test Data Generator");
    println!("========================\n");
    
    // Small test file (1MB)
    println!("Generating small.json (1MB)...");
    if let Err(e) = generate_json_array("tests/small.json", 5000, 100) {
        eprintln!("Error generating small.json: {}", e);
    }
    
    // Medium test file (100MB)
    println!("\nGenerating medium.json (100MB)...");
    if let Err(e) = generate_json_array("tests/medium.json", 500_000, 100) {
        eprintln!("Error generating medium.json: {}", e);
    }
    
    // Large test file (2GB) - This will take a while
    println!("\nGenerating large.json (2GB)...");
    println!("This may take several minutes...");
    if let Err(e) = generate_json_array("tests/large.json", 10_000_000, 100) {
        eprintln!("Error generating large.json: {}", e);
    }
    
    println!("\n\nAll test files generated successfully!");
    println!("Files created:");
    println!("  - tests/small.json  (~1MB)");
    println!("  - tests/medium.json (~100MB)");
    println!("  - tests/large.json  (~2GB)");
}
