use std::path::{Path, PathBuf};
use crate::error::InterpreterError;

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

    /// Run a simulator command (e.g., `"op"`, `"tran 1n 5m"`).
    fn run_command(&mut self, command: &str) -> Result<(), InterpreterError>;

    /// Retrieve all values of a named vector from the current plot.
    /// For OP analysis, returns a single-element Vec.
    /// Vector names follow ngspice convention: `"v(vmid)"`, `"i(v1)"`.
    fn get_vector(&mut self, name: &str) -> Result<Vec<f64>, InterpreterError>;

    /// Print a line to stdout. Default implementation uses `println!`.
    fn print(&self, line: &str) { println!("{line}"); }
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
