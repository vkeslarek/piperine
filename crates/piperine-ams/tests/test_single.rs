use piperine_ams::Document;
use std::path::Path;

#[test]
fn test_single() {
    let result = Document::parse_file(Path::new("tests/fixtures/vams/vams_abs2.vams"));
    if let Err(e) = result {
        panic!("Error: {:?}", e);
    }
}
