//! Tests: AMS (Verilog-A) → IR lowering and pseudo-language printer.

use std::path::{Path, PathBuf};
use piperine_ams::Document;
use piperine_codegen::{ams_to_ir, ContribKind, IrExpr, IrNature, IrStateKind, IrStmt};

fn bundled_headers() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()           // crates/
        .join("piperine-ams")
        .join("headers")
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .join("piperine-ams")
        .join("tests/fixtures_fmt")
}

fn parse_fixture(name: &str) -> Document {
    let path = fixture_dir().join(name);
    let input = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {name}: {e}"));
    let dirs = vec![bundled_headers(), path.parent().unwrap().to_path_buf()];
    Document::parse_with_includes(&input, &dirs)
        .unwrap_or_else(|e| panic!("parse {name}: {e}"))
}

// ─── Conductor ────────────────────────────────────────────────────────────────

#[test]
fn conductor_module_parsed() {
    let doc = parse_fixture("conductor.vams");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "conductor").expect("module");
    assert_eq!(m.ports.len(), 2);
    assert!(m.ports.iter().any(|p| p.name == "p"));
    assert!(m.ports.iter().any(|p| p.name == "n"));
}

#[test]
fn conductor_has_resistive_contrib() {
    let doc = parse_fixture("conductor.vams");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "conductor").expect("module");
    let body = m.analog.as_ref().expect("analog body");
    assert!(!body.stmts.is_empty(), "stmts empty");
    match &body.stmts[0] {
        IrStmt::Contrib { nature: IrNature::Flow(_), kind: ContribKind::Resistive, plus, minus, .. } => {
            assert_eq!(plus, "p");
            assert_eq!(minus, "n");
        }
        other => panic!("expected current resistive contrib, got {other:?}"),
    }
}

#[test]
fn conductor_param_g() {
    let doc = parse_fixture("conductor.vams");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "conductor").expect("module");
    assert!(m.params.iter().any(|p| p.name == "g"), "params: {:?}", m.params.iter().map(|p| &p.name).collect::<Vec<_>>());
}

#[test]
fn snap_conductor() {
    let doc = parse_fixture("conductor.vams");
    let ir = ams_to_ir(&doc);
    println!("\n=== conductor.vams IR ===\n{ir}");
    assert!(!format!("{ir}").is_empty());
}

// ─── Resistor ─────────────────────────────────────────────────────────────────

#[test]
fn resistor_two_modules() {
    let doc = parse_fixture("resistor.va");
    let ir = ams_to_ir(&doc);
    assert!(ir.modules.iter().any(|m| m.name == "res1"), "missing res1");
    assert!(ir.modules.iter().any(|m| m.name == "res2"), "missing res2");
}

#[test]
fn resistor_res1_current_contrib() {
    let doc = parse_fixture("resistor.va");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "res1").expect("res1");
    let body = m.analog.as_ref().expect("analog");
    match body.stmts.iter().find(|s| matches!(s, IrStmt::Contrib { nature: IrNature::Flow(_), .. })) {
        Some(IrStmt::Contrib { nature: IrNature::Flow(_), kind: ContribKind::Resistive, .. }) => {}
        other => panic!("expected current resistive contrib: {other:?}"),
    }
}

#[test]
fn resistor_res2_voltage_contrib() {
    let doc = parse_fixture("resistor.va");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "res2").expect("res2");
    let body = m.analog.as_ref().expect("analog");
    match body.stmts.iter().find(|s| matches!(s, IrStmt::Contrib { nature: IrNature::Potential(_), .. })) {
        Some(IrStmt::Contrib { nature: IrNature::Potential(_), kind: ContribKind::Resistive, .. }) => {}
        other => panic!("expected voltage resistive contrib: {other:?}"),
    }
}

#[test]
fn snap_resistor() {
    let doc = parse_fixture("resistor.va");
    let ir = ams_to_ir(&doc);
    println!("\n=== resistor.va IR ===\n{ir}");
}

// ─── Capacitor ────────────────────────────────────────────────────────────────

