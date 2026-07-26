//! Opvar compilation + ABI bridge (ABI-30 / ABI-31): a JIT-compiled analog
//! device populates `read_opvars` / `list_queries` from a compiled
//! opvar-evaluation function. The kernel compiles one row per non-shadow
//! module `var`, evaluated against the post-solve voltages + state/var
//! banks. Devices without analog vars compile no path (zero overhead).
//!
//! Module `var`s are declared at module scope but assigned in the analog
//! body — the stdlib convention (`var cox = 0.0;` at module, then
//! `cox = …;` in `analog`). The opvar surface reads the value as of the
//! last analog evaluation.

use std::collections::HashMap;

use piperine_lang::parse_and_elaborate;
use piperine_codegen::resolve::LoweredBody;
use piperine_codegen::CircuitCompiler;
use piperine_solver::abi::QueryKind;
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

/// ABI-30: a device with a module `var g` assigned in the analog body
/// compiles an opvar-eval path, and `read_opvars()` returns the post-solve
/// value. Before this feature, the catalog was empty.
#[test]
fn read_opvars_returns_compiled_module_vars() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) {
            param r: Real = 1.0e3;
            var g : Real = 0.0;
        }
        analog R {
            g = 1.0 / r;
            I(p, n) <+ g * V(p, n);
        }
        mod TopR (inout a: Electrical, inout b: Electrical) { R(a, b); }
        ",
        "TopR",
    );
    let dev = &circuit.all_devices()[0];
    let opvars = dev.read_opvars();
    let by_name: std::collections::HashMap<String, f64> = opvars.into_iter().collect();
    // `g` = 1/r = 1/1000 = 1e-3.
    let g = by_name.get("g").copied().unwrap_or_else(|| panic!("missing opvar `g`, got {by_name:?}"));
    assert!(
        (g - 1.0e-3).abs() < 1.0e-12,
        "opvar g should be 1e-3 (1/r for r=1kΩ), got {g}"
    );
}

/// ABI-30 multi-var: a device with several module vars (`g`, `gs`) each
/// assigned in the analog body exposes all of them through `read_opvars()`,
/// each computed from the instance's parameter bank + the body's
/// evaluation order.
#[test]
fn read_opvars_returns_every_module_var() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) {
            param r: Real = 1.0e3;
            param scale: Real = 2.0;
            var g : Real = 0.0;
            var gs : Real = 0.0;
        }
        analog R {
            g = 1.0 / r;
            gs = scale * g;
            I(p, n) <+ gs * V(p, n);
        }
        mod TopR (inout a: Electrical, inout b: Electrical) { R(a, b); }
        ",
        "TopR",
    );
    let dev = &circuit.all_devices()[0];
    let by_name: std::collections::HashMap<String, f64> = dev
        .read_opvars()
        .into_iter()
        .collect();
    let g = by_name.get("g").copied().expect("opvar `g`");
    let gs = by_name.get("gs").copied().expect("opvar `gs`");
    assert!((g - 1.0e-3).abs() < 1.0e-12, "g = 1e-3, got {g}");
    assert!((gs - 2.0e-3).abs() < 1.0e-12, "gs = 2·g = 2e-3, got {gs}");
}

/// ABI-30 zero-overhead: a device with NO module `var` (e.g. a diode-style
/// compact model where every computation is inline) compiles no opvar
/// path. `read_opvars()` returns empty; the catalog is exactly what the
/// device declares.
#[test]
fn device_without_module_vars_compiles_empty_opvar_path() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) { param r: Real = 1.0e3; }
        analog R { I(p, n) <+ V(p, n) / r; }
        mod TopR (inout a: Electrical, inout b: Electrical) { R(a, b); }
        ",
        "TopR",
    );
    let dev = &circuit.all_devices()[0];
    assert!(dev.read_opvars().is_empty(), "no module vars → empty opvars");
    assert!(dev.list_queries().is_empty(), "no module vars → empty queries");
}

/// ABI-31: `list_queries()` returns one `QueryDescriptor` per opvar, typed
/// `OperatingVariable` with the matching name. A host iterating the
/// catalog can pre-render a UI table or scope commands.
#[test]
fn list_queries_returns_typed_opvar_catalog() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) {
            param r: Real = 1.0e3;
            var g : Real = 0.0;
            var v_pn : Real = 0.0;
        }
        analog R {
            g = 1.0 / r;
            v_pn = V(p, n);
            I(p, n) <+ g * v_pn;
        }
        mod TopR (inout a: Electrical, inout b: Electrical) { R(a, b); }
        ",
        "TopR",
    );
    let dev = &circuit.all_devices()[0];
    let queries = dev.list_queries();
    let by_name: std::collections::HashMap<String, QueryKind> = queries
        .into_iter()
        .map(|q| (q.name, q.kind))
        .collect();
    assert_eq!(by_name.get("g"), Some(&QueryKind::OperatingVariable));
    assert_eq!(by_name.get("v_pn"), Some(&QueryKind::OperatingVariable));
}

