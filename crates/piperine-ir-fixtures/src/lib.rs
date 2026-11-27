//! The hand-built IR bodies the test suites of the IR's consumers share, one type per design. A dev-only
//! crate: no product crate depends on it, and no body is built twice (`repeated_type_names` holds this crate
//! too). Every body is made through `IrBuilder`, with the types its MSL source gives it written out at each
//! node: the fixture states the types, `Ir::check` judges them.
//!
//! - `devices`: the five device bodies of `tests/e2e/twins/devices.msl`, the functions they call and the
//!   divider design, with `Ex` (the expression builder whose types follow the operator).
//! - `digital`: the digital and mixed bodies of the buck slice (`Buck`), with `Dx` (the
//!   expression builder whose every node takes its type from the caller) and `ValueAliases` (the value types of
//!   the source, by their aliases).
//! - `mixed`: `TopDesign`, the top instance a hand-built mixed design is placed under.
//! - `circuits`: the M1 circuits over the device bodies (the divider, a diode behind a source, two RC low-passes).
//! - `sources`: `RustSources`, the source text of a crate under test, for the static checks over it.
//! - `operators`: the body `Op(a)` of the analog-operator tests (`Operators`); `structure`: the bodies of the
//!   analog-structure tests (`Switching`, `ParamEvents`).

#![deny(missing_docs)]

pub mod circuits;
pub mod devices;
pub mod digital;
pub mod mixed;
pub mod operators;
pub mod sources;
pub mod structure;
