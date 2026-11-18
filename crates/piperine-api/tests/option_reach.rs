//! Scope: the session options reach every analysis (host-ii HII-01 AC 1, HII-02 AC 2 to 4, "for
//! every analysis"): for each of `op`, `dc`, `ac`, `transient`, `tf`, `pz`, `sens` and `pss`, one
//! run whose result moves with `temp` (the context reaches the solver) and one whose result
//! moves with `nodeset` (the seed reaches the analysis's own operating point), each against a
//! closed form. The design is a bistable node: a source of 0 V through the temperature-dependent
//! resistor `R(T) = r * (1 + tc * (T - 300.15))` into the node `a`, where a capacitor and the
//! cubic conductance `v^3 - 4v` hang to ground. Kirchhoff at `a` is `a / R + a^3 - 4a = 0`, so
//! the operating points are `0` and `+-sqrt(4 - 1/R)`, and the small-signal conductance there is
//! `1/R - 4` and `8 - 2/R`.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use piperine_api::{Cell, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_solver::prelude::Context;
use piperine_transpile::Settings;

/// Keeps nothing: these tests read tables.
struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

/// The design and the closed forms of one bistable node.
struct Bistable;

impl Bistable {
    /// `r` of the resistor, in ohms.
    const R: f64 = 1.0;
    /// Its temperature coefficient, per kelvin.
    const TC: f64 = 0.01;
    /// The temperature the resistor's coefficient is taken from, kelvin.
    const REFERENCE: f64 = 300.15;
    /// The run's `temp`, Celsius: 400 K.
    const HOT: f64 = 126.85;
    /// A tight tolerance, so a closed form is held to far less than the temperature's effect.
    const TIGHT: f64 = 1e-12;

    const SOURCE: &'static str = "pub module Source(p: Electrical, n: Electrical) { dc: f64 = 0.0; }\nanalog for Source { v(p, n) +> dc + ac_stim(1.0); }\npub module Tcr(p: Electrical, n: Electrical) { r: f64 where r > 0.0; tc: f64 = 0.01; }\nanalog for Tcr { i(p, n) +> v(p, n) / (r * (1.0 + tc * (temperature() - 300.15))); }\npub module Cap(p: Electrical, n: Electrical) { c: f64 = 1u; }\nanalog for Cap { i(p, n) +> ddt(c * v(p, n)); }\npub module Cubic(p: Electrical, n: Electrical) {}\nanalog for Cubic { i(p, n) +> v(p, n) * v(p, n) * v(p, n) - 4.0 * v(p, n); }\n";

    fn text() -> String {
        format!(
            "{}module Top() {{\n    signal s: Electrical;\n    signal a: Electrical;\n    let v1 = Source(s, gnd);\n    let r1 = Tcr(s, a) {{ r: {:?} }};\n    let c1 = Cap(a, gnd);\n    let nl = Cubic(a, gnd);\n}}\n",
            Bistable::SOURCE,
            Bistable::R
        )
    }

    /// The resistance at `kelvin`.
    fn resistance(kelvin: f64) -> f64 {
        Bistable::R * (1.0 + Bistable::TC * (kelvin - Bistable::REFERENCE))
    }

    /// The resistance with no `temp` given, and at `HOT`.
    fn default_resistance() -> f64 {
        Bistable::resistance(Context::default().tolerances.temperature)
    }

    fn hot_resistance() -> f64 {
        Bistable::resistance(Bistable::HOT + 273.15)
    }

    /// The operating point away from zero, `sqrt(4 - 1/R)`.
    fn root(r: f64) -> f64 {
        (4.0 - 1.0 / r).sqrt()
    }

    /// The gain `a / v1` at DC for the operating point `a`: `(1/R) / (1/R + 3 a^2 - 4)`.
    fn gain(r: f64, a: f64) -> f64 {
        (1.0 / r) / (1.0 / r + 3.0 * a * a - 4.0)
    }

    /// The pole `-(1/R + 3 a^2 - 4) / C` in radians per second.
    fn pole(r: f64, a: f64) -> f64 {
        -(1.0 / r + 3.0 * a * a - 4.0) / 1e-6
    }

    fn settings() -> Settings {
        static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
        let dir = CACHE.get_or_init(|| tempfile::tempdir().expect("tmp"));
        Settings { rustc: None, cache: dir.path().to_path_buf() }
    }

    /// The table of the run of the bistable with `overrides`, on the interpreter.
    fn run(overrides: Overrides) -> Table {
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("bench.msl");
        std::fs::write(&file, Bistable::text()).expect("write");
        let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { top: Some("Top".into()), backend: overrides.backend.or(Some("interp".into())), reltol: Some(Bistable::TIGHT), vntol: Some(Bistable::TIGHT), print: vec!["a".into()], ..overrides };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let mut session = Session::open(&project, &spec, &Bistable::settings()).unwrap_or_else(|f| panic!("{f}"));
        session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"))
    }

