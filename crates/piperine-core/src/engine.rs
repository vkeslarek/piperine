use crate::analysis::ToControl;
use crate::netlist::ToNetlist;
use crate::result::SimulationResult;

/// Abstract simulation engine.
///
/// Implementations handle the actual simulation execution.
/// This allows swapping ngspice for a different backend in the future.
pub trait SimulationEngine {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Run a simulation with the given circuit and analysis.
    fn run(
        &self,
        circuit: &dyn ToNetlist,
        analysis: &dyn ToControl,
    ) -> Result<SimulationResult, Self::Error>;

    /// Run a simulation with external source callback support.
    fn run_with_external_sources(
        &self,
        circuit: &dyn ToNetlist,
        analysis: &dyn ToControl,
        handler: &dyn ExternalSourceHandler,
    ) -> Result<SimulationResult, Self::Error>;

    /// Run a batch of simulations (potentially in parallel).
    fn run_batch(
        &self,
        jobs: &[(&dyn ToNetlist, &dyn ToControl)],
    ) -> Vec<Result<SimulationResult, Self::Error>> {
        jobs.iter().map(|(c, a)| self.run(*c, *a)).collect()
    }
}

/// Handler for external voltage/current sources.
///
/// During simulation, ngspice calls back to request values for sources
/// defined with the EXTERNAL keyword. Implement this trait to provide
/// those values.
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
