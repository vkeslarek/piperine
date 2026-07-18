//! SC-26 / MD-06 — `Context::default()` has no global side effects: process
//! globals (tracing subscriber, faer parallelism) are owned by the first
//! solver build via `Context::init_global`, never by context construction.
//!
//! This binary runs in its own process, so no solver build can have fired
//! the init `Once` before the negative assertion — the ordering is
//! deterministic within the single test.

use piperine_solver::prelude::Context;

#[test]
fn context_default_does_not_init_globals() {
    // faer's compiled-in default parallelism is Rayon(all cores);
    // `init_global` forces `Par::Rayon(1)`. `Context::default()` must leave
    // the faer default untouched. On a single-core host the two are
    // indistinguishable and the negative assertion is vacuous.
    let _ctx = Context::default();
    let single = faer::Par::Rayon(std::num::NonZeroUsize::new(1).unwrap());
    if std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) > 1 {
        assert_ne!(
            faer::get_global_parallelism(),
            single,
            "Context::default() forced single-threaded faer — global init leaked"
        );
    }

    // Positive control: the explicit init (what every solver build calls
    // first) does force the single-threaded faer configuration.
    Context::init_global();
    assert_eq!(faer::get_global_parallelism(), single, "init_global did not run");
}