    fn real(cell: &Cell) -> f64 {
        match cell {
            Cell::Real(v) => *v,
            other => panic!("a real, found {other:?}"),
        }
    }

    /// `got` is `wanted` within a part in a million of it.
    fn close(got: f64, wanted: f64, what: &str) {
        assert!((got - wanted).abs() <= 1e-6 * wanted.abs().max(1e-3), "{what}: {got} against {wanted}");
    }

    fn seeded(analysis: Overrides, seed: f64) -> Overrides {
        Overrides { nodeset: vec![("a".into(), seed)], ..analysis }
    }

    fn hot(analysis: Overrides) -> Overrides {
        Overrides { temp: Some(Bistable::HOT), ..analysis }
    }

    fn op() -> Overrides {
        Overrides { analysis: Some("op".into()), ..Overrides::default() }
    }

    fn dc() -> Overrides {
        Overrides { analysis: Some("dc".into()), sweep: Some("Top.v1.dc".into()), from: Some(0.0), to: Some(0.0), step: Some(1.0), ..Overrides::default() }
    }

    fn ac() -> Overrides {
        Overrides { analysis: Some("ac".into()), from: Some(1e-3), to: Some(1e-2), per_decade: Some(1), ..Overrides::default() }
    }

    fn transient() -> Overrides {
        Overrides { analysis: Some("transient".into()), stop: Some(1e-6), print_step: Some(5e-7), ..Overrides::default() }
    }

    fn transfer(analysis: &str) -> Overrides {
        Overrides { analysis: Some(analysis.into()), backend: (analysis == "pz").then(|| "rust".to_string()), input: Some("v1".into()), output: Some("a".into()), print: Vec::new(), ..Overrides::default() }
    }

    fn sens() -> Overrides {
        Overrides { analysis: Some("sens".into()), wrt: vec!["Top.v1.dc".into()], ..Overrides::default() }
    }

    fn pss() -> Overrides {
        Overrides { analysis: Some("pss".into()), period: Some(1e-6), tstab: Some(0.0), print_step: Some(5e-7), ..Overrides::default() }
    }

    /// The value in the column `at` of the last row.
    fn last(table: &Table, at: usize) -> f64 {
        Bistable::real(&table.rows.last().expect("a row")[at])
    }

    /// The row of a transfer table named `quantity`.
    fn quantity(table: &Table, quantity: &str) -> f64 {
        let row = table.rows.iter().find(|r| r[0] == Cell::Text(quantity.into())).unwrap_or_else(|| panic!("no row `{quantity}` in {table:?}"));
        Bistable::real(&row[1])
    }

    /// The real parts of a pole-zero table's poles.
    fn poles(table: &Table) -> Vec<f64> {
        table.rows.iter().filter(|r| r[0] == Cell::Text("pole".into())).map(|r| Bistable::real(&r[1])).collect()
    }
}

#[test]
fn an_operating_point_moves_with_temp_and_nodeset() {
    // HII-02 AC 2 to 4 (op): the root is sqrt(4 - 1/R(T)); the seed picks its sign
    let cold = Bistable::default_resistance();
    Bistable::close(Bistable::last(&Bistable::run(Bistable::seeded(Bistable::op(), 1.7)), 1), Bistable::root(cold), "default temperature");
    Bistable::close(Bistable::last(&Bistable::run(Bistable::hot(Bistable::seeded(Bistable::op(), 1.7))), 1), Bistable::root(Bistable::hot_resistance()), "temp = 126.85");
    Bistable::close(Bistable::last(&Bistable::run(Bistable::seeded(Bistable::op(), -1.7)), 1), -Bistable::root(cold), "a seed below zero");
}

#[test]
fn a_dc_sweep_moves_with_temp_and_nodeset() {
    // HII-02 AC 2 to 4 (dc): every point of the sweep takes the context and the seed
    let cold = Bistable::default_resistance();
    Bistable::close(Bistable::last(&Bistable::run(Bistable::seeded(Bistable::dc(), 1.7)), 1), Bistable::root(cold), "default temperature");
    Bistable::close(Bistable::last(&Bistable::run(Bistable::hot(Bistable::seeded(Bistable::dc(), 1.7))), 1), Bistable::root(Bistable::hot_resistance()), "temp = 126.85");
    Bistable::close(Bistable::last(&Bistable::run(Bistable::seeded(Bistable::dc(), -1.7)), 1), -Bistable::root(cold), "a seed below zero");
}

#[test]
fn an_ac_sweep_moves_with_temp_and_nodeset() {
    // HII-02 AC 2 to 4 (ac): the magnitude at 1 mHz is |gain| at the operating point the seed picks
    // (the negative-conductance root 0 against the root sqrt(4 - 1/R)), at the temperature's R
    let cold = Bistable::default_resistance();
    let hot = Bistable::hot_resistance();
    let magnitude = |overrides: Overrides| Bistable::real(&Bistable::run(overrides).rows[0][1]);
    Bistable::close(magnitude(Bistable::seeded(Bistable::ac(), 1.7)), Bistable::gain(cold, Bistable::root(cold)).abs(), "the upper root");
    Bistable::close(magnitude(Bistable::seeded(Bistable::ac(), 0.1)), Bistable::gain(cold, 0.0).abs(), "the root at zero");
    Bistable::close(magnitude(Bistable::hot(Bistable::seeded(Bistable::ac(), 1.7))), Bistable::gain(hot, Bistable::root(hot)).abs(), "temp = 126.85");
}

