//! Fixtures and helpers shared by the native-binding suites — the PHDL
//! circuits and the in-process `_piperine` module construction the suites in
//! this directory drive. Extracted from `src/lib.rs`'s inline
//! `#[cfg(test)] mod tests` (MD-28: these exercise the crate's public binding
//! surface across modules, so they are integration tests).
//!
//! Not a test target — a `tests/common/` module the suites include.

#![allow(dead_code, unused_imports)]

use pyo3::prelude::*;
use pyo3::types::PyModule;

use piperine_python::_piperine;

/// A tiny self-contained PHDL (declares its own discipline + two modules,
/// no `use`/prelude dependency) — the P3/P4 reflection fixture. Resistor
/// carries an `analog` body so behavior reflection is observable.
pub const PHDL: &str = r#"
nature Voltage; units = "V"; access = V; abstol = 1e-6; endnature
nature Current; units = "A"; access = I; abstol = 1e-12; endnature
discipline electrical;
    potential Voltage;
    flow Current;
enddiscipline

module Resistor (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog I(p, n) <+ V(p, n) / r;
endmodule

module DividerBoard ();
    electrical gnd, vin, mid;
    Resistor #(.r(3e3)) r_top (.p(vin), .n(mid));
    Resistor #(.r(2e3)) r_bot (.p(mid), .n(gnd));
endmodule
"#;

/// A runnable fixture for the analysis tests: a 5 V source driving a
/// 3 kΩ/2 kΩ resistor divider, so the `mid` node sits at
/// 5·2/(3+2) = 2.0 V (spec-defined outcome the stage test asserts).
/// Staging `r_top.r = 2e3` moves `mid` to 5·2/(2+2) = 2.5 V (spec AC12).
/// Mirrors the root host-API divider circuit shape — the uniform-host proof.
pub const ANALYSIS_PHDL: &str = r#"
nature Voltage; units = "V"; access = V; abstol = 1e-6; endnature
nature Current; units = "A"; access = I; abstol = 1e-12; endnature
discipline electrical;
    potential Voltage;
    flow Current;
enddiscipline

module VoltageSource (inout electrical p, inout electrical n);
    parameter real voltage = 0.0;
    analog V(p, n) <+ voltage;
endmodule

module Resistor (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog I(p, n) <+ V(p, n) / r;
endmodule

module Divider ();
    electrical gnd, vin, mid;
    VoltageSource #(.voltage(5.0)) src (.p(vin), .n(gnd));
    Resistor #(.r(3e3)) r_top (.p(vin), .n(mid));
    Resistor #(.r(2e3)) r_bot (.p(mid), .n(gnd));
endmodule
"#;

/// Dedicated AC fixture: an `ac_stim` current source driving a 1 kΩ
/// resistor to ground. `ac_stim(mag)` is the small-signal injection —
/// `-ac_stim(1.0)` means 1 A flows out of `p` into
/// the source (the force-branch convention).
pub const AC_PHDL: &str = r#"
nature Voltage; units = "V"; access = V; abstol = 1e-6; endnature
nature Current; units = "A"; access = I; abstol = 1e-12; endnature
discipline electrical;
    potential Voltage;
    flow Current;
enddiscipline

module AcSource (inout electrical p, inout electrical n);
    analog I(p, n) <+ -ac_stim(1.0);
endmodule

module Resistor (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog I(p, n) <+ V(p, n) / r;
endmodule

module AcTest ();
    electrical gnd, out;
    AcSource stim (.p(out), .n(gnd));
    Resistor #(.r(1e3)) r1 (.p(out), .n(gnd));
endmodule
"#;

pub const NOISE_PHDL: &str = r#"
nature Voltage; units = "V"; access = V; abstol = 1e-6; endnature
nature Current; units = "A"; access = I; abstol = 1e-12; endnature
discipline electrical;
    potential Voltage;
    flow Current;
enddiscipline

module NoisyResistor (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog I(p, n) <+ V(p, n) / r + white_noise(4 * 8.617e-5 * 300.15 / r);
endmodule

module NoiseTest ();
    electrical gnd, out;
    NoisyResistor #(.r(1e3)) nr (.p(out), .n(gnd));
endmodule
"#;

/// Build the in-process `_piperine` module under the active interpreter,
/// load the reflection PHDL, and return the loaded `_Design`.
pub fn loaded_design<'py>(py: Python<'py>, path_str: &str) -> PyResult<Bound<'py, PyAny>> {
    let m = PyModule::new(py, "_piperine")?;
    _piperine(&m)?;
    m.getattr("load")?.call1((path_str,))
}

/// Sorted list of an iterable's `.name` attribute (objects expose `name`
/// as a `#[getter]`).
pub fn sorted_names(list: Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let mut names: Vec<String> = list
        .try_iter()?
        .map(|item| {
            let item: Bound<'_, PyAny> = item?;
            item.getattr("name")?.extract::<String>()
        })
        .collect::<PyResult<Vec<String>>>()?;
    names.sort();
    Ok(names)
}

/// `(name, module)` pairs for an iterable of `_Instance`.
pub fn instance_pairs(list: Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    list.try_iter()?
        .map(|item| {
            let it: Bound<'_, PyAny> = item?;
            let name = it.getattr("name")?.extract::<String>()?;
            let module = it.getattr("module")?.extract::<String>()?;
            Ok((name, module))
        })
        .collect()
}
