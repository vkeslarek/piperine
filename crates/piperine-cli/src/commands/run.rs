use std::path::PathBuf;

pub fn execute(entry: Option<String>, file: Option<String>, interactive: bool) {
    // Interactive Python REPL: `piperine run -i [design.phdl]`.
    // Pre-loads `import piperine`; with a `.phdl` arg, loads it as `design`.
    if interactive {
        let design_path = entry.as_deref().filter(|e| e.ends_with(".phdl"));
        if let Err(e) = piperine_python::embed::run_interactive(design_path) {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
        return;
    }

    // Python script path: `piperine run foo.py` embeds CPython and runs the
    // script with `import piperine` available (PY-15 / spec AC16). Detected
    // by `.py` suffix on the positional `entry` arg so no extra flag is
    // needed. Python is the scripting host — the in-language `bench` was
    // removed (bench-removal); project testbenches are `*_tb.py` files run
    // by `piperine test`.
    if entry.as_deref().is_some_and(|e| e.ends_with(".py")) {
        let path = entry.expect("checked above");
        if let Err(e) = piperine_python::embed::run_script(&path) {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
        return;
    }

    // A positional `.phdl`/`.ppr` names the design file to elaborate; any
    // other entry form (the removed bench `module::fn`) is a loud error,
    // never a silent ignore.
    let file = match entry {
        Some(e) if e.ends_with(".phdl") || e.ends_with(".ppr") => Some(e),
        Some(e) => {
            eprintln!(
                "Error: unknown entry `{e}`. The in-language `bench` was removed: run a \
                 Python script (`piperine run foo.py`), elaborate a design (`piperine run \
                 foo.phdl`), or run `*_tb.py` testbenches with `piperine test`."
            );
            std::process::exit(1);
        }
        None => file,
    };

    crate::commands::build::execute(file.clone());

    let path = if let Some(f) = file {
        PathBuf::from(f)
    } else {
        if let Some(root) = piperine_project::get_current_project_root() {
            root.join("src").join("main.phdl")
        } else {
            eprintln!("Error: No Piperine.toml found. Please provide a file.");
            std::process::exit(1);
        }
    };

    // Elaborate for real before claiming success — a design that does not
    // parse must fail loud, not print an "elaborates" notice.
    let (source_map, project_root) = super::utils::build_source_map();
    let body = match std::fs::read_to_string(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error reading {}: {}", path.display(), e);
            std::process::exit(1);
        }
    };
    let plugin_host = super::utils::load_plugin_host(&project_root);
    if let Err(e) = plugin_host.fire_after_parse(&body) {
        eprintln!("Plugin error: {e}");
        std::process::exit(1);
    }
    if let Err(e) = piperine_lang::parse_and_elaborate_seeded(&body, &source_map, |ctx| {
        plugin_host.seed_schemas(ctx);
    }) {
        eprintln!("Error in file {}:\n{:?}", path.display(), e);
        std::process::exit(1);
    }

    println!(
        "{} elaborates. The in-language `bench` was removed: write a Python \
         testbench (`*_tb.py`) and run `piperine test`, or drive the design \
         interactively with `piperine run -i {}`.",
        path.display(),
        path.display()
    );
}
