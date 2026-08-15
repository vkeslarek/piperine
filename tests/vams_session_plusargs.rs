//! §9.12 command-line input (LRMC-C7-09, T48): `SessionBuilder::
//! plusargs(...)` through a real `piperine::Session` — the host surface
//! half of `$test$plusargs`/`$value$plusargs` (JIT dispatch inside a
//! `.vams` body is a documented, disclosed gap — see `SessionBuilder::
//! plusargs`'s own doc comment).

use piperine::Session;
use piperine_lang::{parse_and_elaborate, SourceMap, Value};

const TRIVIAL: &str = "
    module dut ();
        real x;
        analog x = 1.0;
    endmodule
";

#[test]
fn session_builder_plusargs_reach_the_compiled_session() {
    let design = parse_and_elaborate(TRIVIAL, &SourceMap::dummy()).expect("elaborates");
    let session = Session::builder(&design, "dut")
        .plusargs(["+seed=1234", "+verbose", "not-a-plusarg", "+name=hello"])
        .compile()
        .expect("session compiles");

    assert!(session.test_plusarg("seed"));
    assert!(session.test_plusarg("verbose"));
    assert!(!session.test_plusarg("missing"));

    assert_eq!(session.value_plusarg("seed=%d").unwrap(), Some(Value::Int(1234)));
    assert_eq!(session.value_plusarg("name=%s").unwrap(), Some(Value::Str("hello".into())));
    assert_eq!(session.value_plusarg("absent=%d").unwrap(), None);
}

#[test]
fn a_session_built_without_plusargs_answers_false_and_none_not_an_error() {
    let design = parse_and_elaborate(TRIVIAL, &SourceMap::dummy()).expect("elaborates");
    let session = Session::builder(&design, "dut").compile().expect("session compiles");

    assert!(!session.test_plusarg("anything"));
    assert_eq!(session.value_plusarg("anything=%d").unwrap(), None);
}
