//! The commits a reaction makes in the settle phase with no delay that advances time (`docs/spec`
//! part V §15 S1, S6, S7): held until the owner lands them at the next time point or drops them.

use crate::net::NetId;
use crate::queue::{Pending, Queue};
use crate::store::Store;
use crate::tick::Tick;

/// One held commit: the commit as the queue will take it, and whether it replaces its statement's
/// pending commits (abi.md C3) when it lands.
#[derive(Clone, Debug, PartialEq)]
struct Held {
    /// The commit; its `due` is the instant it was made at until it lands.
    pending: Pending,
    /// The statement is `REPLACE`.
    replace: bool,
}

/// The commits a reaction made in the settle phase with no delay that advances time: held until
/// the next time point is accepted (§15 S1), then due at its microstep 0 (abi.md §10 step 4). A
/// commit with an advancing delay never comes here: it is an ordinary delayed commit (S7).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettleCommits {
    /// Each held commit, in the order the reactions made them (§12.1.2).
    held: Vec<Held>,
}

impl SettleCommits {
    /// Holds `pending`, a commit made in the settle phase with no advancing delay (§15 S1).
    pub(crate) fn hold(&mut self, pending: Pending, replace: bool) {
        self.held.push(Held { pending, replace });
    }

    /// Whether a commit is held.
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// Makes every held commit due at microstep 0 of `tick`, the next time point (§15 S1; §15 S6
    /// where time does not advance, the next sweep point). They go to `queue` in the order they
    /// were made, so the winner of a net under §12.1.2 is the one the reactions made last.
    pub(crate) fn land(&mut self, tick: Tick, queue: &mut Queue) {
        for held in self.held.drain(..) {
            let mut pending = held.pending;
            pending.due = (tick, 0);
            queue.push(pending, held.replace);
        }
    }

    /// Forgets every held commit: the stop time was reached, or the sweep's last point (§15 S6).
    pub(crate) fn drop_all(&mut self) {
        self.held.clear();
    }

    /// The nets the analog side reads and the bytes the held commits will give them, in the order
    /// of each net's first commit: what the solve that arrives at the next time point reads (§15 S2).
    /// `store` holds the nets as the settle phase left them.
    pub(crate) fn boundary(&self, specs: &[crate::net::NetSpec], store: &Store) -> Vec<(NetId, Vec<u8>)> {
        let mut nets: Vec<(NetId, Vec<u8>)> = Vec::new();
        for held in &self.held {
            let net = held.pending.net;
            if !specs[net.0 as usize].analog_reads {
                continue;
            }
            let at = match nets.iter().position(|(n, _)| *n == net) {
                Some(at) => at,
                None => {
                    nets.push((net, store.analog_value(net, &specs[net.0 as usize])));
                    nets.len() - 1
                }
            };
            let offset = u64::from(held.pending.offset) + held.pending.element.map_or(0, |e| e * u64::from(specs[net.0 as usize].size));
            store.patch_analog(net, &specs[net.0 as usize], offset, &held.pending.bytes, &mut nets[at].1);
        }
        nets
    }
}
