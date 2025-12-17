use std::fs;
use std::io::Write;

#[test]
fn test_save_matches_viewport() {
    use crate::buffer::Buffer;
    
    // Create test file
    let test_path = "/tmp/jim_test_save.json";
    fs::write(test_path, r#"{"name": "test", "value": 123}"#).unwrap();
    
    // Load into buffer
    let mut buffer = Buffer::new();
    buffer.load_file(test_path).unwrap();
    
    // Make some edits (simulate user typing)
    let insert_pos = buffer.rope().len_bytes() - 1; // Before closing }
    buffer.insert(insert_pos, r#", "new_field": "hello""#).unwrap();
    
    // What we see in viewport
    let viewport_content = buffer.rope().to_string();
    
    // Save
    buffer.save().unwrap();
    
    // Wait for background save to complete
    while buffer.is_saving() {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    buffer.finalize_save().unwrap();
    
    // Read saved file
    let saved_content = fs::read_to_string(test_path).unwrap();
    
    // Clean up
    fs::remove_file(test_path).ok();
    
    // CRITICAL: Saved content MUST match viewport
    assert_eq!(
        saved_content, 
        viewport_content,
        "BUG: Saved file doesn't match viewport!\nViewport: {}\nSaved: {}",
        viewport_content,
        saved_content
    );
}
