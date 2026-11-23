//! What the engine knows of a net: its identity and its description, taken
//! from the pins bound to it. The owner assigns the ids.

/// A net of the design, numbered by the owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NetId(pub u32);

/// A discrete net holds bytes; an event net holds nothing and fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetKind {
    Discrete,
    Event,
}

/// One net, as the owner hands it to bind.
#[derive(Clone, Debug, PartialEq)]
pub struct NetSpec {
    pub name: String,
    /// Bytes held; 0 for an event net.
    pub size: u32,
    pub align: u32,
    pub kind: NetKind,
    /// P7: no `PREVIOUS`, no `CHANGED`, never keeps a microstep alive.
    pub quiet: bool,
    /// P7: delivers every commit as a record; implies `quiet`.
    pub record: bool,
    /// P7: a function cycle may pass through this net.
    pub relax: bool,
    /// The analog side reads this net (R4's boundary).
    pub analog_reads: bool,
    /// A `CROSS` watch writes this event net: what it reaches has the `crossing` horizon (abi.md R5).
    pub crossing: bool,
    /// A host write during a run is refused (P5).
    pub structural: bool,
    /// A param's pin: what it holds when an analysis starts is its value, not a change, so `CHANGED`
    /// is false and `PREVIOUS` is the value until the first `set` during the run (§5.3.1).
    pub param: bool,
    /// The first commit at (0, 0); empty for none. An array net holds one element's bytes (every element starts
    /// there) or every element's bytes.
    pub init: Vec<u8>,
    /// 0: one carrier. `>= 1`: an array net of this many elements of `size` bytes (abi.md §3.1 P4a).
    pub count: u64,
}

impl NetSpec {
    /// A discrete net of `size` bytes aligned to `align`.
    pub fn discrete(name: &str, size: u32, align: u32) -> NetSpec {
        NetSpec {
            name: name.to_string(),
            size,
            align,
            kind: NetKind::Discrete,
            quiet: false,
            record: false,
            relax: false,
            analog_reads: false,
            crossing: false,
            structural: false,
            param: false,
            init: Vec::new(),
            count: 0,
        }
    }

    /// An array net of `count` elements of `size` bytes each, aligned to `align`.
    pub fn array(name: &str, size: u32, align: u32, count: u64) -> NetSpec {
        NetSpec { count, ..NetSpec::discrete(name, size, align) }
    }

    /// An event net.
    pub fn event(name: &str) -> NetSpec {
        NetSpec { kind: NetKind::Event, ..NetSpec::discrete(name, 0, 1) }
    }

    /// Whether the net keeps no change detection (`QUIET`, and `RECORD` implies it).
    pub fn is_quiet(&self) -> bool {
        self.quiet || self.record
    }
}