/// ABI-31 `query(name)` reads through `read_opvars` by default (the
/// `Introspect::query` default impl) — no extra plumbing on the device.
#[test]
fn query_reads_opvar_by_name() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) {
            param r: Real = 500.0;
            var g : Real = 0.0;
        }
        analog R {
            g = 1.0 / r;
            I(p, n) <+ g * V(p, n);
        }
        mod TopR (inout a: Electrical, inout b: Electrical) { R(a, b); }
        ",
        "TopR",
    );
    let dev = &circuit.all_devices()[0];
    let g_value = dev.query("g").expect("opvar `g` is queryable");
    let g_real = g_value.as_real().expect("opvar is real-typed");
    // g = 1/500 = 2e-3.
    assert!((g_real - 2.0e-3).abs() < 1.0e-12, "query(`g`) = {g_real}, want 2e-3");
    assert!(dev.query("nonexistent").is_none(), "unknown opvar returns None");
}

// ── phdl-introspection-attributes PIA-05/07/08 (T4) ────────────────────────
// The opvar-query catalog honors @name/@unit/@description on a `var`; absent
// attributes keep today's bare `QueryDescriptor::opvar(name)` default.

/// PIA-05: `@unit`/`@description` on a var annotate its `QueryDescriptor`.
#[test]
fn query_descriptor_carries_at_unit_and_description() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) {
            param r: Real = 1.0e3;
            @unit(value = \"S\") @description(value = \"transconductance\")
            var g : Real = 0.0;
        }
        analog R {
            g = 1.0 / r;
            I(p, n) <+ g * V(p, n);
        }
        mod TopR (inout a: Electrical, inout b: Electrical) { R(a, b); }
        ",
        "TopR",
    );
    let dev = &circuit.all_devices()[0];
    let q = dev
        .list_queries()
        .into_iter()
        .find(|q| q.name == "g")
        .expect("opvar `g` in query catalog");
    assert_eq!(q.kind, QueryKind::OperatingVariable);
    assert_eq!(q.unit.as_deref(), Some("S"), "@unit must reach QueryDescriptor.unit");
    assert_eq!(q.description.as_deref(), Some("transconductance"), "@description must reach QueryDescriptor.description");
}

/// PIA-07: `@name(value)` on a var is the name surfaced in BOTH the opvar
/// value read and the query catalog — one declaration, consistent catalogs.
/// The kernel id is no longer the surfaced name once `@name` is present, and
/// `query(label)` resolves through the remapped `read_opvars`.
#[test]
fn read_opvars_uses_at_name_label() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) {
            param r: Real = 1.0e3;
            @name(value = \"gm\") var g : Real = 0.0;
        }
        analog R {
            g = 1.0 / r;
            I(p, n) <+ g * V(p, n);
        }
        mod TopR (inout a: Electrical, inout b: Electrical) { R(a, b); }
        ",
        "TopR",
    );
    let dev = &circuit.all_devices()[0];
    // read_opvars surfaces the @name label, not the kernel id `g`.
    let opvars = dev.read_opvars();
    assert!(opvars.iter().any(|(n, _)| n == "gm"), "read_opvars must use the @name label `gm`: {opvars:?}");
    assert!(!opvars.iter().any(|(n, _)| n == "g"), "kernel id `g` must NOT surface once @name is set: {opvars:?}");
    // The value is still 1/r (renaming the label never breaks the value fetch).
    let gm = opvars
        .iter()
        .find(|(n, _)| n == "gm")
        .map(|(_, v)| *v)
        .expect("`gm` value present");
    assert!((gm - 1.0e-3).abs() < 1.0e-12, "gm = 1e-3, got {gm}");
    // query resolves through the remapped read_opvars by the @name label.
    let by_label = dev.query("gm").expect("query(`gm`) resolves by @name label");
    assert!((by_label.as_real().unwrap() - 1.0e-3).abs() < 1.0e-12);
    assert!(dev.query("g").is_none(), "the kernel id `g` is no longer a queryable name once @name is set");
    // The query catalog also uses the @name label (same source — PIA-07).
    let names: Vec<_> = dev.list_queries().into_iter().map(|q| q.name).collect();
    assert!(names.contains(&"gm".to_string()) && !names.contains(&"g".to_string()),
        "query catalog names must match read_opvars: {names:?}");
}

/// PIA-08: a var with no introspection attributes keeps today's default —
/// kernel-id name, no unit, no description (no regression).
#[test]
fn query_descriptor_absent_attrs_keeps_default() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) {
            param r: Real = 1.0e3;
            var g : Real = 0.0;
        }
        analog R {
            g = 1.0 / r;
            I(p, n) <+ g * V(p, n);
        }
        mod TopR (inout a: Electrical, inout b: Electrical) { R(a, b); }
        ",
        "TopR",
    );
    let dev = &circuit.all_devices()[0];
    let q = dev.list_queries().into_iter().find(|q| q.name == "g").expect("opvar `g`");
    assert_eq!(q.kind, QueryKind::OperatingVariable);
    assert!(q.unit.is_none() && q.description.is_none(), "absent attrs → no unit/description");
}
