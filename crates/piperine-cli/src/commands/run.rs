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

    println!(
        "{} elaborates. The in-language `bench` was removed: write a Python \
         testbench (`*_tb.py`) and run `piperine test`, or drive the design \
         interactively with `piperine run -i {}`.",
        path.display(),
        path.display()
    );
}
