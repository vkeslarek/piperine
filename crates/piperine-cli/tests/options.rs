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

#[test]
fn the_stream_flag_keeps_only_that_stream() {
    // CLI-12 AC 1: `step` emits `trip` (the monitor's `emit!`) and `checks` (its `require`);
    // `--stream trip` prints the `trip` record and no other; the full path of the stream keeps
    // the same line
    let all = stdout_of(&piperine_in(&rc(), &["run", "step", "--all-checks"]));
    assert!(all.lines().any(|l| l.starts_with('@') && l.contains(" checks ")), "the unfiltered run prints a checks record:\n{all}");
    let out = stdout_of(&piperine_in(&rc(), &["run", "step", "--stream", "trip"]));
    let records: Vec<&str> = out.lines().filter(|l| l.starts_with('@')).collect();
    assert_eq!(records.len(), 1, "{out}");
    assert!(records[0].contains(" Step.mon trip "), "{}", records[0]);
    let by_path = stdout_of(&piperine_in(&rc(), &["run", "step", "--stream", "Step.mon.trip"]));
    assert_eq!(by_path, out);
    // CLI-12 AC 2: an entry that matches nothing is E0801, exit 1, and nothing is printed
    let bad = piperine_in(&rc(), &["run", "step", "--stream", "nothing"]);
    assert_eq!(bad.status.code(), Some(1));
    assert!(text(&bad.stderr).contains("E0801") && text(&bad.stderr).contains("nothing"), "{}", text(&bad.stderr));
    assert_eq!(text(&bad.stdout), "");
}

#[test]
fn records_go_to_the_file_as_json_lines_and_not_to_stdout() {
    // CLI-12 AC 3 and 5: every kept record is one JSON object per line with the keys time,
    // instance, stream and fields, and no record line reaches standard output. `step` writes
    // `checks` (kind, name, pass) and `trip` (v = 0.5) at the half-way crossing
    let project = rc_copy();
    let out = stdout_of(&piperine_in(project.path(), &["run", "step", "--records", "rec.jsonl"]));
    assert!(!out.lines().any(|l| l.starts_with('@')), "{out}");
    let file = std::fs::read_to_string(project.path().join("rec.jsonl")).expect("rec.jsonl");
    let lines: Vec<&str> = file.lines().collect();
    assert_eq!(lines.len(), 2, "{file}");
    let trip = lines.iter().find(|l| l.contains("\"stream\":\"trip\"")).unwrap_or_else(|| panic!("no trip in {file}"));
    assert!(trip.starts_with("{\"time\":") && trip.contains("\"instance\":\"Step.mon\"") && trip.contains("\"fields\":{\"v\":0.5"), "{trip}");
    // `v` is the crossing's value, 0.5 to the solver's step: a full-precision JSON number
    let v: f64 = trip.split("\"v\":").nth(1).and_then(|t| t.strip_suffix("}}")).expect("v").parse().expect("a number");
    assert!((v - 0.5).abs() <= 1e-3, "{v}");
    // the time reads back as the f64 the solver delivered: 6.931075e-7 to the table's digits
    let time: f64 = trip["{\"time\":".len()..].split(',').next().expect("time").parse().expect("a number");
    assert!((time - 6.931075e-7).abs() <= 1e-12, "{time}");
    let check = lines.iter().find(|l| l.contains("\"stream\":\"checks\"")).unwrap_or_else(|| panic!("no checks in {file}"));
    assert!(check.ends_with("\"fields\":{\"kind\":\"require\",\"name\":\"crossed_upward\",\"pass\":true}}"), "{check}");
}

#[test]
fn an_existing_records_file_is_replaced() {
    // CLI-12 AC 4
    let project = rc_copy();
    std::fs::write(project.path().join("rec.jsonl"), "old\nold\nold\nold\n").expect("write");
    stdout_of(&piperine_in(project.path(), &["run", "step", "--records", "rec.jsonl"]));
    let file = std::fs::read_to_string(project.path().join("rec.jsonl")).expect("rec.jsonl");
    assert!(!file.contains("old"), "{file}");
    assert_eq!(file.lines().count(), 2, "{file}");
}

