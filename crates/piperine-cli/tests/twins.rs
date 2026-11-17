//! The seven parity twins of `tests/ngspice/` from the command line (feature `msl-cli`, T29;
//! CLI-13): each runs through the real binary with flags on the interpreter and on the Rust
//! backend, with the analysis and the grid its `.cir` names (the grids of
//! `piperine-parity/tests/parity_{op,dc,ac,tran}.rs`), and the two backends' standard output is
//! compared byte for byte after the header line. A difference is a failure, never tolerated:
//! no number is compared here, so no tolerance is involved.
//!
//! Every child process gets `PIPERINE_CACHE_DIR` set to a temporary directory shared by this
//! target. No test sets a variable of its own process.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// One twin: its file, and the flags its `.cir` asks for.
struct Twin {
    name: &'static str,
    flags: &'static [&'static str],
}

/// The three operating points, the two DC sweeps (`diode_iv`: -1 V to 0.8 V, 37 rows;
/// `diode_rev`: -1 V to -20 V, 20 rows), the AC sweep (1 kHz to 10 MHz, 41 rows) and the
/// transient (10 us, ngspice's print step of 0.1 us, a step cap of a tenth of it).
const TWINS: [Twin; 7] = [
    Twin { name: "divider", flags: &["--analysis", "op"] },
    Twin { name: "isrc_r", flags: &["--analysis", "op"] },
    Twin { name: "diode_series", flags: &["--analysis", "op"] },
    Twin { name: "diode_iv", flags: &["--analysis", "dc", "--sweep", "v1.dc", "--from", "-1", "--to", "0.8", "--step", "0.05", "--print", "i(v1)"] },
    Twin { name: "diode_rev", flags: &["--analysis", "dc", "--sweep", "v1.dc", "--from", "-1", "--to", "-20", "--step", "-1", "--print", "i(v1)"] },
    Twin { name: "ac_rc", flags: &["--analysis", "ac", "--from", "1e3", "--to", "10e6", "--per-decade", "10", "--print", "out"] },
    Twin { name: "tran_rc", flags: &["--analysis", "transient", "--stop", "10e-6", "--max-step", "1e-8", "--print-step", "1e-7", "--print", "out"] },
];

const BACKENDS: [&str; 2] = ["interp", "rust"];

/// What one command printed.
struct Ran {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn ngspice_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/ngspice")
}

/// `piperine run <twin>.msl <flags> --backend <backend>`.
fn run(twin: &Twin, backend: &str) -> Ran {
    static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
    let cache = CACHE.get_or_init(|| tempfile::tempdir().expect("tmp"));
    let file = ngspice_dir().join(format!("{}.msl", twin.name));
    let out = Command::new(env!("CARGO_BIN_EXE_piperine"))
        .arg("run")
        .arg(&file)
        .args(twin.flags)
        .args(["--backend", backend])
        .env("PIPERINE_CACHE_DIR", cache.path())
        .env_remove("PIPERINE_BACKEND")
        .output()
        .unwrap_or_else(|e| panic!("cannot run piperine: {e}"));
    Ran { code: out.status.code(), stdout: String::from_utf8_lossy(&out.stdout).into_owned(), stderr: String::from_utf8_lossy(&out.stderr).into_owned() }
}

/// The fourteen commands, run once for the whole target.
fn all() -> &'static Vec<(&'static str, &'static str, Ran)> {
    static ALL: OnceLock<Vec<(&'static str, &'static str, Ran)>> = OnceLock::new();
    ALL.get_or_init(|| TWINS.iter().flat_map(|twin| BACKENDS.iter().map(move |backend| (twin.name, *backend, run(twin, backend)))).collect())
}

/// The runs of `names`, on both backends, each exited 0 and printed its header and a table.
fn exit_0(names: &[&str]) {
    for (twin, backend, ran) in all().iter().filter(|(t, ..)| names.contains(t)) {
        assert_eq!(ran.code, Some(0), "{twin} on {backend}: stderr:\n{}", ran.stderr);
        let header = ran.stdout.lines().next().unwrap_or_default();
        assert!(header.starts_with(&format!("run {twin}: ")) && header.ends_with(&format!("backend {backend}")), "{twin} on {backend}: header `{header}`");
        assert!(ran.stdout.lines().count() > 3, "{twin} on {backend}: no table:\n{}", ran.stdout);
    }
}

#[test]
fn the_three_op_twins_run_on_both_backends() {
    // CLI-13 AC 1
    exit_0(&["divider", "isrc_r", "diode_series"]);
}

#[test]
fn the_sweep_and_transient_twins_run_on_both_backends() {
    // CLI-13 AC 2
    exit_0(&["diode_iv", "diode_rev", "ac_rc", "tran_rc"]);
}

#[test]
fn the_two_backends_print_the_same_bytes() {
    // CLI-13 AC 3: for each of the seven twins the standard output on the interpreter and on the
    // Rust backend is the same bytes, the header line (it names the backend) aside
    let after_header = |ran: &Ran| ran.stdout.split_once('\n').map(|(_, rest)| rest.to_string()).unwrap_or_default();
    for twin in &TWINS {
        let on = |backend: &str| all().iter().find(|(t, b, _)| *t == twin.name && *b == backend).map(|(.., ran)| after_header(ran)).expect("both backends ran");
        let (interp, rust) = (on("interp"), on("rust"));
        assert!(!interp.is_empty(), "{}: nothing after the header", twin.name);
        assert_eq!(interp, rust, "{}: the backends print different bytes", twin.name);
    }
}

#[test]
fn tran_rc_prints_the_bytes_it_printed_before_the_long_run() {
    // LR-14 AC 1: `tran_rc`'s standard output after the header line, on each backend, is the file
    // captured from the binary before the long run changed any code
    let golden = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/tran_rc.txt")).expect("the golden file");
    for backend in BACKENDS {
        let (.., ran) = all().iter().find(|(t, b, _)| *t == "tran_rc" && *b == backend).expect("tran_rc ran on both backends");
        let after_header = ran.stdout.split_once('\n').map(|(_, rest)| rest).unwrap_or_default();
        assert_eq!(after_header, golden, "tran_rc on {backend} prints other bytes than the golden");
    }
}

#[test]
fn every_twin_prints_the_bytes_it_printed_before_the_long_run() {
    // LR-14 AC 1: each of the seven twins' standard output after the header line, on each
    // backend, is the file captured from the binary of `a5f23ea4` (the commit before the long
    // run), through the same flags
    for twin in &TWINS {
        let golden = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/goldens/{}.txt", twin.name))).expect("the golden file");
        for backend in BACKENDS {
            let (.., ran) = all().iter().find(|(t, b, _)| *t == twin.name && *b == backend).expect("every twin ran on both backends");
            let after_header = ran.stdout.split_once('\n').map(|(_, rest)| rest).unwrap_or_default();
            assert_eq!(after_header, golden, "{} on {backend} prints other bytes than the golden", twin.name);
        }
    }
}
