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
        module R (inout electrical p, inout electrical n);
            parameter real r = 1.0e3;
            real g;
            analog begin
                g = 1.0 / r;
                I(p, n) <+ g * V(p, n);
            end
        endmodule
        module TopR (inout electrical a, inout electrical b);
            R x1 (a, b);
        endmodule
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
        module R (inout electrical p, inout electrical n);
            parameter real r = 1.0e3;
            parameter real scale = 2.0;
            real g, gs;
            analog begin
                g = 1.0 / r;
                gs = scale * g;
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
        module R (inout electrical p, inout electrical n);
            parameter real r = 1.0e3;
            analog I(p, n) <+ V(p, n) / r;
        endmodule
        module TopR (inout electrical a, inout electrical b);
            R x1 (a, b);
        endmodule
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
        module R (inout electrical p, inout electrical n);
            parameter real r = 1.0e3;
            real g, v_pn;
            analog begin
                g = 1.0 / r;
                v_pn = V(p, n);
                I(p, n) <+ g * v_pn;
            end
        endmodule
        module TopR (inout electrical a, inout electrical b);
            R x1 (a, b);
        endmodule
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
        module R (inout electrical p, inout electrical n);
            parameter real r = 500.0;
            real g;
            analog begin
                g = 1.0 / r;
                I(p, n) <+ g * V(p, n);
            end
        endmodule
        module TopR (inout electrical a, inout electrical b);
            R x1 (a, b);
        endmodule
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

// `query_descriptor_carries_at_unit_and_description` /
// `read_opvars_uses_at_name_label` were
// dropped with the PHDL frontend: Verilog-AMS attribute instances `(* … *)`
// reach no introspection schema (only `device`/`port` are seeded, and
// `lower_port_attrs` materialises only `port`), so nothing in VAMS source can
// produce this metadata — the frontend-dark call ed575c85 already made for
// `introspection_meta`/`introspection_attrs`.

/// PIA-08: a var with no introspection attributes keeps today's default —
/// kernel-id name, no unit, no description (no regression).
#[test]
fn query_descriptor_absent_attrs_keeps_default() {
    let circuit = build(
        "
        module R (inout electrical p, inout electrical n);
            parameter real r = 1.0e3;
            real g;
            analog begin
                g = 1.0 / r;
                I(p, n) <+ g * V(p, n);
            end
        endmodule
        module TopR (inout electrical a, inout electrical b);
            R x1 (a, b);
        endmodule
        ",
        "TopR",
    );
    let dev = &circuit.all_devices()[0];
    let q = dev.list_queries().into_iter().find(|q| q.name == "g").expect("opvar `g`");
    assert_eq!(q.kind, QueryKind::OperatingVariable);
    assert!(q.unit.is_none() && q.description.is_none(), "absent attrs → no unit/description");
}
