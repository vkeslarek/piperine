//! Scope: pole-zero parity (host-ii HII-05 AC 2): the poles and zeros of the twin `pz_lowpass` (a
//! voltage source, a series resistor with a capacitor across it, a shunt resistor and a shunt
//! capacitor: one finite pole, one finite zero) agree, root for root, with ngspice's `.pz <in> 0 <out> 0 vol pz` under `PZ_ROOT`, on each selected backend.
//! ngspice shorts a node a voltage source pins, so its fixture leaves the source out: the
//! `vol` transfer of the network behind `in` is the transfer the source drives.

use piperine_parity::backend::Backend;
use piperine_parity::bench::Bench;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::golden::{Complex, Golden};
use piperine_parity::ngspice::{Ngspice, Reference};
use piperine_solver::prelude::{AnalogVariable, Context, PoleZeroOptions};

/// The roots ngspice printed, by the kind its `display` lists them under.
struct Roots;

impl Roots {
    /// How many `<kind>(k)` vectors ngspice lists (`pole` or `zero`): a line `kind(k) : ...`.
    fn listed(golden: &Golden, kind: &str) -> usize {
        golden.stdout().lines().filter(|line| line.trim_start().starts_with(&format!("{kind}(")) && line.contains(" : ")).count()
    }

    /// The roots ngspice printed as `<kind>(1)`, `<kind>(2)`, ..., sorted by real then imaginary part.
    fn printed(golden: &Golden, kind: &str) -> Vec<Complex> {
        let mut roots: Vec<Complex> = (1..=Roots::listed(golden, kind)).map(|k| golden.printed_complex(&format!("{kind}({k})"))).collect();
        Roots::sort(&mut roots);
        roots
    }

    fn sort(roots: &mut [Complex]) {
        roots.sort_by(|a, b| a.re.total_cmp(&b.re).then(a.im.total_cmp(&b.im)));
    }
}

fn compare(reference: &Reference, backend: Backend) {
    let mut bench = Bench::load_on("pz_lowpass", backend);
    let options = PoleZeroOptions { input_source: bench.source("v1"), output: AnalogVariable::Node(bench.node("out")), output_ref: None };
    let solver = bench.circuit().pz(options, Context::default()).unwrap_or_else(|error| panic!("pz_lowpass: the pencil does not extract on {}: {error}", backend.name()));
    let found = [("pole", solver.poles()), ("zero", solver.zeros())];
    let found: Vec<(&str, Vec<Complex>)> = found
        .into_iter()
        .map(|(kind, roots)| {
            let roots = roots.unwrap_or_else(|error| panic!("pz_lowpass: no {kind}s on {}: {error}", backend.name()));
            let mut roots: Vec<Complex> = roots.iter().map(|root| Complex { re: root.re, im: root.im }).collect();
            Roots::sort(&mut roots);
            (kind, roots)
        })
        .collect();
    // the twin has one pole and one zero: a zero the solver loses is a miss even with no ngspice to compare
    for (kind, roots) in &found {
        assert_eq!(roots.len(), 1, "pz_lowpass: one finite {kind} on {}: {roots:?}", backend.name());
    }
    let Reference::Golden(golden) = reference else { return };
    for (kind, actual) in found {
        let wanted = Roots::printed(golden, kind);
        assert_eq!(wanted.len(), actual.len(), "pz_lowpass: ngspice lists {} {kind}s, piperine gives {} on {}: {actual:?}", wanted.len(), actual.len(), backend.name());
        for (k, (wanted, actual)) in wanted.iter().zip(&actual).enumerate() {
            for (part, golden_value, actual_value) in [("re", wanted.re, actual.re), ("im", wanted.im, actual.im)] {
                let quantity = format!("{kind}({}) {part}", k + 1);
                let at = Point::new("pz_lowpass", &quantity, Coordinate::OperatingPoint).with_ngspice(golden.version()).with_backend(backend.name());
                eprintln!("pz_lowpass {quantity} on {}: ngspice {golden_value:e}, piperine {actual_value:e}, deviation {:e}", backend.name(), (golden_value - actual_value).abs());
                ToleranceContract::PZ_ROOT.assert(&at, golden_value, actual_value);
            }
        }
    }
}

#[test]
fn pz_lowpass() {
    let reference = Ngspice::from_environment().run("pz_lowpass");
    for backend in Backend::selected() {
        compare(&reference, backend);
    }
}
