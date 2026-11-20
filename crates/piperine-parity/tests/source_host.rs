//! Scope: the host's sources on a top's node port against ngspice (PI-07, design D11; ngspice
//! first). The RC of `ac_rc.cir` and `tran_rc.cir` is written as the top `RcTop` with the node
//! port `vin` (`tests/source_host/rc_top.msl`) and driven by `Session::source`: its AC stimulus
//! is compared with `ac_rc.cir` under `AC_MAGNITUDE` and `AC_PHASE`, its DC step with
//! `tran_rc.cir` under `TRAN_VOLTAGE`. No contract is written or changed here. The host source
//! reads the clock, so the runs are on the compiled backend only (the interpreter refuses it by
//! name).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use piperine_api::{NodeSource, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::host::HostRun;
use piperine_parity::ngspice::{Ngspice, Reference};
use piperine_parity::waveform::Waveform;

/// A sink that keeps nothing: these runs read the table.
struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

/// A run of `RcTop` on the Rust backend, its input driven by `source`.
struct HostDrivenRc;

impl HostDrivenRc {
    fn session(overrides: Overrides) -> Session {
        let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "source_host", "rc_top.msl"].iter().collect();
        let project = Project::file(&path).unwrap_or_else(|f| panic!("project: {f}"));
        let overrides = Overrides { backend: Some("rust".to_string()), top: Some("RcTop".to_string()), print: vec!["out".to_string()], ..overrides };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("request: {f}"));
        Session::open(&project, &spec, &HostRun::settings()).unwrap_or_else(|f| panic!("open: {f}"))
    }

    fn table(session: &mut Session) -> Table {
        session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("run: {f}"))
    }
}

#[test]
fn a_host_driven_rc_matches_ac_rc_in_ac() {
    // the ngspice source `V1 vin 0 DC 0 AC 1`: dc 0, magnitude 1, phase 0
    let reference = Ngspice::from_environment().run("ac_rc");
    let rows = match &reference {
        Reference::Golden(golden) => {
            let rows = golden.complex("ac_rc_ac");
            golden.expect_rows("ac_rc_ac", &rows, 41);
            rows
        }
        Reference::Skipped => Vec::new(),
    };
    let overrides = Overrides { analysis: Some("ac".to_string()), from: Some(1e3), to: Some(10e6), per_decade: Some(10), ..Overrides::default() };
    let mut session = HostDrivenRc::session(overrides);
    session.source("vin", NodeSource::dc(0.0).with_ac(1.0, 0.0)).expect("a source on vin");
    let table = HostDrivenRc::table(&mut session);
    let Reference::Golden(golden) = &reference else { return };
    assert_eq!(table.rows.len(), rows.len(), "the sweep has ngspice's grid");
    for ((frequency, wanted), row) in rows.iter().zip(&table.rows) {
        // the table's columns are `freq`, `mag(RcTop.out)` and `phase(RcTop.out)` in degrees
        let (grid, magnitude, degrees) = (HostRun::real(&row[0]), HostRun::real(&row[1]), HostRun::real(&row[2]));
        assert!((grid - frequency).abs() <= 1e-6 * frequency, "the grid point {grid} is ngspice's {frequency}");
        let at = Point::new("rc_top", "|v(out)|", Coordinate::Frequency(*frequency)).with_ngspice(golden.version()).with_backend("rust");
        ToleranceContract::AC_MAGNITUDE.assert(&at, wanted.magnitude(), magnitude);
        if ToleranceContract::phase_applies(wanted.magnitude()) {
            let at = Point::new("rc_top", "arg v(out)", Coordinate::Frequency(*frequency)).with_ngspice(golden.version()).with_backend("rust");
            ToleranceContract::AC_PHASE.assert(&at, wanted.phase(), degrees.to_radians());
        }
    }
}

#[test]
fn a_host_driven_rc_step_matches_tran_rc() {
    // the ngspice source `V1 vin 0 DC 1` with the capacitor discharged (`uic`): dc 1 from t = 0
    let reference = Ngspice::from_environment().run("tran_rc");
    let rows = match &reference {
        Reference::Golden(golden) => {
            let rows = golden.real("tran_rc_tran");
            golden.expect_rows("tran_rc_tran", &rows, 100);
            rows
        }
        Reference::Skipped => Vec::new(),
    };
    // the run goes one print step past ngspice's last time (10 us): the table's grid is the sum of
    // print steps, whose last row falls one ulp short of 10 us, and a waveform is not extrapolated
    let overrides = Overrides { analysis: Some("transient".to_string()), stop: Some(10e-6 + 1e-7), max_step: Some(1e-8), print_step: Some(1e-7), ..Overrides::default() };
    let mut session = HostDrivenRc::session(overrides);
    session.source("vin", NodeSource::dc(1.0)).expect("a source on vin");
    let table = HostDrivenRc::table(&mut session);
    let Reference::Golden(golden) = &reference else { return };
    let waveform = Waveform::new(table.rows.iter().map(|row| HostRun::real(&row[0])).collect(), table.rows.iter().map(|row| HostRun::real(&row[1])).collect());
    for (time, wanted) in rows {
        let actual = waveform.at(time).unwrap_or_else(|| panic!("the time {time:e} s lies outside the result"));
        let at = Point::new("rc_top", "v(out)", Coordinate::Time(time)).with_ngspice(golden.version()).with_backend("rust");
        ToleranceContract::TRAN_VOLTAGE.assert(&at, wanted, actual);
    }
}
