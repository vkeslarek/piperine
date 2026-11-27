//! Suite: the invariants that keep the test suite honest, enforced by the suite itself.
//! `suite_hygiene` walks the repository's own sources: no switched-off test code, no ignored
//! test, no file-scope lint suppression, every target declares its scope. `target_plan` holds each crate
//! to its plan of integration-test targets. `test_code` holds the ratchets over test code. `repository_records` holds the structural guards over `GAPS.md`,
//! `ROADMAP.md`, `CLAUDE.md` and the buck's README.

mod repository_records;
mod suite_hygiene;
mod target_plan;
mod test_code;