#[test]
fn capacitor_cap1_has_ddt() {
    let doc = parse_fixture("capacitor.va");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "cap1").expect("cap1");
    let body = m.analog.as_ref().expect("analog");
    assert!(!body.state_vars.is_empty(), "expected ddt state var");
    assert!(matches!(body.state_vars[0].kind, IrStateKind::Ddt));
}

#[test]
fn capacitor_cap1_reactive_contrib() {
    let doc = parse_fixture("capacitor.va");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "cap1").expect("cap1");
    let body = m.analog.as_ref().expect("analog");
    let has_reactive = body.stmts.iter().any(|s| matches!(s, IrStmt::Contrib { kind: ContribKind::Reactive(_), .. }));
    assert!(has_reactive, "expected reactive contrib");
}

#[test]
fn capacitor_cap2_has_if() {
    let doc = parse_fixture("capacitor.va");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "cap2").expect("cap2");
    let body = m.analog.as_ref().expect("analog");
    let has_if = body.stmts.iter().any(|s| matches!(s, IrStmt::If { .. }));
    assert!(has_if, "cap2 should have if stmt for analysis(\"ic\")");
}

#[test]
fn snap_capacitor() {
    let doc = parse_fixture("capacitor.va");
    let ir = ams_to_ir(&doc);
    println!("\n=== capacitor.va IR ===\n{ir}");
}

// ─── Diode ────────────────────────────────────────────────────────────────────

#[test]
fn snap_diode() {
    let doc = parse_fixture("diode_basic.va");
    let ir = ams_to_ir(&doc);
    println!("\n=== diode_basic.va IR ===\n{ir}");
    let m = ir.modules.iter().find(|m| m.name == "diode_basic").expect("module");
    // Should have at least one contribution
    let body = m.analog.as_ref().expect("analog");
    assert!(!body.stmts.is_empty(), "expected stmts");
}

// ─── Inline AMS source ────────────────────────────────────────────────────────

const SIMPLE_DIODE: &str = r#"
`include "disciplines.h"
module simple_diode(anode, cathode);
    inout anode, cathode;
    electrical anode, cathode;
    parameter real Is = 1e-14;
    parameter real n = 1.0;

    analog begin
        I(anode, cathode) <+ Is * (exp(V(anode, cathode) / ($vt * n)) - 1.0);
    end
endmodule
"#;

#[test]
fn inline_diode_nonlinear_contrib() {
    let dirs = vec![bundled_headers()];
    let doc = Document::parse_with_includes(SIMPLE_DIODE, &dirs).expect("parse");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "simple_diode").expect("module");
    let body = m.analog.as_ref().expect("analog");
    assert!(!body.stmts.is_empty());
    let out = format!("{ir}");
    assert!(out.contains("exp("), "output: {out}");
    assert!(out.contains("$vt"), "output: {out}");
    println!("\n=== inline simple_diode IR ===\n{out}");
}

// ─── Resistor with if (res3) ──────────────────────────────────────────────────

#[test]
fn resistor_res3_if_structure() {
    let doc = parse_fixture("resistor.va");
    let ir = ams_to_ir(&doc);
    // res3 has an if stmt
    if let Some(m) = ir.modules.iter().find(|m| m.name == "res3") {
        if let Some(body) = &m.analog {
            let has_if = body.stmts.iter().any(|s| matches!(s, IrStmt::If { .. }));
            assert!(has_if, "res3 should have if stmt");
        }
    }
    // Print all resistor modules
    let out = format!("{ir}");
    println!("\n=== resistor.va (all modules) IR ===\n{out}");
}

// ─── Printer coverage ─────────────────────────────────────────────────────────

#[test]
fn printer_produces_source_header() {
    let doc = parse_fixture("conductor.vams");
    let ir = ams_to_ir(&doc);
    let out = format!("{ir}");
    assert!(out.contains("// IR pseudo-language (source: ams)"), "output: {out}");
}

// ─── Shift operators ──────────────────────────────────────────────────────────

const SHIFT_TEST: &str = r#"
`include "disciplines.h"
module shift_test(a, b);
    inout a, b;
    electrical a, b;
    analog begin
        I(a, b) <+ V(a, b) << 2;
    end
