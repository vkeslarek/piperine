//! Observable catalog + ProbeSelection (ABI-32 / ABI-33): a JIT-compiled
//! device declares its recordable observables — branch currents, state
//! slots, module vars — through `Introspect::list_observables`. The host
//! pairs this with `ProbeSelection` to request a subset for per-step
//! recording.

use std::collections::HashMap;

use piperine_lang::parse_and_elaborate;
use piperine_codegen::resolve::LoweredBody;
use piperine_codegen::CircuitCompiler;
use piperine_solver::abi::ObservableKind;
use piperine_solver::prelude::CircuitInstance;

fn from_ir(design: &piperine_lang::pom::Design, bodies: &HashMap<String, LoweredBody>, top: &str) -> CircuitInstance {
    let mut c = CircuitCompiler::new(design, bodies);
    c.build_circuit(top).expect("circuit compiles")
}

fn build(src: &str, top: &str) -> CircuitInstance {
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("PHDL parses + elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering");
    from_ir(&elab, &bodies, top)
}

/// ABI-32: a reactive device with a state slot declares a `State`
/// observable named after the kernel's state slot catalog entry.
#[test]
fn reactive_device_declares_state_observable() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod C (inout p: Electrical, inout n: Electrical) { param c: Real = 1.0e-9; }
        analog C { I(p, n) <+ c * ddt(V(p, n)); }
        mod TopC (inout a: Electrical, inout b: Electrical) { C(a, b); }
        ",
        "TopC",
    );
    let dev = &circuit.all_devices()[0];
    let observables = dev.list_observables();
    assert!(
        observables.iter().any(|o| o.kind == ObservableKind::State && o.name.starts_with("ddt[")),
        "reactive device declares a `ddt[…]` State observable, got {observables:?}"
    );
}

/// ABI-32: a device with module vars declares `Var` observables, one per
/// var slot. The kernel does not surface source-level var names today, so
/// the bridge synthesizes `var[k]` names — a host requesting `var[0]`
/// against a device with one var sees its value recorded.
#[test]
fn device_with_vars_declares_var_observables() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) {
            param r: Real = 1.0e3;
            var g : Real = 0.0;
            var gs : Real = 0.0;
        }
        analog R {
            g = 1.0 / r;
            gs = 2.0 * g;
            I(p, n) <+ gs * V(p, n);
        }
        mod TopR (inout a: Electrical, inout b: Electrical) { R(a, b); }
        ",
        "TopR",
    );
    let dev = &circuit.all_devices()[0];
    let observables = dev.list_observables();
    let var_obs: Vec<_> = observables.iter().filter(|o| o.kind == ObservableKind::Var).collect();
    assert_eq!(var_obs.len(), 2, "two module vars, got {var_obs:?}");
    assert!(
        var_obs.iter().any(|o| o.name == "var[0]"),
        "first var synthesized as `var[0]`, got {var_obs:?}"
    );
    assert!(
        var_obs.iter().any(|o| o.name == "var[1]"),
        "second var synthesized as `var[1]`, got {var_obs:?}"
    );
}

/// ABI-32: a device with a forced potential carrying a series-R current
/// term declares a `BranchCurrent` observable named `i(<plus>,<minus>)`.
/// A pure voltage source (no series-R) declares no `BranchCurrent`
/// observable (the branch has no current to report).
#[test]
fn device_with_force_current_declares_branch_current_observable() {
    // A Vsrc with `V(p,n) <- dc` plus a series resistance: the kernel
    // surfaces a force terminal pair with a series-R current term.
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod VsrcR (inout p: Electrical, inout n: Electrical) {
            param dc: Real = 1.0;
            param rout: Real = 1.0;
        }
        analog VsrcR {
            V(p, n) <- dc + rout * (0.0 - I(p, n));
        }
        mod TopV (inout a: Electrical, inout b: Electrical) { VsrcR(a, b); }
        ",
        "TopV",
    );
    let dev = &circuit.all_devices()[0];
    let observables = dev.list_observables();
    assert!(
        observables
            .iter()
            .any(|o| o.kind == ObservableKind::BranchCurrent && o.name == "i(p,n)"),
        "force-current device declares a `BranchCurrent` observable `i(p,n)`, got {observables:?}"
    );
}

/// ABI-32: a digital-only device with no analog body declares no
/// observables. A purely digital gate has nothing for an analog
/// `ProbeSelection` to record.
#[test]
fn digital_only_device_declares_no_observables() {
    let circuit = build(
        "
        discipline Bit {
            storage Boolean;
        }
        mod BitDriver(output q : Bit) {
            param level : Real = 0.0;
            var b : Bit = 0;
        }
        digital BitDriver {
            b = level > 0.5;
            q <- b;
        }
        mod TopD(output qout : Bit) { BitDriver(qout); }
        ",
        "TopD",
    );
    // Find the BitDriver (the child device, not the top wrapper).
    let driver = circuit
        .all_devices()
        .iter()
        .find(|d| d.name().contains("BitDriver") || d.list_terminals().iter().any(|t| t.name == "q"))
        .expect("BitDriver device present");
    let observables = driver.list_observables();
    assert!(
        observables.is_empty(),
        "digital-only device declares no observables, got {observables:?}"
    );
}

/// ABI-32: observable costs are present and in [0, 1] — a host budgets
/// recording against simulation cost.
#[test]
fn observable_costs_are_in_unit_range() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod RC (inout p: Electrical, inout n: Electrical) {
            param r: Real = 1.0e3;
            param c: Real = 1.0e-9;
        }
        analog RC {
            I(p, n) <+ V(p, n) / r + c * ddt(V(p, n));
        }
        mod Top (inout a: Electrical, inout b: Electrical) { RC(a, b); }
        ",
        "Top",
    );
    let dev = &circuit.all_devices()[0];
    for o in dev.list_observables() {
        assert!(
            (0.0..=1.0).contains(&o.cost),
            "observable `{}` cost {} not in [0, 1]",
            o.name,
            o.cost
        );
    }
}
