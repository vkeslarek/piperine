use std::path::PathBuf;

use piperine_circuit::{
    HardwareRegistry, ParameterDefinition,
    elaborate, extract_va_modules, eval_default_expr,
};
use piperine_interpreter::{Plugin, SystemTaskRegistry, Interpreter, Scope};
use piperine_ngspice::NgspicePlugin;
use piperine_interpreter::AnalogCompilerBackend;
use piperine_openvaf::{LibraryCompiler, OpenVafPlugin, OsdiHardwareDefinition, compile_va};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: piperine <file.ppr>");
        std::process::exit(1);
    }
    if let Err(error) = run(PathBuf::from(&args[1])) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run(path: PathBuf) -> Result<(), String> {
    // ── 1. Parse ─────────────────────────────────────────────────────────────
    let document = piperine_parser::parse_file(&path).map_err(|e| format!("parse: {e}"))?;

    // ── 2. Find VA modules (analog block, no initial block) ───────────────────
    let va_modules = extract_va_modules(&document);

    // ── 3. Build OsdiHardwareDefinitions from parsed metadata ─────────────────
    let osdi_defs: Vec<OsdiHardwareDefinition> = va_modules.iter()
        .map(|info| OsdiHardwareDefinition {
            module_name: info.module_name.clone(),
            port_names: info.port_names.clone(),
            parameter_definitions: info.parameter_defaults.iter()
                .map(|(name, expr)| ParameterDefinition {
                    name: name.clone(),
                    is_expr: false,
                    is_ref: false,
                    default: eval_default_expr(expr),
                })
                .collect(),
        })
        .collect();

    // ── 4. Register OSDI and ngspice hardware/tasks ───────────────────────────
    let mut hardware_registry = HardwareRegistry::new();
    for def in osdi_defs {
        hardware_registry.register(Box::new(def));
    }

    let mut task_registry = SystemTaskRegistry::new();
    let plugins: Vec<Box<dyn Plugin>> = vec![
        Box::new(NgspicePlugin::default()),
        Box::new(OpenVafPlugin::default()),
    ];

    let mut simulator_backend = None;
    for plugin in &plugins {
        plugin.register_hardware(&mut hardware_registry);
        plugin.register_tasks(&mut task_registry);
        if simulator_backend.is_none() {
            simulator_backend = plugin.simulator_backend();
        }
    }

    let mut simulator = simulator_backend
        .ok_or("no simulator backend — is piperine-ngspice registered?")?;

    // ── 5. Compile VA → OSDI, pre_osdi BEFORE load_circuit ───────────────────
    // ngspice must know OSDI models before parsing the netlist that uses them.
    if !va_modules.is_empty() {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("piperine/osdi");

        // One .ppr file → one .osdi (may contain multiple module descriptors).
        let osdi_path = compile_va(&path, &cache_dir)
            .map_err(|e| format!("openvaf: {e}"))?;

        // Use pre_load() which issues the correct ngspice command ("osdi <path>").
        LibraryCompiler.pre_load(&osdi_path, simulator.as_mut())
            .map_err(|e| format!("osdi load: {e}"))?;
    }

    // ── 6. Elaborate testbench ────────────────────────────────────────────────
    let mut elaboration = elaborate(&document, &hardware_registry)
        .map_err(|e| format!("elaboration: {e}"))?;
    elaboration.spice_lines.push(".end".to_string());

    // ── 7. Load netlist (OSDI models already registered in step 5) ────────────
    simulator
        .load_circuit(&elaboration.spice_lines)
        .map_err(|e| format!("circuit load: {e}"))?;

    // ── 8. Run interpreter ────────────────────────────────────────────────────
    let mut interpreter = Interpreter::new(simulator.as_mut(), &task_registry);
    interpreter.set_functions(elaboration.functions);
    let mut scope = Scope::default();
    interpreter
        .exec(&elaboration.initial_statement, &mut scope)
        .map_err(|e| format!("runtime: {e}"))?;

    Ok(())
}
