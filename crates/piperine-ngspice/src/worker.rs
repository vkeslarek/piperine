//! Worker process entry point.
//!
//! For external sources: the callback writes a request directly to stdout (dup'd fd)
//! and reads the response from stdin (dup'd fd). Fully synchronous — no extra threads.
//! This works because ngspice calls callbacks synchronously from the simulation thread.

use crate::instance::NgspiceInstance;
use crate::protocol::*;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

fn read_msg<T: serde::de::DeserializeOwned>(r: &mut impl Read) -> io::Result<T> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    bincode::deserialize(&buf)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn write_msg<T: serde::Serialize>(w: &mut impl Write, msg: &T) -> io::Result<()> {
    let bytes = bincode::serialize(msg)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    w.write_all(&(bytes.len() as u32).to_le_bytes())?;
    w.write_all(&bytes)?;
    w.flush()
}

/// Create dup'd file descriptors that don't conflict with Rust's stdin/stdout locks.
mod dup_io {
    use std::fs::File;
    use std::os::unix::io::FromRawFd;

    pub fn dup_stdin() -> File {
        let fd = unsafe { libc::dup(0) };
        assert!(fd >= 0, "dup(0) failed");
        unsafe { File::from_raw_fd(fd) }
    }

    pub fn dup_stdout() -> File {
        let fd = unsafe { libc::dup(1) };
        assert!(fd >= 0, "dup(1) failed");
        unsafe { File::from_raw_fd(fd) }
    }
}

pub fn worker_main() -> io::Result<()> {
    let instance = NgspiceInstance::new()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();

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
                let result = if has_external_sources {
                    // Drop Rust's locked stdin/stdout — we'll use dup'd fds in callbacks
                    drop(stdin);
                    drop(stdout);

                    let r = run_with_external(&instance, &netlist_lines, &control_commands);

                    // Re-acquire Rust locks for the main loop
                    stdin = io::stdin().lock();
                    stdout = io::stdout().lock();
                    r
                } else {
                    run_simple(&instance, &netlist_lines, &control_commands)
                };

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
            MainToWorker::Shutdown => break,
            MainToWorker::ExternalSourceValue { .. } => {}
        }
    }

    Ok(())
}

fn run_simple(
    instance: &NgspiceInstance,
    netlist: &[String],
    commands: &[String],
) -> Result<WorkerToMain, Box<dyn std::error::Error>> {
    instance.load_circuit(netlist)?;
    for cmd in commands {
        instance.command(cmd)?;
    }
    let result = instance.collect_results()?;
    let _ = instance.command("destroy all");
    Ok(to_protocol(result))
}

fn run_with_external(
    instance: &NgspiceInstance,
    netlist: &[String],
    commands: &[String],
) -> Result<WorkerToMain, Box<dyn std::error::Error>> {
    // Synchronous bridge: callbacks write request to dup'd stdout,
    // read response from dup'd stdin. No threads needed.
    let bridge = Arc::new(SyncBridge::new());

    let vb = bridge.clone();
    instance.set_vsrc_handler(move |name, time| vb.request_value(name, time));
    let ib = bridge.clone();
    instance.set_isrc_handler(move |name, time| ib.request_value(name, time));

    instance.load_circuit(netlist)?;
    for cmd in commands {
        instance.command(cmd)?;
    }

    let result = instance.collect_results()?;
    instance.clear_external_handlers();
    let _ = instance.command("destroy all");

    Ok(to_protocol(result))
}

/// Synchronous bridge for external source callbacks.
///
/// Callbacks happen on the same thread as ngSpice_Command (synchronous simulation).
/// We write the request and read the response directly on dup'd fds.
struct SyncBridge {
    next_id: AtomicU64,
    io: Mutex<(std::fs::File, std::fs::File)>,
}

impl SyncBridge {
    fn new() -> Self {
        Self {
            next_id: AtomicU64::new(0),
            io: Mutex::new((dup_io::dup_stdin(), dup_io::dup_stdout())),
        }
    }

    fn request_value(&self, source_name: &str, time: f64) -> f64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut io = self.io.lock().unwrap();
        let (ref mut inp, ref mut out) = *io;

        let req = WorkerToMain::ExternalSourceRequest {
            request_id: id,
            source_name: source_name.to_string(),
            time,
        };
        if write_msg(out, &req).is_err() {
            return 0.0;
        }

        match read_msg::<MainToWorker>(inp) {
            Ok(MainToWorker::ExternalSourceValue { value, .. }) => value,
            _ => 0.0,
        }
    }
}

fn to_protocol(result: piperine_api::result::SimulationResult) -> WorkerToMain {
    let mut plots = HashMap::new();
    for (name, plot) in &result.plots {
        let mut vectors = HashMap::new();
        for (vname, vec) in &plot.vectors {
            let vdata = match vec {
                piperine_api::result::Vector::Real(rv) => VectorData::Real {
                    name: rv.name.clone(), data: rv.data.clone(),
                },
                piperine_api::result::Vector::Complex(cv) => VectorData::Complex {
                    name: cv.name.clone(), data: cv.data.clone(),
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
    WorkerToMain::SimulationComplete {
        plots,
        measurements: result.measurements,
        log: result.log,
    }
}
