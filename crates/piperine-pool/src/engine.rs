//! NgspiceEngine: implements SimulationEngine using a worker pool.

use crate::pool::WorkerPool;
use piperine_api::engine::{ExternalSourceHandler, SimulationEngine};
use piperine_api::result::*;
use piperine_api::spice::{SpiceAnalysis, ToSpiceNetlist};
use piperine_ngspice::protocol::*;
use std::collections::HashMap;
use std::sync::Mutex;

/// Error type for the NgspiceEngine.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("simulation error: {0}")]
    Simulation(String),
    #[error("worker pool error: {0}")]
    Pool(String),
}

/// Simulation engine backed by ngspice worker processes.
pub struct NgspiceEngine {
    pool: Mutex<WorkerPool>,
}

impl NgspiceEngine {
    /// Create a new engine with the default number of workers.
    pub fn new() -> Result<Self, EngineError> {
        Self::with_workers(num_cpus::get().max(1))
    }

    /// Create a new engine with the specified number of workers.
    pub fn with_workers(n: usize) -> Result<Self, EngineError> {
        let exe = std::env::current_exe()
            .map_err(|e| EngineError::Pool(format!("cannot find executable: {e}")))?;
        let pool = WorkerPool::new(exe.to_str().unwrap(), n)?;
        Ok(Self { pool: Mutex::new(pool) })
    }

    /// Create engine with a specific executable path.
    pub fn with_exe(exe_path: &str, n: usize) -> Result<Self, EngineError> {
        let pool = WorkerPool::new(exe_path, n)?;
        Ok(Self { pool: Mutex::new(pool) })
    }
}

impl SimulationEngine for NgspiceEngine {
    type Error = EngineError;

    fn run(
        &self,
        circuit: &dyn ToSpiceNetlist,
        analysis: &dyn SpiceAnalysis,
    ) -> Result<SimulationResult, Self::Error> {
        let netlist_lines = circuit.to_spice_netlist();
        let control_commands = analysis.to_spice_control_commands();

        let (idx, mut worker) = {
            let mut pool = self.pool.lock().unwrap();
            pool.take_worker()?
        };

        let msg = MainToWorker::RunSimulation {
            netlist_lines,
            control_commands,
            has_external_sources: false,
        };
        worker.send(&msg)?;

        let response = worker.drive_simulation(None)?;

        // Return worker to pool
        {
            let mut pool = self.pool.lock().unwrap();
            pool.return_worker(idx, worker);
        }

        match response {
            WorkerToMain::SimulationComplete { plots, measurements, log } => {
                Ok(convert_result(plots, measurements, log))
            }
            WorkerToMain::Error { message } => Err(EngineError::Simulation(message)),
            _ => Err(EngineError::Simulation("unexpected response".into())),
        }
    }

    fn run_with_external_sources(
        &self,
        circuit: &dyn ToSpiceNetlist,
        analysis: &dyn SpiceAnalysis,
        handler: &dyn ExternalSourceHandler,
    ) -> Result<SimulationResult, Self::Error> {
        let netlist_lines = circuit.to_spice_netlist();
        let control_commands = analysis.to_spice_control_commands();

        let (idx, mut worker) = {
            let mut pool = self.pool.lock().unwrap();
            pool.take_worker()?
        };

        let msg = MainToWorker::RunSimulation {
            netlist_lines,
            control_commands,
            has_external_sources: true,
        };
        worker.send(&msg)?;

        let response = worker.drive_simulation(Some(handler))?;

        {
            let mut pool = self.pool.lock().unwrap();
            pool.return_worker(idx, worker);
        }

        match response {
            WorkerToMain::SimulationComplete { plots, measurements, log } => {
                Ok(convert_result(plots, measurements, log))
            }
            WorkerToMain::Error { message } => Err(EngineError::Simulation(message)),
            _ => Err(EngineError::Simulation("unexpected response".into())),
        }
    }

    fn run_batch(
        &self,
        jobs: &[(&dyn ToSpiceNetlist, &dyn SpiceAnalysis)],
    ) -> Vec<Result<SimulationResult, Self::Error>> {
        // Simple sequential implementation for now.
        // Parallel version would take multiple workers and use threads.
        jobs.iter().map(|(c, a)| self.run(*c, *a)).collect()
    }
}

/// Convert protocol PlotData into core result types.
fn convert_result(
    plots: HashMap<String, PlotData>,
    measurements: HashMap<String, f64>,
    log: Vec<String>,
) -> SimulationResult {
    let mut result_plots = HashMap::new();

    for (name, pd) in plots {
        let mut vectors = HashMap::new();
        for (vname, vd) in pd.vectors {
            let vec = match vd {
                VectorData::Real { name, data } => Vector::Real(RealVector { name, data }),
                VectorData::Complex { name, data } => Vector::Complex(ComplexVector { name, data }),
            };
            vectors.insert(vname, vec);
        }

        let plot_type = match pd.plot_type.as_str() {
            "OpPoint" => PlotType::OpPoint,
            "DcSweep" => PlotType::DcSweep,
            "AcAnalysis" => PlotType::AcAnalysis,
            "Transient" => PlotType::Transient,
            "Noise" => PlotType::Noise,
            "PoleZero" => PlotType::PoleZero,
            "Sensitivity" => PlotType::Sensitivity,
            "TransferFunction" => PlotType::TransferFunction,
            "SParameter" => PlotType::SParameter,
            _ => PlotType::Unknown,
        };

        result_plots.insert(name.clone(), Plot {
            name: pd.name,
            plot_type,
            vectors,
        });
    }

    SimulationResult {
        plots: result_plots,
        measurements,
        log,
    }
}
