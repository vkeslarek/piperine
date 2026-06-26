use piperine_parser::parse_file;
use std::path::Path;

#[test]
fn test_single() {
    let result = parse_file(Path::new("tests/fixtures/vams/vams_abs2.vams"));
    if let Err(e) = result {
        panic!("Error: {:?}", e);
    }
}
