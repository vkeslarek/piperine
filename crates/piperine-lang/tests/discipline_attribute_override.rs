//! LRM-COVERS: 3.6.2.5
//!
//! lrm-conformance-wave3 T23: a `discipline` overrides a bound nature's
//! attribute (`flow.abstol = 10u;`, LRM §3.6.2.5's own worked example) —
//! parses, survives to `DisciplineDecl::attr_overrides`, and is
//! *observable*: [`piperine_lang::pom::nature::apply_discipline_overrides`]
//! folds the override onto the bound nature, and feeding that value into
//! `piperine_solver::prelude::Tolerances::node_vntol_overrides` (the
//! §6.5.8 carrying mechanism wave-2's T48 built and proved discriminating
//! in `crates/piperine-solver/tests/nature_abstol_inheritance.rs`)
//! observably changes whether a Newton step converges — never merely
//! stored.
//!
//! §3.6.1.2 restricts which attributes a discipline may override:
//! `access`/`units` are illegal to change from any binding (base or
//! derived nature, or a discipline's own override) — enforced at parse
//! time here, citing both clauses.

use std::collections::HashMap;
use std::rc::Rc;

use piperine_lang::pom::nature::apply_discipline_overrides;
use piperine_lang::{parse_and_elaborate, SourceMap};

use piperine_solver::prelude::{NodeIdentifier, Netlist, Tolerances};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

/// LRM §3.6.2.5's own worked example, with numbers chosen so the override
/// is a genuine *tightening* relative to the flow nature's own base
/// `abstol` (the LRM's literal 1u -> 10u is a loosening, which would not
/// demonstrate a step going from accepted to rejected).
const FIXTURE: &str = r#"
    nature ttl_volt;
        units = "V";
        access = V;
        abstol = 100u;
    endnature
    nature ttl_curr;
        units = "A";
        access = I;
        abstol = 1e-3;
    endnature
    discipline ttl;
        potential ttl_volt;
        flow ttl_curr;
        flow.abstol = 1e-9;
    enddiscipline
    module m (inout ttl p);
    endmodule
"#;

#[test]
fn override_survives_to_discipline_decl_attr_overrides() {
    let d = elab(FIXTURE);
    let disc = d.discipline("ttl").expect("discipline `ttl` elaborated");
    assert_eq!(disc.attr_overrides.len(), 1, "exactly one override declared");
    let (slot, attr, _expr) = &disc.attr_overrides[0];
    assert_eq!(*slot, piperine_lang::ast::NatureSlot::Flow);
    assert_eq!(attr, "abstol");
}

/// The override, applied on top of the bound `ttl_curr` nature, replaces
/// its `abstol` (1e-3) with the discipline's own value (1e-9) — not
/// merely stored, but foldable into an effective nature.
#[test]
fn override_replaces_the_bound_natures_abstol() {
    let d = elab(FIXTURE);
    let disc = d.discipline("ttl").expect("discipline elaborated");
    let base = d.nature("ttl_curr").expect("ttl_curr nature elaborated");
    assert_eq!(base.abstol(), Some(1e-3), "base nature keeps its own declared abstol");

    let effective =
        apply_discipline_overrides(base, disc, piperine_lang::ast::NatureSlot::Flow).expect("override const-evaluates");
    assert_eq!(effective.abstol(), Some(1e-9), "discipline override wins over the bound nature's own abstol");
}

/// AC1 (T23 Done-when): the override reaches the solver and is
/// observable — a Newton step that converges under the flow nature's own
/// (loose) `abstol` fails to converge once fed the discipline-tightened
/// value, via the exact `node_vntol_overrides` mechanism T48 built.
/// Mirrors `nature_abstol_inheritance.rs`'s own accept/reject pair, but
/// the tightened value comes from *this test's own elaboration*, not a
/// hardcoded literal — proving the override, not merely the mechanism.
#[test]
fn tightened_override_changes_convergence_acceptance() {
    let d = elab(FIXTURE);
    let disc = d.discipline("ttl").expect("discipline elaborated");
    let base = d.nature("ttl_curr").expect("ttl_curr nature elaborated");
    let effective =
        apply_discipline_overrides(base, disc, piperine_lang::ast::NatureSlot::Flow).expect("override const-evaluates");
    let overridden_abstol = effective.abstol().expect("override sets abstol");

    let mut netlist = Netlist::new();
    let _n = netlist.connect_node(NodeIdentifier::Anonymous(0));
    let old = ndarray::Array1::from(vec![0.0]);
    let new = ndarray::Array1::from(vec![5e-7]); // +5e-7, same gap as T48's own fixture

    // Baseline: the flow nature's own (loose, 1e-3) abstol accepts a
    // 5e-7 step under the default `vntol` (1e-6) with no override at all.
    let base_tol = Tolerances::default();
    assert!(
        base_tol.has_converged(Some(old.view()), &new.view(), &netlist),
        "5e-7 step must pass under the default vntol with no override in force"
    );

    // The discipline's own tightened override (1e-9), read straight from
    // the elaborated design, rejects the identical step.
    let mut overrides = HashMap::new();
    overrides.insert(NodeIdentifier::Anonymous(0), overridden_abstol);
    let tightened_tol = Tolerances { node_vntol_overrides: Rc::new(overrides), ..Tolerances::default() };
    assert!(
        !tightened_tol.has_converged(Some(old.view()), &new.view(), &netlist),
        "the same 5e-7 step must fail once fed the discipline-tightened abstol (1e-9)"
    );
}

/// §3.6.1.2's restriction, cited by §3.6.2.5: `access` may never be
/// overridden, from a discipline or otherwise.
#[test]
fn overriding_access_fails_loud() {
    let msg = elab_err(
        r#"
        nature ttl_curr;
            units = "A";
            access = I;
            abstol = 1u;
        endnature
        discipline ttl;
            flow ttl_curr;
            flow.access = V;
        enddiscipline
        module m (inout ttl p);
        endmodule
        "#,
    );
    assert!(msg.contains("access"), "error must name the forbidden attribute, got: {msg}");
    assert!(msg.contains("3.6.1.2"), "error must cite §3.6.1.2, got: {msg}");
}

/// §3.6.1.2's restriction on `units`, from the `potential` side.
#[test]
fn overriding_units_fails_loud() {
    let msg = elab_err(
        r#"
        nature ttl_volt;
            units = "V";
            access = V;
            abstol = 1u;
        endnature
        discipline ttl;
            potential ttl_volt;
            potential.units = "kV";
        enddiscipline
        module m (inout ttl p);
        endmodule
        "#,
    );
    assert!(msg.contains("units"), "error must name the forbidden attribute, got: {msg}");
    assert!(msg.contains("3.6.1.2"), "error must cite §3.6.1.2, got: {msg}");
}
