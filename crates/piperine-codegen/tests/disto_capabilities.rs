//! Codegen disto capability declaration (ABI-26): a JIT-compiled
//! `PiperineDevice` sets `HAS_DISTO2`/`HAS_DISTO3` on its
//! `ElementCapabilities` exactly when its `AnalogKernel` compiled the
//! matching analytic derivative kernel (symbolic differentiation). The
//! `.disto` driver reads these bits to decide whether a device contributes
//! nonlinear currents (DISTO-03).

use std::collections::HashMap;

use piperine_lang::parse_and_elaborate;
use piperine_codegen::resolve::LoweredBody;
use piperine_codegen::CircuitCompiler;
use piperine_solver::abi::ElementCapabilities;
use piperine_solver::prelude::CircuitInstance;

fn from_ir(design: &piperine_lang::pom::Design, bodies: &HashMap<String, LoweredBody>, top: &str) -> CircuitInstance {
    let mut c = CircuitCompiler::new(design, bodies);
    c.build_circuit(top).expect("circuit compiles")
}

/// Compile `src` and build the named top-module circuit.
fn build(src: &str, top: &str) -> CircuitInstance {
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("PHDL parses + elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering");
    from_ir(&elab, &bodies, top)
}

/// ABI-26: a device with both `v²` and `v³` nonlinear contributions compiles
/// disto2 AND disto3 kernels, so its capabilities declare both bits (the
/// MOSFET / polynomial-VCCS case). A plain resistor compiles neither.
#[test]
fn nonlinear_device_declares_disto2_and_disto3_resistor_neither() {
    let nonlinear = build(
        "
        module Poly (inout electrical p, inout electrical n);
            parameter real g1 = 0.1;
            parameter real g2 = 0.02;
            parameter real g3 = 0.003;
            analog I(p, n) <+ g1*V(p,n) + g2*V(p,n)*V(p,n) + g3*V(p,n)*V(p,n)*V(p,n);
        endmodule
        module Top (inout electrical a, inout electrical b);
            Poly x1 (a, b);
        endmodule
        ",
        "Top",
    );
    let caps = nonlinear.all_devices()[0].capabilities();
    assert!(
        caps.contains(ElementCapabilities::HAS_DISTO2),
        "nonlinear device (v² term) must declare HAS_DISTO2, got {caps:?}"
    );
    assert!(
        caps.contains(ElementCapabilities::HAS_DISTO3),
        "nonlinear device (v³ term) must declare HAS_DISTO3, got {caps:?}"
    );

    let linear = build(
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
    let caps = linear.all_devices()[0].capabilities();
    assert!(
        !caps.contains(ElementCapabilities::HAS_DISTO2),
        "linear resistor must NOT declare HAS_DISTO2, got {caps:?}"
    );
    assert!(
        !caps.contains(ElementCapabilities::HAS_DISTO3),
        "linear resistor must NOT declare HAS_DISTO3, got {caps:?}"
    );
}

/// ABI-26 edge case: a device with only a `v²` contribution (quadratic, no
/// cubic term) compiles disto2 but NOT disto3 — the two bits are
/// independent, matching the kernel's `has_disto2`/`has_disto3` accessors.
#[test]
fn quadratic_device_declares_disto2_only() {
    let quad = build(
        "
        module Quad (inout electrical p, inout electrical n);
            parameter real g1 = 0.1;
            parameter real g2 = 0.02;
            analog I(p, n) <+ g1*V(p,n) + g2*V(p,n)*V(p,n);
        endmodule
        module TopQ (inout electrical a, inout electrical b);
            Quad x1 (a, b);
        endmodule
        ",
        "TopQ",
    );
    let caps = quad.all_devices()[0].capabilities();
    assert!(
        caps.contains(ElementCapabilities::HAS_DISTO2),
        "quadratic device (v² term) must declare HAS_DISTO2, got {caps:?}"
    );
    assert!(
        !caps.contains(ElementCapabilities::HAS_DISTO3),
        "quadratic device (no v³ term) must NOT declare HAS_DISTO3, got {caps:?}"
    );
}

/// ABI-26: a JIT device never declares `NUMERIC_JACOBIAN` — its Jacobian is
/// always the product of symbolic differentiation (analytic). That bit is
/// reserved for finite-difference plugins; the `.disto` driver fail-louds
/// on it (ABI-25, wired in the solver).
#[test]
fn jit_device_never_declares_numeric_jacobian() {
    let nonlinear = build(
        "
        module Poly (inout electrical p, inout electrical n);
            parameter real g1 = 0.1;
            parameter real g2 = 0.02;
            parameter real g3 = 0.003;
            analog I(p, n) <+ g1*V(p,n) + g2*V(p,n)*V(p,n) + g3*V(p,n)*V(p,n)*V(p,n);
        endmodule
        module Top (inout electrical a, inout electrical b);
            Poly x1 (a, b);
        endmodule
        ",
        "Top",
    );
    let caps = nonlinear.all_devices()[0].capabilities();
    assert!(
        !caps.contains(ElementCapabilities::NUMERIC_JACOBIAN),
        "JIT device has an analytic Jacobian — must NOT declare NUMERIC_JACOBIAN, got {caps:?}"
    );
}
