//! The seam of the owner's protocol (feature `solver-abi-core`, SAC-13): the
//! module `src/protocol/` reaches the sub-solvers through `DiscreteSide` and
//! `AnalogSide` only and names no item of either sub-solver crate (the
//! analog crate's `error::` aside, the owner's `Result`); the scripted stubs
//! the other protocol targets use replay their script and record every call.

use piperine_device_fixtures::scripted::Call;
use piperine_device_fixtures::scripted::ScriptedAnalog;
use piperine_device_fixtures::scripted::ScriptedDiscrete;
use piperine_device_fixtures::scripted::ScriptedLog;
use std::path::PathBuf;

use piperine_solver::protocol::{
    Accepted, Advance, LandingCause, AnalogSide, Attempt, Boundary, DiscreteSide, InstantMode, Outcome, Restart, Shadow,
};

#[test]
fn the_protocol_module_names_no_sub_solver_item() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/protocol");
    let mut files = 0;
    for entry in std::fs::read_dir(&dir).expect("read src/protocol") {
        let path = entry.expect("dir entry").path();
        let text = std::fs::read_to_string(&path).expect("read a protocol source");
        files += 1;
        let found = SubSolverNames::sub_solver_names(&text);
        assert!(found.is_empty(), "{} names a sub-solver item: {found:?}", path.display());
    }
    assert!(files >= 2, "the scan saw {files} files in src/protocol");
    // The scan catches what it forbids.
    assert_eq!(SubSolverNames::sub_solver_names("use piperine_digital_solver::events::LogicValue;").len(), 1);
    assert_eq!(SubSolverNames::sub_solver_names("use piperine_analog_solver::netlist::Netlist;").len(), 1);
    assert_eq!(SubSolverNames::sub_solver_names("x: piperine_analog_solver::error::Result<()>").len(), 0);
}

#[test]
fn the_stubs_replay_their_script_and_record_calls() {
    let log = ScriptedLog::new();
    let mut d = ScriptedDiscrete::new(&log);
    let mut a = ScriptedAnalog::new(&log);
    d.advances.push_back(Advance::Landing { tick: 7, cause: LandingCause::AnalogReadChanged });
    d.instants.push_back(Outcome { finish: true, ..Outcome::default() });
    d.shadows.borrow_mut().push_back(Shadow::Refused { check: "r > 0".into() });
    a.proposals.push_back(3.0);
    a.attempts.push_back(Attempt::Rejected);
    a.accepts.push_back(Accepted { fired: vec![4], samples: vec![0.5] });

    let mode = InstantMode { is_static: false, settle: true };
    assert_eq!(d.advance(10).unwrap(), Advance::Landing { tick: 7, cause: LandingCause::AnalogReadChanged });
    assert_eq!(d.advance(10).unwrap(), Advance::Reached, "an exhausted script answers Reached");
    a.deliver(&[Boundary { slot: 1, bytes: vec![1] }], Restart::Integrator);
    assert!(d.run_instant(7, &[0.5], ScriptedLog::horizon(99, 7), mode).unwrap().finish);
    assert_eq!(d.shadow(2, 0, &[9]).unwrap(), Shadow::Refused { check: "r > 0".into() });
    assert_eq!(d.shadow(2, 0, &[9]).unwrap(), Shadow::Accept);
    let m = d.mark();
    d.restore(&m);
    assert_eq!(a.propose(0.0, 5.0, None), 3.0);
    assert_eq!(a.propose(3.0, 5.0, None), 5.0, "an exhausted script proposes the stop time");
    assert_eq!(a.attempt(3.0, false).unwrap(), Attempt::Rejected);
    assert_eq!(a.attempt(3.0, false).unwrap(), Attempt::Converged);
    assert_eq!(a.accept().unwrap(), Accepted { fired: vec![4], samples: vec![0.5] });
    let am = a.mark();
    a.restore(&am);

    assert_eq!(*log.borrow(), vec![
        Call::Advance(10),
        Call::Advance(10),
        Call::Deliver(vec![Boundary { slot: 1, bytes: vec![1] }], Restart::Integrator),
        Call::RunInstant { at: 7, samples: vec![0.5], limit: ScriptedLog::horizon(99, 7), mode },
        Call::Shadow(2, vec![9]),
        Call::Shadow(2, vec![9]),
        Call::DiscreteMark(0),
        Call::DiscreteRestore(0),
        Call::Propose(0.0, 5.0),
        Call::Propose(3.0, 5.0),
        Call::Attempt { to: 3.0, landing: false },
        Call::Attempt { to: 3.0, landing: false },
        Call::Accept,
        Call::AnalogMark(0),
        Call::AnalogRestore(0),
    ]);
}

/// The names of the sub-solvers a run lists.
struct SubSolverNames;

impl SubSolverNames {
    /// Each line of `text` that names an item of a sub-solver other than the
    /// analog crate's `error` module.
    fn sub_solver_names(text: &str) -> Vec<String> {
        let mut found = Vec::new();
        for line in text.lines() {
            if line.contains("piperine_digital_solver") {
                found.push(line.trim().to_string());
            }
            let mut rest = line;
            while let Some(i) = rest.find("piperine_analog_solver") {
                let after = &rest[i + "piperine_analog_solver".len()..];
                if !after.starts_with("::error::") {
                    found.push(line.trim().to_string());
                    break;
                }
                rest = after;
            }
        }
        found
    }
}
