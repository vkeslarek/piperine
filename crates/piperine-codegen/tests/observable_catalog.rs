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
        module C (inout electrical p, inout electrical n);
            parameter real c = 1.0e-9;
            analog I(p, n) <+ c * ddt(V(p, n));
        endmodule
        module TopC (inout electrical a, inout electrical b);
            C x1 (a, b);
        endmodule
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
/// var — vars computed in the analog body are opvars and surface by their
/// authored name (`g`, `gs`), never duplicated as a `var[k]` bank slot.
#[test]
fn device_with_vars_declares_var_observables() {
    let circuit = build(
        "
        module R (inout electrical p, inout electrical n);
            parameter real r = 1.0e3;
            real g, gs;
            analog begin
                g = 1.0 / r;
                gs = 2.0 * g;
                I(p, n) <+ gs * V(p, n);
            end
        endmodule
        module TopR (inout electrical a, inout electrical b);
            R x1 (a, b);
        endmodule
        ",
        "TopR",
    );
    let dev = &circuit.all_devices()[0];
    let observables = dev.list_observables();
    let var_obs: Vec<_> = observables.iter().filter(|o| o.kind == ObservableKind::Var).collect();
    assert_eq!(var_obs.len(), 2, "two module vars, got {var_obs:?}");
    assert!(
        var_obs.iter().any(|o| o.name == "g"),
        "first var by its authored name, got {var_obs:?}"
    );
    assert!(
        var_obs.iter().any(|o| o.name == "gs"),
        "second var by its authored name, got {var_obs:?}"
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
        module VsrcR (inout electrical p, inout electrical n);
            parameter real dc = 1.0;
            parameter real rout = 1.0;
            analog V(p, n) <+ dc + rout * (0.0 - I(p, n));
        endmodule
        module TopV (inout electrical a, inout electrical b);
            VsrcR x1 (a, b);
        endmodule
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
        discipline Bit;
            domain discrete;
        enddiscipline
        module BitDriver(output Bit q);
            parameter real level = 0.0;
            integer b;
            always @(*) begin
                b = level > 0.5;
                q = b;
            end
        endmodule
        module TopD(output Bit qout);
            BitDriver x1 (qout);
        endmodule
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
        module RC (inout electrical p, inout electrical n);
            parameter real r = 1.0e3;
            parameter real c = 1.0e-9;
            analog I(p, n) <+ V(p, n) / r + c * ddt(V(p, n));
        endmodule
        module Top (inout electrical a, inout electrical b);
            RC x1 (a, b);
        endmodule
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

// ── phdl-introspection-attributes PIA-06/07/09 (T5) ────────────────────────
// A var carrying @name/@kind surfaces in the observable catalog by that name
// and kind (NOT positional var[k]); the SAME @name feeds the opvar-query
// catalog too (one declaration, both catalogs).

// `observable_named_and_kinded_by_at_name_and_at_kind` /
// `at_name_feeds_both_query_and_observable_catalogs` were
// dropped with the PHDL frontend: Verilog-AMS attribute instances `(* … *)`
// reach no introspection schema (only `device`/`port` are seeded, and
// `lower_port_attrs` materialises only `port`), so nothing in VAMS source can
// produce this metadata — the frontend-dark call ed575c85 already made for
// `introspection_meta`/`introspection_attrs`.

