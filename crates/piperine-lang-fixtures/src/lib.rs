//! The source texts, designs and readers the test suites of `piperine-lang` share, one type per concern. A dev-only
//! crate: no product crate depends on it, and no type is declared twice (`repeated_type_names` holds this crate too).
//!
//! - `pom`: `PomSource`, a design built from one source text, and the codes of its diagnostics; `lowering`:
//!   `LoweringCodes`, the codes of a lowering without the binding errors; `checked`: `CheckedSource`, a source run through
//!   the type checker; `elaboration` and `elaboration_forms`: the elaboration of a source to an IR and the reading of it.
//! - `buck`: `BuckSource`, the text of items of `examples/buck/src`; `generic_sources` and `nested_reach`: the sources
//!   of the generic and nested-reach tests.
//! - `fragments`: `FragmentSource`, a fragment of MSL wrapped into the smallest context that parses it; `prelude_design`:
//!   `PreludeDesign`, a design over the embedded standard library and the reads of its diagnostics; `refusals`:
//!   `ElaborationRefusals`, the codes and messages a source's elaboration refuses with.
//! - `std_crate`: `StdCrate`, a standard library written in a test and a root crate over it.
//! - `twin`: `TwinSource`, a twin of `tests/e2e/twins/` loaded from disk.
//! - `spec_corpus` and `spec_tables`: the fenced blocks and the tables of the specification's markdown.

#![deny(missing_docs)]

pub mod buck;
pub mod checked;
pub mod elaboration;
pub mod elaboration_forms;
pub mod fragments;
pub mod generic_sources;
pub mod lowering;
pub mod nested_reach;
pub mod pom;
pub mod prelude_design;
pub mod refusals;
pub mod std_crate;
pub mod spec_corpus;
pub mod spec_tables;
pub mod twin;
