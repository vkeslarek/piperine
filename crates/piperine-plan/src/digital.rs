//! The digital half of a body's plan (spec RS-12, design "Phase B: from a body to a shape"):
//! what the discrete forms of the IR become in the ABI. Everything here is appended after the
//! M1 entries of `plan.rs`, so an M1 body's pins, functions and their indices do not move.
//!
//! Carriers (RS-12 AC 1): one `DISCRETE` pin per carrier, in declaration order, after every M1
//! pin; `PORT` for an `in` or `out` port and for a `signal` a child's carrier port is bound to
//! (`Body::child_carriers`: part II §5, the parent's signal is the net its children meet on, so
//! the netlist binds it to that net like a port; msl-digital T65), `PRIVATE` for any other
//! `signal`. Its layout is one field `v`
//! holding the value and nothing else (abi.md P3): `BIT` for `bool`, `F64` for `f64`, and for
//! `UInt<N>`/`Int<N>` the narrowest of `U8` to `U64` or `I8` to `I64` that holds `N` bits, the
//! size the value's own. An initial value gives `HAS_INIT` with its bytes: the value
//! little-endian. An integer param takes the same layout and default bytes as a carrier of its
//! type.
//!
//! Events (RS-12 AC 3): one `PRIVATE` `EVENT` pin `event:<k>` per event of the body, after the
//! carrier pins. A `Changed` event is written by the discrete function `event:<k>`, which reads
//! the `VALUE` and `CHANGED` of its carrier and the pins its guard reads.
//!
//! Drives (RS-12 AC 2, AC 5): a drive without `after` is the discrete function
//! `drive:<carrier>` reading the pins its value reads and writing the carrier's pin. A drive
//! with `after` writes, instead, the `PRIVATE` pin `drive:<carrier>.now` (the carrier's layout);
//! a `changed` event on it, the event pin and function `drive:<carrier>.changed`, fires the
//! reaction `drive:<carrier>`, which reads the undelayed value and what the delay reads and
//! commits the value to the carrier after the delay. Those two pins follow the event pins, a
//! drive's pair in drive order.
//!
//! Reactions (RS-12 AC 4, AC 8): one ABI reaction `reaction:<k>` per reaction of the body, on
//! the event pins of its events, reading the `VALUE` of every pin its body reads (carriers,
//! params, and the continuous pins it probes), with one statement per commit in source order
//! (a depth-first walk of its statements, `then` before `else`). A statement commits the whole
//! of its carrier's pin. It is `LATER` with `min_after` the delay only when the delay is a real
//! literal greater than 0 (abi.md §6 C1: a bound the generator can state); a delay that reads
//! anything, or none, gives no flag. The reaction of a delayed drive follows the same rule.
//!
//! Crossings (RS-12 AC 6, abi.md §11.5): a `cross` event is a `CROSS` watch `cross:<k>`
//! (one `F64`, default tolerances) firing the event's pin, its value written by the analog
//! function `cross:<k>` with a partial against each continuous pin of its dependency set. A
//! guarded `cross` fires a private event pin `cross:<k>` instead (after the event pins), and the
//! event function `event:<k>` reads its `CHANGED` and the guard's pins and fires the event pin.
//!
//! An analog read of a carrier (RS-12 AC 7) is in the dependency set of the term that reads it:
//! `terms` reads the carrier's `VALUE`, with no partial (it is no differentiable read).
//!
//! Timers (msl-digital A3; abi.md R1: "a timer is a delayed commit to an event pin"): an `at(t)`
//! event `k` is fired by its own **arming reaction** `at:<k>`, which listens to the `GLOBAL`
//! `START` pin (one per body, `start`) and to one private event pin `at:<k>.<carrier>` per
//! carrier `t` reads, each fired by the discrete function of the same name when that carrier
//! changed. The reaction reads what `t` reads and the ambient `TIME`, and has one `REPLACE`
//! statement on the event pin (C3: a commit through it removes every pending one, so a re-armed
//! timer drops its earlier firing). A guarded `at` fires a private event pin `at:<k>` instead,
//! and its event function `event:<k>` reads that pin's `CHANGED` and the guard's pins, as a
//! guarded `cross` does. These pins follow the delayed pins: the timer pins in event order, the
//! `START` pin, then the re-arming pins in event and read order.
//!
//! `settle` (unknown-semantics-events, §13 rule 17): a body with a `settle` event has one
//! `GLOBAL` `SETTLE` pin (`settle`, after every other pin), and the event function `event:<k>` of
//! each `settle` event reads its `CHANGED` byte (`Source::Settle`) and the guard's pins, as a
//! `changed` event reads its carrier's.
//!
//! Records (msl-digital A4; abi.md P7, C5): each stream of a body is one `PRIVATE` `DISCRETE`
//! pin named by the stream and flagged `RECORD` (so `QUIET`: no reader, never a microstep). Its
//! layout holds one field per value field of the stream, in the stream's order, each laid out
//! as a carrier of its type (its value bytes) at the next offset its alignment allows, the size
//! rounded up to the largest alignment (`RecordLayout`): a site that leaves a field out writes
//! it all zero (P3, C5). Each emit site is one statement of
//! its reaction, in the order of the commits (`Body::reaction_statements`), committing the
//! whole layout. The record pins follow the re-arming pins, in stream order.
//!
//! Function order after the M1 functions: `cross:*` in event order, `drive:*` in drive order,
//! then `event:*` in event order, then each `drive:<carrier>.changed`, then each
//! `at:<k>.<carrier>`. Reactions: the body's in source order, then one per delayed drive in
//! drive order, then one arming reaction per `at` event in event order.

mod carriers;
mod events;
mod reactions;
mod records;

pub use carriers::CarrierLayout;
pub use reactions::{ReactionMode, ReactionPlan, StatementPlan};
pub use records::RecordLayout;

/// The discrete entries of a body's shape.
pub(crate) struct Digital;
