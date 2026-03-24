use crate::result::SimulationResult;
use crate::spice::{SpiceAnalysis, ToSpiceNetlist};

/// Abstract simulation engine.
///
/// Implementations handle the actual simulation execution (e.g. ngspice FFI).
/// This trait is engine-agnostic — it accepts `&dyn ToSpiceNetlist` and `&dyn SpiceAnalysis`,
/// completely decoupled from any specific simulator.
pub trait SimulationEngine {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Run a simulation with the given circuit and analysis.
    fn run(
        &self,
        circuit: &dyn ToSpiceNetlist,
        analysis: &dyn SpiceAnalysis,
    ) -> Result<SimulationResult, Self::Error>;

    /// Run a simulation with external source callback support.
    fn run_with_external_sources(
        &self,
        circuit: &dyn ToSpiceNetlist,
        analysis: &dyn SpiceAnalysis,
        handler: &dyn ExternalSourceHandler,
    ) -> Result<SimulationResult, Self::Error>;

    /// Run a batch of simulations (potentially in parallel).
    fn run_batch(
        &self,
        jobs: &[(&dyn ToSpiceNetlist, &dyn SpiceAnalysis)],
    ) -> Vec<Result<SimulationResult, Self::Error>> {
        jobs.iter().map(|(c, a)| self.run(*c, *a)).collect()
    }
}

/// Handler for external voltage/current sources.
///
/// During simulation, the engine calls back to request values for sources
/// defined with the EXTERNAL keyword.
pub trait ExternalSourceHandler: Send + Sync {
    /// Get the value for an external source at the given simulation time.
    fn get_value(&self, source_name: &str, time: f64) -> f64;
}

/// Blanket impl: closures can be used as ExternalSourceHandler.
impl<F> ExternalSourceHandler for F
where
    F: Fn(&str, f64) -> f64 + Send + Sync,
{
    fn get_value(&self, source_name: &str, time: f64) -> f64 {
        (self)(source_name, time)
    }
}
