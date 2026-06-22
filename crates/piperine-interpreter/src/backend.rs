use std::path::{Path, PathBuf};
use std::collections::HashMap;
use piperine_common::{EventAction, SimEventKind};
use crate::error::InterpreterError;
use crate::value::{AnalysisResult, RunError, RunErrorKind, VectorData, parse_analysis_kind};

/// A simulator event returned by `poll_analysis`.
#[derive(Debug)]
pub enum AnalysisEvent {
    /// The simulator fired a simulation event (step, crossing, etc.).
    /// The interpreter must dispatch handler bodies and then call
    /// `respond_to_analysis_event` with the resulting `EventAction`.
    Event { kind: SimEventKind, time: f64, crossing_id: u32 },
    /// The analysis finished. `plot_name` is the ngspice plot to harvest
    /// vectors from. `had_run_errors` is set if the worker detected SOA
    /// violations or other non-fatal errors that were NOT reported through
    /// `EventAction::RunError` (e.g. ngspice-internal limits).
    Done { plot_name: String, had_run_errors: bool },
}

/// A live simulator session — the interpreter calls into this to run analyses
/// and read results.
///
/// The ngspice implementation (`NgspiceBackend` in `piperine-ngspice`) wraps
/// a process-isolated worker via IPC. Future backends (Xyce, FOSS SPICE) implement
/// the same trait without touching the interpreter.
pub trait SimulatorBackend: Send {
    /// Load a SPICE netlist into the simulator.
    /// Must be called once before any analysis.
    fn load_circuit(&mut self, lines: &[String]) -> Result<(), InterpreterError>;

    /// Run a simulator command (e.g., `"op"`, `"tran 1n 5m"`) without event streaming.
    fn run_command(&mut self, command: &str) -> Result<(), InterpreterError>;

    /// Retrieve all values of a named vector from the current plot.
    fn get_vector(&mut self, name: &str) -> Result<Vec<f64>, InterpreterError>;

    /// List all vector names available in the given plot.
    fn list_vectors(&mut self, plot_name: &str) -> Result<Vec<String>, InterpreterError>;

    /// Print a line to stdout. Default implementation uses `println!`.
    fn print(&self, line: &str) { println!("{line}"); }

    // --- Analysis streaming primitives ---
    //
    // These three methods form a protocol used by `Interpreter::run_analysis`:
    //
    //   start_analysis(cmd, fire_step)
    //   loop {
    //       match poll_analysis()? {
    //           Event { .. } => { let action = dispatch(..); respond_to_analysis_event(action)?; }
    //           Done { .. }  => break,
    //       }
    //   }
    //
    // The backend owns the IPC/FFI channel; the interpreter owns the handler dispatch.
    // No callbacks, no trait objects crossing the borrow boundary.

    /// Start an analysis command. After this returns, `poll_analysis` drives the loop.
    /// `fire_step`: set true only if the testbench has `always @(step)` blocks;
    /// per-step round-trips are expensive and skipped otherwise.
    fn start_analysis(&mut self, cmd: &str, fire_step: bool) -> Result<(), InterpreterError>;

    /// Wait for the next event or completion from the running analysis.
    /// Must only be called after `start_analysis` and before `Done` is returned.
    fn poll_analysis(&mut self) -> Result<AnalysisEvent, InterpreterError>;

    /// Send the interpreter's response to the last `AnalysisEvent::Event`.
    /// Must be called exactly once per `Event` before the next `poll_analysis`.
    fn respond_to_analysis_event(&mut self, action: EventAction) -> Result<(), InterpreterError>;

    /// Run a full analysis without always-block event streaming and return the
    /// collected result. Events are acknowledged with `Continue`. Use for analysis
    /// tasks ($noise, $tf, $ac, etc.) that don't need step callbacks.
    fn run_analysis_simple(&mut self, cmd: &str) -> Result<AnalysisResult, InterpreterError> {
        self.start_analysis(cmd, false)?;
        loop {
            match self.poll_analysis()? {
                AnalysisEvent::Done { plot_name, had_run_errors } => {
                    let names = self.list_vectors(&plot_name)?;
                    let mut vectors = HashMap::new();
                    for name in names {
                        let data = self.get_vector(&name)?;
                        vectors.insert(name, VectorData::Real(data));
                    }
                    let run_errors = if had_run_errors {
                        vec![RunError {
                            message: "simulator reported errors".into(),
                            time: None,
                            kind: RunErrorKind::SoaViolation,
                        }]
                    } else {
                        vec![]
                    };
                    return Ok(AnalysisResult {
                        kind: parse_analysis_kind(cmd),
                        dataset: plot_name,
                        vectors,
                        run_errors,
                    });
                }
                AnalysisEvent::Event { .. } => {
                    self.respond_to_analysis_event(EventAction::Continue)?;
                }
            }
        }
    }
}

/// Convenience: harvest all vectors from `plot_name` via the trait methods.
/// Called by `Interpreter::run_analysis` after the `Done` event.
pub(crate) fn collect_vectors(
    backend: &mut dyn SimulatorBackend,
    plot_name: &str,
) -> Result<HashMap<String, VectorData>, InterpreterError> {
    let names = backend.list_vectors(plot_name)?;
    let mut map = HashMap::new();
    for name in names {
        let data = backend.get_vector(&name)?;
        map.insert(name, VectorData::Real(data));
    }
    Ok(map)
}

/// A backend that compiles Verilog-A modules to loadable simulator objects.
///
/// The OpenVAF implementation (`OpenVafCompiler` in `piperine-openvaf`, Phase 2)
/// invokes the `openvaf` binary and caches results by source hash.
pub trait AnalogCompilerBackend: Send + Sync {
    /// Compiler identifier (e.g., `"openvaf"`).
    fn name(&self) -> &str;

    /// Compile a Verilog-A source file to a simulator-loadable artifact (e.g., `.osdi`).
    /// `output_directory`: where to place the compiled artifact.
    /// Returns the path of the compiled file.
    fn compile(
        &self,
        source_path: &Path,
        output_directory: &Path,
    ) -> Result<PathBuf, InterpreterError>;

    /// Load the compiled artifact into a live simulator session.
    /// For OSDI this runs `pre_osdi <path>` via the simulator backend.
    fn pre_load(
        &self,
        artifact_path: &Path,
        simulator: &mut dyn SimulatorBackend,
    ) -> Result<(), InterpreterError>;
}
