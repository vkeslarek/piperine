//! Process pool for ngspice workers
//!
//! Each worker runs in a separate process to avoid ngspice's
//! thread-safety issues. Communication happens via JSON over stdin/stdout.

use crate::errors::{Result, SpiceError};
use crate::protocol::{SimulationResult, WorkerRequest, WorkerResponse};
use serde_json;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use tracing::{debug, error, info, trace};

/// A worker process running ngspice
struct WorkerProcess {
    process: Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl WorkerProcess {
    /// Spawn a new worker process
    fn spawn() -> Result<Self> {
        debug!("Spawning worker process");

        // Find ngspice-worker binary
        let worker_path = find_worker_binary()?;
        debug!("Using worker binary: {}", worker_path.display());

        let mut process = Command::new(&worker_path)
            .arg("--worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| {
                error!("Failed to spawn '{}': {}", worker_path.display(), e);
                SpiceError::WorkerSpawnFailed(format!(
                    "Failed to spawn '{}': {}",
                    worker_path.display(),
                    e
                ))
            })?;

        debug!("Worker process spawned successfully");

        let stdin = process.stdin.take().ok_or(SpiceError::WorkerSpawnFailed(
            "Failed to get stdin".to_string(),
        ))?;

        let stdout = process.stdout.take().ok_or(SpiceError::WorkerSpawnFailed(
            "Failed to get stdout".to_string(),
        ))?;

        let stdout = BufReader::new(stdout);

        Ok(Self {
            process,
            stdin,
            stdout,
        })
    }

    /// Send a request to the worker and get response
    fn request(&mut self, req: &WorkerRequest) -> Result<WorkerResponse> {
        debug!("Sending request to worker");

        // Serialize and send request
        let json = serde_json::to_string(req)
            .map_err(|e| SpiceError::SerializationError(e.to_string()))?;

        trace!("Request JSON: {}", json);

        self.stdin.write_all(json.as_bytes()).map_err(|e| {
            error!("Failed to write to worker stdin: {}", e);
            SpiceError::WorkerCommunicationFailed(e.to_string())
        })?;

        self.stdin
            .write_all(b"\n")
            .map_err(|e| SpiceError::WorkerCommunicationFailed(e.to_string()))?;

        self.stdin.flush().map_err(|e| {
            error!("Failed to flush worker stdin: {}", e);
            SpiceError::WorkerCommunicationFailed(e.to_string())
        })?;

        debug!("Request sent, waiting for response");

        // Read response
        let mut line = String::new();
        self.stdout.read_line(&mut line).map_err(|e| {
            error!("Failed to read from worker stdout: {}", e);
            SpiceError::WorkerCommunicationFailed(e.to_string())
        })?;

        trace!("Response JSON: {}", line);

        let response: WorkerResponse = serde_json::from_str(&line).map_err(|e| {
            error!("Failed to deserialize response: {}", e);
            SpiceError::DeserializationError(e.to_string())
        })?;

        debug!("Response received successfully");
        Ok(response)
    }

    /// Kill the worker process
    fn kill(&mut self) -> Result<()> {
        self.process
            .kill()
            .map_err(|e| SpiceError::WorkerSpawnFailed(e.to_string()))?;
        Ok(())
    }
}

/// Thread pool for ngspice simulations
///
/// Creates a pool of worker processes, each running ngspice independently.
/// Workers are reused across simulations.
pub struct NgspicePool {
    workers: Vec<WorkerProcess>,
    next_worker: usize,
}

impl NgspicePool {
    /// Create a new pool with default size (num_cpus * 2)
    pub fn new() -> Result<Self> {
        let size = num_cpus::get() * 2;
        Self::with_size(size)
    }

    /// Create a pool with a specific number of workers
    pub fn with_size(size: usize) -> Result<Self> {
        assert!(size > 0, "Pool size must be greater than 0");

        debug!("Creating pool with {} workers", size);
        let mut workers = Vec::with_capacity(size);

        for i in 0..size {
            debug!("Spawning worker {}/{}", i + 1, size);
            workers.push(WorkerProcess::spawn()?);
        }

        info!("Pool created with {} workers", size);
        Ok(Self {
            workers,
            next_worker: 0,
        })
    }

    /// Run a simple netlist with a command
    ///
    /// Convenience method that loads a netlist, runs a command, and returns results.
    pub fn run_netlist(&mut self, netlist: &[&str], command: &str) -> Result<SimulationResult> {
        debug!("run_netlist: {} lines, command: {}", netlist.len(), command);

        let req = WorkerRequest::RunSimulation {
            netlist: netlist.iter().map(|s| s.to_string()).collect(),
            command: command.to_string(),
        };

        let idx = self.next_worker % self.workers.len();
        self.next_worker = (self.next_worker + 1) % self.workers.len();

        debug!("Dispatching to worker {}", idx);

        let response = self.workers[idx].request(&req)?;

        match response {
            WorkerResponse::SimulationResult { plots, output } => {
                info!(
                    "Simulation complete: {} plots, {} output lines",
                    plots.len(),
                    output.len()
                );
                Ok(SimulationResult { plots, output })
            }
            WorkerResponse::Error { message } => {
                error!("Worker error: {}", message);
                Err(SpiceError::WorkerError(message))
            }
        }
    }

    /// Get the number of workers in the pool
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    /// Shutdown the pool gracefully
    ///
    /// Kills all worker processes.
    pub fn shutdown(mut self) {
        debug!("Shutting down pool with {} workers", self.workers.len());
        for (i, worker) in self.workers.iter_mut().enumerate() {
            debug!("Killing worker {}", i);
            let _ = worker.kill();
        }
        info!("Pool shutdown complete");
    }
}

impl Default for NgspicePool {
    fn default() -> Self {
        Self::new().expect("Failed to create default pool")
    }
}

impl Drop for NgspicePool {
    fn drop(&mut self) {
        for worker in &mut self.workers {
            let _ = worker.kill();
        }
    }
}

/// Find the worker binary (defaults to current executable)
fn find_worker_binary() -> Result<std::path::PathBuf> {
    if let Ok(path) = std::env::var("NGSPICE_WORKER_BIN") {
        debug!("Using NGSPICE_WORKER_BIN from env: {}", path);
        return Ok(std::path::PathBuf::from(path));
    }

    let current_exe = std::env::current_exe().map_err(|e| {
        error!("Failed to get current exe: {}", e);
        SpiceError::WorkerSpawnFailed(format!("Failed to get current exe: {}", e))
    })?;

    debug!(
        "Using current executable for worker: {}",
        current_exe.display()
    );
    Ok(current_exe)
}
