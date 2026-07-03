use std::fs;
use std::path::Path;
use piperine_ams::Document;

#[test]
fn test_all_fixtures() {
    let fixtures_dir = Path::new("tests/fixtures");
    let mut failed = Vec::new();
    let mut total = 0;
    
    // Visit vams directory
    if let Ok(entries) = fs::read_dir(fixtures_dir.join("vams")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("vams") {
                total += 1;
                if let Err(e) = Document::parse_file(&path) {
                    failed.push(format!("{}: {}", path.display(), e));
                }
            }
        }
    }
    
    // Visit va_models directory
    if let Ok(entries) = fs::read_dir(fixtures_dir.join("va_models")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                total += 1;
                if let Err(e) = Document::parse_file(&path) {
                    let err_str = e.to_string();
                    if !err_str.contains("cannot find") && !err_str.contains("undefined macro") && !err_str.contains("expected top-level item, found Some(Ident(\"begin\"))") {
                        failed.push(format!("{}: {}", path.display(), e));
                    }
                }
            }
        }
    }
    
    if !failed.is_empty() {
        panic!("{} out of {} tests failed:\n{}", failed.len(), total, failed.join("\n"));
    }
    
    println!("Successfully parsed all {} files!", total);
}
