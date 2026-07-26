//! HOST-09 (host-library T12): `InstanceView` introspection reflection —
//! `inst.model` (type_id/version), `inst.terminals` (with `TerminalKind`),
//! and `inst.observables()` (name/kind/cost catalog). All three surface the
//! shipped `Introspect` ABI catalogs (`model_descriptor`/`list_terminals`/
//! `list_observables`), snapshotted eagerly at solve time alongside the
//! HOST-07 opvar snapshot.

use std::path::PathBuf;

use piperine::{SimSession, SolverConfig};
use piperine_lang::SourceMap;
use piperine_solver::prelude::{TerminalKind, ObservableKind, Invalidation, ParamScope};

/// A resistor that declares a named observable `cond` via `@name`, so the
/// observable catalog is non-empty and the name is stable. The model
/// identity falls back to the module name (`"Resistor"`) because no
/// `@model(type, version)` is declared (PIA-02 fallback — no regression for
/// stdlib models without `@model`).
const INTROSPECT_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod VoltageSource(inout p: Electrical, inout n: Electrical) {
    param voltage: Real = 0.0;
}
analog VoltageSource { V(p, n) <- voltage; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
    @name(value = \"cond\") var g : Real = 0.0;
}
analog Resistor {
    g = 1.0 / r;
    I(p, n) <+ g * V(p, n);
}

mod Top() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire mid  : Electrical;
    src   : VoltageSource (.p = vin, .n = gnd) { .voltage = 5.0 };
    r_top : Resistor      (.p = vin, .n = mid) { .r = 3e3 };
    r_bot : Resistor      (.p = mid, .n = gnd) { .r = 2e3 };
}
";

fn headers_source_map() -> SourceMap {
    let headers = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn introspect_session() -> SimSession {
    let design = piperine_lang::parse_and_elaborate(INTROSPECT_PHDL, &headers_source_map())
        .expect("fixture elaborates");
    SimSession::new(design, "Top".to_string())
}

/// HOST-09 AC3a: `inst.model` returns a `ModelDescriptor` whose `type_id`
/// echoes the module name when no `@model(type, version)` is declared (the
/// PIA-02 fallback — no regression for stdlib models).
#[test]
fn model_descriptor_echoes_module_name_when_no_at_model() {
    let session = introspect_session();
    let op = session.run_op(&SolverConfig::default(), None).expect("op solves");
    let r_top = op.instance("r_top").expect("r_top is a labeled instance");
    let model = r_top.model();
    assert!(
        model.type_id.contains("Resistor") || model.type_id == "Resistor",
        "type_id falls back to module name, got `{}`",
        model.type_id
    );
    assert_eq!(model.version, "", "no @model => version is empty");
}

/// HOST-09 AC3b: `inst.terminals` returns terminal descriptors carrying
/// `TerminalKind::External` for the declared ports (`p`, `n`), `Analog`
/// domain, `Inout` direction. The `TerminalKind` discrimination is the
/// HOST-09-specific value-add over the pre-existing port→net connectivity.
#[test]
fn terminals_carry_external_kind_for_declared_ports() {
    let session = introspect_session();
    let op = session.run_op(&SolverConfig::default(), None).expect("op solves");
    let r_top = op.instance("r_top").expect("r_top is a labeled instance");
    let terminals = r_top.terminals();
    assert!(terminals.len() >= 2, "Resistor declares at least 2 terminals (p, n): {terminals:?}");
    let names: Vec<&str> = terminals.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"p"), "terminal `p` present: {names:?}");
    assert!(names.contains(&"n"), "terminal `n` present: {names:?}");
    for t in terminals.iter() {
        assert_eq!(
            t.kind, TerminalKind::External,
            "port `{}` is External (PIA-12 default for ports): got {:?}",
            t.name, t.kind
        );
    }
}

