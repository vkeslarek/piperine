//! [`SimHooks`] — the simulation lifecycle hooks a plugin host (or any
//! external observer) wires into a [`SimSession`](crate::session::SimSession)
//! build/solve pipeline (SPEC Part VI §8). All hooks are optional no-ops by
//! default; hook errors abort the analysis (fail loud).

use piperine_lang::Design;

/// Lifecycle hook surface fired by the session around circuit builds and
/// solves. `transform_design` is the one mutable hook: the plugin stages its
/// mutations on the design, and the re-elaboration below consumes them like
/// any staged override.
pub trait SimHooks {
    /// Fired before each circuit build's re-elaboration: the host stages
    /// its design mutations (declared instances, param writes).
    fn transform_design(&self, design: &Design) -> Result<(), String>;

    /// Fired with the applied (staged) design, read-only, just before
    /// lowering to resolved bodies.
    fn before_lower(&self, design: &Design) -> Result<(), String>;

    /// Fired after an analysis solves, with its name (`"op"`, `"tran"`,
    /// `"ac"`, `"noise"`) and — for operating points — the node voltages.
    fn after_solve(&self, analysis: &str, node_voltages: &[(String, f64)]) -> Result<(), String>;
}
