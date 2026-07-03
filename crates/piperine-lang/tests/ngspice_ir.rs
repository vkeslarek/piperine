//! Tests: NGSPICE faithful headers (`headers/ngspice/*.phdl`) parse,
//! elaborate, and lower to IR end-to-end via `use piperine::ngspice::*;`.
//!
//! `ngspice_parse_tests.rs` only exercises `parse_str` directly against the
//! raw header source (no elaboration). These tests go the whole distance:
//! `parse_str` → `SourceFile::elaborate()` (which expands `use` through the
//! `Resolver`) → `ppr_to_ir`. Each header is wrapped in a small top module
//! that instantiates the device(s) under test so elaboration has a concrete
//! module to resolve against.

use piperine_lang::{parse_str, ppr_to_ir};

fn elaborate_ir(src: &str) -> piperine_codegen::ir::IrProgram {
    let file = parse_str(src).unwrap_or_else(|e| panic!("parse error: {e}"));
    let design = file.elaborate(&piperine_lang::SourceMap::dummy()).unwrap_or_else(|e| panic!("elaborate error: {e}"));
    ppr_to_ir(&design).expect("lowering failed")
}

#[test]
fn ngspice_diode_elaborates_to_ir() {
    let src = "
use piperine::ngspice::diode;
mod Top(inout p: Electrical, inout n: Electrical) {
    d1 : dio(p, n);
}
";
    let ir = elaborate_ir(src);
    let m = ir.modules.iter().find(|m| m.name == "dio").expect("dio module");
    let body = m.analog.as_ref().expect("dio should have an analog body");
    assert!(!body.noise.is_empty(), "dio should carry white/flicker noise sources");
    assert!(
        m.symbols.params().map(|(_, p)| p).any(|p| p.name == "model_is"),
        "dio should have a flattened model_is param, got {:?}",
        m.symbols.params().map(|(_, p)| &p.name).collect::<Vec<_>>()
    );
    assert!(
        body.states.iter().any(|&s| matches!(m.symbols.state(s).kind, piperine_codegen::ir::IrStateKind::Ddt)),
        "dio should have a ddt state var (charge storage)"
    );
}

#[test]
fn ngspice_passives_elaborate_to_ir() {
    let src = "
use piperine::ngspice::passives;
mod Top(inout p: Electrical, inout n: Electrical) {
    r1 : res(p, n);
    c1 : cap(p, n);
    l1 : ind(p, n);
}
";
    let ir = elaborate_ir(src);
    let res = ir.modules.iter().find(|m| m.name == "res").expect("res module");
    let cap = ir.modules.iter().find(|m| m.name == "cap").expect("cap module");
    let ind = ir.modules.iter().find(|m| m.name == "ind").expect("ind module");
    assert!(res.symbols.params().map(|(_, p)| p).any(|p| p.name == "model_rsh"), "res model_rsh param");
    assert!(
        !res.analog.as_ref().unwrap().noise.is_empty(),
        "res should carry thermal noise"
    );
    assert!(
        cap.analog.as_ref().unwrap().states.iter()
            .any(|sv| matches!(cap.symbols.state(*sv).kind, piperine_codegen::ir::IrStateKind::Ddt)),
        "cap should have a ddt state var"
    );
    assert!(ind.analog.is_some());
}

#[test]
fn ngspice_switches_elaborate_to_ir() {
    let src = "
use piperine::ngspice::switches;
mod Top(inout p: Electrical, inout n: Electrical, inout cp: Electrical, inout cn: Electrical) {
    s1 : sw(p, n, cp, cn);
}
";
    let ir = elaborate_ir(src);
    let m = ir.modules.iter().find(|m| m.name == "sw").expect("sw module");
    assert!(m.symbols.vars().map(|(_, v)| v).any(|v| v.name == "sw_state"), "sw should have a persistent sw_state var");
    let body = m.analog.as_ref().expect("sw analog body");
    fn contains_above(stmts: &[piperine_codegen::ir::IrStmt]) -> bool {
        stmts.iter().any(|s| match s {
            piperine_codegen::ir::IrStmt::AnalogEvent(piperine_codegen::ir::IrAnalogEvent { source: piperine_codegen::ir::EventSource::Above { .. }, .. }) => true,
            piperine_codegen::ir::IrStmt::If { then_, else_, .. } => contains_above(then_) || contains_above(else_),
            _ => false,
        })
    }
    assert!(contains_above(&body.stmts), "sw should have an @ above(...) event");
}

