//! The byte store of the ABI engine (feature `solver-abi-digital`, SAD-04;
//! abi.md §3.1 P2, P3, P7, §4.1): bytes per net that start at zero, a change
//! as a bytewise difference, `PREVIOUS`, `CHANGED` for one microstep, event
//! nets that fire without bytes, `QUIET` nets that keep no previous.

use piperine_abi::layout::{HIGH, LOW};
use piperine_digital_solver::net::{NetId, NetSpec};
use piperine_digital_solver::store::Store;

const BIT: NetId = NetId(0);
const REAL: NetId = NetId(1);
const EVENT: NetId = NetId(2);
const STATE: NetId = NetId(3);

fn store() -> Store {
    Store::new(&[
        NetSpec::discrete("bit", 2, 1),
        NetSpec::discrete("real", 16, 8),
        NetSpec::event("ev"),
        NetSpec { quiet: true, ..NetSpec::discrete("state", 4, 4) },
    ])
}

fn real(v: f64, known: u8) -> [u8; 16] {
    let mut b = [0u8; 16];
    b[..8].copy_from_slice(&v.to_le_bytes());
    b[8] = known;
    b
}

#[test]
fn a_net_starts_as_zero_bytes() {
    let s = store();
    assert_eq!(s.value(BIT), [0, 0]);
    assert_eq!(s.value(REAL), [0u8; 16]);
    assert_eq!(s.previous(BIT), [0, 0]);
    assert!(!s.changed(BIT) && !s.changed(REAL));
    assert!(!s.fired(EVENT));
    assert_eq!(s.value(EVENT), []);
}

#[test]
fn a_net_is_aligned_for_its_layout() {
    let s = store();
    assert_eq!(s.value(REAL).as_ptr() as usize % 8, 0, "a REAL is 8-aligned, so a device can read an F64");
    assert_eq!(s.previous(REAL).as_ptr() as usize % 8, 0);
    assert_eq!(s.value(STATE).as_ptr() as usize % 4, 0);
}

#[test]
fn a_write_of_equal_bytes_is_not_a_change() {
    let mut s = store();
    assert!(!s.write(BIT, 0, &LOW[..1]), "zero over zero");
    assert!(!s.changed(BIT));
    assert!(s.write(BIT, 0, &HIGH));
    assert!(!s.write(BIT, 0, &HIGH), "the same bytes again");
    // 0.0 to -0.0 is a change; the same NaN bytes are not
    assert!(s.write(REAL, 0, &real(0.0, 1)));
    assert!(s.write(REAL, 0, &real(-0.0, 1)), "0.0 to -0.0 differs bytewise");
    assert!(s.write(REAL, 0, &real(f64::NAN, 1)));
    assert!(!s.write(REAL, 0, &real(f64::NAN, 1)), "the same NaN bytes");
}

#[test]
fn a_change_keeps_the_previous_bytes() {
    let mut s = store();
    s.write(BIT, 0, &HIGH);
    assert_eq!(s.value(BIT), HIGH);
    assert_eq!(s.previous(BIT), [0, 0], "before the first change: zero");
    s.begin_microstep();
    s.write(BIT, 0, &LOW);
    assert_eq!(s.value(BIT), LOW);
    assert_eq!(s.previous(BIT), HIGH);
    // a write that changes nothing keeps `previous`
    s.begin_microstep();
    s.write(BIT, 0, &LOW);
    assert_eq!(s.previous(BIT), HIGH);
    // a partial write at an offset changes only those bytes
    s.write(REAL, 8, &[1]);
    assert_eq!(s.value(REAL), {
        let mut b = [0u8; 16];
        b[8] = 1;
        b
    });
}

#[test]
fn changed_reads_one_for_exactly_one_microstep() {
    let mut s = store();
    assert!(!s.changed(BIT));
    s.write(BIT, 0, &HIGH);
    assert!(s.changed(BIT), "in the microstep of the change");
    s.begin_microstep();
    assert!(!s.changed(BIT), "0 in the next one");
    s.begin_microstep();
    assert!(!s.changed(BIT));
}

#[test]
fn an_event_net_fires_without_bytes() {
    let mut s = store();
    assert!(!s.fired(EVENT));
    s.fire(EVENT);
    assert!(s.fired(EVENT));
    assert!(s.changed(EVENT), "CHANGED of an event pin is 1 when it fired");
    assert_eq!(s.value(EVENT), []);
    s.begin_microstep();
    assert!(!s.fired(EVENT));
}

#[test]
fn a_quiet_net_keeps_no_previous() {
    let mut s = store();
    assert!(s.write(STATE, 0, &[1, 2, 3, 4]), "a quiet net still stores its bytes");
    assert_eq!(s.value(STATE), [1, 2, 3, 4]);
    assert_eq!(s.previous(STATE), [], "no PREVIOUS");
    assert!(!s.changed(STATE), "no CHANGED");
}

#[test]
fn a_clone_compares_equal_byte_for_byte() {
    let mut s = store();
    s.write(BIT, 0, &HIGH);
    s.fire(EVENT);
    let c = s.clone();
    assert_eq!(c, s);
    let mut d = c.clone();
    d.write(REAL, 0, &real(1.0, 1));
    assert_ne!(d, s);
    assert_eq!(c.value(REAL), [0u8; 16], "a clone is independent");
    assert_eq!(c.value(REAL).as_ptr() as usize % 8, 0, "a clone is aligned too");
}
