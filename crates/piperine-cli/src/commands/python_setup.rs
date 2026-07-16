//! Python venv setup for a Piperine project — creates a `.venv/` and installs
//! the `_piperine` native extension + the typed `piperine` facade so IDEs see
//! full autocomplete and `import piperine` resolves from a plain `python` run
//! (no `piperine run` needed).
//!
//! The native extension is **bundled in the binary at build time** (via
//! `build.rs` + `include_bytes!`) so the end user doesn't need a `target/`
//! dir or cargo — just `piperine new` and the venv is ready.

use std::path::Path;
#[cfg(bundled_python)]
use std::path::PathBuf;
#[cfg(bundled_python)]
use std::process::Command;

/// The typed pure-Python facade — the same source the embed path materializes
/// (see `piperine-python/src/embed.rs`). Embedded so there's no file drift.
#[cfg(bundled_python)]
const FACADE_SRC: &str = include_str!("../../../piperine-python/python/piperine/__init__.py");

/// The pre-built native extension, embedded at build time by `build.rs`.
/// `None` when the `.so` wasn't available when the CLI was compiled (first
/// build or extension-module not yet built).
#[cfg(bundled_python)]
const NATIVE_SO: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/_piperine.so"));

/// Run the full Python setup: venv + native extension + facade.
///
/// Called by `piperine new` after the project skeleton is created. Prints
/// progress + instructions; never aborts `new` on a Python failure (the PHDL
/// project is still valid without Python — the setup is a bonus).
pub fn setup(project_path: &Path) {
    println!("Setting up Python environment...");

    #[cfg(not(bundled_python))]
    {
        // The `.so` wasn't available when the CLI was compiled — the PHDL
        // project is still valid without Python, so instruct and skip.
        let _ = project_path;
        eprintln!("  ! This piperine binary was built without a bundled _piperine.so.");
        eprintln!("    Rebuild with: cargo build -p piperine-python --features extension-module");
        eprintln!("    Then rebuild the CLI. Skipping Python venv setup.");
    }

    #[cfg(bundled_python)]
    setup_bundled(project_path);
}

/// The bundled-extension path: venv + native extension + facade.
#[cfg(bundled_python)]
fn setup_bundled(project_path: &Path) {
    let venv_path = project_path.join(".venv");
    let site_packages = match create_venv(project_path, &venv_path) {
        Some(sp) => sp,
        None => {
            eprintln!("  ! failed to create .venv — is python3 installed?");
            return;
        }
    };

    if install_extension(&site_packages).is_err() {
        eprintln!("  ! failed to install _piperine.so into the venv.");
        return;
    }
    if install_facade(&site_packages).is_err() {
        eprintln!("  ! failed to install the piperine facade.");
        return;
    }

    println!("  ✓ Python venv ready at .venv/");
    println!("  ✓ `import piperine` resolves with full autocomplete.");
    println!();
    println!("  IDE: select {}/.venv/bin/python as your interpreter.", project_path.display());
    println!("  Terminal: source {}/.venv/bin/activate", project_path.display());
    println!("  Then: pip install numpy matplotlib  (per-project deps)");
}

/// Create a `.venv/` in the project and return the path to its site-packages.
#[cfg(bundled_python)]
fn create_venv(project_path: &Path, venv_path: &Path) -> Option<PathBuf> {
    if venv_path.exists() {
        println!("  • .venv already exists — reusing");
    } else {
        print!("  • creating .venv...");
        std::io::Write::flush(&mut std::io::stdout()).ok();
        let status = Command::new("python3")
            .args(["-m", "venv", venv_path.to_str()?])
            .current_dir(project_path)
            .status()
            .ok()?;
        if !status.success() {
            eprintln!(" FAILED");
            return None;
        }
        println!(" done");
    }

    // Discover site-packages: .venv/lib/pythonX.Y/site-packages/
    let lib = venv_path.join("lib");
    std::fs::read_dir(&lib).ok()?.find_map(|entry| {
        let path = entry.ok()?.path();
        if path.file_name()?.to_str()?.starts_with("python") {
            Some(path.join("site-packages"))
        } else {
            None
        }
    })
}

/// Write the bundled `.so` as `_piperine.so` into site-packages.
#[cfg(bundled_python)]
fn install_extension(site_packages: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(site_packages)?;
    std::fs::write(site_packages.join("_piperine.so"), NATIVE_SO)?;
    println!("  ✓ installed _piperine.so ({} KB)", NATIVE_SO.len() / 1024);
    Ok(())
}

/// Write the typed facade `piperine/__init__.py` into site-packages.
#[cfg(bundled_python)]
fn install_facade(site_packages: &Path) -> std::io::Result<()> {
    let pkg_dir = site_packages.join("piperine");
    std::fs::create_dir_all(&pkg_dir)?;
    std::fs::write(pkg_dir.join("__init__.py"), FACADE_SRC)?;
    println!("  ✓ installed piperine facade (autocomplete)");
    Ok(())
}
