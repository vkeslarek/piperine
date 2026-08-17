//! `HAS_SENSITIVITY` (dv-gradients T5, DVG-05): the capability bit a
//! PHDL-JIT element declares when it compiled a `∂F/∂p` kernel (DVG-02). The
//! adjoint asks for analytic `param_stamps` only on elements that set it;
//! elements without it fall back to stamp perturbation (DVG-06). Plugin/OSDI
//! default clear.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{Element, ElementCapabilities};
use piperine_solver::prelude::Context;

fn capabilities_of(top: &str, label: &str) -> ElementCapabilities {
    let design = parse_and_elaborate(top, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let mut circuit = compiler.build_circuit("Top").expect("circuit builds");
    // A DC solve is not required — capabilities are structural, declared at
    // build time from the compiled kernel.
    let _ = circuit.dc(Context::default()).map(|mut d| d.solve());
    circuit
        .devices
        .iter()
        .find(|d| d.name() == label)
        .expect("device exists")
        .capabilities()
}

// ── a PHDL device with a ∂F/∂p kernel declares the bit ──────────────────────

#[test]
fn a_phdl_device_with_a_kernel_declares_has_sensitivity() {
    // `g` is a Real restamp-class parameter the kernel differentiates.
    let top = "\
module G(inout electrical p, inout electrical n);
    parameter real g = 1e-3;
    analog I(p, n) <+ g * V(p, n);
endmodule
module Top();
    electrical gnd, a;
    G r (.p(a), .n(gnd));
endmodule
";
    let caps = capabilities_of(top, "r");
    assert!(
        caps.contains(ElementCapabilities::HAS_SENSITIVITY),
        "a PHDL device with a ∂F/∂p kernel declares HAS_SENSITIVITY (got {caps:?})"
    );
}

// ── a PHDL device without a kernel does NOT set it ───────────────────────────

#[test]
fn a_phdl_device_without_a_kernel_does_not_declare_has_sensitivity() {
    // No parameter at all (a literal resistance) → no ∂F/∂p kernel. Being
    // PHDL-JIT is necessary but not sufficient — the bit tracks the kernel,
    // not the lineage.
    let top = "\
module R(inout electrical p, inout electrical n);
    analog I(p, n) <+ V(p, n) / 1e3;
endmodule
module Top();
    electrical gnd, a;
    R x (.p(a), .n(gnd));
endmodule
";
    let caps = capabilities_of(top, "x");
    assert!(
        !caps.contains(ElementCapabilities::HAS_SENSITIVITY),
        "no differentiable param → no kernel → HAS_SENSITIVITY clear (got {caps:?})"
    );

    // Only a discrete parameter → excluded → no kernel → clear.
    let top_int = "\
module N(inout electrical p, inout electrical n);
    parameter integer k = 3;
    analog I(p, n) <+ k * V(p, n);
endmodule
module Top();
    electrical gnd, a;
    N y (.p(a), .n(gnd));
endmodule
";
    let caps_int = capabilities_of(top_int, "y");
    assert!(
        !caps_int.contains(ElementCapabilities::HAS_SENSITIVITY),
        "only an Integer param → no kernel → clear (got {caps_int:?})"
    );
}

// ── plugin / OSDI elements default clear (they never set it) ─────────────────

#[test]
fn a_non_phdl_element_never_declares_has_sensitivity() {
    // A bare AnalogDevice (the shape of a plugin or OSDI element) inherits the
    // default capabilities — HAS_SENSITIVITY is never set by anything but the
    // PHDL device's explicit declaration.
    struct Plugin;
    impl piperine_solver::abi::AnalogDevice for Plugin {}
    impl piperine_solver::abi::DigitalDevice for Plugin {}
    impl piperine_solver::abi::Introspect for Plugin {}
    impl piperine_solver::abi::Element for Plugin {
        fn name(&self) -> &str { "plugin" }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG
        }
    }
    let p = Plugin;
    assert!(
        !p.capabilities().contains(ElementCapabilities::HAS_SENSITIVITY),
        "a plugin/OSDI element defaults clear (got {:?})", p.capabilities()
    );
}
