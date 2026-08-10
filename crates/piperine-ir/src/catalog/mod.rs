//! The intrinsic catalog — one declaration per intrinsic, subsuming the
//! runtime-semantic families (LOV-12, ideal.md §1.6, design.md §4). This
//! is where MD-24 (declared language surface) finally lands cleanly: the
//! catalog IS the declared surface, one place, and BOTH backends plus the
//! interpreter read the same entry.
//!
//! ## Shape
//!
//! [`contract`] carries the five questions every entry answers (a trait,
//! [`contract::Intrinsic`]) plus their value types; [`registry`] carries
//! the data table ([`registry::Catalog::entries`]) that lists the entries.
//! Table + trait, **never a macro** (MD-13 rule 5). Each existing family
//! module (`ops.rs`/`events.rs`/`limit.rs`/`behavior/`/`measure/`/
//! `wave.rs`/`logic.rs`) is the implementation behind one or more
//! entries — its rules travel intact (ideal.md §3 lineage). T11 made
//! every family reachable through the catalog; T12's diff pass and T13's
//! interpreter consult these entries.
//!
//! ## The five questions (`contract`)
//!
//! 1. **Signature** — argument types and return type.
//! 2. **State shape** — private state the intrinsic carries between
//!    evaluations. Most are stateless; reactive operators (`ddt`, `idt`,
//!    `transition`, `delay`, `slew`) carry history that is backend
//!    bookkeeping, never a `Cell` (ideal.md §1.4).
//! 3. **Derivative rule** — how this intrinsic differentiates w.r.t. its
//!    real-valued inputs. The diff pass (T12) consults this; it is what
//!    makes `d/dx sin(x) = cos(x)·dx` work without a hardcoded math table
//!    at the pass site.
//! 4. **Breakpoints** — declared integration breakpoints, if any. Empty
//!    for most; non-empty for `cross`/`timer`/`above` and the ramp edges
//!    a `transition` publishes.
//! 5. **Legal domains** — analog/digital/both. An `analog`-only intrinsic
//!    in a digital block is a verifier error.
//!
//! ## Families (ideal.md §1.6, one row each)
//!
//! | family | file | entries |
//! |---|---|---|
//! | math | [`math`] | `sin`, `cos`, `tan`, `exp`, `ln`, `sqrt`, `pow`, `abs`, `min`, `max`, `floor`, … |
//! | analog operators | [`analog_ops`] | `ddt`, `idt`, `idtmod`, `transition`, `delay`, `slew`, `absdelay`, `table`, `ac_stim` |
//! | detectors | [`detectors`] | `cross`, `above`, `timer` |
//! | convergence | [`convergence`] | `pnjlim`, `fetlim`, `limvds` (the `$limit` family) |
//! | noise | [`noise`] | `white_noise`, `flicker_noise` |
//! | logic | [`logic_ops`] | 4-state bitwise and reduction ops over `bits<N>` |
//! | system | [`system`] | `$temperature`, `$abstime`, `$analysis` |

pub mod analog_ops;
pub mod contract;
pub mod convergence;
pub mod detectors;
pub mod logic_ops;
pub mod math;
pub mod noise;
pub mod registry;
pub mod system;

pub use contract::{Breakpoint, DerivativeRule, Family, Intrinsic, LegalDomains, Signature, StateShape};
pub use registry::Catalog;