endmodule
"#;

#[test]
fn shift_operator_preserved() {
    let dirs = vec![bundled_headers()];
    let doc = Document::parse_with_includes(SHIFT_TEST, &dirs).expect("parse");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "shift_test").expect("module");
    let body = m.analog.as_ref().expect("analog");
    let out = format!("{ir}");
    assert!(out.contains("<<"), "expected shift operator, output: {out}");
}

// ─── Reduction operators ──────────────────────────────────────────────────────

const REDUCTION_TEST: &str = r#"
`include "disciplines.h"
module reduction_test(a, b);
    inout a, b;
    electrical a, b;
    integer x;
    analog begin
        x = 4'b1010;
        I(a, b) <+ &x;
    end
endmodule
"#;

#[test]
fn reduction_operator_preserved() {
    let dirs = vec![bundled_headers()];
    let doc = Document::parse_with_includes(REDUCTION_TEST, &dirs).expect("parse");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "reduction_test").expect("module");
    let body = m.analog.as_ref().expect("analog");
    let out = format!("{ir}");
    // Should contain the reduction operator ~ not just drop the expression
    assert!(out.contains("&(") || out.contains("~&") || out.contains("~|") || out.contains("|("),
        "expected reduction operator, output: {out}");
}

// ─── Named port connections ───────────────────────────────────────────────────

const NAMED_PORTS_TEST: &str = r#"
`include "disciplines.h"
module sub(p, n);
    inout p, n;
    electrical p, n;
endmodule
module named_ports(a, b);
    inout a, b;
    electrical a, b;
    sub u1(.p(a), .n(b));
endmodule
"#;

#[test]
fn named_port_connection_preserved() {
    let dirs = vec![bundled_headers()];
    let doc = Document::parse_with_includes(NAMED_PORTS_TEST, &dirs).expect("parse");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "named_ports").expect("module");
    let inst = m.instances.iter().find(|i| i.label == "u1").expect("instance");
    // Should have 2 named connections
    assert_eq!(inst.connections.len(), 2);
    assert!(inst.connections.iter().all(|c| c.port.is_some()), "all should be named");
    let out = format!("{ir}");
    assert!(out.contains(".p("), "expected named port, output: {out}");
}

// ─── String literal ───────────────────────────────────────────────────────────

const STRING_TEST: &str = r#"
`include "disciplines.h"
module string_test(a, b);
    inout a, b;
    electrical a, b;
    parameter string name = "test";
    analog begin
        I(a, b) <+ V(a, b) / 1000.0;
    end
endmodule
"#;

#[test]
fn string_literal_preserved() {
    let dirs = vec![bundled_headers()];
    let doc = Document::parse_with_includes(STRING_TEST, &dirs).expect("parse");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "string_test").expect("module");
    let p = m.params.iter().find(|p| p.name == "name").expect("name param");
    match &p.default {
        Some(IrExpr::String(s)) => assert_eq!(s, "test"),
        other => panic!("expected String, got {other:?}"),
    }
}

// ─── Functions ────────────────────────────────────────────────────────────────

const FUNCTION_TEST: &str = r#"
`include "disciplines.h"
module function_test(a, b);
    inout a, b;
    electrical a, b;
    parameter real scale = 1.0;
    analog begin
        I(a, b) <+ scale * compute(V(a, b));
    end
    function real compute;
        input real x;
        begin
            compute = x * 2.0;
        end
    endfunction
endmodule
"#;

#[test]
fn function_lowered() {
    let dirs = vec![bundled_headers()];
    let doc = Document::parse_with_includes(FUNCTION_TEST, &dirs).expect("parse");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "function_test").expect("module");
    assert!(m.functions.iter().any(|f| f.name == "compute"), "expected compute function");
    let out = format!("{ir}");
    assert!(out.contains("fn compute"), "output: {out}");
}

// ─── Noise sources ────────────────────────────────────────────────────────────

