use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(|s| s.as_str()) {
        Some("new")   => cmd_new(args.get(2).map(|s| s.as_str())),
        Some("setup") => cmd_setup(&project_root()),
        Some("run")   => match args.get(2) {
            Some(script) => cmd_run(Path::new(script), &project_root()),
            None => { eprintln!("usage: piperine run <script.py>"); std::process::exit(1); }
        },
        Some("check") => match args.get(2) {
            Some(file) => cmd_check(Path::new(file)),
            None => { eprintln!("usage: piperine check <file.ppr>"); std::process::exit(1); }
        },
        _ => {
            eprintln!("usage:");
            eprintln!("  piperine new <name>       scaffold a new project");
            eprintln!("  piperine setup            build piperine.so into .venv");
            eprintln!("  piperine run <script.py>  run a bench script");
            eprintln!("  piperine check <file.ppr> parse and elaborate a hardware file");
            std::process::exit(1);
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

// ── piperine new <name> ───────────────────────────────────────────────────────

fn cmd_new(name: Option<&str>) -> Result<(), String> {
    let name = name.ok_or("usage: piperine new <name>")?;
    let root = PathBuf::from(name);
    if root.exists() {
        return Err(format!("'{name}' already exists"));
    }
    // Design files live together: hello/hello.ppr + hello/hello.py
    std::fs::create_dir_all(root.join("hello")).map_err(|e| e.to_string())?;

    std::fs::write(root.join("piperine.toml"), format!(
        "[project]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n[backend]\nsimulator = \"ngspice\"\n"
    )).map_err(|e| e.to_string())?;

    std::fs::write(root.join("hello").join("hello.ppr"),
        include_str!("templates/hello.ppr"),
    ).map_err(|e| e.to_string())?;

    std::fs::write(root.join("hello").join("hello.py"),
        include_str!("templates/hello.py"),
    ).map_err(|e| e.to_string())?;

    println!("created project '{name}'");
    println!("running setup...");
    cmd_setup(&root)?;
    println!();
    println!("ready! try:");
    println!("  cd {name}");
    println!("  piperine run hello/hello.py");
    Ok(())
}

// ── piperine setup ────────────────────────────────────────────────────────────

fn cmd_setup(root: &Path) -> Result<(), String> {
    let venv = root.join(".venv");

    // 1. Create venv if absent
    if !venv.exists() {
        println!("creating .venv ...");
        run_cmd(Command::new("python3").args(["-m", "venv", venv.to_str().unwrap()]))?;
    }

    let pip = venv.join("bin").join("pip");

    // 2. Install Python deps
    println!("installing Python deps ...");
    run_cmd(Command::new(&pip).args(["install", "--quiet", "numpy", "matplotlib"]))?;

    // 3. Install maturin into venv
    run_cmd(Command::new(&pip).args(["install", "--quiet", "maturin"]))?;

    // 4. Build piperine-python and install into venv
    let piperine_python_dir = piperine_python_crate_dir()?;
    println!("building piperine extension ...");
    let maturin = venv.join("bin").join("maturin");
    run_cmd(
        Command::new(&maturin)
            .args(["develop", "--manifest-path",
                   piperine_python_dir.join("Cargo.toml").to_str().unwrap()])
            .env("VIRTUAL_ENV", &venv)
    )?;

    // 5. Copy piperine-worker into .venv/bin/ so ProcessPool finds it from Python.
    let worker_src = find_worker_binary(&piperine_python_dir)?;
    let worker_dst = venv.join("bin").join("piperine-worker");
    std::fs::copy(&worker_src, &worker_dst)
        .map_err(|e| format!("copy piperine-worker: {e}"))?;

    println!("setup complete — piperine extension ready in .venv");
    Ok(())
}

// ── piperine run <script.py> ──────────────────────────────────────────────────

fn cmd_run(script: &Path, root: &Path) -> Result<(), String> {
    let venv = root.join(".venv");
    if !venv.exists() {
        return Err("no .venv found — run `piperine setup` first".into());
    }
    let python = venv.join("bin").join("python3");
    let worker = venv.join("bin").join("piperine-worker");
    let status = Command::new(&python)
        .arg(script)
        .current_dir(root)
        .env("PIPERINE_WORKER", &worker)
        .status()
        .map_err(|e| format!("python3: {e}"))?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

// ── piperine check <file.ppr> ─────────────────────────────────────────────────

fn cmd_check(path: &Path) -> Result<(), String> {
    use piperine_circuit::{HardwareRegistry, elaborate_circuit};
    use piperine_ngspice::register_hardware;

    let doc = piperine_parser::parse_file(path).map_err(|e| format!("parse: {e}"))?;
    let mut registry = HardwareRegistry::new();
    register_hardware(&mut registry);
    let circuit = elaborate_circuit(&doc, &registry, None)
        .map_err(|e| format!("elaboration: {e}"))?;
    println!("ok — {} SPICE lines, {} SOA checks",
             circuit.spice_lines.len(), circuit.soa_checks.len());
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn project_root() -> PathBuf {
    // Walk up from cwd looking for piperine.toml
    let mut dir = std::env::current_dir().unwrap_or_default();
    loop {
        if dir.join("piperine.toml").exists() { return dir; }
        if !dir.pop() { break; }
    }
    std::env::current_dir().unwrap_or_default()
}

fn find_worker_binary(piperine_python_dir: &Path) -> Result<PathBuf, String> {
    // Walk up from piperine-python crate to workspace root, then look in target/.
    let workspace = piperine_python_dir.parent().and_then(|p| p.parent())
        .ok_or("cannot resolve workspace root from piperine-python dir")?;
    let candidates = [
        workspace.join("target").join("debug").join("piperine-worker"),
        workspace.join("target").join("release").join("piperine-worker"),
    ];
    for c in &candidates {
        if c.exists() { return Ok(c.clone()); }
    }
    Err(format!("piperine-worker not found in {}; run `cargo build -p piperine-worker` first",
                workspace.join("target").display()))
}

fn piperine_python_crate_dir() -> Result<PathBuf, String> {
    // When running from the built binary, look for piperine-python relative
    // to the executable (same workspace layout).
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // target/debug/piperine  →  ../../crates/piperine-python
    let candidates = [
        exe.parent().and_then(|p| p.parent()).and_then(|p| p.parent())
            .map(|p| p.join("crates").join("piperine-python")),
        // Also try relative to cwd (dev workflow)
        Some(std::env::current_dir().unwrap_or_default().join("crates").join("piperine-python")),
    ];
    for c in candidates.iter().flatten() {
        if c.join("Cargo.toml").exists() { return Ok(c.clone()); }
    }
    Err("cannot find piperine-python crate — run from the workspace root".into())
}

fn run_cmd(cmd: &mut Command) -> Result<(), String> {
    let status = cmd
        .stdin(Stdio::null())
        .status()
        .map_err(|e| {
            let prog = cmd.get_program().to_string_lossy().to_string();
            format!("{prog}: {e}")
        })?;
    if !status.success() {
        let prog = cmd.get_program().to_string_lossy().to_string();
        return Err(format!("{prog} exited with code {}", status.code().unwrap_or(-1)));
    }
    Ok(())
}
