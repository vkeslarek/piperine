//! Worker runtime used by NgspicePool processes
//!
//! This module contains the logic that runs inside each spawned worker
//! process. It is intentionally kept self-contained so the main binary can
//! start workers by re-executing itself with the `--worker` flag.

use crate::ngspice::NgspiceWorker;
use crate::protocol::{PlotData, WorkerRequest, WorkerResponse};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use tracing::{debug, error, info, trace};

/// Entry point for worker processes.
pub fn worker_main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing with default filter from RUST_LOG environment variable
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("debug")),
        )
        .with_writer(io::stderr)
        .with_target(true)
        .with_level(true)
        .init();

    info!("ngspice-worker starting");

    // Initialize ngspice
    let worker = match NgspiceWorker::new() {
        Ok(w) => {
            info!("ngspice initialized successfully");
            w
        }
        Err(e) => {
            error!("Failed to initialize ngspice: {}", e);
            let resp = WorkerResponse::Error {
                message: format!("Initialization failed: {}", e),
            };
            send_response(&resp);
            return Err(Box::new(e));
        }
    };

    debug!("Starting main request loop");

    // Main request loop
    let stdin = io::stdin();
    let reader = stdin.lock();

    for line in reader.lines() {
        match line {
            Ok(json_line) => {
                if json_line.trim().is_empty() {
                    trace!("Received empty line, skipping");
                    continue;
                }

                debug!("Received request: {}", json_line);
                match process_request(&json_line, &worker) {
                    Ok(response) => {
                        trace!("Sending response");
                        send_response(&response)
                    }
                    Err(e) => {
                        error!("Request processing error: {}", e);
                        let resp = WorkerResponse::Error {
                            message: format!("Processing error: {}", e),
                        };
                        send_response(&resp);
                    }
                }
            }
            Err(e) => {
                error!("Failed to read from stdin: {}", e);
                break;
            }
        }
    }

    info!("ngspice-worker shutting down");
    Ok(())
}

/// Process a single request and return response
fn process_request(
    json: &str,
    worker: &NgspiceWorker,
) -> Result<WorkerResponse, Box<dyn std::error::Error>> {
    debug!("Parsing JSON request");
    let request: WorkerRequest = serde_json::from_str(json)?;

    match request {
        WorkerRequest::RunSimulation { netlist, command } => {
            info!(
                "Running simulation with {} netlist lines and command: {}",
                netlist.len(),
                command
            );

            // Convert Vec<String> to Vec<&str>
            let netlist_refs: Vec<&str> = netlist.iter().map(|s| s.as_str()).collect();

            // Load netlist
            debug!("Loading netlist");
            worker.load_netlist(&netlist_refs)?;
            debug!("Netlist loaded");

            // Run command
            debug!("Running command");
            worker.run_command(&command)?;
            info!("Command executed: {}", command);

            // Get results
            debug!("Retrieving results");
            let sim_data = worker.get_results()?;
            info!("Results collected: {} plots", sim_data.plots.len());
            trace!("Output buffer size: {}", sim_data.output_buffer.len());

            // Reset for next simulation
            debug!("Resetting worker state");
            worker.reset()?;
            debug!("Worker reset complete");

            // Convert to response format
            let plots: HashMap<String, PlotData> = sim_data
                .plots
                .into_iter()
                .map(|(name, plot)| {
                    debug!(
                        "Processing plot: {} with {} vectors",
                        name,
                        plot.vectors.len()
                    );
                    (
                        name.clone(),
                        PlotData {
                            name,
                            vectors: plot.vectors,
                        },
                    )
                })
                .collect();

            Ok(WorkerResponse::SimulationResult {
                plots,
                output: sim_data.output_buffer,
            })
        }

        WorkerRequest::LoadNetlist { lines } => {
            info!("Loading netlist with {} lines", lines.len());
            let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
            worker.load_netlist(&refs)?;
            debug!("Netlist loaded successfully");

            Ok(WorkerResponse::SimulationResult {
                plots: HashMap::new(),
                output: vec![],
            })
        }

        WorkerRequest::RunCommand { command } => {
            info!("Running command: {}", command);
            worker.run_command(&command)?;
            debug!("Command completed");

            Ok(WorkerResponse::SimulationResult {
                plots: HashMap::new(),
                output: vec![],
            })
        }

        WorkerRequest::GetResults => {
            debug!("Retrieving results");
            let sim_data = worker.get_results()?;

            let plots: HashMap<String, PlotData> = sim_data
                .plots
                .into_iter()
                .map(|(name, plot)| {
                    debug!(
                        "Processing plot: {} with {} vectors",
                        name,
                        plot.vectors.len()
                    );
                    (
                        name.clone(),
                        PlotData {
                            name,
                            vectors: plot.vectors,
                        },
                    )
                })
                .collect();

            Ok(WorkerResponse::SimulationResult {
                plots,
                output: sim_data.output_buffer,
            })
        }

        WorkerRequest::Reset => {
            info!("Resetting worker state");
            worker.reset()?;
            debug!("Reset complete");

            Ok(WorkerResponse::SimulationResult {
                plots: HashMap::new(),
                output: vec![],
            })
        }
    }
}

/// Send a response to stdout (parent process)
fn send_response(response: &WorkerResponse) {
    debug!("Serializing response");
    match serde_json::to_string(response) {
        Ok(json) => {
            trace!("Response JSON: {}", json);
            if let Err(e) = writeln!(io::stdout(), "{}", json) {
                error!("Failed to write response: {}", e);
            } else {
                trace!("Response sent successfully");
            }
        }
        Err(e) => {
            error!("Failed to serialize response: {}", e);
        }
    }
}
