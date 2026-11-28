//! The sixteen parity twins of `tests/e2e/twins/` from the command line (feature `msl-cli`, T29;
//! CLI-13; feature `root-e2e`, T2): each runs through the real binary with flags on the
//! interpreter and on the Rust backend (the Rust backend only for the four twins that read
//! `now()`, which the interpreter refuses), with the analysis and the grid its `.cir` names (the
//! grids of `piperine-parity/tests/parity_*.rs`), and the two backends' standard output is
//! compared byte for byte after the header line. A difference is a failure, never tolerated:
//! no number is compared here, so no tolerance is involved.
//!
//! Every child process gets `PIPERINE_CACHE_DIR` set to a temporary directory shared by this
//! target. No test sets a variable of its own process.

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use piperine_parity::corpus::Corpus;
use piperine_parity::compile_cache::CompileCache;

/// One twin through the binary: the arguments of its `<name>.command`, the scenario directory it
/// runs from and the backends that run it.
struct Twin {
    name: String,
    arguments: Vec<String>,
    directory: PathBuf,
    backends: &'static [&'static str],
}

impl Twin {
    /// Every twin of the corpus with the command its `.command` file holds, by name.
    fn all() -> &'static Vec<Twin> {
        static TWINS: OnceLock<Vec<Twin>> = OnceLock::new();
        TWINS.get_or_init(|| {
            Corpus::of_repository()
                .twins()
                .into_iter()
                .map(|twin| {
                    let backends = if COMPILED_ONLY.contains(&twin.name()) { &["rust"][..] } else { BOTH };
                    Twin { name: twin.name().to_string(), arguments: twin.command_arguments(), directory: twin.directory().to_path_buf(), backends }
                })
                .collect()
        })
    }
}

/// Both backends run the twin.
const BOTH: &[&str] = &["interp", "rust"];

/// The twins only the Rust backend runs: their source reads `now()` or `analysis(..)`, which the
/// interpreter (the M1 analog subset) refuses.
const COMPILED_ONLY: [&str; 10] = ["pss_rc", "tran_jump", "tran_corner", "tran_floor", "uic_rc", "diode_rs", "delay_tline", "noise_rc", "noise_switch", "noise_diode"];

/// The twins whose standard output was captured before the long run (`a5f23ea4`), each held to
/// `<name>.stdout` by `every_twin_prints_the_bytes_it_printed_before_the_long_run`.
const BEFORE_THE_LONG_RUN: [&str; 7] = ["divider", "isrc_r", "diode_series", "diode_iv", "diode_rev", "ac_rc", "tran_rc"];

/// What one command printed.
struct Ran {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// Associated test helpers of `twins`, grouped on one fixture type.
struct TwinsFixture;

impl TwinsFixture {
    /// `piperine <the twin's command arguments> --backend <backend>`, run from the twin's scenario directory.
    fn run(twin: &Twin, backend: &str) -> Ran {
        let cache = CompileCache::shared();
        let out = Command::new(env!("CARGO_BIN_EXE_piperine"))
            .args(&twin.arguments)
            .args(["--backend", backend])
            .current_dir(&twin.directory)
            .env("PIPERINE_CACHE_DIR", cache.path())
            .env_remove("PIPERINE_BACKEND")
            .output()
            .unwrap_or_else(|e| panic!("cannot run piperine: {e}"));
        Ran { code: out.status.code(), stdout: String::from_utf8_lossy(&out.stdout).into_owned(), stderr: String::from_utf8_lossy(&out.stderr).into_owned() }
    }

    /// Every twin on every backend that runs it, run once for the whole target.
    fn all() -> &'static Vec<(&'static str, &'static str, Ran)> {
        static ALL: OnceLock<Vec<(&'static str, &'static str, Ran)>> = OnceLock::new();
        ALL.get_or_init(|| Twin::all().iter().flat_map(|twin| twin.backends.iter().map(move |backend| (twin.name.as_str(), *backend, TwinsFixture::run(twin, backend)))).collect())
    }

    /// The runs of `names`, on both backends, each exited 0 and printed its header and a table.
    fn exit_0(names: &[&str]) {
        for (twin, backend, ran) in TwinsFixture::all().iter().filter(|(t, ..)| names.contains(t)) {
            assert_eq!(ran.code, Some(0), "{twin} on {backend}: stderr:\n{}", ran.stderr);
            let header = ran.stdout.lines().next().unwrap_or_default();
            assert!(header.starts_with(&format!("run {twin}: ")) && header.ends_with(&format!("backend {backend}")), "{twin} on {backend}: header `{header}`");
            assert!(ran.stdout.lines().count() > 3, "{twin} on {backend}: no table:\n{}", ran.stdout);
        }
    }
}

