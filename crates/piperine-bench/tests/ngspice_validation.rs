//! ngspice golden cross-validation (spice-stdlib SPICE-05..08).
//!
//! Each circuit pair in `tests/ngspice/` describes the *same* circuit twice:
//! `<name>.cir` for ngspice (the golden reference, run as a subprocess) and
//! `<name>.phdl` for piperine (elaborated and solved in-process — node
//! voltages are read from the result objects, never parsed from stdout).
//! Every node ngspice prints must match piperine within
//! `|Δ| ≤ abstol + reltol·max(|a|,|b|)`.
//!
//! When `ngspice` is not on PATH every golden test prints a SKIP notice and
//! passes — the binary cannot be a hard dependency. All other failure modes
//! are loud: unparseable ngspice output, zero shared nodes (contract
//! violation), piperine non-convergence, per-node mismatch.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

use piperine_bench::{OpResult, NetRef, SimSession, SolverConfig};
use piperine_lang::eval::Value;
use piperine_lang::SourceMap;

/// The piperine-vs-ngspice comparison harness. Owns detection of the ngspice
/// binary, both simulation paths and the tolerance contract.
struct NgspiceHarness {
    exe: PathBuf,
}

impl NgspiceHarness {
    /// ngspice's own defaults for DC node voltages (validation contract).
    const RELTOL: f64 = 1e-3;
    const ABSTOL_V: f64 = 1e-6;
    /// Current abstol for sweep comparisons (A).
    const ABSTOL_I: f64 = 1e-9;

    /// The harness for the `ngspice` binary on PATH, or `None` (skip).
    fn detect() -> Option<Self> {
        Self::detect_with_path(std::env::var_os("PATH"))
    }

    /// PATH-injectable detection seam so the skip branch is testable
    /// without mutating the process environment.
    fn detect_with_path(path: Option<OsString>) -> Option<Self> {
        let path = path?;
        std::env::split_paths(&path)
            .map(|dir| dir.join("ngspice"))
            .find(|candidate| candidate.is_file())
            .map(|exe| Self { exe })
    }

    /// `|a − b| ≤ abstol + reltol·max(|a|, |b|)` — the run.py contract.
    fn within_tolerance(a: f64, b: f64, abstol: f64) -> bool {
        (a - b).abs() <= abstol + Self::RELTOL * a.abs().max(b.abs())
    }

    fn circuits_dir() -> PathBuf {
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/ngspice"))
    }

    /// Source map rooted at the real stdlib headers (same shape
    /// `piperine-project` builds for a project).
    fn headers_source_map() -> SourceMap {
        let headers =
            PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
        let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
        map.add_namespace("piperine", headers.clone());
        map.add_namespace("spice", headers.join("spice"));
        map
    }

