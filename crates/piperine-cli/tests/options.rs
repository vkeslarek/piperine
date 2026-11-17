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

/// A copy of `examples/rc` in a fresh directory, so a test may write beside its manifest.
fn rc_copy() -> tempfile::TempDir {
    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).expect("mkdir");
        for entry in std::fs::read_dir(from).expect("read_dir") {
            let entry = entry.expect("entry");
            let target = to.join(entry.file_name());
            if entry.path().is_dir() { copy(&entry.path(), &target) } else { std::fs::copy(entry.path(), &target).expect("copy"); }
        }
    }
    let dir = tempfile::tempdir().expect("tmp");
    copy(&rc(), dir.path());
    dir
}

#[test]
fn csv_writes_the_table_and_prints_wrote() {
    // CLI-11 AC 1: `step` has `print_step = 1e-6` and `stop = 5e-6`, so 6 rows (CLI-11 AC 4);
    // the file is a header row and those rows, cells separated by `,` in the text table's number
    // format, and standard output prints `wrote <file> (<n> rows)` in place of the table
    let project = rc_copy();
    let table = stdout_of(&piperine_in(project.path(), &["run", "step"]));
    let blank = table.lines().position(str::is_empty).expect("an empty line");
    let printed: Vec<Vec<&str>> = table.lines().skip(blank + 1).map(cells).collect();
    let out = stdout_of(&piperine_in(project.path(), &["run", "step", "--csv", "out.csv"]));
    let tail: Vec<&str> = out.lines().skip_while(|l| !l.is_empty()).collect();
    assert_eq!(tail, ["", "wrote out.csv (6 rows)"], "{out}");
    let file = std::fs::read_to_string(project.path().join("out.csv")).expect("out.csv");
    let lines: Vec<Vec<&str>> = file.lines().map(|l| l.split(',').collect()).collect();
    assert_eq!(lines[0], ["time", "Step.vin", "Step.vout"]);
    assert_eq!(lines.len(), 7, "{file}");
    assert_eq!(lines, printed, "the file holds the cells the text table prints");
}

#[test]
fn an_existing_csv_is_replaced() {
    // CLI-11 AC 2
    let project = rc_copy();
    std::fs::write(project.path().join("out.csv"), "old,content\nand,more\nand,more\nand,more\nand,more\nand,more\nand,more\nand,more\nand,more\n").expect("write");
    stdout_of(&piperine_in(project.path(), &["run", "step", "--csv", "out.csv"]));
    let file = std::fs::read_to_string(project.path().join("out.csv")).expect("out.csv");
    assert!(!file.contains("old") && !file.contains("and,more"), "{file}");
    assert_eq!(file.lines().count(), 7, "{file}");
}

#[test]
fn an_unwritable_csv_names_the_path_and_exits_1() {
    // CLI-11 AC 3: the directory does not exist
    let project = rc_copy();
    let out = piperine_in(project.path(), &["run", "step", "--csv", "no/such/dir/out.csv"]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains("no/such/dir/out.csv"), "{}", text(&out.stderr));
    assert!(!text(&out.stdout).contains("wrote"), "{}", text(&out.stdout));
}

#[test]
fn a_relative_output_path_is_under_the_project() {
    // CLI-11 AC 6: from `benches/` of the project, `out.csv` lands in the project directory,
    // not in `benches/`
    let project = rc_copy();
    stdout_of(&piperine_in(&project.path().join("benches"), &["run", "step", "--csv", "out.csv"]));
    assert!(project.path().join("out.csv").is_file());
    assert!(!project.path().join("benches/out.csv").exists());
    // a lone file is under the working directory
    let divider = twin("divider.msl");
    let work = tempfile::tempdir().expect("tmp");
    stdout_of(&piperine_in(work.path(), &["run", divider.to_str().expect("path"), "--analysis", "op", "--csv", "op.csv"]));
    assert!(work.path().join("op.csv").is_file());
}
