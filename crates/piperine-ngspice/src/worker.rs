//! Worker process entry point.
//!
//! When the piperine binary is invoked with `--worker`, it calls `worker_main()`
//! which enters the IPC loop: read commands from stdin, execute ngspice operations,
//! write results to stdout.

use crate::instance::NgspiceInstance;
use crate::protocol::*;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

/// Length-prefixed bincode read from a reader.
fn read_msg<T: serde::de::DeserializeOwned>(r: &mut impl Read) -> io::Result<T> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    bincode::deserialize(&buf)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Length-prefixed bincode write to a writer.
fn write_msg<T: serde::Serialize>(w: &mut impl Write, msg: &T) -> io::Result<()> {
    let bytes = bincode::serialize(msg)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    w.write_all(&(bytes.len() as u32).to_le_bytes())?;
    w.write_all(&bytes)?;
    w.flush()
}

/// Main entry point for the worker process.
pub fn worker_main() -> io::Result<()> {
    let instance = NgspiceInstance::new()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();

    // Signal ready
    write_msg(&mut stdout, &WorkerToMain::Ready)?;

    loop {
        let msg: MainToWorker = match read_msg(&mut stdin) {
            Ok(m) => m,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        };

        match msg {
            MainToWorker::RunSimulation {
                netlist_lines,
                control_commands,
                has_external_sources,
            } => {
                let result = run_simulation(
                    &instance,
                    &netlist_lines,
                    &control_commands,
                    has_external_sources,
                    &mut stdin,
                    &mut stdout,
                );
                match result {
                    Ok(resp) => write_msg(&mut stdout, &resp)?,
                    Err(e) => write_msg(&mut stdout, &WorkerToMain::Error {
                        message: e.to_string(),
                    })?,
                }
            }
            MainToWorker::Reset => {
                let _ = instance.command("destroy all");
                write_msg(&mut stdout, &WorkerToMain::Ok)?;
            }
            MainToWorker::Shutdown => {
                break;
            }
            MainToWorker::ExternalSourceValue { .. } => {
                // Unexpected outside of simulation - ignore
            }
        }
    }

    Ok(())
}

fn run_simulation(
    instance: &NgspiceInstance,
    netlist_lines: &[String],
    control_commands: &[String],
    has_external_sources: bool,
    _stdin: &mut impl Read,
    stdout: &mut impl Write,
) -> Result<WorkerToMain, Box<dyn std::error::Error>> {
    // If we have external sources, set up the bridge
    if has_external_sources {
        let bridge = Arc::new(ExternalSourceBridge::new(stdout as *mut _ as usize));
        let bridge_clone = bridge.clone();

        instance.set_vsrc_handler(move |name, time| {
            bridge_clone.request_value(name, time)
        });
        // Note: for a full implementation, isrc_handler would also be set up similarly
    }

    // Load circuit
    instance.load_circuit(netlist_lines)?;

    // Execute control commands
    for cmd in control_commands {
        instance.command(cmd)?;
    }

    // Collect results
    let result = instance.collect_results()?;

    // Clear handlers
    instance.clear_external_handlers();

    // Convert to protocol types
    let mut plots = HashMap::new();
    for (name, plot) in &result.plots {
        let mut vectors = HashMap::new();
        for (vname, vec) in &plot.vectors {
            let vdata = match vec {
                piperine_core::result::Vector::Real(rv) => VectorData::Real {
                    name: rv.name.clone(),
                    data: rv.data.clone(),
                },
                piperine_core::result::Vector::Complex(cv) => VectorData::Complex {
                    name: cv.name.clone(),
                    data: cv.data.clone(),
                },
            };
            vectors.insert(vname.clone(), vdata);
        }
        plots.insert(name.clone(), PlotData {
            name: plot.name.clone(),
            plot_type: format!("{:?}", plot.plot_type),
            vectors,
        });
    }

    // Reset for next simulation
    let _ = instance.command("destroy all");

    Ok(WorkerToMain::SimulationComplete {
        plots,
        measurements: result.measurements,
        log: result.log,
    })
}

/// Bridge for external source callbacks.
///
/// When ngspice calls back requesting an external source value,
/// this bridge sends a request to the main process via IPC and
/// blocks until it receives the response.
///
/// NOTE: This is a simplified single-threaded version. The full
/// bilateral async version with separate IPC thread will be
/// implemented when background simulation (bg_run) is used.
struct ExternalSourceBridge {
    _next_id: AtomicU64,
    // For the simple synchronous case, we store pending values here.
    // In the full implementation, this would use channels.
    _stdout_addr: usize,
}

impl ExternalSourceBridge {
    fn new(stdout_addr: usize) -> Self {
        Self {
            _next_id: AtomicU64::new(0),
            _stdout_addr: stdout_addr,
        }
    }

    fn request_value(&self, _source_name: &str, _time: f64) -> f64 {
        // TODO: Full bilateral IPC implementation.
        // For now, return 0.0 as a placeholder.
        // The real implementation will:
        // 1. Send ExternalSourceRequest to main via stdout
        // 2. Block waiting for ExternalSourceValue response from main via stdin
        // 3. Return the value
        0.0
    }
}
