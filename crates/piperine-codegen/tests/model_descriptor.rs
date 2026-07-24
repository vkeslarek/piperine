//! Model descriptor + kernel named catalogs (ABI-46 / ABI-47 / ABI-48): a
//! JIT-compiled device surfaces its model identity (`type_id`, `version`)
//! and the kernel's named runtime-state / force-terminal / noise-terminal
//! catalogs through the introspection ABI. A host queries "what can this
//! device report?" through one uniform surface (terminals + opvars + state
//! slots + force/noise terminal names).

use std::collections::HashMap;

use piperine_lang::parse_and_elaborate;
use piperine_codegen::resolve::LoweredBody;
use piperine_codegen::CircuitCompiler;
use piperine_solver::abi::Introspect;
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

/// ABI-46: a JIT-compiled device's `model_descriptor()` returns the kernel
/// module name as `type_id`. Version is empty (no version declaration in
/// the language today). A device author pre-rendering a UI distinguishes
/// `"RCap"` from `"Bjt"` without name-matching the instance label.
#[test]
fn model_descriptor_carries_kernel_module_name() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod RCap (inout p: Electrical, inout n: Electrical) {
            param r: Real = 1.0e3;
            param c: Real = 1.0e-9;
        }
        analog RCap {
            I(p, n) <+ V(p, n) / r + c * ddt(V(p, n));
        }
        mod Top (inout a: Electrical, inout b: Electrical) { RCap(a, b); }
        ",
        "Top",
    );
    let dev = &circuit.all_devices()[0];
    let descriptor = dev.model_descriptor();
    assert_eq!(descriptor.type_id, "RCap", "type_id is the kernel module name");
    assert_eq!(descriptor.version, "", "no version declaration → empty string");
}

/// PIA-01: an author-declared `@model(type, version)` on a module populates
/// `ModelDescriptor` from the attribute (not the module-name echo). The full
/// parse→POM→codegen→ABI path: the sidecar resolved by `CircuitCompiler`
/// reaches `model_descriptor()`.
#[test]
fn model_descriptor_reads_at_model_attribute() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        @model(type = \"mos\", version = \"3\")
        mod Mos (inout d: Electrical, inout s: Electrical) {
            param r: Real = 1.0e3;
        }
        analog Mos {
            I(d, s) <+ V(d, s) / r;
        }
        mod Top (inout a: Electrical, inout b: Electrical) { Mos(a, b); }
        ",
        "Top",
    );
    let dev = &circuit.all_devices()[0];
    let descriptor = dev.model_descriptor();
    assert_eq!(descriptor.type_id, "mos", "@model type populates type_id, not the module name");
    assert_eq!(descriptor.version, "3", "@model version populates the version field");
    // The module name ("Mos") must NOT leak into type_id when @model is present.
    assert_ne!(descriptor.type_id, "Mos", "module-name echo must be overridden by @model");
}

/// PIA-01 negative placement: `@model` on a param fails loud at circuit build
/// (the sidecar resolver rejects the misplacement, surfacing as a
/// `CodegenError::Invalid`).
#[test]
fn at_model_on_param_fails_loud_at_build() {
    let elab = parse_and_elaborate(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) {
            @model(type = \"r\", version = \"1\") param r: Real = 1.0e3;
        }
        analog R {
            I(p, n) <+ V(p, n) / r;
        }
        mod Top (inout a: Electrical, inout b: Electrical) { R(a, b); }
        ",
        &piperine_lang::SourceMap::dummy(),
    )
    .expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering");
    let err = CircuitCompiler::new(&elab, &bodies)
        .build_circuit("Top")
        .err()
        .expect("@model on a param must fail loud at build");
    let msg = err.to_string();
    assert!(msg.contains("model"), "error should name the misplaced schema: {msg}");
}

/// ABI-46 default: a host-built Element with no kernel (the composed
/// surface's Resistor test device) returns the empty descriptor — a host
/// falls back to the instance name in this case.
#[test]
fn model_descriptor_default_is_empty() {
    // A device with no compiled kernel has nothing to declare. Verify the
    // default `Introspect::model_descriptor` returns the empty sentinel
    // for any Element that doesn't override it.
    struct Stateless;
    impl piperine_solver::abi::AnalogDevice for Stateless {}
    impl piperine_solver::abi::DigitalDevice for Stateless {}
    impl piperine_solver::abi::Introspect for Stateless {}
    impl piperine_solver::abi::Element for Stateless {
        fn name(&self) -> &str { "stateless" }
        fn capabilities(&self) -> piperine_solver::abi::ElementCapabilities {
            piperine_solver::abi::ElementCapabilities::empty()
        }
    }
    let dev = Stateless;
    let descriptor = dev.model_descriptor();
    assert_eq!(descriptor.type_id, "");
    assert_eq!(descriptor.version, "");
}

