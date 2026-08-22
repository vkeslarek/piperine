//! LRM-COVERS: 4.5.4
//! LRMW2-29 (`lrm-conformance-wave2`, T34): `idt(expr, ic, assert)`'s
//! third argument was silently discarded at lowering
//! (`resolve/pom/analog_ops.rs::Idt::lower` only ever read `args[0..1]`)
//! — `StateKind::Idt` carried no `assert` field at all. Checks the
//! lowering step in isolation, complementing the full-transient
//! discriminating proof in root `tests/idt_assert_ramp_generator.rs`.

use piperine_codegen::resolve::{StateKind, StateVar};

fn idt_state(src: &str) -> StateVar {
    let design = piperine_lang::parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let body = &bodies["dut"];
    body.symbols
        .states()
        .find_map(|(_, s)| matches!(s.kind, StateKind::Idt { .. }).then(|| s.clone()))
        .expect("an idt state")
}

/// §4.5.4: an omitted third argument leaves `assert` unset.
#[test]
fn idt_without_an_assert_argument_has_no_assert() {
    let src = "module dut (inout electrical p, inout electrical n);\n\
        analog begin\n\
        I(p, n) <+ idt(V(p, n), 0);\n\
        end\n\
        endmodule\n";
    let StateKind::Idt { assert, .. } = idt_state(src).kind else { unreachable!() };
    assert!(assert.is_none(), "no third argument must leave assert unset");
}

/// §4.5.4: an explicit `assert` argument is carried, not discarded.
#[test]
fn idt_with_an_explicit_assert_carries_it() {
    let src = "module dut (inout electrical p, inout electrical n);\n\
        integer reset;\n\
        analog begin\n\
        I(p, n) <+ idt(V(p, n), 0, reset);\n\
        end\n\
        endmodule\n";
    let StateKind::Idt { assert, .. } = idt_state(src).kind else { unreachable!() };
    assert!(assert.is_some(), "an explicit assert argument must be carried, not discarded");
}
