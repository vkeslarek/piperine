//! Piperine → Verilog-A lowering tests.
//!
//! Device (analog) modules may use Piperine's brace blocks, `++`/compound
//! assignment, etc. Before OpenVAF (which parses *standard* Verilog-A) sees them,
//! `va_emit::emit_veriloga` lowers them. These tests assert the emitted VA text —
//! they do not run OpenVAF (which needs LLVM); the invariants below are what make
//! the lowered text valid standard VA.

use piperine_circuit::va_emit::emit_veriloga;
use piperine_parser::parser::parse;

fn lower(src: &str) -> String {
    let doc = parse(src).expect("parse failed");
    emit_veriloga(&doc)
}

/// The emitted analog body must be brace-free standard VA.
fn assert_standard_va(out: &str) {
    // The lowered source must not carry Piperine brace blocks into VA.
    let body = out.split("module").nth(1).unwrap_or(out);
    assert!(!body.contains('{'), "lowered VA still has a `{{` block: \n{out}");
    assert!(!body.contains('}'), "lowered VA still has a `}}` block: \n{out}");
    assert!(out.contains("`include \"disciplines.vams\""), "missing disciplines include");
}

#[test]
fn test_brace_block_lowers_to_begin_end() {
    let out = lower(r#"
`include "disciplines.vams"
module soft_clip(in, out);
    inout in, out;
    electrical in, out;
    parameter real vmax = 1.0 from (0:inf);
    analog begin
        real vi;
        vi = V(in) * 2.0;
        if (vi > vmax) {
            V(out) <+ vmax;
        } else {
            V(out) <+ vi;
        }
    end
endmodule
"#);
    assert_standard_va(&out);
    assert!(out.contains("begin"), "no begin/end: \n{out}");
    assert!(out.contains("end"));
    assert!(out.contains("if (vi > vmax)"));
    assert!(out.contains("else"));
    assert!(out.contains("V(out) <+ vmax"), "contribution lost: \n{out}");
    // parameter range constraint preserved
    assert!(out.contains("from (0:inf)"), "constraint lost: \n{out}");
}

#[test]
fn test_compound_assign_desugars() {
    let out = lower(r#"
`include "disciplines.vams"
module poly(in, out);
    inout in, out;
    electrical in, out;
    parameter real a2 = 0.1;
    analog begin
        real y;
        y = V(in);
        y += a2 * V(in) * V(in);
    end
endmodule
"#);
    assert_standard_va(&out);
    // `y += e` must become `y = y + e` (no `+=` survives into VA)
    assert!(!out.contains("+="), "compound op survived: \n{out}");
    assert!(out.contains("y = y + "), "compound not desugared: \n{out}");
}

#[test]
fn test_for_loop_and_increment_lower() {
    let out = lower(r#"
`include "disciplines.vams"
module staircase(in, out);
    inout in, out;
    electrical in, out;
    parameter integer levels = 5;
    analog begin
        integer k;
        real acc;
        acc = 0.0;
        for (k = 0; k < levels; k++) {
            acc = acc + 1.0;
        }
        V(out) <+ acc;
    end
endmodule
"#);
    assert_standard_va(&out);
    assert!(out.contains("for ("), "for lost: \n{out}");
    // `k++` is `k += 1` in the AST → desugars to `k = k + 1`
    assert!(out.contains("k = k + 1"), "increment not desugared: \n{out}");
    assert!(!out.contains("++"));
}

#[test]
fn test_single_statement_analog() {
    // `analog V(out) <+ …;` (no block) must round-trip.
    let out = lower(r#"
`include "disciplines.vams"
module mixer(a, b, out);
    inout a, b, out;
    electrical a, b, out;
    parameter real k = 1.0;
    analog
        V(out) <+ k * V(a) * V(b);
endmodule
"#);
    assert_standard_va(&out);
    assert!(out.contains("V(out) <+ k * V(a) * V(b)"), "expr mangled: \n{out}");
}

#[test]
fn test_testbench_module_not_emitted() {
    // A module with an `initial` block is the testbench, not a VA device — it must
    // not appear in the lowered VA.
    let out = lower(r#"
`include "disciplines.vams"
module dev(p, n);
    inout p, n;
    electrical p, n;
    parameter real r = 1000.0;
    analog I(p, n) <+ V(p, n) / r;
endmodule
module tb;
    initial begin
        $op();
    end
endmodule
"#);
    assert!(out.contains("module dev"), "device module missing");
    assert!(!out.contains("module tb"), "testbench leaked into VA: \n{out}");
}

#[test]
fn test_example_files_emit() {
    // The shipped complex VA examples must lower to brace-free standard VA.
    for name in ["soft_clip", "poly_dist", "staircase"] {
        let path = format!("examples/va/{name}.ppr");
        let src = std::fs::read_to_string(&path).expect(&path);
        let out = lower(&src);
        assert_standard_va(&out);
        assert!(out.contains(&format!("module {name}")), "{name} not emitted");
    }
}
