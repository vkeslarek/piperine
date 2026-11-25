//! The values a net the analog side reads passes through within one instant (docs/spec Part V
//! §15.2 rule 4): a value the net holds at the end of one microstep and loses at a later
//! microstep of the same instant, such as the high of a one-microstep pulse. The analog side
//! reads only what an instant leaves behind, so without this record a pulse that ends where it
//! began would never reach it; the owner instead reads, at the instant, the state
//! `read_state` builds, by a zero-width solve.

use std::collections::BTreeMap;

use crate::net::{NetId, NetSpec};
use crate::store::Store;

/// What the analog-read nets held during one run of the microstep loop.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PassingValues {
    /// Each analog-read net that changed in the instant, with the value it held after its last
    /// change so far.
    held: BTreeMap<NetId, Vec<u8>>,
    /// Each analog-read net that lost a value it held, with the first value it lost.
    passed: BTreeMap<NetId, Vec<u8>>,
}

impl PassingValues {
    /// Notes that the analog-read `net` changed at the end of a microstep, to `value` (an array net's
    /// windows): a value it already held in this instant is lost, and the first one lost is kept.
    pub(crate) fn changed(&mut self, net: NetId, value: &[u8]) {
        if let Some(lost) = self.held.insert(net, value.to_vec()) {
            self.passed.entry(net).or_insert(lost);
        }
    }

    /// Forgets what the owner has read (`Digital::reopen`): the values passed through and the
    /// values held, which its last solve read. The record starts again from the values the nets
    /// hold now, so a net that changes once from here passes through nothing.
    pub(crate) fn consume(&mut self) {
        self.passed.clear();
        self.held.clear();
    }

    /// Whether no analog-read net passed through a value.
    pub(crate) fn is_empty(&self) -> bool {
        self.passed.is_empty()
    }

    /// The state the zero-width solve at the instant reads: every analog-read net the instant changed,
    /// at the first value it passed through when it passed through one, and at its value as the
    /// store holds it otherwise; an array net as the analog side reads it, its windows (`specs`). Empty when no net passed
    /// through a value.
    pub(crate) fn read_state(&self, store: &Store, specs: &[NetSpec]) -> Vec<(NetId, Vec<u8>)> {
        if self.is_empty() {
            return Vec::new();
        }
        self.held
            .keys()
            .map(|net| match self.passed.get(net) {
                Some(value) => (*net, value.clone()),
                None => (*net, store.analog_value(*net, &specs[net.0 as usize])),
            })
            .collect()
    }
}