    /// Golden side: run `<circuit>.cir` via `ngspice -b` and parse the
    /// operating-point node voltages it prints.
    fn ngspice_op(&self, circuit: &str) -> Result<BTreeMap<String, f64>, String> {
        let cir = Self::circuits_dir().join(format!("{circuit}.cir"));
        let output = std::process::Command::new(&self.exe)
            .arg("-b")
            .arg(&cir)
            .output()
            .map_err(|e| format!("{circuit}: failed to run ngspice: {e}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Self::parse_op_output(circuit, &stdout)
    }

    /// Parse `v(node) = <value>` lines. Zero parsed nodes is a loud failure
    /// (locale/version drift must never silently compare an empty set).
    fn parse_op_output(circuit: &str, out: &str) -> Result<BTreeMap<String, f64>, String> {
        let mut vals = BTreeMap::new();
        for line in out.lines() {
            let line = line.trim();
            let Some(rest) = line.strip_prefix("v(") else { continue };
            let Some((node, rest)) = rest.split_once(')') else { continue };
            let Some((_, value)) = rest.split_once('=') else { continue };
            let value: f64 = value
                .trim()
                .parse()
                .map_err(|e| format!("{circuit}: unparseable ngspice value in `{line}`: {e}"))?;
            vals.insert(node.trim().to_lowercase(), value);
        }
        if vals.is_empty() {
            let excerpt: String = out.chars().take(600).collect();
            return Err(format!(
                "{circuit}: no `v(node) = …` lines in ngspice output — raw excerpt:\n{excerpt}"
            ));
        }
        Ok(vals)
    }

    /// Piperine side: elaborate `<circuit>.phdl` and solve the DC operating
    /// point in-process, returning the result object.
    fn piperine_op(&self, circuit: &str) -> Result<OpResult, String> {
        let phdl = Self::circuits_dir().join(format!("{circuit}.phdl"));
        let src = std::fs::read_to_string(&phdl)
            .map_err(|e| format!("{circuit}: {}: {e}", phdl.display()))?;
        let design = piperine_lang::parse_and_elaborate(&src, &Self::headers_source_map())
            .map_err(|e| format!("{circuit}: elaboration failed: {e:?}"))?;
        let session = SimSession::new(design, "Top".to_string());
        session
            .run_op(&SolverConfig::default(), &Value::Unit)
            .map_err(|e| format!("{circuit}: piperine DC solve failed: {e}"))
    }

    /// Compare every ngspice-reported node against piperine. `0`/ground is
    /// dropped; zero *shared* nodes is a contract violation.
    fn compare_op(
        circuit: &str,
        golden: &BTreeMap<String, f64>,
        piperine: impl Fn(&str) -> Option<f64>,
    ) -> Result<(), String> {
        let mut shared = 0usize;
        let mut mismatches = Vec::new();
        for (node, ng) in golden {
            if node == "0" || piperine_lang::pom::is_ground(node) {
                continue;
            }
            let Some(pp) = piperine(node) else { continue };
            shared += 1;
            if !Self::within_tolerance(*ng, pp, Self::ABSTOL_V) {
                mismatches.push(format!(
                    "    v({node}): ngspice={ng:.6e}  piperine={pp:.6e}  Δ={:.3e}",
                    (ng - pp).abs()
                ));
            }
        }
        if shared == 0 {
            return Err(format!(
                "{circuit}: 0 shared nodes between ngspice ({:?}) and piperine — contract violation",
                golden.keys().collect::<Vec<_>>()
            ));
        }
        if !mismatches.is_empty() {
            return Err(format!("{circuit}: {} node(s) out of tolerance:\n{}", mismatches.len(), mismatches.join("\n")));
        }
        Ok(())
    }

    /// One full OP golden case; panics with the loud failure text.
    fn op_case(&self, circuit: &str) {
        let golden = self.ngspice_op(circuit).unwrap_or_else(|e| panic!("{e}"));
        let op = self.piperine_op(circuit).unwrap_or_else(|e| panic!("{e}"));
        Self::compare_op(circuit, &golden, |node| {
            op.v(&NetRef { name: node.to_string() }, None).ok()
        })
        .unwrap_or_else(|e| panic!("{e}"));
        eprintln!("PASS {circuit} ({} golden nodes)", golden.len());
    }

    // ── DC sweeps via `wrdata` (SPICE-08) ───────────────────────────────────

    /// Golden side of a sweep: run the `.cir` (whose `.control` block does
    /// `dc … + wrdata <circuit>_sweep …`) in a scratch directory and parse
    /// the exported ASCII columns.
    fn ngspice_sweep(&self, circuit: &str) -> Result<Vec<(f64, f64)>, String> {
        let cir = Self::circuits_dir().join(format!("{circuit}.cir"));
        let scratch = std::env::temp_dir().join(format!(
            "piperine-ngspice-{circuit}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&scratch)
            .map_err(|e| format!("{circuit}: scratch dir: {e}"))?;
        let run = std::process::Command::new(&self.exe)
            .arg("-b")
            .arg(&cir)
            .current_dir(&scratch)
            .output()
            .map_err(|e| format!("{circuit}: failed to run ngspice: {e}"));
        let wrdata = scratch.join(format!("{circuit}_sweep"));
        let content = run.and_then(|_| {
            std::fs::read_to_string(&wrdata)
                .map_err(|e| format!("{circuit}: ngspice wrote no wrdata file {}: {e}", wrdata.display()))
        });
        let _ = std::fs::remove_dir_all(&scratch);
        Self::parse_wrdata(circuit, &content?)
    }

    /// Strict `wrdata` parser: every non-empty line must be exactly
    /// `sweep_value  value` (two floats); anything else fails loud.
    fn parse_wrdata(circuit: &str, content: &str) -> Result<Vec<(f64, f64)>, String> {
        let mut points = Vec::new();
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let cols: Vec<&str> = line.split_whitespace().collect();
            let [x, y] = cols.as_slice() else {
                return Err(format!(
                    "{circuit}: malformed wrdata line (expected 2 columns): `{line}`"
                ));
            };
            let parse = |s: &str| {
                s.parse::<f64>()
                    .map_err(|e| format!("{circuit}: unparseable wrdata number `{s}`: {e}"))
            };
            points.push((parse(x)?, parse(y)?));
        }
        if points.is_empty() {
            return Err(format!("{circuit}: empty wrdata export — contract violation"));
        }
        Ok(points)
    }

    /// One full sweep golden case: ngspice `.dc`+`wrdata` vs a piperine
    /// bench-loop (stage `source`.dc per point, DC solve, read the current
    /// through the `(branch_a, branch_b)` two-terminal instance — the swept
    /// source's force branch, matching ngspice's `i(v1)` sign convention).
    fn sweep_case(&self, circuit: &str, source: &str, branch_a: &str, branch_b: &str, abstol: f64) {
        let golden = self.ngspice_sweep(circuit).unwrap_or_else(|e| panic!("{e}"));
        assert!(
            golden.len() >= 20,
            "{circuit}: sweep needs ≥20 points, got {}",
            golden.len()
        );

        let phdl = Self::circuits_dir().join(format!("{circuit}.phdl"));
        let src = std::fs::read_to_string(&phdl)
            .unwrap_or_else(|e| panic!("{circuit}: {}: {e}", phdl.display()));
        let design = piperine_lang::parse_and_elaborate(&src, &Self::headers_source_map())
            .unwrap_or_else(|e| panic!("{circuit}: elaboration failed: {e:?}"));
        let session = SimSession::new(design, "Top".to_string());

        let mut mismatches = Vec::new();
        for (x, i_golden) in &golden {
            session.stage(source, "dc", Value::Real(*x));
            let op = session
                .run_op(&SolverConfig::default(), &Value::Unit)
                .unwrap_or_else(|e| panic!("{circuit}: piperine DC solve failed at {source}={x}: {e}"));
            let i_piperine = op
                .i(&NetRef { name: branch_a.to_string() }, Some(&NetRef { name: branch_b.to_string() }))
                .unwrap_or_else(|e| panic!("{circuit}: current readback failed at {source}={x}: {e:?}"));
            if !Self::within_tolerance(*i_golden, i_piperine, abstol) {
                mismatches.push(format!(
                    "    {source}={x:+.4e}: i ngspice={i_golden:+.6e}  piperine={i_piperine:+.6e}  Δ={:.3e}",
                    (i_golden - i_piperine).abs()
                ));
            }
        }
        assert!(
            mismatches.is_empty(),
            "{circuit}: {}/{} sweep point(s) out of tolerance:\n{}",
            mismatches.len(),
            golden.len(),
            mismatches.join("\n")
        );
        eprintln!("PASS {circuit} ({} sweep points)", golden.len());
    }
}

/// Run one OP circuit against live ngspice, or skip (and pass) without it.
fn ngspice_op_case(circuit: &str) {
    match NgspiceHarness::detect() {
        Some(harness) => harness.op_case(circuit),
        None => eprintln!("SKIP {circuit}: ngspice not on PATH"),
    }
}

// ── Golden OP circuits (SPICE-05) ───────────────────────────────────────────

#[test]
fn ngspice_divider() {
    ngspice_op_case("divider");
}

#[test]
fn ngspice_rdiode() {
    ngspice_op_case("rdiode");
}

#[test]
fn ngspice_diode_series() {
    ngspice_op_case("diode_series");
}

#[test]
fn ngspice_nmos_fixed() {
    ngspice_op_case("nmos_fixed");
}

#[test]
fn ngspice_nmos_load() {
    ngspice_op_case("nmos_load");
}

#[test]
fn ngspice_jfet_bias() {
    ngspice_op_case("jfet_bias");
}

#[test]
#[ignore = "fixed in T7-T10 (BJT saturation needs source stepping — SPICE-12)"]
fn ngspice_bjt_ce() {
    ngspice_op_case("bjt_ce");
}

#[test]
#[ignore = "fixed in T7-T10 (BJT mirror DC convergence — SPICE-13)"]
fn ngspice_bjt_mirror() {
    ngspice_op_case("bjt_mirror");
}

// ── DC sweep circuits (SPICE-08) ────────────────────────────────────────────

/// Diode I–V (forward + reverse, 37 points): ngspice `.dc` + `wrdata` export
/// vs piperine staging `v1.dc` per point — the source branch current must
/// match within reltol 1e-3 + abstol 1e-9 A.
#[test]
fn ngspice_diode_iv_sweep() {
    match NgspiceHarness::detect() {
        Some(harness) => {
            harness.sweep_case("diode_iv", "v1", "vin", "gnd", NgspiceHarness::ABSTOL_I)
        }
        None => eprintln!("SKIP diode_iv: ngspice not on PATH"),
    }
}

/// NMOS Id–Vgs (21 points, vds = 2 V, bulk at −1 V): cutoff → saturation →
/// linear with body effect — the harness stages `vg.dc` per point and reads
/// the drain supply's branch current (SPICE-10).
#[test]
fn ngspice_nmos_id_vgs_sweep() {
    match NgspiceHarness::detect() {
        Some(harness) => {
            harness.sweep_case("nmos_id_vgs", "vg", "d", "gnd", NgspiceHarness::ABSTOL_I)
        }
        None => eprintln!("SKIP nmos_id_vgs: ngspice not on PATH"),
    }
}

/// NMOS Id–Vds (26 points, vgs = 3 V, rd/rs = 100 Ω): linear → saturation,
/// exercising the series-resistance force branches (SPICE-10).
#[test]
fn ngspice_nmos_id_vds_sweep() {
    match NgspiceHarness::detect() {
        Some(harness) => {
            harness.sweep_case("nmos_id_vds", "vd", "d", "gnd", NgspiceHarness::ABSTOL_I)
        }
        None => eprintln!("SKIP nmos_id_vds: ngspice not on PATH"),
    }
}

/// N-JFET Id–Vds (26 points, vgs = −0.5 V, rd/rs = 100 Ω): linear →
/// saturation through the series-resistance force branches (SPICE-11).
#[test]
fn ngspice_jfet_id_vds_sweep() {
    match NgspiceHarness::detect() {
        Some(harness) => {
            harness.sweep_case("jfet_id_vds", "vd", "d", "gnd", NgspiceHarness::ABSTOL_I)
        }
        None => eprintln!("SKIP jfet_id_vds: ngspice not on PATH"),
    }
}

// ── Harness failure modes (SPICE-06/SPICE-07) ───────────────────────────────

/// SPICE-06: without a binary on the (injected) PATH, detection yields the
/// skip branch — the case runner prints a notice and passes.
#[test]
fn ngspice_absent_takes_the_skip_branch() {
    let empty = std::env::temp_dir();
    let path = Some(std::env::join_paths([&empty]).unwrap());
    assert!(
        NgspiceHarness::detect_with_path(path).is_none(),
        "no ngspice in an empty PATH must select the skip branch"
    );
    assert!(NgspiceHarness::detect_with_path(None).is_none(), "unset PATH must skip");
}

/// Edge case: unparseable ngspice output fails loud with a raw excerpt,
/// never an empty comparison set.
#[test]
fn ngspice_unparseable_output_fails_loud() {
    let err = NgspiceHarness::parse_op_output("bogus", "Note: nothing to see here\n")
        .expect_err("no v(node) lines must be an error");
    assert!(err.contains("bogus"), "names the circuit: {err}");
    assert!(err.contains("nothing to see here"), "carries a raw excerpt: {err}");
}

/// SPICE-07 edge case: both sides agree on 0 shared nodes → contract
/// violation, not a pass.
#[test]
fn ngspice_zero_shared_nodes_is_a_contract_violation() {
    let golden = BTreeMap::from([("out".to_string(), 1.0)]);
    let err = NgspiceHarness::compare_op("lonely", &golden, |_| None)
        .expect_err("0 shared nodes must fail");
    assert!(err.contains("lonely"), "names the circuit: {err}");
    assert!(err.contains("0 shared nodes"), "names the violation: {err}");
}

/// SPICE-07: a mismatch names the circuit, the node, both values and the
/// delta.
#[test]
fn ngspice_mismatch_failure_is_actionable() {
    let golden = BTreeMap::from([("out".to_string(), 3.0)]);
    let err = NgspiceHarness::compare_op("offby", &golden, |_| Some(1.92))
        .expect_err("out-of-tolerance node must fail");
    assert!(err.contains("offby"), "names the circuit: {err}");
    assert!(err.contains("v(out)"), "names the node: {err}");
    assert!(err.contains("3.0") && err.contains("1.92"), "shows both values: {err}");
    assert!(err.contains("Δ="), "shows the delta: {err}");
}

/// The tolerance contract itself: `|Δ| ≤ abstol + reltol·max(|a|,|b|)`.
#[test]
fn ngspice_tolerance_contract() {
    // 7.5 V vs 7.51 V: Δ=1e-2 > 1e-6 + 1e-3·7.51 → out.
    assert!(!NgspiceHarness::within_tolerance(7.5, 7.51, NgspiceHarness::ABSTOL_V));
    // 7.5 V vs 7.507: Δ=7e-3 ≤ 1e-6 + 1e-3·7.507 ≈ 7.508e-3 → in.
    assert!(NgspiceHarness::within_tolerance(7.5, 7.507, NgspiceHarness::ABSTOL_V));
    // Near zero the abstol floor governs.
    assert!(NgspiceHarness::within_tolerance(0.0, 9.9e-7, NgspiceHarness::ABSTOL_V));
    assert!(!NgspiceHarness::within_tolerance(0.0, 1.1e-6, NgspiceHarness::ABSTOL_V));
}

/// SPICE-08 edge case: the wrdata parser is strict — malformed columns,
/// unparseable numbers and an empty export all fail loud.
#[test]
fn ngspice_wrdata_parsed_strictly() {
    let ok = NgspiceHarness::parse_wrdata("s", "-1.0e0  1.0e-12\n0.5 2e-3\n").unwrap();
    assert_eq!(ok, vec![(-1.0, 1.0e-12), (0.5, 2e-3)]);

    let err = NgspiceHarness::parse_wrdata("s", "-1.0e0 1.0e-12 3.0\n")
        .expect_err("3 columns must fail");
    assert!(err.contains("malformed wrdata line"), "{err}");

    let err = NgspiceHarness::parse_wrdata("s", "-1.0e0 bogus\n")
        .expect_err("non-numeric must fail");
    assert!(err.contains("unparseable wrdata number"), "{err}");

    let err = NgspiceHarness::parse_wrdata("s", "\n \n").expect_err("empty must fail");
    assert!(err.contains("empty wrdata export"), "{err}");
}

/// Regression guard for the DC device-bypass fix (runs without ngspice):
/// two identical series diodes must show equal drops. The old global-scale
/// bypass threshold froze stamps inside a `reltol·max|v|` window (5 mV
/// here), locking in a ~0.65 mV inconsistency between the two junctions.
#[test]
fn ngspice_series_junctions_are_self_consistent() {
    let harness_less = NgspiceHarness { exe: PathBuf::new() };
    let op = harness_less.piperine_op("diode_series").unwrap_or_else(|e| panic!("{e}"));
    let va = op.v(&NetRef { name: "a".to_string() }, None).unwrap();
    let vb = op.v(&NetRef { name: "b".to_string() }, None).unwrap();
    let d1 = va - vb;
    let d2 = vb;
    assert!(
        (d1 - d2).abs() < 5e-4,
        "identical series diodes must drop equally: d1={d1:.6e} d2={d2:.6e} Δ={:.3e}",
        (d1 - d2).abs()
    );
}