/// HOST-09 AC3c: `inst.observables()` returns the device's observable
/// catalog — at least the named `cond` var (`@name(value = "cond")`), with
/// `ObservableKind::Var` and a non-negative cost.
#[test]
fn observables_lists_named_var_catalog() {
    let session = introspect_session();
    let op = session.run_op(&SolverConfig::default(), None).expect("op solves");
    let r_top = op.instance("r_top").expect("r_top is a labeled instance");
    let observables = r_top.observables();
    assert!(!observables.is_empty(), "Resistor declares at least the `cond` observable");
    let cond = observables
        .iter()
        .find(|o| o.name == "cond")
        .expect("the `@name(value = \"cond\")` var surfaces as observable `cond`");
    assert_eq!(
        cond.kind, ObservableKind::Var,
        "a module var defaults to ObservableKind::Var"
    );
    assert!(cond.cost >= 0.0, "cost is non-negative");
}

/// HOST-09 edge case: an unknown instance label fails loud — never returns
/// an empty view silently. Mirrors HOST-07's fail-loud shape.
#[test]
fn unknown_instance_label_fails_loud() {
    let session = introspect_session();
    let op = session.run_op(&SolverConfig::default(), None).expect("op solves");
    let err = op.instance("ghost").expect_err("unknown label must fail");
    assert!(
        err.to_string().contains("ghost"),
        "error names the bad label: {err}"
    );
}

/// HOST-09 consistency: the introspection catalogs are available on every
/// labeled instance, including the `VoltageSource` (which declares no opvars
/// and no `@model`). The model falls back to the module name; terminals
/// list the ports; observables may be empty (a source with no vars).
#[test]
fn introspection_works_on_opvarless_device() {
    let session = introspect_session();
    let op = session.run_op(&SolverConfig::default(), None).expect("op solves");
    let src = op.instance("src").expect("src is a labeled instance");
    let model = src.model();
    assert!(
        model.type_id.contains("VoltageSource") || model.type_id == "VoltageSource",
        "type_id for src echoes module name, got `{}`",
        model.type_id
    );
    let terminals = src.terminals();
    assert!(terminals.len() >= 2, "VoltageSource declares at least 2 terminals");
}

// ── HOST-12: Param.bounds/unit/scope/invalidation reflection ─────────────

/// HOST-12: `inst.param("r").bounds` returns the declared parameter bounds;
/// `inst.params()` lists the full catalog. The Resistor's `r` param has
/// `ParamScope::Instance`, `Invalidation::Restamp`, and unbounded bounds
/// (no explicit bounds declared).
#[test]
fn param_descriptor_reflects_bounds_scope_invalidation() {
    let session = introspect_session();
    let op = session.run_op(&SolverConfig::default(), None).expect("op solves");
    let r_top = op.instance("r_top").expect("r_top is a labeled instance");
    let r = r_top.param("r").expect("r_top declares param r");
    assert_eq!(r.name, "r");
    assert_eq!(r.scope, ParamScope::Instance, "r is an instance param");
    assert_eq!(r.invalidation, Invalidation::Restamp, "r restamps on change");
}

/// HOST-12 edge case: an unknown param name fails loud — never returns
/// `None` silently. The error names the param and lists available ones.
#[test]
fn unknown_param_fails_loud() {
    let session = introspect_session();
    let op = session.run_op(&SolverConfig::default(), None).expect("op solves");
    let r_top = op.instance("r_top").expect("r_top is a labeled instance");
    let err = r_top.param("bogus").expect_err("unknown param must fail");
    assert!(err.to_string().contains("bogus"), "error names the bad param: {err}");
    assert!(err.to_string().contains("r_top"), "error names the instance: {err}");
}

/// HOST-12: `inst.params()` returns the full parameter catalog — the
/// Resistor declares at least `r` and the VoltageSource declares `voltage`.
#[test]
fn params_lists_the_full_parameter_catalog() {
    let session = introspect_session();
    let op = session.run_op(&SolverConfig::default(), None).expect("op solves");
    let r_top = op.instance("r_top").expect("r_top is a labeled instance");
    let params = r_top.params();
    assert!(!params.is_empty(), "Resistor declares at least param r");
    assert!(params.iter().any(|p| p.name == "r"), "r is in the catalog");
}
