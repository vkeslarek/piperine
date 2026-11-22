//! A user nature's tolerances and compatible natures, read from MSL source (analog-operators
//! AO-10, U7; docs/spec Part V §17.1): the `abstol` a nature declares reaches the bound circuit
//! as the tolerance of the node (potential) or of the branch unknown and its row (flow); two
//! compatible natures meeting at one node give it the smaller potential `abstol` and leave the
//! operating point where an all-`Electrical` design puts it; natures whose flow names differ
//! are `E0251` at a port. Every design is a lone file built on the interpreter.

use std::fs;

use piperine_api::{Failure, Overrides, Probe, Project, Session};
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_solver::prelude::{BranchIdentifier, Context, Tolerances};
use piperine_transpile::Settings;

/// `HvElectrical` is compatible with `Electrical` (same quantity names) and tolerates 1 mV of
/// potential; `Fine` has a flow `abstol` of 1 nA; `Odd` names its flow quantity otherwise.
const NATURES: &str = "\
pub nature HvElectrical {
    potential: Voltage { access: v, units: \"V\", abstol: 1m },
    flow:      Current { access: i, units: \"A\", abstol: 1p },
}
pub nature Fine {
    potential: Voltage { access: v, units: \"V\", abstol: 1u },
    flow:      Current { access: i, units: \"A\", abstol: 1n },
}
pub nature Odd {
    potential: Voltage { access: v, units: \"V\", abstol: 1u },
    flow:      Flux    { access: i, units: \"A\", abstol: 1p },
}
pub module Source(p: Electrical, n: Electrical) { dc: f64 = 1.0; }
analog for Source { v(p, n) +> dc; }
pub module Res(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }
analog for Res { i(p, n) +> v(p, n) / r; }
pub module HvRes(p: HvElectrical, n: HvElectrical) { r: f64 where r > 0.0; }
analog for HvRes { i(p, n) +> v(p, n) / r; }
pub module FineSource(p: Fine, n: Fine) { dc: f64 = 1.0; }
analog for FineSource { v(p, n) +> dc; }
pub module OddLoad(p: Odd) {}
analog for OddLoad { i(p, gnd) +> v(p, gnd) / 1k; }
";

/// The sessions of the designs of this target: `NATURES` followed by a top, on the interpreter.
struct Designs;

impl Designs {
    /// A session of the lone file `NATURES` followed by `top`, on the interpreter.
    fn open(top: &str) -> Result<Session, Failure> {
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("bench.msl");
        fs::write(&file, format!("{NATURES}{top}")).expect("write");
        let project = Project::file(&file).unwrap_or_else(|failure| panic!("{failure}"));
        let overrides = Overrides { analysis: Some("op".into()), backend: Some("interp".into()), top: Some("Top".into()), ..Overrides::default() };
        let spec = project.request(None, &overrides).unwrap_or_else(|failure| panic!("{failure}"));
        Session::open(&project, &spec, &Settings { rustc: None, cache: dir.path().to_path_buf() })
    }

    /// The session of `top`, which must build.
    fn session(top: &str) -> Session {
        match Designs::open(top) {
            Ok(session) => session,
            Err(Failure::Diagnostics { found, .. }) => panic!("{:?}", found.iter().map(|found| format!("{} {}", found.diagnostic.code, found.diagnostic.message)).collect::<Vec<_>>()),
            Err(other) => panic!("{other}"),
        }
    }

    /// The node `entry` of the top of `session`.
    fn node(session: &Session, entry: &str) -> piperine_solver::prelude::NodeIdentifier {
        match Probe::resolve(session.ir(), "Top", entry).unwrap_or_else(|failure| panic!("{failure}")) {
            Probe::Node { id, .. } => id,
            other => panic!("a node, found {}", other.label()),
        }
    }

    /// The tolerance the bound circuit of `session` gives node `entry`.
    fn node_tolerance(session: &mut Session, entry: &str) -> f64 {
        let id = Designs::node(session, entry);
        let bound = session.circuit().expect("a circuit").bound().expect("a bound circuit");
        *bound.vntol.get(&id).unwrap_or_else(|| panic!("no tolerance reached `{entry}`"))
    }

    /// `v(entry)` at the operating point of `session`.
    fn operating_point(session: &mut Session, entry: &str) -> f64 {
        let id = Designs::node(session, entry);
        let solved = session.circuit().expect("a circuit").dc(Context::default()).expect("dc").solve().expect("the operating point solves");
        solved.get_node(&id).expect("a solved node")
    }
}

#[test]
fn a_potential_abstol_of_one_millivolt_reaches_its_node() {
    // AO-10 AC 1: a node of `HvElectrical` (potential `abstol` 1m) has the tolerance 1m in the bound
    // circuit, which is the update and the residual tolerance of its unknown (§17.1 rules 1, 2).
    let mut session = Designs::session("module Top() { signal a: HvElectrical; let r1 = HvRes(a, gnd) { r: 1k }; }\n");
    assert_eq!(Designs::node_tolerance(&mut session, "a"), 1e-3);
    // the control: a node of `Electrical` has 1 uV
    let mut session = Designs::session("module Top() { signal a: Electrical; let r1 = Res(a, gnd) { r: 1k }; }\n");
    assert_eq!(Designs::node_tolerance(&mut session, "a"), 1e-6);
}

#[test]
fn a_flow_abstol_of_one_nanoampere_reaches_its_branch() {
    // AO-10 AC 2: the private flow unknown of a potential branch of `Fine` (flow `abstol` 1n) has the
    // tolerance 1n, in its update test and in the residual test of its row: a 5e-10 step and a
    // 5e-10 residual pass the bound circuit's tolerances and fail the default 1 pA.
    let mut session = Designs::session("module Top() { signal a: Fine; let v1 = FineSource(a, gnd); }\n");
    let branch = BranchIdentifier::new("Top.v1", "b0.flow");
    let circuit = session.circuit().expect("a circuit");
    let bound = circuit.bound().expect("a bound circuit");
    assert_eq!(bound.branch_abstol.get(&branch).copied(), Some(1e-9), "{:?}", bound.branch_abstol);
    let mut tolerances = Tolerances::default();
    bound.tighten(&mut tolerances);
    let netlist = circuit.netlist();
    let unknowns = netlist.all_references().into_iter().filter_map(|r| r.idx()).count();
    let at = netlist.reference_for(&piperine_solver::prelude::AnalogVariable::Branch(branch)).and_then(|r| r.idx()).expect("the branch is an unknown");
    let mut moved = ndarray::Array1::zeros(unknowns);
    moved[at] = 5e-10;
    let still = ndarray::Array1::zeros(unknowns);
    let mut residual = vec![0.0; unknowns];
    residual[at] = 5e-10;
    let scale = vec![0.0; unknowns];
    assert!(tolerances.has_converged(Some(still.view()), &moved.view(), netlist), "the update is within 1n");
    assert!(tolerances.residual_test(netlist, &residual, &scale), "the residual is within 1n");
    let flat = Tolerances::default();
    assert!(!flat.has_converged(Some(still.view()), &moved.view(), netlist), "the control: 1 pA rejects the update");
    assert!(!flat.residual_test(netlist, &residual, &scale), "the control: 1 pA rejects the residual");
}

#[test]
fn compatible_natures_take_the_smaller_abstol_and_the_divider_holds() {
    // AO-10 AC 3: `mid` is declared `HvElectrical` (1 mV) and meets the `Electrical` port of `Res`
    // (1 uV): it takes the smaller potential `abstol`, 1 uV, not its own nature's; the divider of one resistor of each nature equals the all-`Electrical`
    // divider (1 V over 1 kOhm and 3 kOhm: 0.75 V) within `DC_VOLTAGE`.
    let mixed = "module Top() { signal s: Electrical; signal mid: HvElectrical; let v1 = Source(s, gnd) { dc: 1.0 }; let r1 = Res(s, mid) { r: 1k }; let r2 = HvRes(mid, gnd) { r: 3k }; }\n";
    let plain = "module Top() { signal s: Electrical; signal mid: Electrical; let v1 = Source(s, gnd) { dc: 1.0 }; let r1 = Res(s, mid) { r: 1k }; let r2 = Res(mid, gnd) { r: 3k }; }\n";
    let mut session = Designs::session(mixed);
    assert_eq!(Designs::node_tolerance(&mut session, "mid"), 1e-6);
    let divided = Designs::operating_point(&mut session, "mid");
    let wanted = Designs::operating_point(&mut Designs::session(plain), "mid");
    let at = Point::new("divider", "v(mid)", Coordinate::OperatingPoint);
    ToleranceContract::DC_VOLTAGE.assert(&at, 0.75, divided);
    ToleranceContract::DC_VOLTAGE.assert(&at, wanted, divided);
}

#[test]
fn natures_with_different_flow_names_at_a_port_are_e0251() {
    // AO-10 AC 4 (§4 rule 13): `Odd` names its flow `Flux`, `Electrical` names it `Current`: a node of
    // one does not meet a port of the other.
    match Designs::open("module Top() { signal a: Electrical; let x = OddLoad(a); }\n") {
        Err(Failure::Diagnostics { found, .. }) => {
            assert_eq!(found.len(), 1, "{found:?}");
            assert_eq!(found[0].diagnostic.code, "E0251", "{}", found[0].diagnostic.message);
        }
        Err(other) => panic!("E0251, found {other}"),
        Ok(_) => panic!("incompatible natures meet at a port"),
    }
}
