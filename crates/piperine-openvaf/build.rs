/// Build script for piperine-openvaf.
///
/// Verifies that LLVM 18 is available. The actual LLVM link flags are
/// handled by llvm-sys (pulled in transitively by openvaf). This script
/// exists only to produce a clear error message when LLVM is missing,
/// instead of a cryptic link failure.
fn main() {
    // llvm-sys uses the LLVM_SYS_181_PREFIX env var to locate LLVM 18.
    // If that's not set, it falls back to llvm-config on PATH.
    // We just emit a cargo:rerun-if-env-changed so changes to the env
    // variable trigger a rebuild.
    println!("cargo:rerun-if-env-changed=LLVM_SYS_181_PREFIX");
    println!("cargo:rerun-if-env-changed=PIPERINE_OPENVAF_SKIP_LLVM_CHECK");

    // Allow CI or developers without LLVM to skip the check and get a
    // compile-time error later (from llvm-sys) rather than a confusing
    // build-script error.
    if std::env::var("PIPERINE_OPENVAF_SKIP_LLVM_CHECK").is_ok() {
        return;
    }

    // Locate llvm-config. Prefer the prefix-qualified binary so the version
    // check is precise.
    let llvm_config = if let Ok(prefix) = std::env::var("LLVM_SYS_181_PREFIX") {
        format!("{prefix}/bin/llvm-config")
    } else {
        // Try common version-suffixed names first, then plain llvm-config.
        ["llvm-config-18", "llvm-config"]
            .iter()
            .find(|name| which_exists(name))
            .map(|s| s.to_string())
            .unwrap_or_else(|| "llvm-config".to_string())
    };

    let output = std::process::Command::new(&llvm_config)
        .arg("--version")
        .output();

    match output {
        Err(e) => {
            eprintln!(
                "\n\
                 piperine-openvaf: LLVM 18 not found.\n\
                 \n\
                 openvaf requires LLVM 18.1.x. Install it, then either:\n\
                 \n\
                 Option A — set LLVM_SYS_181_PREFIX:\n\
                 \n\
                   export LLVM_SYS_181_PREFIX=$(llvm-config-18 --prefix)\n\
                   cargo build\n\
                 \n\
                 Option B — install via apt:\n\
                 \n\
                   sudo apt install clang-18 llvm-18-dev lld-18\n\
                   export LLVM_SYS_181_PREFIX=$(llvm-config-18 --prefix)\n\
                   cargo build\n\
                 \n\
                 Error: {e}\n"
            );
            std::process::exit(1);
        }
        Ok(out) => {
            let version = String::from_utf8_lossy(&out.stdout);
            let version = version.trim();
            if !version.starts_with("18.") {
                eprintln!(
                    "\n\
                     piperine-openvaf: wrong LLVM version.\n\
                     \n\
                     Found `{llvm_config}` version {version}.\n\
                     openvaf requires LLVM 18.1.x.\n\
                     \n\
                     Set LLVM_SYS_181_PREFIX to the LLVM 18 prefix:\n\
                     \n\
                       export LLVM_SYS_181_PREFIX=$(llvm-config-18 --prefix)\n\
                       cargo build\n"
                );
                std::process::exit(1);
            }
        }
    }
}

fn which_exists(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
