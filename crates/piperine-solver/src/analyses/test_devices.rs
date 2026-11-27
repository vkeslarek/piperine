//! The builders the `tf`, `pz` and `sp` unit tests place their circuits' devices with: each adds one device of
//! the `piperine-device-fixtures` crate (the `circuit` resistor, capacitor, voltage source and inductor, the
//! `analyses` bias, wire and quadratic doubles) to a `CircuitBuilder`.

use piperine_analog_solver::netlist::AnalogReference;
use piperine_device_fixtures::analyses::{Bias, Quadratic, Wire};
use piperine_device_fixtures::circuit::{Capacitor, Inductor, Resistor, VoltageSource};

use crate::core::bind::{ParamValue, Port};
use crate::core::builder::CircuitBuilder;

/// Places the fixtures crate's two-terminal devices on a circuit builder.
pub(crate) struct AnalysisDevices;

impl AnalysisDevices {
    /// The two node ports a two-terminal device is bound to.
    fn nodes(p: &AnalogReference, n: &AnalogReference) -> [Port; 2] {
        [Port::Node(p.clone()), Port::Node(n.clone())]
    }

    /// A resistor between `p` and `n`.
    pub(crate) fn resistor(b: &mut CircuitBuilder, path: &str, p: &AnalogReference, n: &AnalogReference, r: f64) {
        b.device(Resistor::module(), path, &[("r", ParamValue::Real(r))], &Self::nodes(p, n));
    }

    /// A capacitor between `p` and `n`.
    pub(crate) fn capacitor(b: &mut CircuitBuilder, path: &str, p: &AnalogReference, n: &AnalogReference, c: f64) {
        b.device(Capacitor::module(), path, &[("c", ParamValue::Real(c))], &Self::nodes(p, n));
    }

    /// A 1 nS conductance from `n` to ground that exists at DC only: a node that is otherwise floating gets a
    /// defined operating point and no small-signal admittance.
    pub(crate) fn dc_bias(b: &mut CircuitBuilder, path: &str, n: &AnalogReference) {
        b.device(Bias::module(), path, &[], &[Port::Node(n.clone())]);
    }

    /// An ideal wire between two distinct nodes.
    pub(crate) fn wire(b: &mut CircuitBuilder, path: &str, p: &AnalogReference, n: &AnalogReference) {
        b.device(Wire::module(), path, &[], &Self::nodes(p, n));
    }

    /// An ideal DC voltage source between `p` and `n`.
    pub(crate) fn vsrc(b: &mut CircuitBuilder, path: &str, p: &AnalogReference, n: &AnalogReference, dc: f64) {
        b.device(VoltageSource::module(), path, &[("dc", ParamValue::Real(dc))], &Self::nodes(p, n));
    }

    /// An ideal inductor between `p` and `n`: a DC short whose AC branch row adds `-jωL`.
    pub(crate) fn inductor(b: &mut CircuitBuilder, path: &str, p: &AnalogReference, n: &AnalogReference, l: f64) {
        b.device(Inductor::module(), path, &[("l", ParamValue::Real(l))], &Self::nodes(p, n));
    }

    /// A device whose AC stamp at (`n1`, `n2`) is `jω²k`: not affine in `jω`.
    pub(crate) fn freq_nonlinear(b: &mut CircuitBuilder, path: &str, n1: &AnalogReference, n2: &AnalogReference, k: f64) {
        b.device(Quadratic::module(), path, &[("k", ParamValue::Real(k))], &Self::nodes(n1, n2));
    }
}
