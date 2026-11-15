//! The bytes of every net (abi.md §3.1, §4.1): the value, the `PREVIOUS`
//! bytes, and the microstep stamps behind `CHANGED` and a fired event.
//!
//! A device reads a cell through a typed lens that refuses a misaligned
//! buffer, so each net is stored over-aligned to 16 bytes. The alignment is
//! found, not assumed, so the store needs no `unsafe`.

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

/// Every net's bytes at one point of a run.
#[derive(Clone, Debug, PartialEq)]
pub struct Store {
    nets: Vec<NetBytes>,
    /// The current microstep; starts at 1 so a stamp of 0 means "never".
    step: u64,
}

impl Store {
    /// A store with every net at zero bytes.
    pub fn new(specs: &[NetSpec]) -> Store {
        let nets = specs
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
        Store { nets, step: 1 }
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
        let n = &mut self.nets[net.0 as usize];
        let at = offset as usize;
        if n.value.bytes()[at..at + bytes.len()] == *bytes {
            return false;
        }
        if !n.quiet {
            n.previous.bytes_mut().copy_from_slice(n.value.bytes());
            n.changed_at = step;
        }
        n.value.bytes_mut()[at..at + bytes.len()].copy_from_slice(bytes);
        true
    }

    /// The saved state of `net`.
    pub fn save_net(&self, net: NetId) -> NetState {
        NetState(self.net(net).clone())
    }

    /// Puts `net` back as `state` saved it.
    pub fn restore_net(&mut self, net: NetId, state: &NetState) {
        self.nets[net.0 as usize] = state.0.clone();
    }

    /// Fires an event net in this microstep.
    pub fn fire(&mut self, net: NetId) {
        let step = self.step;
        self.nets[net.0 as usize].fired_at = step;
    }

    /// Starts the next microstep: every `CHANGED` and every firing of the last one reads 0.
    pub fn begin_microstep(&mut self) {
        self.step += 1;
    }
}