#[test]
fn the_three_op_twins_run_on_both_backends() {
    // CLI-13 AC 1
    TwinsFixture::exit_0(&["divider", "isrc_r", "diode_series"]);
}

#[test]
fn the_sweep_and_transient_twins_run_on_both_backends() {
    // CLI-13 AC 2
    TwinsFixture::exit_0(&["diode_iv", "diode_rev", "ac_rc", "tran_rc"]);
}

#[test]
fn the_two_backends_print_the_same_bytes() {
    // CLI-13 AC 3: for each twin both backends run, the standard output on the interpreter and on
    // the Rust backend is the same bytes, the header line (it names the backend) aside
    let after_header = |ran: &Ran| ran.stdout.split_once('\n').map(|(_, rest)| rest.to_string()).unwrap_or_default();
    for twin in Twin::all().iter().filter(|twin| twin.backends == BOTH) {
        let on = |backend: &str| TwinsFixture::all().iter().find(|(t, b, _)| *t == twin.name && *b == backend).map(|(.., ran)| after_header(ran)).expect("both backends ran");
        let (interp, rust) = (on("interp"), on("rust"));
        assert!(!interp.is_empty(), "{}: nothing after the header", twin.name);
        assert_eq!(interp, rust, "{}: the backends print different bytes", twin.name);
    }
}

#[test]
fn tran_rc_prints_the_bytes_it_printed_before_the_long_run() {
    // LR-14 AC 1: `tran_rc`'s standard output after the header line, on each backend, is the file
    // captured from the binary before the long run changed any code
    let golden = std::fs::read_to_string(Corpus::of_repository().twin("tran_rc").expected_stdout()).expect("the golden file");
    for backend in BOTH.iter().copied() {
        let (.., ran) = TwinsFixture::all().iter().find(|(t, b, _)| *t == "tran_rc" && *b == backend).expect("tran_rc ran on both backends");
        let after_header = ran.stdout.split_once('\n').map(|(_, rest)| rest).unwrap_or_default();
        assert_eq!(after_header, golden, "tran_rc on {backend} prints other bytes than the golden");
    }
}

#[test]
fn every_twin_prints_the_bytes_it_printed_before_the_long_run() {
    // LR-14 AC 1: each of the seven twins' standard output after the header line, on each
    // backend, is the file captured from the binary of `a5f23ea4` (the commit before the long
    // run), through the same flags
    for twin in Twin::all().iter().filter(|twin| BEFORE_THE_LONG_RUN.contains(&twin.name.as_str())) {
        let golden = std::fs::read_to_string(Corpus::of_repository().twin(&twin.name).expected_stdout()).expect("the golden file");
        for backend in BOTH.iter().copied() {
            let (.., ran) = TwinsFixture::all().iter().find(|(t, b, _)| *t == twin.name && *b == backend).expect("every twin ran on both backends");
            let after_header = ran.stdout.split_once('\n').map(|(_, rest)| rest).unwrap_or_default();
            assert_eq!(after_header, golden, "{} on {backend} prints other bytes than the golden", twin.name);
        }
    }
}

#[test]
fn every_twin_prints_the_bytes_it_printed_before_the_move() {
    // RE-04 AC 1 (feature `root-e2e`, T2): each of the sixteen twins' standard output after the
    // header line, on every backend that runs it, is the file captured from the binary at the
    // commit before `tests/e2e/twins/` moved, through the same flags (`tran_floor` fails `E0508`
    // by design, so its golden is empty)
    for twin in Twin::all() {
        let golden = std::fs::read_to_string(Corpus::of_repository().twin(&twin.name).expected_stdout()).expect("the golden file");
        for backend in twin.backends {
            let (.., ran) = TwinsFixture::all().iter().find(|(t, b, _)| *t == twin.name && b == backend).expect("every twin ran on each of its backends");
            let after_header = ran.stdout.split_once('\n').map(|(_, rest)| rest).unwrap_or_default();
            assert_eq!(after_header, golden, "{} on {backend} prints other bytes than the golden", twin.name);
        }
    }
}
