//! JSON IPC protocol for communication with workers

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Request sent to worker process
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WorkerRequest {
    /// Run a complete simulation: load netlist + run command
    RunSimulation {
        netlist: Vec<String>,
        command: String,
    },
    /// Load a netlist
    LoadNetlist { lines: Vec<String> },
    /// Run a SPICE command
    RunCommand { command: String },
    /// Get all results
    GetResults,
    /// Reset worker state
    Reset,
}

/// Response from worker process
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkerResponse {
    /// Simulation completed successfully
    SimulationResult {
        plots: HashMap<String, PlotData>,
        output: Vec<String>,
    },
    /// Error occurred
    Error { message: String },
}

/// Simulation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub plots: HashMap<String, PlotData>,
    pub output: Vec<String>,
}

/// Plot data from simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotData {
    pub name: String,
    pub vectors: HashMap<String, Vec<f64>>,
}
