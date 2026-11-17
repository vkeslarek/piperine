//! Scope: DC sensitivity parity (host-ii HII-06 AC 2): the derivatives of `v(mid)` of the twin
//! `sens_divider` with respect to both resistances and the source's value agree with ngspice's
//! `.sens v(mid)` under `SENSITIVITY`, on each selected backend.

use piperine_parity::backend::Backend;
use piperine_parity::bench::Bench;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::ngspice::{Ngspice, Reference};
use piperine_solver::prelude::{AnalogVariable, Context, SensAnalysisOptions};

/// The params ngspice differentiates by, as `(its name, the instance, the param)`.
const WRT: [(&str, &str, &str); 3] = [("r1", "r1", "r"), ("r2", "r2", "r"), ("v1", "v1", "dc")];

fn compare(reference: &Reference, backend: Backend) {
    let mut bench = Bench::load_on("sens_divider", backend);
    let node = AnalogVariable::Node(bench.node("mid"));
    let params: Vec<(String, String)> = WRT.iter().map(|(_, instance, param)| (bench.path(instance), param.to_string())).collect();
    let output = bench.circuit().nets().into_iter().find(|net| net.analog_variable().is_some_and(|variable| **variable == node)).unwrap_or_else(|| panic!("sens_divider: no net for v(mid) on {}", backend.name()));
    let label = output.label().to_string();
    let result = bench
        .circuit()
        .sens(SensAnalysisOptions::new(vec![output], params), Context::default())
        .and_then(|solver| solver.solve())
        .unwrap_or_else(|error| panic!("sens_divider: the sensitivity does not solve on {}: {error}", backend.name()));
    let Reference::Golden(golden) = reference else { return };
    for (printed, instance, param) in WRT {
        let actual = result.get(&label, &bench.path(instance), param).unwrap_or_else(|| panic!("sens_divider: no d(v(mid))/d({instance}.{param}) on {}", backend.name()));
        let quantity = format!("d v(mid) / d {instance}.{param}");
        let at = Point::new("sens_divider", &quantity, Coordinate::OperatingPoint).with_ngspice(golden.version()).with_backend(backend.name());
        let wanted = golden.printed(printed);
        eprintln!("sens_divider {quantity} on {}: ngspice {wanted:e}, piperine {actual:e}, deviation {:e}", backend.name(), (wanted - actual).abs());
        ToleranceContract::SENSITIVITY.assert(&at, golden.printed(printed), actual);
    }
}

#[test]
fn sens_divider() {
    let reference = Ngspice::from_environment().run("sens_divider");
    for backend in Backend::selected() {
        compare(&reference, backend);
    }
}
