//! The bytes of every net (abi.md §3.1, §4.1): the value, the `PREVIOUS`
//! bytes, and the microstep stamps behind `CHANGED` and a fired event.
//!
//! A device reads a cell through a typed lens that refuses a misaligned
//! buffer, so each net is stored over-aligned to 16 bytes. The alignment is
//! found, not assumed, so the store needs no `unsafe`.

use crate::journal::Journal;
use crate::net::{NetId, NetKind, NetSpec};

/// The over-alignment of every buffer (the largest leaf is 8 bytes).
const ALIGN: usize = 16;

/// `len` bytes starting on a 16-byte boundary inside a larger allocation.
#[derive(Debug)]
pub(crate) struct Aligned {
    buf: Vec<u8>,
    start: usize,
    len: usize,
}

impl Aligned {
    pub(crate) fn zeroed(len: usize) -> Aligned {
        let buf = vec![0u8; len + ALIGN];
        let start = buf.as_ptr().align_offset(ALIGN);
        Aligned { buf, start, len }
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.buf[self.start..self.start + self.len]
    }

    pub(crate) fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.buf[self.start..self.start + self.len]
    }
}

impl Clone for Aligned {
    /// A new allocation has its own alignment offset: the bytes are copied, not the buffer.
    fn clone(&self) -> Aligned {
        let mut c = Aligned::zeroed(self.len);
        c.bytes_mut().copy_from_slice(self.bytes());
        c
    }
}

impl PartialEq for Aligned {
    fn eq(&self, other: &Aligned) -> bool {
        self.bytes() == other.bytes()
    }
}

#[derive(Clone, Debug, PartialEq)]
struct NetBytes {
    kind: NetKind,
    quiet: bool,
    value: Aligned,
    previous: Aligned,
    /// The microstep of the last change (0: never).
    changed_at: u64,
    /// The microstep of the last firing (0: never).
    fired_at: u64,
}

/// One net's bytes and stamps, saved to put it back (the relaxation of a cycle writes
/// intermediate values the run must not see).
#[derive(Clone, Debug, PartialEq)]
pub struct NetState(NetBytes);

/// One net's state before its first change under a mark: the stamps and where its bytes sit in
/// the journal's arena (value, then previous). The buffers keep their size, so a rewind copies
/// the bytes back in place.
#[derive(Debug)]
struct Saved {
    net: usize,
    changed_at: u64,
    fired_at: u64,
    at: usize,
}

/// The first previous state of each net written since a mark (HYG-14). The bytes live in one
/// arena, so journaling a net allocates nothing once the arena has grown.
#[derive(Debug)]
struct NetJournal {
    log: Journal<Saved>,
    arena: Vec<u8>,
    /// Bumped by every mark and rewind: a net stamped with the current epoch is already in the
    /// log for the innermost mark.
    epoch: u32,
    stamped: Vec<u32>,
}

impl Clone for NetJournal {
    /// A copy of a store carries no work in progress: an empty, closed log.
    fn clone(&self) -> NetJournal {
        NetJournal { log: Journal::default(), arena: Vec::new(), epoch: 1, stamped: vec![0; self.stamped.len()] }
    }
}

impl PartialEq for NetJournal {
    fn eq(&self, _: &NetJournal) -> bool {
        true
    }
}

/// Where a store stood when it was marked: the length of its log and its microstep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoreMark {
    len: usize,
    arena: usize,
    step: u64,
}

/// Every net's bytes at one point of a run.
#[derive(Clone, Debug, PartialEq)]
pub struct Store {
    nets: Vec<NetBytes>,
    /// The current microstep; starts at 1 so a stamp of 0 means "never".
    step: u64,
    journal: NetJournal,
}

impl Store {
    /// A store with every net at zero bytes.
    pub fn new(specs: &[NetSpec]) -> Store {
        let nets: Vec<NetBytes> = specs
            .iter()
            .map(|s| {
                let quiet = s.is_quiet();
                let size = s.size as usize;
                NetBytes {
                    kind: s.kind,
                    quiet,
                    value: Aligned::zeroed(size),
                    previous: Aligned::zeroed(if quiet { 0 } else { size }),
                    changed_at: 0,
                    fired_at: 0,
                }
            })
            .collect();
        let journal = NetJournal { log: Journal::default(), arena: Vec::new(), epoch: 1, stamped: vec![0; nets.len()] };
        Store { nets, step: 1, journal }
    }

    /// Starts journaling: from here to the matching `release`, the first previous state of each
    /// net written is kept, so `rewind` can put the store back without a copy of every net.
    /// Marks nest.
    pub fn mark(&mut self) -> StoreMark {
        self.journal.epoch += 1;
        StoreMark { len: self.journal.log.open(), arena: self.journal.arena.len(), step: self.step }
    }