#[test]
fn csv_and_records_leave_the_header_and_the_wrote_line() {
    // spec edge case: `--csv` with `--records` prints the header line and the `wrote` line only
    // (the empty line that always separates the two is no content)
    let project = rc_copy();
    let out = stdout_of(&piperine_in(project.path(), &["run", "step", "--csv", "o.csv", "--records", "r.jsonl"]));
    let lines: Vec<&str> = out.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines, ["run step: transient, top Step, backend rust", "wrote o.csv (6 rows)"], "{out}");
    assert!(project.path().join("o.csv").is_file() && project.path().join("r.jsonl").is_file());
}

/// A lone bench whose monitor passes one check (`ok`) and fails one (`bad`) at 1 us.
fn two_checks() -> (tempfile::TempDir, PathBuf) {
    let text = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\npub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\n\nmodule M() {}\ndigital for M { on at(1u) { require(\"ok\", true); require(\"bad\", false); emit!(\"note\", n = 1); } }\n\nmodule Top() {\n    signal a: Electrical;\n    let v1 = Vsource(a, gnd) { dc: 1.0 };\n    let r1 = Resistor(a, gnd) { r: 1k };\n    let m = M();\n}\n";
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("checks.msl");
    std::fs::write(&file, text).expect("write");
    (dir, file)
}

#[test]
fn a_passing_check_is_hidden_a_failing_one_shown_and_all_checks_shows_both() {
    // CLI-06 AC 9 (user, 2026-11-16): by default the terminal hides `checks` records with
    // pass=true and shows pass=false, other streams stay; `--all-checks` shows both; the
    // records file keeps every record either way
    let (dir, file) = two_checks();
    let file = file.to_str().expect("path");
    let base = ["run", file, "--top", "Top", "--analysis", "transient", "--stop", "3e-6", "--max-step", "1e-7"];
    let records = |extra: &[&str]| -> Vec<String> {
        let args: Vec<&str> = base.iter().chain(extra).copied().collect();
        stdout_of(&piperine_in(dir.path(), &args)).lines().filter(|l| l.starts_with('@')).map(str::to_string).collect()
    };
    let default = records(&[]);
    assert_eq!(default.len(), 2, "{default:?}");
    assert!(default.iter().any(|l| l.contains("checks") && l.contains("pass=false") && l.contains("\"bad\"")), "{default:?}");
    assert!(default.iter().any(|l| l.contains(" note ")), "{default:?}");
    assert!(!default.iter().any(|l| l.contains("pass=true")), "{default:?}");
    let all = records(&["--all-checks"]);
    assert_eq!(all.len(), 3, "{all:?}");
    assert!(all.iter().any(|l| l.contains("pass=true") && l.contains("\"ok\"")), "{all:?}");
    stdout_of(&piperine_in(dir.path(), &[base.as_slice(), &["--records", "r.jsonl"]].concat()));
    let kept = std::fs::read_to_string(dir.path().join("r.jsonl")).expect("r.jsonl");
    assert_eq!(kept.lines().count(), 3, "the file keeps the passing check:\n{kept}");
    assert!(kept.contains("\"pass\":true") && kept.contains("\"pass\":false"), "{kept}");
}

/// The numbers of the path `d` attribute of the `series` path for `signal`, as (x, y) pairs.
fn series_points(svg: &str, signal: &str) -> Vec<(f64, f64)> {
    let line = svg.lines().find(|l| l.contains("class=\"series\"") && l.contains(&format!("data-signal=\"{signal}\""))).unwrap_or_else(|| panic!("no series for {signal}"));
    let d = line.split("d=\"").nth(1).and_then(|r| r.split('"').next()).expect("a d attribute");
    let numbers: Vec<f64> = d.split(|c: char| c == ' ' || c == 'M' || c == 'L').filter(|t| !t.is_empty()).map(|t| t.parse().expect("a number")).collect();
    numbers.chunks(2).map(|p| (p[0], p[1])).collect()
}

#[test]
fn plot_writes_an_svg_and_prints_wrote() {
    // CLI-22 AC 1 and 2: the file is one SVG with a series per printed signal (`Step.vin`,
    // `Step.vout`) and standard output ends with `wrote step.svg` after the table. The plot area
    // is x 70..780, y 20..430: `vout` starts at (70, 430) (0 V at t = 0, the axis range 0..1)
    // and ends at x = 780 (t = stop = 5 us) at 430 - 410 * 0.99326 (1 - exp(-5), the RC step)
    let project = rc_copy();
    let out = stdout_of(&piperine_in(project.path(), &["run", "step", "--plot", "step.svg"]));
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.last(), Some(&"wrote step.svg"), "{out}");
    assert!(lines.iter().any(|l| l.starts_with("time")), "the table is printed first:\n{out}");
    let svg = std::fs::read_to_string(project.path().join("step.svg")).expect("step.svg");
    assert!(svg.starts_with("<svg ") && svg.trim_end().ends_with("</svg>"));
    assert_eq!(svg.matches("<path class=\"series\"").count(), 2);
    let vout = series_points(&svg, "Step.vout");
    assert_eq!(vout.first(), Some(&(70.0, 430.0)));
    let last = vout.last().expect("a point");
    assert_eq!(last.0, 780.0);
    assert!((last.1 - (430.0 - 410.0 * (1.0 - (-5f64).exp()))).abs() <= 1.0, "{last:?}");
    let vin = series_points(&svg, "Step.vin");
    assert!(vin.iter().all(|p| p.1 == 20.0), "vin holds 1 V, the top of the 0..1 axis: {vin:?}");
}

#[test]
fn a_plot_draws_every_accepted_step_whatever_the_print_step() {
    // CLI-22 AC 7: `step` prints 6 rows (`print_step = 1e-6`) but the plot draws every accepted
    // step, many more points
    let project = rc_copy();
    let out = stdout_of(&piperine_in(project.path(), &["run", "step", "--plot", "step.svg"]));
    let rows = out.lines().skip_while(|l| !l.starts_with("time")).count() - 2;
    assert_eq!(rows, 6, "{out}");
    let svg = std::fs::read_to_string(project.path().join("step.svg")).expect("step.svg");
    let points = series_points(&svg, "Step.vout").len();
    assert!(points > 20, "{points} points for {rows} printed rows");
    // as many points as the run without `print_step` has rows: one per accepted step
    let manifest = project.path().join("Piperine.toml");
    let text = std::fs::read_to_string(&manifest).expect("the manifest");
    assert!(text.contains("print_step = 1e-6\n"), "{text}");
    std::fs::write(&manifest, text.replace("print_step = 1e-6\n", "")).expect("the manifest");
    let all = stdout_of(&piperine_in(project.path(), &["run", "step"]));
    let accepted = all.lines().skip_while(|l| !l.starts_with("time")).count() - 1;
    assert_eq!(points, accepted, "the plot draws every accepted step:\n{all}");
}

#[test]
fn an_unwritable_plot_names_the_path_and_exits_1() {
    // CLI-22 AC 11
    let project = rc_copy();
    let out = piperine_in(project.path(), &["run", "step", "--plot", "no/such/dir/step.svg"]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains("no/such/dir/step.svg"), "{}", text(&out.stderr));
    assert!(!text(&out.stdout).contains("wrote"), "{}", text(&out.stdout));
    // CLI-22 AC 8: an operating point has nothing to plot, P0010, exit 1
    let op = piperine_in(project.path(), &["run", "op", "--plot", "op.svg"]);
    assert_eq!(op.status.code(), Some(1));
    assert!(text(&op.stderr).contains("P0010") && text(&op.stderr).contains("nothing to plot"), "{}", text(&op.stderr));
    assert!(!project.path().join("op.svg").exists());
}
