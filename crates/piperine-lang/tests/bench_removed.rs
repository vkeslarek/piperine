//! BRM-01/BRM-02: the in-language `bench` is gone — a `bench` block is a
//! plain syntax error (the keyword no longer exists), and no bench surface
//! remains in the elaborated `Design`.

use piperine_lang::SourceMap;

/// A `bench` block fails to parse: total grammar removal (user decision —
/// a generic syntax error, no friendly migration path).
#[test]
fn bench_block_is_a_syntax_error() {
    let src = "
        mod Top() {}
        bench Top {
            fn test_something() {
                $assert(true, \"ok\");
            }
        }
    ";
    let err = piperine_lang::parse_str(src).expect_err("a bench block must not parse");
    assert!(!format!("{err}").is_empty(), "a real diagnostic: {err}");
}

/// `$task` calls outside any bench never had meaning at the module level;
/// a design elaborates exactly as before (const-eval is untouched).
#[test]
fn designs_elaborate_unchanged_without_benches() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod Resistor(inout p: Electrical, inout n: Electrical) {
            param r: Real = 1.0e3;
        }
        analog Resistor { I(p, n) <+ V(p, n) / r; }
        mod Top() {
            wire gnd : Electrical;
            r1 : Resistor(.p = gnd, .n = gnd) { .r = 2.0e3 };
        }
    ";
    let design = piperine_lang::parse_and_elaborate(src, &SourceMap::dummy())
        .expect("elaborates");
    assert!(design.module("Top").is_some());
}
