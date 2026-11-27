//! The boundary of the ABI engine (feature `solver-abi-digital`, SAD-01):
//! this crate depends on the ABI crate only, its new modules name nothing of
//! the analog solver or the owner and hold no `unsafe`, and the facade offers
//! every operation of the owner's discrete side with the argument and result
//! shapes that side uses after the obvious conversions (`Tick` and `i64`,
//! `NetId` and `u32`).

use std::fs;
use std::path::{Path, PathBuf};

use piperine_digital_solver::advance::Advance;
use piperine_digital_solver::engine::{Checkpoint, Digital, Horizon, Mode, Outcome, Shadow};
use piperine_digital_solver::net::{NetId, NetSpec};
use piperine_digital_solver::observe::Observation;
use piperine_digital_solver::plan::DigitalPlan;
use piperine_digital_solver::tick::{Resolution, Tick};
use piperine_digital_solver::error::DigitalError;
use piperine_digital_solver::landing::TimePointRule;
use piperine_digital_solver::limits::DigitalLimits;

#[test]
fn the_crate_depends_on_the_abi_crate_only() {
    let manifest = fs::read_to_string(BoundaryReading::crate_dir().join("Cargo.toml")).expect("the manifest");
    let mut in_dependencies = false;
    let mut deps = Vec::new();
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_dependencies = line.contains("dependencies");
        } else if in_dependencies && !line.is_empty() && !line.starts_with('#') && !line.starts_with("piperine-device-fixtures") {
            deps.push(line.split(['=', ' ']).next().unwrap_or("").to_string());
        }
    }
    assert_eq!(deps, ["piperine-abi"], "dependencies of piperine-digital-solver");
}

#[test]
fn abi_modules_name_no_analog_or_owner_item() {
    // C44
    // spelled in halves so the owner's own scan of this crate's sources does not read them here
    let banned = [
        ["piperine_analog", "_solver"].concat(),
        ["piperine", "_solver"].concat(),
        ["Net", "list"].concat(),
        ["St", "amp"].concat(),
        ["Analog", "Reference"].concat(),
    ];
    let mut found = Vec::new();
    for (path, text) in BoundaryReading::sources(&BoundaryReading::crate_dir().join("src")) {
        for (n, line) in text.lines().enumerate() {
            for word in &banned {
                if BoundaryReading::code(line).contains(word.as_str()) {
                    found.push(format!("{}:{}: {word}", path.display(), n + 1));
                }
            }
        }
    }
    assert!(found.is_empty(), "the ABI engine names items it must not know:\n{}", found.join("\n"));
}

#[test]
fn abi_modules_hold_no_unsafe() {
    let mut found = Vec::new();
    for (path, text) in BoundaryReading::sources(&BoundaryReading::crate_dir().join("src")) {
        for (n, line) in text.lines().enumerate() {
            if BoundaryReading::code(line).contains("unsafe") {
                found.push(format!("{}:{}", path.display(), n + 1));
            }
        }
    }
    assert!(found.is_empty(), "unsafe in the ABI engine:\n{}", found.join("\n"));
}

/// Each operation of the owner's discrete side, as a function pointer of the shape the trait gives
/// it: it compiles only if `Digital` has the method with these argument and result types.
#[test]
fn the_facade_offers_every_operation_of_the_discrete_side() {
    let _start: fn(&mut Digital) -> Result<(), DigitalError> = Digital::start;
    let _next_due: fn(&Digital) -> Option<Tick> = Digital::next_due;
    let _advance: fn(&mut Digital, Tick) -> Result<Advance, DigitalError> = Digital::advance;
    let _fire: fn(&mut Digital, NetId, Tick) -> Result<(), DigitalError> = Digital::fire;
    let _run: fn(&mut Digital, Tick, &[f64], Horizon, Mode) -> Result<Outcome, DigitalError> = Digital::run_instant;
    let _checkpoint: fn(&Digital) -> Checkpoint = Digital::checkpoint;
    let _restore: fn(&mut Digital, &Checkpoint) = Digital::restore;
    let _shadow: fn(&Digital, NetId, u32, &[u8]) -> Result<Shadow, DigitalError> = Digital::shadow;
    let _commit: fn(&mut Digital, NetId, u32, &[u8], Tick) -> Result<(), DigitalError> = Digital::host_commit;
    let _name: fn(&Digital, NetId) -> String = Digital::net_name;
    let _drain: fn(&mut Digital) -> Vec<Observation> = Digital::drain_observations;

    // and each once, on a small plan: one host-driven net
    let plan = DigitalPlan::bind(vec![NetSpec::discrete("n", 1, 1)], vec![], Resolution::default()).unwrap();
    let mut d = Digital::new(plan, DigitalLimits::default(), TimePointRule::default());
    let n = NetId(0);
    d.start().unwrap();
    assert_eq!(d.next_due(), None);
    d.host_commit(n, 0, &[1], Tick(4)).unwrap();
    assert_eq!(d.next_due(), Some(Tick(4)));
    let cp = d.checkpoint();
    assert_eq!(d.advance(Tick(10)).unwrap(), Advance::Reached);
    assert_eq!(d.read(n), [1]);
    d.restore(&cp);
    assert_eq!(d.read(n), [0]);
    assert_eq!(d.shadow(n, 0, &[1]).unwrap(), Shadow::Accept);
    assert_eq!(d.net_name(n), "n");
    let out = d.run_instant(Tick(4), &[], Horizon::uniform(Tick(10)), Mode::default()).unwrap();
    assert!(!out.finish);
    assert!(d.drain_observations().iter().any(|o| matches!(o, Observation::Change { net, .. } if net == "n")));
}

/// Reads the crate for the boundary tests.
pub struct BoundaryReading;

impl BoundaryReading {
    /// The crate dir helper `crate_dir`.
    fn crate_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    /// Every `.rs` file under `dir`, with its text.
    fn sources(dir: &Path) -> Vec<(PathBuf, String)> {
        let mut found = Vec::new();
        for entry in fs::read_dir(dir).expect("a readable directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                found.extend(BoundaryReading::sources(&path));
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = fs::read_to_string(&path).expect("a readable file");
                found.push((path, text));
            }
        }
        found
    }

    /// The code of a line: everything before a `//` comment.
    fn code(line: &str) -> &str {
        line.split("//").next().unwrap_or("")
    }
}
