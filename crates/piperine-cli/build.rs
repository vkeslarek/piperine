// Bundle the pre-built `_piperine.so` (extension-module cdylib) into the CLI
// binary so `piperine new` can install it into a project venv WITHOUT a
// workspace / `target/` on the end user's machine.
//
// Build ordering: the cdylib must be built BEFORE the CLI. CI / Makefile /
// the developer runs `cargo build -p piperine-python --features extension-module`
// first, then `cargo build --bin piperine`. This script finds the `.so`,
// copies it to `OUT_DIR`, and sets `--cfg bundled_python` so the CLI
// `include_bytes!`s it. If the `.so` isn't found yet, the CLI still builds
// (Python setup degrades to a helpful error at runtime).

use std::path::PathBuf;

fn main() {
    // Declare the cfg so the compiler doesn't warn about `#[cfg(bundled_python)]`.
    println!("cargo::rustc-check-cfg=cfg(bundled_python)");

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace = manifest.parent().unwrap().parent().unwrap();

    for profile in ["release", "debug"] {
        let so = workspace
            .join("target")
            .join(profile)
            .join("libpiperine_python.so");
        if so.exists() {
            let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("_piperine.so");
            std::fs::copy(&so, &out).unwrap();
            println!("cargo:rustc-cfg=bundled_python");
            println!("cargo:rerun-if-changed={}", so.display());
            return;
        }
    }

    // First build or the extension wasn't built yet — warn but don't fail.
    println!(
        "cargo:warning=piperine-python .so not found — build it first with:"
    );
    println!(
        "cargo:warning=  cargo build -p piperine-python --features extension-module"
    );
    println!(
        "cargo:warning=`piperine new` Python venv setup will be skipped until rebuilt."
    );
}