const NOISE_TEST: &str = r#"
`include "disciplines.h"
module noise_test(a, b);
    inout a, b;
    electrical a, b;
    analog begin
        I(a, b) <+ white_noise(1e-24, "rn1");
    end
endmodule
"#;

#[test]
fn noise_source_registered_ams() {
    let dirs = vec![bundled_headers()];
    let doc = Document::parse_with_includes(NOISE_TEST, &dirs).expect("parse");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "noise_test").expect("module");
    let body = m.analog.as_ref().expect("analog");
    assert_eq!(body.noise_sources.len(), 1);
    assert_eq!(body.noise_sources[0].label.as_deref(), Some("rn1"));
}

// ─── $discontinuity ───────────────────────────────────────────────────────────

const DISCONTINUITY_TEST: &str = r#"
`include "disciplines.h"
module disc_test(a, b);
    inout a, b;
    electrical a, b;
    analog begin
        if (V(a, b) > 0.0) begin
            I(a, b) <+ V(a, b);
        end else begin
            $discontinuity(1);
            I(a, b) <+ 0.0;
        end
    end
endmodule
"#;

#[test]
fn discontinuity_stmt() {
    let dirs = vec![bundled_headers()];
    let doc = Document::parse_with_includes(DISCONTINUITY_TEST, &dirs).expect("parse");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "disc_test").expect("module");
    let body = m.analog.as_ref().expect("analog");
    let has_disc = body.stmts.iter().any(|s| {
        if let IrStmt::If { else_, .. } = s {
            else_.iter().any(|es| matches!(es, IrStmt::Discontinuity(1)))
        } else {
            false
        }
    });
    assert!(has_disc, "expected $discontinuity(1) in else branch");
}

// ─── transition analog operator ───────────────────────────────────────────────

const TRANSITION_TEST: &str = r#"
`include "disciplines.h"
module trans_test(a, b);
    inout a, b;
    electrical a, b;
    analog begin
        I(a, b) <+ transition(V(a, b), 0.0, 1e-6, 1e-6);
    end
endmodule
"#;

#[test]
fn transition_state_var_ams() {
    let dirs = vec![bundled_headers()];
    let doc = Document::parse_with_includes(TRANSITION_TEST, &dirs).expect("parse");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "trans_test").expect("module");
    let body = m.analog.as_ref().expect("analog");
    assert!(body.state_vars.iter().any(|sv| matches!(sv.kind, IrStateKind::Transition { .. })));
}

// ─── $simparam ────────────────────────────────────────────────────────────────

const SIMPARAM_TEST: &str = r#"
`include "disciplines.h"
module simparam_test(a, b);
    inout a, b;
    electrical a, b;
    analog begin
        I(a, b) <+ $simparam("temp", 300.0);
    end
endmodule
"#;

#[test]
fn simparam_query_ams() {
    let dirs = vec![bundled_headers()];
    let doc = Document::parse_with_includes(SIMPARAM_TEST, &dirs).expect("parse");
    let ir = ams_to_ir(&doc);
    let out = format!("{ir}");
    assert!(out.contains("$simparam"), "output: {out}");
}

// ─── delay analog operator ────────────────────────────────────────────────────

const DELAY_TEST: &str = r#"
`include "disciplines.h"
module delay_test(a, b);
    inout a, b;
    electrical a, b;
    analog begin
        I(a, b) <+ absdelay(V(a, b), 1e-6);
    end
endmodule
"#;

#[test]
fn delay_state_var_ams() {
    let dirs = vec![bundled_headers()];
    let doc = Document::parse_with_includes(DELAY_TEST, &dirs).expect("parse");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "delay_test").expect("module");
    let body = m.analog.as_ref().expect("analog");
    assert!(body.state_vars.iter().any(|sv| matches!(sv.kind, IrStateKind::Delay { .. })));
}

// ─── laplace filter ───────────────────────────────────────────────────────────

const LAPLACE_TEST: &str = r#"
`include "disciplines.h"
module laplace_test(a, b);
    inout a, b;
    electrical a, b;
    analog begin
        I(a, b) <+ laplace_np(V(a, b), {1.0, 2.0}, {1.0, 1.0});
    end