    /// Puts every net written since `mark` back, and the microstep. The mark stays open.
    pub fn rewind(&mut self, mark: &StoreMark) {
        while let Some(saved) = self.journal.log.pop_above(mark.len) {
            let net = &mut self.nets[saved.net];
            let (value, previous) = (net.value.len, net.previous.len);
            let bytes = &self.journal.arena[saved.at..saved.at + value + previous];
            net.value.bytes_mut().copy_from_slice(&bytes[..value]);
            net.previous.bytes_mut().copy_from_slice(&bytes[value..]);
            net.changed_at = saved.changed_at;
            net.fired_at = saved.fired_at;
        }
        self.journal.arena.truncate(mark.arena);
        self.step = mark.step;
        self.journal.epoch += 1;
    }

    /// Ends the newest open mark; the log empties with the outermost.
    pub fn release(&mut self, _mark: StoreMark) {
        self.journal.log.close();
        if !self.journal.log.is_open() {
            self.journal.arena.clear();
        }
    }

    /// The number of nets the log holds a previous state of (a diagnostic).
    pub fn journal_len(&self) -> usize {
        self.journal.log.len()
    }

    /// Keeps `net`'s state before its first change under the innermost mark.
    fn journal_net(&mut self, net: usize) {
        if self.journal.log.is_open() && self.journal.stamped[net] != self.journal.epoch {
            self.journal.stamped[net] = self.journal.epoch;
            let n = &self.nets[net];
            let saved = Saved { net, changed_at: n.changed_at, fired_at: n.fired_at, at: self.journal.arena.len() };
            self.journal.arena.extend_from_slice(n.value.bytes());
            self.journal.arena.extend_from_slice(n.previous.bytes());
            self.journal.log.push(saved);
        }
    }

    fn net(&self, net: NetId) -> &NetBytes {
        &self.nets[net.0 as usize]
    }

    /// The bytes of `net` (`VALUE`).
    pub fn value(&self, net: NetId) -> &[u8] {
        self.net(net).value.bytes()
    }

    /// The bytes before the most recent change (`PREVIOUS`); empty for a `QUIET` net.
    pub fn previous(&self, net: NetId) -> &[u8] {
        self.net(net).previous.bytes()
    }

    /// `CHANGED`: the net changed in this microstep, or an event net fired in it.
    /// Always false for a `QUIET` net.
    pub fn changed(&self, net: NetId) -> bool {
        let n = self.net(net);
        match n.kind {
            NetKind::Event => n.fired_at == self.step,
            NetKind::Discrete => !n.quiet && n.changed_at == self.step,
        }
    }

    /// Whether an event net fired in this microstep.
    pub fn fired(&self, net: NetId) -> bool {
        let n = self.net(net);
        n.kind == NetKind::Event && n.fired_at == self.step
    }

    /// Writes `bytes` at `offset`; returns whether the net's bytes differ
    /// afterwards. A change keeps the old bytes as `PREVIOUS` (unless `QUIET`)
    /// and stamps the current microstep. Panics when the range leaves the net.
    pub fn write(&mut self, net: NetId, offset: u32, bytes: &[u8]) -> bool {
        let step = self.step;
        let at = offset as usize;
        if self.nets[net.0 as usize].value.bytes()[at..at + bytes.len()] == *bytes {
            return false;
        }
        self.journal_net(net.0 as usize);
        let n = &mut self.nets[net.0 as usize];
        if !n.quiet {
            n.previous.bytes_mut().copy_from_slice(n.value.bytes());
            n.changed_at = step;
        }
        n.value.bytes_mut()[at..at + bytes.len()].copy_from_slice(bytes);
        true
    }

    /// Gives `net` the value `bytes` as the value it had from the start: `PREVIOUS` is the same
    /// bytes and nothing is stamped as a change. For the initial value of a param, which is no change.
    pub fn seed(&mut self, net: NetId, bytes: &[u8]) {
        self.journal_net(net.0 as usize);
        let n = &mut self.nets[net.0 as usize];
        n.value.bytes_mut()[..bytes.len()].copy_from_slice(bytes);
        if !n.quiet {
            n.previous.bytes_mut()[..bytes.len()].copy_from_slice(bytes);
        }
    }

    /// Makes the value `net` holds its `PREVIOUS` as well, keeping the stamp of the change that wrote it:
    /// after the commit of an initial value (`start`), `prev()` reads that value until a later change
    /// (part II §5.3, D8).
    pub fn keep_as_previous(&mut self, net: NetId) {
        self.journal_net(net.0 as usize);
        let n = &mut self.nets[net.0 as usize];
        if !n.quiet {
            let value = n.value.bytes().to_vec();
            n.previous.bytes_mut().copy_from_slice(&value);
        }
    }

    /// The saved state of `net`.
    pub fn save_net(&self, net: NetId) -> NetState {
        NetState(self.net(net).clone())
    }

    /// Puts `net` back as `state` saved it.
    pub fn restore_net(&mut self, net: NetId, state: &NetState) {
        self.journal_net(net.0 as usize);
        self.nets[net.0 as usize] = state.0.clone();
    }

    /// Fires an event net in this microstep.
    pub fn fire(&mut self, net: NetId) {
        self.journal_net(net.0 as usize);
        let step = self.step;
        self.nets[net.0 as usize].fired_at = step;
    }

    /// Starts the next microstep: every `CHANGED` and every firing of the last one reads 0.
    pub fn begin_microstep(&mut self) {
        self.step += 1;
    }
}
