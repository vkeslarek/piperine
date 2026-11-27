//! Suite: the invariants that keep the test suite honest, enforced by the suite itself.
//! `suite_hygiene` walks the repository's own sources: no switched-off test code, no ignored
//! test, no file-scope lint suppression, every target declares its scope. `target_plan` holds each crate
//! to its plan of integration-test targets.

mod suite_hygiene;
mod target_plan;
