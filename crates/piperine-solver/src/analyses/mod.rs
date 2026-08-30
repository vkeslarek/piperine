//! The analyses layer (design §1/§2 — Scheme B): one module per analysis,
//! each holding both its request/state types (what element and host
//! exchange) and its driver (how it runs). The run configuration every
//! driver speaks — `Context` (immutable `Tolerances`) and `Policy`
//! (per-analysis convergence tunables, MD-04) — lives in `context.rs`, the
//! `Solver` host entry that hands out the analyses in `solver.rs`, the
//! tunable literals in `config.rs`, and the Newton/homotopy/stepper
//! machinery in `convergence.rs`. A driver may call down into `analog`,
//! `digital`, `math`, and read config — never sideways into another
//! analysis, never up into the host.

pub mod ac;
pub mod adjoint;
pub mod config;
pub mod context;
pub mod convergence;
pub mod dc;
pub mod disto;
pub mod events;
pub mod noise;
pub mod pss;
pub mod pz;
pub mod sens;
pub mod sensitivity;
pub mod solver;
pub mod sp;
pub mod stamp_validation;
pub mod tf;
pub mod transient;

pub use context::{AnalysisKind, CheckPosture, Context, CoverPosture, Policy, RequireScope, Tolerances};
pub use solver::Solver;
pub use stamp_validation::non_finite_stamp_error;
