//! Regression tests for the round-1 verification findings of
//! `.specs/features/solver-abi-digital/verification.md` (probes of the
//! Verifier, made permanent): HORIZON after the reactions of the microstep,
//! a settled RELAX group leaves nothing due, the RELAX rule per cycle,
//! a never-due REPLACE, host writes range-checked, P3 on every write.

#[path = "support/devices.rs"]
#[allow(dead_code)] // each test target uses the devices it needs
mod devices;

use devices::*;
use piperine_abi::layout::HIGH;
use piperine_digital_solver::abi::engine::{Digital, Horizon, Mode};
use piperine_digital_solver::abi::net::NetId;
use piperine_digital_solver::abi::tick::{Resolution, Tick};
use piperine_digital_solver::limits::DigitalLimits;

fn engine(devices: Vec<Placed<'_>>) -> Digital {
    Digital::new(bind_by_name(devices, Resolution::default()).expect("binds"), DigitalLimits::default())
}

fn net(d: &Digital, name: &str) -> NetId {
    net_named(d.plan(), name)
}

fn instant(d: &mut Digital, at: Tick) {
    d.run_instant(at, &[], Horizon::uniform(Tick(i64::MAX)), Mode::default()).unwrap();
}

#[test]
fn a_settled_relax_group_leaves_nothing_due_after_its_instant() {
    // finding 2, F3: the group's own closing write must not re-dirty it
    reset_calls();
    let mut d = engine(vec![
        ("res", resolver(), vec![("d0", "d"), ("d1", "e"), ("v", "v")]),
        ("keep", and2(), vec![("a", "v"), ("b", "v"), ("y", "d")]),
    ]);
    d.start().unwrap();
    d.host_commit(net(&d, "e"), 0, &HIGH, Tick(0)).unwrap();
    instant(&mut d, Tick(0));
    assert_eq!(d.read(net(&d, "v")), HIGH);
    assert_eq!(d.next_due(), None, "nothing queued, nothing changed: no second instant at tick 0");
    let before = calls("resolver").0;
    d.advance(Tick(100)).unwrap();
    assert_eq!(calls("resolver").0, before, "and the group's functions are not called again");
}
