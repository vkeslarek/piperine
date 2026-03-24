//! IPC protocol messages between main process and worker processes.
//!
//! Uses bincode serialization with length-prefixed framing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Messages sent from main process to a worker.
#[derive(Debug, Serialize, Deserialize)]
pub enum MainToWorker {
    /// Load and run a simulation.
    RunSimulation {
        netlist_lines: Vec<String>,
        control_commands: Vec<String>,
        has_external_sources: bool,
    },
    /// Response to an external source callback request.
    ExternalSourceValue {
        request_id: u64,
        value: f64,
    },
    /// Reset the ngspice instance (destroy all circuits).
    Reset,
    /// Shut down the worker process.
    Shutdown,
}

/// Messages sent from worker to main process.
#[derive(Debug, Serialize, Deserialize)]
pub enum WorkerToMain {
    /// Simulation completed successfully.
    SimulationComplete {
        plots: HashMap<String, PlotData>,
        measurements: HashMap<String, f64>,
        log: Vec<String>,
    },
    /// Worker needs an external source value (callback from ngspice).
    ExternalSourceRequest {
        request_id: u64,
        source_name: String,
        time: f64,
    },
    /// Worker reports an error.
    Error {
        message: String,
    },
    /// Ack for Reset / other commands.
    Ok,
    /// Worker is ready after initialization.
    Ready,
}

/// Serializable plot data (mirrors piperine_api::result::Plot).
#[derive(Debug, Serialize, Deserialize)]
pub struct PlotData {
    pub name: String,
    pub plot_type: String,
    pub vectors: HashMap<String, VectorData>,
}

/// Serializable vector data.
#[derive(Debug, Serialize, Deserialize)]
pub enum VectorData {
    Real { name: String, data: Vec<f64> },
    Complex { name: String, data: Vec<(f64, f64)> },
}
