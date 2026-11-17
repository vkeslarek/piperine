//! The output options of `piperine run` (feature `msl-cli`, phase 4; CLI-10, CLI-11, CLI-12 and
//! CLI-22): `--param`, `--csv`, `--stream`, `--records` and `--plot`, each run through the real
//! binary on a project or a lone file on disk and held to the spec's formats.
//!
//! Every child process gets `PIPERINE_CACHE_DIR` set to a temporary directory shared by this
//! target (a design compiles once). No test sets a variable of its own process.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

/// The binary cargo built for this target.
fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_piperine"))
}

/// `examples/rc`.
fn rc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/rc")
}

/// A parity twin of `tests/ngspice/`.
fn twin(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/ngspice").join(name)
}

/// One cache directory for every child of this target.
fn shared_cache() -> &'static Path {
    static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
    CACHE.get_or_init(|| tempfile::tempdir().expect("tmp")).path()
}

/// `piperine <args>` in `dir`.
fn piperine_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(binary()).args(args).current_dir(dir).env("PIPERINE_CACHE_DIR", shared_cache()).env_remove("PIPERINE_BACKEND").output().unwrap_or_else(|e| panic!("cannot run {}: {e}", binary().display()))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The standard output of a command that must have exited 0.
fn stdout_of(out: &Output) -> String {
    assert_eq!(out.status.code(), Some(0), "stderr:\n{}", text(&out.stderr));
    text(&out.stdout)
}

/// The cells of a line, split at runs of spaces.
fn cells(line: &str) -> Vec<&str> {
    line.split_whitespace().collect()
}

#[test]
fn the_divider_with_r2_at_1k_gives_5() {
    // CLI-10 AC 4: divider.msl is 10 V over r1 = 1k and r2 = 3k; `--param r2.r=1000` makes it
    // 10 * 1000 / (1000 + 1000) = 5 for `Divider.mid`, and `5.000000e0` in the table
    let divider = twin("divider.msl");
    let divider = divider.to_str().expect("utf-8 path");
    let out = stdout_of(&piperine_in(&rc(), &["run", divider, "--analysis", "op", "--print", "mid", "--param", "r2.r=1000"]));
    let row = out.lines().find(|l| l.starts_with("Divider.mid")).unwrap_or_else(|| panic!("no mid row in\n{out}"));
    assert_eq!(cells(row), ["Divider.mid", "5.000000e0"], "{out}");
}
