//! One settled value per microstep on the solver (feature `solver-crossings`, DSV-01 AC 4;
//! docs/spec Part III §8.2.3, Part IV §12.1.2, §12.1.18): a reaction that commits `x <- HIGH;
//! x <- LOW;` to a net holding `LOW` leaves it as it was, so a function reading `x.CHANGED`
//! fires nothing. The pure-ABI repro of `ir-rust` T42.

use piperine_device_fixtures::mixed;
use piperine_device_fixtures::solver_devices::settled_value::{FIRED, SettledWatch, TwoWrites};

use piperine_abi::layout::LOW;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Solver, TransientAnalysisOptions};

#[test]
fn x_changed_does_not_fire_when_the_settled_value_is_unchanged() {
    let mut b = CircuitBuilder::new("settled");
    b.device(mixed::Delay::module(), "kick", &[("at", ParamValue::Real(1.0e-7))], &[SettledNet::net("go")]);
    b.device(TwoWrites::module(), "tw", &[], &[SettledNet::net("x"), SettledNet::net("go")]);
    b.device(SettledWatch::module(), "w", &[], &[SettledNet::net("x")]);
    let mut solver =
        Solver::new(b.build().expect("build")).with_tran_opts(TransientAnalysisOptions::new(2.0e-7, 1.0e-8)).build();
    solver.tran().expect("tran").solve().expect("solve");
    // `x` changes once, at (0, 0), when its init LOW is committed over the zeroed store (Part IV
    // §13: the initial value is a change). At 100 ns `x <- HIGH; x <- LOW;` settles to LOW, the
    // value it held: no change, so `x.changed()` fires no second time.
    assert_eq!(FIRED.with(|f| f.get()), 1, "x.changed() fired although the settled value is the one before the microstep");
    let bound = solver.circuit().bound();
    assert_eq!(bound.digital.read(bound.names.nets["net:x"]), LOW, "the later statement wins");
}

/// The net whose settled value is read.
struct SettledNet;

impl SettledNet {
    /// The net helper `net`.
    fn net(name: &str) -> Port {
        Port::Net(name.to_string())
    }
}
