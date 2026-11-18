//! The run configuration every analysis driver speaks: `Tolerances` (immutable
//! per-run numerics plus the convergence tests that read only them), `Policy`
//! (the mutable convergence tunables a driver owns, MD-04), and `Context` (the
//! shared immutable bundle handed to every analysis, and the owner of the
//! one-time process init, MD-06).

use piperine_analog_solver::math::iv::InitialValue;
use piperine_analog_solver::netlist::AnalogReference;
use piperine_analog_solver::tolerances::Tolerances;
use faer::{Par, set_global_parallelism};
use std::num::NonZeroUsize;
use std::sync::Once;

static INIT: Once = Once::new();

// ── Context (shared, immutable) ────────────────────────────────────────────

/// Which analysis is currently running, used to gate scoped requires
/// (dv-core T28, DVC-19). A `require` inside `@ dc { … }` is evaluated
/// only when the running analysis is `Dc`; an analysis that never runs
/// leaves the scoped require "not exercised" (never a silent vacuous
/// pass). The names match the constraint event-term names (`dc`/`tran`/`ac`)
/// exactly — the `as_str` is the comparison key the device reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisKind {
    Dc,
    Tran,
    Ac,
}

impl AnalysisKind {
    /// The analysis's event-term name — the comparison key the device
    /// matches against a margin's `analyses` set.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dc => "dc",
            Self::Tran => "tran",
            Self::Ac => "ac",
        }
    }
}

/// The shared context every analysis receives: the immutable
/// [`Tolerances`].
/// Mutable convergence state lives on [`Policy`], owned by the driver;
/// simulation time reaches elements through their analysis context or as an
/// explicit argument (MD-04).
///
/// `Debug`/`Default` are hand-written — see the impls below.
#[derive(Clone)]
pub struct Context {
    pub tolerances: Tolerances,
    /// The bound on the rounds of an operating point (`E0506`); 20 unless the host says otherwise.
    pub max_rounds: u32,
    /// The first Newton iterate of every operating point the analysis takes: node values the
    /// host's `nodeset` gives (ngspice `.nodeset`). Empty leaves the solver's own start. The
    /// solution still converges to the circuit's own equation; only the start moves.
    pub nodeset: Vec<InitialValue<AnalogReference, f64>>,
}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("tolerances", &self.tolerances)
            .field("max_rounds", &self.max_rounds)
            .field("nodeset", &self.nodeset.len())
            .finish()
    }
}

impl Default for Context {
    fn default() -> Self {
        Self {
            tolerances: Tolerances::default(),
            max_rounds: Self::DEFAULT_MAX_ROUNDS,
            nodeset: Vec::new(),
        }
    }
}

impl Context {
    /// The default bound on the rounds of an operating point.
    pub const DEFAULT_MAX_ROUNDS: u32 = 20;

    pub fn init_global() {
        INIT.call_once(|| {
            // Keep piperine's own diagnostics at INFO, but silence the Cranelift
            // JIT backend (it logs every compiled function's CL IR at INFO via
            // the `log` crate, which drowns real output). `cranelift*=off` keeps
            // the log-compat shim on for any other crate without the IR spam.
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::new("info,cranelift_jit=off,cranelift_codegen=off"),
                )
                .with_thread_ids(true)
                .with_thread_names(true)
                .init();

            set_global_parallelism(Par::Rayon(NonZeroUsize::new(1).unwrap()));
        });
    }
}
