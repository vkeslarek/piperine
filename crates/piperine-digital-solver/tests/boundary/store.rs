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

#[test]
fn a_net_starts_as_zero_bytes() {
    let s = StoreCases::store();
    assert_eq!(s.value(BIT), [0]);
    assert_eq!(s.value(REAL), [0u8; 8]);
    assert_eq!(s.previous(BIT), [0]);
    assert!(!s.changed(BIT) && !s.changed(REAL));
    assert!(!s.fired(EVENT));
    assert_eq!(s.value(EVENT), []);
}

#[test]
fn a_net_is_aligned_for_its_layout() {
    let s = StoreCases::store();
    assert_eq!(s.value(REAL).as_ptr() as usize % 8, 0, "a REAL is 8-aligned, so a device can read an F64");
    assert_eq!(s.previous(REAL).as_ptr() as usize % 8, 0);
    assert_eq!(s.value(STATE).as_ptr() as usize % 4, 0);
}

#[test]
fn a_write_of_equal_bytes_is_not_a_change() {
    let mut s = StoreCases::store();
    assert!(!s.write(BIT, 0, &LOW[..1]), "zero over zero");
    assert!(!s.changed(BIT));
    assert!(s.write(BIT, 0, &HIGH));
    assert!(!s.write(BIT, 0, &HIGH), "the same bytes again");
    // zero bytes are `0.0`; `0.0` to `-0.0` is a change; the same NaN bytes are not
    assert!(!s.write(REAL, 0, &StoreCases::real(0.0)), "0.0 over the zero bytes");
    assert!(s.write(REAL, 0, &StoreCases::real(-0.0)), "0.0 to -0.0 differs bytewise");
    assert!(s.write(REAL, 0, &StoreCases::real(f64::NAN)));
    assert!(!s.write(REAL, 0, &StoreCases::real(f64::NAN)), "the same NaN bytes");
}

#[test]
fn a_change_keeps_the_previous_bytes() {
    let mut s = StoreCases::store();
    s.write(BIT, 0, &HIGH);
    assert_eq!(s.value(BIT), HIGH);
    assert_eq!(s.previous(BIT), [0], "before the first change: zero");
    s.begin_microstep();
    s.write(BIT, 0, &LOW);
    assert_eq!(s.value(BIT), LOW);
    assert_eq!(s.previous(BIT), HIGH);
    // a write that changes nothing keeps `previous`
    s.begin_microstep();
    s.write(BIT, 0, &LOW);
    assert_eq!(s.previous(BIT), HIGH);
    // a partial write at an offset changes only those bytes
    s.write(REAL, 7, &[1]);
    assert_eq!(s.value(REAL), {
        let mut b = [0u8; 8];
        b[7] = 1;
        b
    });
}

#[test]
fn changed_reads_one_for_exactly_one_microstep() {
    let mut s = StoreCases::store();
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
    let mut s = StoreCases::store();
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
    let mut s = StoreCases::store();
    assert!(s.write(STATE, 0, &[1, 2, 3, 4]), "a quiet net still stores its bytes");
    assert_eq!(s.value(STATE), [1, 2, 3, 4]);
    assert_eq!(s.previous(STATE), [], "no PREVIOUS");
    assert!(!s.changed(STATE), "no CHANGED");
}

#[test]
fn a_clone_compares_equal_byte_for_byte() {
    let mut s = StoreCases::store();
    s.write(BIT, 0, &HIGH);
    s.fire(EVENT);
    let c = s.clone();
    assert_eq!(c, s);
    let mut d = c.clone();
    d.write(REAL, 0, &StoreCases::real(1.0));
    assert_ne!(d, s);
    assert_eq!(c.value(REAL), [0u8; 8], "a clone is independent");
    assert_eq!(c.value(REAL).as_ptr() as usize % 8, 0, "a clone is aligned too");
}

/// HYG-14 AC 1: a rewind puts back every net written since the mark, the microstep and the
/// stamps behind `CHANGED`, so the store equals the copy taken at the mark; a net written twice
/// is journaled once, and marks nest.
#[test]
fn a_store_rewind_restores_every_written_net() {
    let mut s = StoreCases::store();
    s.write(BIT, 0, &HIGH);
    s.begin_microstep();
    let outer_copy = s.clone();
    let outer = s.mark();
    s.write(BIT, 0, &LOW);
    s.write(BIT, 0, &HIGH); // the second write of a net adds no entry
    s.write(REAL, 7, &[1]);
    s.begin_microstep();
    s.fire(EVENT);
    assert_eq!(s.journal_len(), 3, "the bit, the real and the event: one entry each");
    let inner_copy = s.clone();
    let inner = s.mark();
    s.write(REAL, 0, &StoreCases::real(2.5)); // the real again: journaled for the inner mark
    s.write(STATE, 0, &[9, 9, 9, 9]);
    s.begin_microstep();
    assert_eq!(s.journal_len(), 5);
    s.rewind(&inner);
    assert_eq!(s, inner_copy, "the inner rewind equals the copy at the inner mark");
    s.release(inner);
    s.rewind(&outer);
    assert_eq!(s, outer_copy, "the outer rewind equals the copy at the outer mark");
    assert!(!s.fired(EVENT) && !s.changed(REAL));
    // the mark is still open: a second rewind after more writes works too
    s.write(STATE, 0, &[1, 2, 3, 4]);
    s.rewind(&outer);
    assert_eq!(s, outer_copy);
    s.release(outer);
    assert_eq!(s.journal_len(), 0, "the outermost release empties the log");
    s.write(BIT, 0, &LOW);
    assert_eq!(s.journal_len(), 0, "no mark open: nothing is journaled");
}

#[test]
fn a_seed_is_a_value_that_was_never_a_change() {
    // Feature `analog-structure` AS-14 (§5.3.1): a param's start value is its value from the start:
    // `PREVIOUS` holds the same bytes, `CHANGED` is false, and a later write is a change from it.
    let mut s = StoreCases::store();
    s.seed(REAL, &StoreCases::real(2.0));
    assert_eq!(s.value(REAL), StoreCases::real(2.0));
    assert_eq!(s.previous(REAL), StoreCases::real(2.0), "PREVIOUS is the seed itself");
    assert!(!s.changed(REAL), "a seed is no change");
    assert!(s.write(REAL, 0, &StoreCases::real(5.0)));
    assert!(s.changed(REAL));
    assert_eq!(s.previous(REAL), StoreCases::real(2.0), "the write keeps the seed as its past");
}

/// The stores the store tests build.
pub struct StoreCases;

impl StoreCases {
    /// The store helper `store`.
    fn store() -> Store {
        Store::new(&[
            NetSpec::discrete("bit", 1, 1),
            NetSpec::discrete("real", 8, 8),
            NetSpec::event("ev"),
            NetSpec { quiet: true, ..NetSpec::discrete("state", 4, 4) },
        ])
    }

    /// The real helper `real`.
    fn real(v: f64) -> [u8; 8] {
        v.to_le_bytes()
    }
}
