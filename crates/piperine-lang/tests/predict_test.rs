use piperine_lang::parse::predict_at_cursor;

#[test]
fn test_prediction() {
    let source = "mod Res2 (inout p: Electrical, inout n: Electrical) {\n    \n}";
    let cursor = 58;
    let expected = predict_at_cursor(source, cursor);
    println!("EXPECTATIONS 58: {:#?}", expected);
}

#[test]
fn test_port_prediction() {
    let source = "mod Res2 ( )";
    // offset 10 is inside ( )
    let cursor = 10;
    let expected = predict_at_cursor(source, cursor);
    println!("EXPECTATIONS 10: {:#?}", expected);
}
