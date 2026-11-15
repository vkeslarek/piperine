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
    /// A host write during a run is refused (P5).
    pub structural: bool,
    /// The first commit at (0, 0); empty for none.
    pub init: Vec<u8>,
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
            structural: false,
            init: Vec::new(),
        }
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
