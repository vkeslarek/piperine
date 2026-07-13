//! The bench-side plugin seam (SPEC Part VI §8, Plugin plan Phase 3) — the
//! same dependency-inversion pattern as codegen's `DeviceProvider` (D4):
//! this crate defines the trait it needs, the plugin host implements it,
//! and the wiring happens where both are visible (CLI, tests).

use piperine_lang::eval::{EvalError, Value};
use piperine_lang::Design;

/// Everything the bench pipeline asks a plugin host per analysis. Every
/// method defaults to a no-op so the trait is also a null object.
pub trait BenchPlugins {
    /// `transform_design` (Part VI §8.1 hook 3): fired before an analysis
    /// consumes staged overrides — plugins stage param overrides and
    /// instance/connection injections through the design's staging surface.
    fn transform_design(&self, design: &Design) -> Result<(), String> {
        let _ = design;
        Ok(())
    }

    /// `before_lower` (hook 4): fired on the applied design, read-only.
    fn before_lower(&self, design: &Design) -> Result<(), String> {
        let _ = design;
        Ok(())
    }

    /// `after_solve` (hook 7): fired after an analysis with its kind
    /// (`"op"`/`"tran"`/`"ac"`/`"noise"`) and, for `$op`, the solved node
    /// voltages.
    fn after_solve(&self, analysis: &str, node_voltages: &[(String, f64)]) -> Result<(), String> {
        let _ = (analysis, node_voltages);
        Ok(())
    }

    /// Plugin-contributed bench task dispatch (`$name(...)`, Part VI §6).
    /// `None` when no loaded plugin registered `name`.
    fn run_bench_task(&self, name: &str, args: Vec<Value>) -> Option<Result<Value, EvalError>> {
        let _ = (name, args);
        None
    }
}
