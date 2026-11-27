//! Scope: the integrated suite (feature `integrated-tests`, IT-03 to IT-08; MD-77): six small mixed-signal
//! designs of the scenario package `tests/e2e/integrated/`, run at once through the host library
//! (`piperine::Session`, by `piperine_parity::scenario`) and judged against closed forms, reference
//! models and exact counts. This is the per-task gate that replaced the buck converter's long
//! targets, which run on demand (`examples/buck/README.md`, "On demand"). One module per concern:
//! the package and its batch, the closed-form helpers, then one module per scenario.
#![cfg(unix)]

// The compile-and-run support of the mixed designs of `piperine-transpile`'s tests, with the digital
// builders it names, included by path as the parity targets do.
mod clock;
mod half_bridge;
mod package;
mod pi_loop;
mod pwm_filter;
mod rc;
mod sample_hold;
mod state_machine;
mod suite;
mod table;
mod threshold;
