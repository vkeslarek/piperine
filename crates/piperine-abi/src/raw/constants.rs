//! The named `u32` constants of the contract and the groups of them (`kind`, `role`, `part`, ...).

// ─── Constants ──────────────────────────────────────────────────────────────

/// `Function.elements`/`Reaction.elements`: a read of every element of an array pin, as an [`ArrayView`].
pub const WHOLE: u64 = u64::MAX;
/// The `plus`/`minus` of a term that names the reference node.
pub const GROUND: u32 = u32::MAX;
/// A tolerance or bound index meaning "the session default".
pub const DEFAULT: u32 = u32::MAX - 1;
/// The ABI major; it is the exported symbol's suffix.
pub const MAJOR: u32 = 1;
/// The ABI minor this crate implements.
pub const MINOR: u32 = 0;
/// The one exported symbol of a library.
pub const SYMBOL: &str = "piperine_abi_1";

/// `Pin.kind`.
pub mod kind {
    pub const CONTINUOUS: u32 = 1;
    pub const DISCRETE: u32 = 2;
    pub const EVENT: u32 = 3;
}

/// `Pin.role`.
pub mod role {
    pub const PORT: u32 = 1;
    pub const PARAM: u32 = 2;
    pub const GLOBAL: u32 = 3;
    pub const PRIVATE: u32 = 4;
}

/// `Pin.global`, for role `GLOBAL`.
pub mod global {
    pub const TEMPERATURE: u32 = 1;
    pub const RESOLUTION: u32 = 2;
    pub const DC: u32 = 3;
    pub const TRAN: u32 = 4;
    pub const AC: u32 = 5;
    pub const NOISE: u32 = 6;
    pub const START: u32 = 7;
    pub const SETTLE: u32 = 8;
    /// The host's session parameter called `Pin.name`.
    pub const NAMED: u32 = 9;
    /// Whether the run set its temperature, a `BIT`: `1` when it did, `0` when the `TEMPERATURE` pin holds the
    /// session's default. Appended in place (`MINOR` 0).
    pub const TEMPERATURE_GIVEN: u32 = 10;
}

/// `Pin.flags` bits.
pub mod pin_flag {
    pub const STRUCTURAL: u32 = 1 << 0;
    pub const OPTIONAL: u32 = 1 << 1;
    pub const HAS_INIT: u32 = 1 << 2;
    pub const HAS_CHECK: u32 = 1 << 3;
    pub const QUIET: u32 = 1 << 4;
    pub const RECORD: u32 = 1 << 5;
    pub const RELAX: u32 = 1 << 6;
}

/// `Field.leaf`.
pub mod leaf {
    pub const BOOL: u32 = 1;
    pub const U8: u32 = 2;
    pub const U16: u32 = 3;
    pub const U32: u32 = 4;
    pub const U64: u32 = 5;
    pub const I8: u32 = 6;
    pub const I16: u32 = 7;
    pub const I32: u32 = 8;
    pub const I64: u32 = 9;
    pub const F32: u32 = 10;
    pub const F64: u32 = 11;
}

/// `Term.op`.
pub mod op {
    pub const FLOW: u32 = 1;
    pub const DDT: u32 = 2;
    pub const DELAY: u32 = 3;
}

/// `Input.kind` (held inputs).
pub mod input {
    pub const LIMIT: u32 = 1;
    pub const NOISE: u32 = 2;
    pub const STIM: u32 = 3;
}

/// `Watch.kind`.
pub mod watch {
    pub const VAR: u32 = 1;
    pub const CROSS: u32 = 2;
    pub const CAP: u32 = 3;
    pub const TRACE: u32 = 4;
}

/// `Cell.kind`.
pub mod cell {
    pub const PIN: u32 = 1;
    pub const TERM: u32 = 2;
    pub const INPUT: u32 = 3;
    pub const WATCH: u32 = 4;
    pub const AMBIENT: u32 = 5;
}

/// `Cell.part` (abi.md §4.1).
pub mod part {
    pub const VALUE: u32 = 1;
    pub const PREVIOUS: u32 = 2;
    pub const CHANGED: u32 = 3;
    pub const TIME: u32 = 4;
    pub const LAST: u32 = 5;
    pub const AGE: u32 = 6;
    pub const PROPOSED: u32 = 7;
    pub const LIMITED: u32 = 8;
    pub const PSD: u32 = 9;
    pub const RE: u32 = 10;
    pub const IM: u32 = 11;
}

/// `Cell.index` of an `AMBIENT` cell (abi.md §4.2).
pub mod ambient {
    pub const TIME: u32 = 1;
    pub const FREQ: u32 = 2;
    pub const STATIC: u32 = 3;
    pub const GMIN: u32 = 4;
    pub const SOURCE_SCALE: u32 = 5;
    pub const TICK: u32 = 6;
    pub const HORIZON: u32 = 7;
}

/// `Function.flags` bits.
pub mod fn_flag {
    pub const OWNS_CORRECTION: u32 = 1 << 0;
}

/// `Reaction.flags` bits.
pub mod reaction_flag {
    pub const IRREVERSIBLE: u32 = 1 << 0;
}

/// `Statement.flags` bits.
pub mod statement_flag {
    pub const LATER: u32 = 1 << 0;
    pub const REPLACE: u32 = 1 << 1;
}

/// The status a call returns (abi.md §1, concept 13).
pub mod status {
    pub const OK: u32 = 1;
    pub const FAULT: u32 = 2;
    pub const RESHAPE: u32 = 3;
    pub const FINISH: u32 = 4;
}


