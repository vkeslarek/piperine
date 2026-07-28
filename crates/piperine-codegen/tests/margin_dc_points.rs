//! DC margin evaluation at the final converged solution only (dv-core
//! T18, DVC-11), end to end through compiled PHDL. The acceptance
//! fixture: a Shockley diode whose plain Newton is starved into gmin
//! stepping, carrying `require cond : V(p, n) >= 0.5` — satisfied at the
//! final operating point (~0.63 V) but violated at every homotopy
//! intermediate (the shunt conductance collapses the anode to ~0 V).
//! `strict` must pass: an intermediate-stage evaluation would abort on a
//! solve that is converging correctly (design §C3). The recorded margin
//! proves the flip side: the final solution *was* evaluated, exactly its
//! own value — never an intermediate's.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{CheckPosture, MarginKind};
use piperine_solver::prelude::{CircuitInstance, Context, Policy};

const DISCIPLINE: &str = "discipline Electrical { potential v : Real; flow i : Real; }";

/// The homotopy fixture: 1 V through 1 kΩ into a Shockley diode
/// (is = 1e-14, vt ≈ 25.85 mV). Plain Newton starved at max_iter = 4
/// fails and the plan escalates to gmin stepping. The diode's
/// `require cond : V(p, n) >= 0.5` holds at the final OP
/// (V ≈ vt·ln(I/is) ≈ 0.63 V, margin ≈ +0.13) but is violated at every
/// gmin-stepping stage, where the ramped shunt collapses the anode
/// toward 0 V (margin ≈ −0.5).
fn homotopy_circuit() -> (CircuitInstance, piperine_codegen::device::CircuitBuildInfo) {
    let src = format!(
        "{DISCIPLINE}
mod Vsrc(inout p: Electrical, inout n: Electrical) {{ param dc: Real = 1.0; }}
analog Vsrc {{ V(p, n) <- dc; }}
mod R(inout p: Electrical, inout n: Electrical) {{ param r: Real = 1.0e3; }}
analog R {{ I(p, n) <+ V(p, n) / r; }}
mod D(inout p: Electrical, inout n: Electrical) {{ param is: Real = 1e-14; param vt: Real = 0.025852; }}
analog D {{ I(p, n) <+ is * (exp(V(p, n) / vt) - 1.0); }}
constraint D {{ require cond : V(p, n) >= 0.5; }}
mod Top() {{
    wire gnd : Electrical;
    wire in  : Electrical;
    wire a   : Electrical;
    v1 : Vsrc(.p = in, .n = gnd);
    r1 : R(.p = in, .n = a);
    d1 : D(.p = a, .n = gnd);
}}
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    compiler.build_circuit_mapped("Top").expect("circuit builds")
}

/// Solve the fixture with plain Newton starved into gmin stepping.
fn solve_starved(circuit: &mut CircuitInstance, posture: CheckPosture) -> piperine_solver::prelude::DcAnalysisResult {
    let ctx = Context { checks: posture, ..Default::default() };
    let mut dc = circuit.dc(ctx).unwrap();
    dc.policy = Policy { max_iter: 4, ..Policy::default() };
    dc.solve().expect("gmin stepping converges")
}

/// DVC-11 AC4 (the acceptance test that the rule is real): the fixture
/// requires gmin stepping and its intermediate stages violate `cond`,
/// yet `strict` — which aborts on any evaluated violation — passes,
/// because only the final converged solution is an evaluation point.
#[test]
fn strict_dc_passes_when_only_homotopy_intermediates_violate() {
    let (mut circuit, _) = homotopy_circuit();
    let result = solve_starved(&mut circuit, CheckPosture::Strict);
    assert_eq!(
        result.stats.homotopy_strategy.as_deref(),
        Some("gmin-stepping"),
        "the fixture must actually engage the homotopy"
    );
    assert!(
        result.stats.validation.is_empty(),
        "no findings: the violated intermediates were never evaluated"
    );
}

/// DVC-11: the final solution is still evaluated — `collect` records the
/// require's margin at the converged point (worst = V_op − 0.5 ≥ 0,
/// argmin = the DC coordinate 0.0), never an intermediate's worse value.
#[test]
fn collect_dc_records_the_margin_of_the_final_solution() {
    let (mut circuit, info) = homotopy_circuit();
    let result = solve_starved(&mut circuit, CheckPosture::Collect);
    assert_eq!(
        result.stats.homotopy_strategy.as_deref(),
        Some("gmin-stepping"),
        "the fixture must actually engage the homotopy"
    );

    let anode = info.nets.get("a").expect("net `a`");
    let v_a = result.get_node(anode).expect("the anode node is solved");
    let final_margin = v_a - 0.5;
    assert!(final_margin > 0.0, "the require is satisfied at the final OP (V = {v_a})");

    assert_eq!(result.stats.margins.len(), 1, "one declared margin on the diode");
    let margin = &result.stats.margins[0];
    assert_eq!(margin.label, "cond");
    assert_eq!(margin.kind, MarginKind::Require);
    assert_eq!(margin.instance_path, "d1");
    assert!(
        (margin.worst - final_margin).abs() < 1e-9,
        "the recorded worst is the final solution's margin {final_margin}, got {} — \
         an intermediate evaluation would drag it to ≈ −0.5",
        margin.worst
    );
    assert_eq!(margin.argmin, Some(0.0), "the DC argmin coordinate is 0.0");
    assert!(
        result.stats.validation.is_empty(),
        "satisfied at the final point: no findings recorded"
    );
}

/// The divider from `margin_postures.rs` (0.5 V across a 0.4 V headroom
/// require, violated at every point): `strict` DC now aborts at the
/// converged solution, naming the constraint, the instance path, the
/// coordinate, and the margin value — the DC half of DVC-15 AC1.
#[test]
fn strict_dc_aborts_on_a_violated_require_naming_all_fields() {
    let src = format!(
        "{DISCIPLINE}
mod Vsrc(inout p: Electrical, inout n: Electrical) {{ param dc: Real = 1.0; }}
analog Vsrc {{ V(p, n) <- dc; }}
mod R(inout p: Electrical, inout n: Electrical) {{ param r: Real = 1.0e3; }}
analog R {{ I(p, n) <+ V(p, n) / r; }}
mod Dut(inout p: Electrical, inout n: Electrical) {{ param hi: Real = 0.4; }}
analog Dut {{ I(p, n) <+ V(p, n) / 1e3; }}
constraint Dut {{ require head : V(p, n) <= hi; }}
mod Top() {{
    wire gnd : Electrical;
    wire in  : Electrical;
    wire out : Electrical;
    v1   : Vsrc(.p = in,  .n = gnd);
    r1   : R(.p = in,     .n = out);
    dut1 : Dut(.p = out,  .n = gnd);
}}
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let mut circuit = compiler.build_circuit("Top").expect("circuit builds");

    let err = circuit
        .dc(Context::default())
        .unwrap()
        .solve()
        .expect_err("strict + a violated require fails the DC analysis");
    let msg = err.to_string();
    assert!(msg.contains("head"), "names the constraint: {msg}");
    assert!(msg.contains("dut1"), "names the instance path: {msg}");
    assert!(msg.contains("at t = 0"), "names the DC coordinate: {msg}");
    assert!(
        msg.contains(&format!("margin = {:.6}", 0.4 - 0.5)),
        "names the margin value (−0.1): {msg}"
    );
}