endmodule
"#;

#[test]
fn laplace_state_var_ams() {
    let dirs = vec![bundled_headers()];
    let doc = Document::parse_with_includes(LAPLACE_TEST, &dirs).expect("parse");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "laplace_test").expect("module");
    let body = m.analog.as_ref().expect("analog");
    assert!(body.state_vars.iter().any(|sv| matches!(sv.kind, IrStateKind::Laplace { variant: ref v, .. } if v == "np")));
}

// ─── timer event with period ──────────────────────────────────────────────────

const TIMER_EVENT_TEST: &str = r#"
`include "disciplines.h"
module timer_event_test(a, b);
    inout a, b;
    electrical a, b;
    analog begin
        @(timer(1e-3)) begin
            I(a, b) <+ 1.0;
        end
    end
endmodule
"#;

#[test]
fn timer_event_ams() {
    let dirs = vec![bundled_headers()];
    let doc = Document::parse_with_includes(TIMER_EVENT_TEST, &dirs).expect("parse");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "timer_event_test").expect("module");
    let body = m.analog.as_ref().expect("analog");
    let has_timer = body.stmts.iter().any(|s| matches!(
        s,
        IrStmt::AnalogEvent { kind: piperine_codegen::IrEventKind::Timer { .. }, .. }
    ));
    assert!(has_timer, "expected Timer event");
}

// ─── Cross event with expression ──────────────────────────────────────────────

const CROSS_EVENT_TEST: &str = r#"
`include "disciplines.h"
module cross_event_test(a, b);
    inout a, b;
    electrical a, b;
    analog begin
        @(cross(V(a, b), 1)) begin
            I(a, b) <+ 1.0;
        end
    end
endmodule
"#;

#[test]
fn cross_event_with_expr_ams() {
    let dirs = vec![bundled_headers()];
    let doc = Document::parse_with_includes(CROSS_EVENT_TEST, &dirs).expect("parse");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "cross_event_test").expect("module");
    let body = m.analog.as_ref().expect("analog");
    let has_cross = body.stmts.iter().any(|s| matches!(
        s,
        IrStmt::AnalogEvent { kind: piperine_codegen::IrEventKind::Cross { dir: 1, .. }, .. }
    ));
    assert!(has_cross, "expected Cross event with dir=1");
}

// ─── Branch declarations ──────────────────────────────────────────────────────

const BRANCH_TEST: &str = r#"
`include "disciplines.h"
module branch_test(a, b);
    inout a, b;
    electrical a, b;
    branch (a, b) br;
    analog begin
        I(a, b) <+ V(br) / 1000.0;
    end
endmodule
"#;

#[test]
fn branch_declaration_preserved() {
    let dirs = vec![bundled_headers()];
    let doc = Document::parse_with_includes(BRANCH_TEST, &dirs).expect("parse");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "branch_test").expect("module");
    assert!(m.branches.iter().any(|b| b.name == "br"), "expected branch 'br'");
}

// ─── Parameter type preserved ─────────────────────────────────────────────────

const PARAM_TYPE_TEST: &str = r#"
`include "disciplines.h"
module param_type_test(a, b);
    inout a, b;
    electrical a, b;
    parameter real r = 1000.0;
    parameter integer n = 42;
    parameter string s = "hello";
endmodule
"#;

#[test]
fn parameter_types_preserved() {
    let dirs = vec![bundled_headers()];
    let doc = Document::parse_with_includes(PARAM_TYPE_TEST, &dirs).expect("parse");
    let ir = ams_to_ir(&doc);
    let m = ir.modules.iter().find(|m| m.name == "param_type_test").expect("module");
    let r = m.params.iter().find(|p| p.name == "r").expect("r");
    assert_eq!(r.ty, piperine_codegen::IrType::Real);
    let n = m.params.iter().find(|p| p.name == "n").expect("n");
    assert_eq!(n.ty, piperine_codegen::IrType::Integer);
    let s = m.params.iter().find(|p| p.name == "s").expect("s");
    assert_eq!(s.ty, piperine_codegen::IrType::String);
}