#[test]
fn a_transient_moves_with_temp_and_nodeset() {
    // HII-02 AC 2 to 4 (transient): the initial operating point is the seeded root and, with
    // only a source of 0 V, the node holds it for the whole run
    let cold = Bistable::default_resistance();
    Bistable::close(Bistable::last(&Bistable::run(Bistable::seeded(Bistable::transient(), 1.7)), 1), Bistable::root(cold), "default temperature");
    Bistable::close(Bistable::last(&Bistable::run(Bistable::hot(Bistable::seeded(Bistable::transient(), 1.7))), 1), Bistable::root(Bistable::hot_resistance()), "temp = 126.85");
    Bistable::close(Bistable::last(&Bistable::run(Bistable::seeded(Bistable::transient(), -1.7)), 1), -Bistable::root(cold), "a seed below zero");
}

#[test]
fn a_transfer_function_moves_with_temp_and_nodeset() {
    // HII-02 AC 2 to 4 (tf): the gain row is the DC gain at the seeded operating point
    let cold = Bistable::default_resistance();
    let hot = Bistable::hot_resistance();
    let gain = |overrides: Overrides| Bistable::quantity(&Bistable::run(overrides), "gain");
    Bistable::close(gain(Bistable::seeded(Bistable::transfer("tf"), 1.7)), Bistable::gain(cold, Bistable::root(cold)), "the upper root");
    Bistable::close(gain(Bistable::seeded(Bistable::transfer("tf"), 0.1)), Bistable::gain(cold, 0.0), "the root at zero");
    Bistable::close(gain(Bistable::hot(Bistable::seeded(Bistable::transfer("tf"), 1.7))), Bistable::gain(hot, Bistable::root(hot)), "temp = 126.85");
}

#[test]
fn the_poles_move_with_temp_and_nodeset() {
    // HII-02 AC 2 to 4 (pz): the one pole is -(1/R + 3a^2 - 4) / C at the seeded operating point
    let cold = Bistable::default_resistance();
    let hot = Bistable::hot_resistance();
    let pole = |overrides: Overrides| Bistable::poles(&Bistable::run(overrides));
    let upper = pole(Bistable::seeded(Bistable::transfer("pz"), 1.7));
    assert_eq!(upper.len(), 1, "{upper:?}");
    Bistable::close(upper[0], Bistable::pole(cold, Bistable::root(cold)), "the upper root");
    Bistable::close(pole(Bistable::seeded(Bistable::transfer("pz"), 0.1))[0], Bistable::pole(cold, 0.0), "the root at zero");
    Bistable::close(pole(Bistable::hot(Bistable::seeded(Bistable::transfer("pz"), 1.7)))[0], Bistable::pole(hot, Bistable::root(hot)), "temp = 126.85");
}

#[test]
fn a_sensitivity_moves_with_temp_and_nodeset() {
    // HII-02 AC 2 to 4 (sens): d a / d dc of the source is the DC gain at the seeded operating point
    let cold = Bistable::default_resistance();
    let hot = Bistable::hot_resistance();
    let derivative = |overrides: Overrides| Bistable::real(&Bistable::run(overrides).rows[0][1]);
    Bistable::close(derivative(Bistable::seeded(Bistable::sens(), 1.7)), Bistable::gain(cold, Bistable::root(cold)), "the upper root");
    Bistable::close(derivative(Bistable::seeded(Bistable::sens(), 0.1)), Bistable::gain(cold, 0.0), "the root at zero");
    Bistable::close(derivative(Bistable::hot(Bistable::seeded(Bistable::sens(), 1.7))), Bistable::gain(hot, Bistable::root(hot)), "temp = 126.85");
}

#[test]
fn a_periodic_steady_state_moves_with_temp_and_nodeset() {
    // HII-02 AC 2 to 4 (pss): a constant circuit's orbit is its operating point, the seeded root
    let cold = Bistable::default_resistance();
    Bistable::close(Bistable::last(&Bistable::run(Bistable::seeded(Bistable::pss(), 1.7)), 1), Bistable::root(cold), "default temperature");
    Bistable::close(Bistable::last(&Bistable::run(Bistable::hot(Bistable::seeded(Bistable::pss(), 1.7))), 1), Bistable::root(Bistable::hot_resistance()), "temp = 126.85");
    Bistable::close(Bistable::last(&Bistable::run(Bistable::seeded(Bistable::pss(), -1.7)), 1), -Bistable::root(cold), "a seed below zero");
}