/// ABI-47: a reactive device's `list_state_slot_names()` returns one entry
/// per state slot. A capacitor (`ddt(V)`) has a `ddt` state; an `RC` with
/// a capacitor surfaces a non-empty catalog with the kind name in it.
#[test]
fn state_slot_names_catalog_is_non_empty_for_reactive_device() {
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
    let slots = dev.list_state_slot_names();
    assert!(!slots.is_empty(), "reactive device has ≥1 state slot, got {slots:?}");
    // The slot is a `ddt` charge state; its name includes the kind.
    assert!(
        slots.iter().any(|s| s.starts_with("ddt[")),
        "expected a `ddt[…]` state slot name, got {slots:?}"
    );
}

/// ABI-47: a device with `$limit` junction limiters surfaces the trailing
/// `vold` slots in the state catalog. A diode-like device with one limit
/// has one `"vold[0]"` entry appended after the runtime states.
#[test]
fn state_slot_names_includes_vold_for_limiter() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod Diode (inout p: Electrical, inout n: Electrical) {
            param is: Real = 1.0e-15;
            param vt: Real = 0.026;
        }
        analog Diode {
            var vd : Real = 0.0;
            vd = $limit(\"pnjlim\", V(p, n), 0.0, 0.7, 0.7);
            I(p, n) <+ is * (exp(vd / vt) - 1.0);
        }
        mod TopD (inout a: Electrical, inout b: Electrical) { Diode(a, b); }
        ",
        "TopD",
    );
    let dev = &circuit.all_devices()[0];
    let slots = dev.list_state_slot_names();
    assert!(
        slots.iter().any(|s| s.starts_with("vold[")),
        "limiter device surfaces a `vold[…]` slot, got {slots:?}"
    );
}

/// ABI-47: a device with a forced potential (`V(p, n) <- …`) surfaces the
/// force branch terminal pair in its named catalog. The plus/minus names
/// are looked up from the kernel's symbol table.
#[test]
fn force_terminal_pairs_catalog_surfaces_branch_names() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod Vsrc (inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; }
        analog Vsrc { V(p, n) <- dc; }
        mod TopV (inout a: Electrical, inout b: Electrical) { Vsrc(a, b); }
        ",
        "TopV",
    );
    let dev = &circuit.all_devices()[0];
    let pairs = dev.list_force_terminal_pairs();
    assert_eq!(pairs.len(), 1, "one force branch, got {pairs:?}");
    assert_eq!(pairs[0], ("p".to_string(), "n".to_string()));
}

/// ABI-47: a device with a noise contribution surfaces the noise source's
/// `(plus, minus)` terminal pair as a named entry. A resistor with thermal
/// noise emits one pair.
#[test]
fn noise_terminal_pairs_catalog_surfaces_branch_names() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) {
            param r: Real = 1.0e3;
        }
        analog R {
            I(p, n) <+ V(p, n) / r;
            I(p, n) <+ white_noise(1.0e-24);
        }
        mod TopR (inout a: Electrical, inout b: Electrical) { R(a, b); }
        ",
        "TopR",
    );
    let dev = &circuit.all_devices()[0];
    let pairs = dev.list_noise_terminal_pairs();
    assert_eq!(pairs.len(), 1, "one noise source, got {pairs:?}");
    assert_eq!(pairs[0], ("p".to_string(), "n".to_string()));
}

/// ABI-48 unified query: a host calling the full introspection surface on
/// one device gets terminals (P6) + opvars (P6) + state slots (P10) +
/// force/noise terminal names (P10) — all named, all through one trait.
/// Sanity check that the catalogs together form a coherent surface.
#[test]
fn unified_report_surface_is_non_empty_for_real_device() {
    let circuit = build(
        "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod RC (inout p: Electrical, inout n: Electrical) {
            param r: Real = 1.0e3;
            param c: Real = 1.0e-9;
            var g : Real = 0.0;
        }
        analog RC {
            g = 1.0 / r;
            I(p, n) <+ g * V(p, n) + c * ddt(V(p, n));
        }
        mod Top (inout a: Electrical, inout b: Electrical) { RC(a, b); }
        ",
        "Top",
    );
    let dev = &circuit.all_devices()[0];
    // Every catalog is non-empty where the device has the data: 2
    // terminals (p/n), 1 opvar (g), ≥1 state slot (ddt), no forces, no
    // noise. The "unified report" is the union a host iterates.
    assert!(!dev.list_terminals().is_empty(), "terminals");
    assert!(!dev.read_opvars().is_empty(), "opvars");
    assert!(!dev.list_state_slot_names().is_empty(), "state slots");
    // Force/noise catalogs are legitimately empty for this device.
    assert!(dev.list_force_terminal_pairs().is_empty());
    assert!(dev.list_noise_terminal_pairs().is_empty());
    // Identity carries the kernel module name.
    assert_eq!(dev.model_descriptor().type_id, "RC");
}
