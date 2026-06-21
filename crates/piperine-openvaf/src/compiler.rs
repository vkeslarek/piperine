use std::path::{Path, PathBuf};

use camino::Utf8PathBuf;
use openvaf::{
    compile, host_triple, CompilationDestination, CompilationTermination, LLVMCodeGenOptLevel,
    Opts, Target,
};
use piperine_interpreter::{AnalogCompilerBackend, InterpreterError, SimulatorBackend};

pub struct LibraryCompiler;

impl AnalogCompilerBackend for LibraryCompiler {
    fn name(&self) -> &str {
        "openvaf"
    }

    fn compile(
        &self,
        source_path: &Path,
        output_directory: &Path,
    ) -> Result<PathBuf, InterpreterError> {
        let input = Utf8PathBuf::from_path_buf(source_path.to_path_buf()).map_err(|p| {
            InterpreterError::SimulatorError(format!(
                "openvaf: source path is not valid UTF-8: {}",
                p.display()
            ))
        })?;

        let stem = input.file_stem().unwrap_or("module");
        let lib_file: Utf8PathBuf =
            Utf8PathBuf::from_path_buf(output_directory.join(format!("{stem}.osdi")))
                .map_err(|p| {
                    InterpreterError::SimulatorError(format!(
                        "openvaf: output path is not valid UTF-8: {}",
                        p.display()
                    ))
                })?;

        let host = host_triple();
        let target = Target::search(host).ok_or_else(|| {
            InterpreterError::SimulatorError(format!(
                "openvaf: host triple '{host}' not supported"
            ))
        })?;

        let opts = Opts {
            input,
            output: CompilationDestination::Path { lib_file: lib_file.clone() },
            defines: vec!["__OPENVAF_COMPILER__".to_string()],
            lints: Vec::new(),
            codegen_opts: Vec::new(),
            include: Vec::new(),
            opt_lvl: LLVMCodeGenOptLevel::LLVMCodeGenLevelDefault,
            target,
            target_cpu: "native".to_string(),
            dry_run: false,
            dump_mir: false,
            dump_unopt_mir: false,
            dump_ir: false,
            dump_unopt_ir: false,
        };

        match compile(&opts)
            .map_err(|e| InterpreterError::SimulatorError(format!("openvaf compile: {e}")))?
        {
            CompilationTermination::Compiled { lib_file: out } => {
                Ok(out.into_std_path_buf())
            }
            CompilationTermination::FatalDiagnostic => Err(InterpreterError::SimulatorError(
                "openvaf: compilation failed with fatal diagnostic (see stderr)".into(),
            )),
        }
    }

    fn pre_load(
        &self,
        artifact_path: &Path,
        simulator: &mut dyn SimulatorBackend,
    ) -> Result<(), InterpreterError> {
        let path_str = artifact_path.to_str().ok_or_else(|| {
            InterpreterError::SimulatorError(format!(
                "openvaf: artifact path is not valid UTF-8: {}",
                artifact_path.display()
            ))
        })?;
        // ngspice 46+ uses "osdi" (not "pre_osdi") to load OSDI shared libraries.
        // "pre_osdi" only works in netlist control blocks, not as a shared-lib command.
        simulator.run_command(&format!("osdi {path_str}"))
    }
}