#[test]
fn ngspice_sources_elaborate_to_ir() {
    let src = "
use piperine::ngspice::sources;
mod Top(inout p: Electrical, inout n: Electrical) {
    v1 : vsrc(p, n);
    i1 : isrc(p, n);
}
";
    let ir = elaborate_ir(src);
    let vsrc = ir.modules.iter().find(|m| m.name == "vsrc").expect("vsrc module");
    let body = vsrc.analog.as_ref().expect("vsrc analog body");
    // vsrc forces V(p,n) inside `if ($analysis("tran")) {...} else {...}`,
    // then adds an ac_stim contribution — both must reach the IR.
    let has_conditional_force = body.stmts.iter().any(|s| match s {
        piperine_codegen::ir::IrStmt::If { then_, else_, .. } => {
            then_.iter().chain(else_.iter()).any(|s| matches!(s, piperine_codegen::ir::IrStmt::Force { .. }))
        }
        _ => false,
    });
    assert!(has_conditional_force, "vsrc should force V(p,n) under the tran/dc branch");
    let has_ac_stim = body.stmts.iter().any(|s| matches!(
        s,
        piperine_codegen::ir::IrStmt::Contrib { expr: piperine_codegen::ir::IrExpr::AcStim { .. }, .. }
    ));
    assert!(has_ac_stim, "vsrc should contribute an ac_stim term");
    assert!(ir.modules.iter().any(|m| m.name == "isrc"));
}

#[test]
fn ngspice_controlled_elaborate_to_ir() {
    let src = "
use piperine::ngspice::controlled;
mod Top(inout p: Electrical, inout n: Electrical, inout cp: Electrical, inout cn: Electrical) {
    e1 : vcvs(p, n, cp, cn);
    g1 : vccs(p, n, cp, cn);
    h1 : ccvs(p, n, cp, cn);
    f1 : cccs(p, n, cp, cn);
}
";
    let ir = elaborate_ir(src);
    for name in ["vcvs", "vccs", "ccvs", "cccs"] {
        assert!(ir.modules.iter().any(|m| m.name == name), "missing {name}");
    }
    let vcvs = ir.modules.iter().find(|m| m.name == "vcvs").unwrap();
    assert!(
        vcvs.analog.as_ref().unwrap().stmts.iter()
            .any(|s| matches!(s, piperine_codegen::ir::IrStmt::Force { .. })),
        "vcvs should force V(p,n) = gain * V(cp,cn)"
    );
    let ccvs = ir.modules.iter().find(|m| m.name == "ccvs").unwrap();
    let ccvs_body = ccvs.analog.as_ref().unwrap();
    let reads_branch_current = ccvs_body.stmts.iter().any(|s| match s {
        piperine_codegen::ir::IrStmt::Force { expr, .. } => {
            matches!(expr, piperine_codegen::ir::IrExpr::Binary(_, _, r)
                if matches!(**r, piperine_codegen::ir::IrExpr::Branch { .. }))
        }
        _ => false,
    });
    assert!(reads_branch_current, "ccvs should read I(cp,cn) in its force expr");
}

#[test]
fn ngspice_bjt_elaborates_to_ir() {
    let src = "
use piperine::ngspice::bjt;
mod Top(inout c: Electrical, inout b: Electrical, inout e: Electrical, inout sub: Electrical) {
    q1 : bjt(c, b, e, sub);
}
";
    let ir = elaborate_ir(src);
    let m = ir.modules.iter().find(|m| m.name == "bjt").expect("bjt module");
    assert!(m.symbols.params().map(|(_, p)| p).any(|p| p.name == "model_is"), "bjt model_is param");
    assert!(
        !m.analog.as_ref().unwrap().noise.is_empty(),
        "bjt should carry shot noise"
    );
}

#[test]
fn ngspice_jfet_elaborates_to_ir() {
    let src = "
use piperine::ngspice::jfet;
mod Top(inout d: Electrical, inout g: Electrical, inout s: Electrical) {
    j1 : jfet(d, g, s);
}
";
    let ir = elaborate_ir(src);
    assert!(ir.modules.iter().any(|m| m.name == "jfet"));
}

#[test]
fn ngspice_mos_elaborates_to_ir() {
    let src = "
use piperine::ngspice::mos;
mod Top(inout d: Electrical, inout g: Electrical, inout s: Electrical, inout b: Electrical) {
    m1 : mos1(d, g, s, b);
}
";
    let ir = elaborate_ir(src);
    let m = ir.modules.iter().find(|m| m.name == "mos1").expect("mos1 module");
    assert!(m.symbols.params().map(|(_, p)| p).any(|p| p.name == "model_kp"), "mos1 model_kp param");
    let body = m.analog.as_ref().unwrap();
    assert!(
        body.states.iter().any(|&s| matches!(m.symbols.state(s).kind, piperine_codegen::ir::IrStateKind::Ddt)),
        "mos1 should have ddt state vars (gate/junction caps)"
    );
}
