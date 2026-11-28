//! Scope: transfer-function parity (host-ii HII-04 AC 2): the gain, the output resistance and the
//! input resistance of the twin `tf_divider` (`.tf v(out) v1`) agree with the values ngspice
//! prints under `TF_RATIO`, on each selected backend, the input named by its design path.

use piperine_parity::backend::{Backend, BackendSelection};
use piperine_parity::bench::Bench;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::host::HostRun;
use piperine_parity::ngspice::{Ngspice, Reference};
use piperine_solver::prelude::{AnalogVariable, Context, TransferFunctionAnalysisOptions};

/// Associated test helpers of `tf`, grouped on one fixture type.
struct TfFixture;

impl TfFixture {
    fn compare(reference: &Reference, backend: Backend) {
        let mut bench = Bench::load_on("tf_divider", backend);
        let options = TransferFunctionAnalysisOptions { output: AnalogVariable::Node(bench.node("vout")), output_ref: None, input_source: bench.source("v1") };
        let result = bench
            .circuit()
            .transfer_function(options, Context::default())
            .and_then(|mut solver| solver.solve())
            .unwrap_or_else(|error| panic!("tf_divider: the transfer does not solve on {}: {error}", backend.name()));
        let Reference::Golden(golden) = reference else { return };
        let rows = [
            ("transfer_function", "gain", result.gain),
            ("output_impedance_at_v(out)", "output_resistance", result.output_resistance),
            ("v1#input_impedance", "input_resistance", result.input_resistance),
        ];
        for (printed, quantity, actual) in rows {
            let wanted = golden.printed(printed);
            eprintln!("tf_divider {quantity} on {}: ngspice {wanted:e}, piperine {actual:e}, deviation {:e}", backend.name(), (wanted - actual).abs());
            let at = Point::new("tf_divider", quantity, Coordinate::OperatingPoint).with_ngspice(golden.version()).with_backend(backend.name());
            ToleranceContract::TF_RATIO.assert(&at, wanted, actual);
        }
    }
}

#[test]
fn tf_divider() {
    let reference = Ngspice::from_environment().run("tf_divider");
    for backend in BackendSelection::selected() {
        TfFixture::compare(&reference, backend);
    }
}

#[test]
fn tf_divider_through_a_session() {
    // HII-04 AC 2: the run keys `input = "v1"` and `output = "vout"`, the three rows of the table
    // against what ngspice prints
    let reference = Ngspice::from_environment().run("tf_divider");
    let Reference::Golden(golden) = &reference else { return };
    for backend in BackendSelection::selected() {
        let table = HostRun::new("tf_divider", "tf", backend)
            .keys(|keys| {
                keys.input = Some("v1".into());
                keys.output = Some("vout".into());
            })
            .table();
        for (printed, quantity) in [("transfer_function", "gain"), ("output_impedance_at_v(out)", "output_impedance"), ("v1#input_impedance", "input_impedance")] {
            let actual = HostRun::real(&HostRun::row(&table, quantity)[1]);
            let label = format!("{quantity} through a session");
            let at = Point::new("tf_divider", &label, Coordinate::OperatingPoint).with_ngspice(golden.version()).with_backend(backend.name());
            ToleranceContract::TF_RATIO.assert(&at, golden.printed(printed), actual);
        }
    }
}
