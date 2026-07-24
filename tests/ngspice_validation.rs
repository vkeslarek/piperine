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

use piperine::{OpResult, SimSession, SolverConfig};
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
            PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
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
            .run_op(&SolverConfig::default(), None)
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
            op.v(node.to_string()).ok()
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
    /// compile-once sweep (MD-18: elaborate/JIT once, restamp `source`.dc on
    /// the compiled circuit per point, DC solve, read the current through
    /// the `(branch_a, branch_b)` two-terminal instance — the swept source's
    /// force branch, matching ngspice's `i(v1)` sign convention).
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

        let values: Vec<f64> = golden.iter().map(|(x, _)| *x).collect();
        let ops = session
            .run_op_sweep(source, "dc", &values, &SolverConfig::default(), None)
            .unwrap_or_else(|e| panic!("{circuit}: piperine DC sweep failed: {e}"));

        let mut mismatches = Vec::new();
        for ((x, i_golden), op) in golden.iter().zip(&ops) {
            let i_piperine = op
                .i((branch_a.to_string(), branch_b.to_string()))
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

    /// Piperine side shared by the spectral cross-checks: elaborate
    /// `<circuit>.phdl` into a session.
    fn piperine_session(&self, circuit: &str) -> Result<SimSession, String> {
        let phdl = Self::circuits_dir().join(format!("{circuit}.phdl"));
        let src = std::fs::read_to_string(&phdl)
            .map_err(|e| format!("{circuit}: {}: {e}", phdl.display()))?;
        let design = piperine_lang::parse_and_elaborate(&src, &Self::headers_source_map())
            .map_err(|e| format!("{circuit}: elaboration failed: {e:?}"))?;
        Ok(SimSession::new(design, "Top".to_string()))
    }

    // ── Spectral analyses (.disto/.four/.pz) ─────────────────────────────

    /// Golden side of `.disto`: parse the two `DISTORTION - Nth harmonic`
    /// tables (the 2nd/3rd-harmonic components of `v(out)`) and the `.ac`
    /// fundamental `v(out) = <re>,<im>`; return the magnitudes
    /// `(fundamental, hd2_component, hd3_component)`.
    fn ngspice_disto(&self, circuit: &str) -> Result<(f64, f64, f64), String> {
        let cir = Self::circuits_dir().join(format!("{circuit}.cir"));
        let output = std::process::Command::new(&self.exe)
            .arg("-b")
            .arg(&cir)
            .output()
            .map_err(|e| format!("{circuit}: failed to run ngspice: {e}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut fundamental = None;
        let mut hd2 = None;
        let mut hd3 = None;
        let mut harmonic: Option<u8> = None;
        for line in stdout.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("v(out) =") {
                let re = rest
                    .split(',')
                    .next()
                    .and_then(|s| s.trim().parse::<f64>().ok());
                fundamental = Some(re.ok_or_else(|| format!("{circuit}: unparseable ac line `{line}`"))?.abs());
                continue;
            }
            if line.starts_with("DISTORTION - 2nd harmonic") {
                harmonic = Some(2);
                continue;
            }
            if line.starts_with("DISTORTION - 3rd harmonic") {
                harmonic = Some(3);
                continue;
            }
            if line.starts_with("DISTORTION") {
                harmonic = None;
                continue;
            }
            // Table data row: `<idx> <freq> <re>, <im>` — first row only.
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() >= 3 && cols[0] == "0" {
                if let Ok(re) = cols[2].trim_end_matches(',').parse::<f64>() {
                    match harmonic {
                        Some(2) if hd2.is_none() => hd2 = Some(re.abs()),
                        Some(3) if hd3.is_none() => hd3 = Some(re.abs()),
                        _ => {}
                    }
                }
            }
        }
        match (fundamental, hd2, hd3) {
            (Some(f), Some(h2), Some(h3)) => Ok((f, h2, h3)),
            _ => {
                let excerpt: String = stdout.chars().take(600).collect();
                Err(format!(
                    "{circuit}: missing ac fundamental or distortion table in ngspice output — raw excerpt:\n{excerpt}"
                ))
            }
        }
    }

    /// `.disto` golden case: piperine's HD2/HD3 ratios against ngspice's
    /// harmonic components normalized by its AC fundamental.
    fn disto_case(&self, circuit: &str, f1: f64, reltol: f64) {
        let (fund, ng_hd2, ng_hd3) =
            self.ngspice_disto(circuit).unwrap_or_else(|e| panic!("{e}"));
        assert!(fund > 0.0, "{circuit}: ngspice fundamental must be positive");
        let (ng_hd2, ng_hd3) = (ng_hd2 / fund, ng_hd3 / fund);

        let session = self.piperine_session(circuit).unwrap_or_else(|e| panic!("{e}"));
        let result = session
            .run_disto(f1, None, 1.0, "out", None, &SolverConfig::default())
            .unwrap_or_else(|e| panic!("{circuit}: piperine .disto failed: {e}"));
        let pp_hd2 = result.hd2.expect("single-tone reports HD2");
        let pp_hd3 = result.hd3.expect("single-tone reports HD3");

        for (name, ng, pp) in [("HD2", ng_hd2, pp_hd2), ("HD3", ng_hd3, pp_hd3)] {
            assert!(
                (ng - pp).abs() <= reltol * ng.max(pp),
                "{circuit}: {name} ngspice={ng:.6e} piperine={pp:.6e} Δ={:.3e}",
                (ng - pp).abs()
            );
        }
        eprintln!("PASS {circuit} (.disto HD2 {pp_hd2:.4e} vs {ng_hd2:.4e}, HD3 {pp_hd3:.4e} vs {ng_hd3:.4e})");
    }

    /// Golden side of `.four`: parse `THD: <x> %` and the harmonic table's
    /// `Norm. Mag` column; return `(thd_fraction, norm_mags)` with
    /// `norm_mags[k - 1]` the normalized magnitude of harmonic `k ≥ 1`.
    fn ngspice_fourier(&self, circuit: &str) -> Result<(f64, Vec<f64>), String> {
        let cir = Self::circuits_dir().join(format!("{circuit}.cir"));
        let output = std::process::Command::new(&self.exe)
            .arg("-b")
            .arg(&cir)
            .output()
            .map_err(|e| format!("{circuit}: failed to run ngspice: {e}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut thd = None;
        let mut norm_mags = Vec::new();
        for line in stdout.lines() {
            let line = line.trim();
            if line.contains("THD:") {
                let after = line.split("THD:").nth(1).unwrap_or("");
                let pct = after
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .parse::<f64>()
                    .map_err(|e| format!("{circuit}: unparseable THD in `{line}`: {e}"))?;
                thd = Some(pct / 100.0);
                continue;
            }
            // Harmonic table row: `<k> <freq> <mag> <phase> <norm_mag> <norm_phase>`.
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() == 6
                && let Ok(k) = cols[0].parse::<u32>()
                && k >= 1
                && let Ok(norm) = cols[4].parse::<f64>()
            {
                norm_mags.push(norm);
            }
        }
        match (thd, norm_mags.is_empty()) {
            (Some(t), false) => Ok((t, norm_mags)),
            _ => {
                let excerpt: String = stdout.chars().take(600).collect();
                Err(format!(
                    "{circuit}: missing THD line or harmonic table in ngspice output — raw excerpt:\n{excerpt}"
                ))
            }
        }
    }

    /// `.four` golden case: piperine's transient + `Waveform::fourier`
    /// against ngspice's `fourier` on the same hard-driven stage.
    fn four_case(&self, circuit: &str, f0: f64, stop: f64, reltol: f64) {
        let (ng_thd, ng_norms) = self.ngspice_fourier(circuit).unwrap_or_else(|e| panic!("{e}"));

        let session = self.piperine_session(circuit).unwrap_or_else(|e| panic!("{e}"));
        let trace = session
            .run_tran(stop, None, 0.0, &SolverConfig::default(), None, false, &[])
            .unwrap_or_else(|e| panic!("{circuit}: piperine transient failed: {e}"));
        let wf = trace
            .v("out".to_string())
            .unwrap_or_else(|e| panic!("{circuit}: no v(out) waveform: {e}"));
        let result = wf
            .fourier(f0, 10)
            .unwrap_or_else(|e| panic!("{circuit}: piperine fourier failed: {e}"));

        assert!(
            (result.thd - ng_thd).abs() <= reltol * result.thd.max(ng_thd),
            "{circuit}: THD ngspice={ng_thd:.6e} piperine={:.6e} Δ={:.3e}",
            result.thd,
            (result.thd - ng_thd).abs()
        );
        // The dominant harmonics (2 and 3) track individually.
        for k in [2usize, 3] {
            let ng = ng_norms[k - 1];
            let pp = result.harmonics[k].norm_magnitude;
            assert!(
                (ng - pp).abs() <= reltol * ng.max(pp),
                "{circuit}: norm mag H{k} ngspice={ng:.6e} piperine={pp:.6e} Δ={:.3e}",
                (ng - pp).abs()
            );
        }
        eprintln!("PASS {circuit} (.four THD {:.4e} vs {ng_thd:.4e})", result.thd);
    }

    /// Golden side of `.pz`: parse the `Pole-Zero Analysis` table rows
    /// (`<idx> <re>, <im>`) into pole values.
    fn ngspice_poles(&self, circuit: &str) -> Result<Vec<(f64, f64)>, String> {
        let cir = Self::circuits_dir().join(format!("{circuit}.cir"));
        let output = std::process::Command::new(&self.exe)
            .arg("-b")
            .arg(&cir)
            .output()
            .map_err(|e| format!("{circuit}: failed to run ngspice: {e}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut in_table = false;
        let mut poles = Vec::new();
        for line in stdout.lines() {
            let line = line.trim();
            if line.contains("Pole-Zero Analysis") {
                in_table = true;
                continue;
            }
            if !in_table {
                continue;
            }
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() >= 3
                && cols[0].parse::<usize>().is_ok()
                && let (Ok(re), Ok(im)) = (
                    cols[1].trim_end_matches(',').parse::<f64>(),
                    cols[2].trim_end_matches(',').parse::<f64>(),
                )
            {
                poles.push((re, im));
            }
        }
        if poles.is_empty() {
            let excerpt: String = stdout.chars().take(600).collect();
            return Err(format!(
                "{circuit}: no poles in ngspice output — raw excerpt:\n{excerpt}"
            ));
        }
        Ok(poles)
    }

    /// `.pz` golden case: the natural poles of the shared RC network.
    fn pz_case(&self, circuit: &str, reltol: f64) {
        let ng_poles = self.ngspice_poles(circuit).unwrap_or_else(|e| panic!("{e}"));

        let session = self.piperine_session(circuit).unwrap_or_else(|e| panic!("{e}"));
        let result = session
            .run_pz("v1", "out", None, &SolverConfig::default())
            .unwrap_or_else(|e| panic!("{circuit}: piperine .pz failed: {e}"));

        assert_eq!(
            result.poles.len(),
            ng_poles.len(),
            "{circuit}: pole count piperine={} ngspice={}",
            result.poles.len(),
            ng_poles.len()
        );
        for (pp, &(ng_re, ng_im)) in result.poles.iter().zip(&ng_poles) {
            assert!(
                (pp.re - ng_re).abs() <= reltol * ng_re.abs().max(pp.re.abs()),
                "{circuit}: pole Re ngspice={ng_re:.6e} piperine={:.6e}",
                pp.re
            );
            assert_eq!(pp.im, ng_im, "{circuit}: pole must be real");
        }
        eprintln!("PASS {circuit} (.pz pole {:.6e} vs {ng_re:.6e})", result.poles[0].re, ng_re = ng_poles[0].0);
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
fn ngspice_nmos2_fixed() {
    ngspice_op_case("nmos2_fixed");
}

#[test]
fn ngspice_nmos2_load() {
    ngspice_op_case("nmos2_load");
}

#[test]
fn ngspice_nmos3_fixed() {
    ngspice_op_case("nmos3_fixed");
}

#[test]
fn ngspice_nmos3_load() {
    ngspice_op_case("nmos3_load");
}

#[test]
fn ngspice_jfet_bias() {
    ngspice_op_case("jfet_bias");
}

#[test]
fn ngspice_bjt_ce() {
    ngspice_op_case("bjt_ce");
}

#[test]
fn ngspice_bjt_mirror() {
    ngspice_op_case("bjt_mirror");
}

// ── URC lumped RC line (FLAT-04) ───────────────────────────────────────────
//
// Three DC operating-point cross-checks of the FlattenHierarchy pass at
// work: each `Top` instantiates a mid-level `urcN` module that the flatten
// pass inlines into N series `res` + N shunt `cap` leaves. ngspice sees the
// SAME ladder topology built from discrete R/C elements (`urc_lumpN.cir`),
// so the DC operating point must match exactly. Each lump value yields a
// distinct Vout (Vout = 5·1000/(100·N+1000)) — the test is discriminating:
// dropping or duplicating a segment shifts Vout by ≥0.13 V, well outside
// the harness's 1e-3 reltol. Capacitors are open at DC, so the comparison
// isolates the flatten pass's structural correctness.

#[test]
fn ngspice_urc_lump2() {
    ngspice_op_case("urc_lump2");
}

#[test]
fn ngspice_urc_lump5() {
    ngspice_op_case("urc_lump5");
}

#[test]
fn ngspice_urc_lump10() {
    ngspice_op_case("urc_lump10");
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

/// NMOS level 2 Id–Vds (26 points, vgs = 3 V, rd/rs = 100 Ω): linear →
/// saturation with the level-2 channel-length modulation (SC-14).
#[test]
fn ngspice_nmos2_id_vds_sweep() {
    match NgspiceHarness::detect() {
        Some(harness) => {
            harness.sweep_case("nmos2_id_vds", "vd", "d", "gnd", NgspiceHarness::ABSTOL_I)
        }
        None => eprintln!("SKIP nmos2_id_vds: ngspice not on PATH"),
    }
}

/// NMOS level 2 Id–Vgs (26 points, vds = 3 V, nsub + nfs): cutoff →
/// subthreshold → strong inversion (SC-14).
#[test]
fn ngspice_nmos2_id_vgs_sweep() {
    match NgspiceHarness::detect() {
        Some(harness) => {
            harness.sweep_case("nmos2_id_vgs", "vg", "d", "gnd", NgspiceHarness::ABSTOL_I)
        }
        None => eprintln!("SKIP nmos2_id_vgs: ngspice not on PATH"),
    }
}

/// NMOS level 3 Id–Vds (26 points, vgs = 3 V, rd/rs = 100 Ω): short-channel
/// physics (theta, eta, kappa, vmax, xj) across linear → saturation (SC-14).
#[test]
fn ngspice_nmos3_id_vds_sweep() {
    match NgspiceHarness::detect() {
        Some(harness) => {
            harness.sweep_case("nmos3_id_vds", "vd", "d", "gnd", NgspiceHarness::ABSTOL_I)
        }
        None => eprintln!("SKIP nmos3_id_vds: ngspice not on PATH"),
    }
}

/// NMOS level 3 Id–Vgs (26 points, vds = 3 V, nsub + nfs): cutoff →
/// subthreshold → strong inversion with short-channel physics (SC-14).
#[test]
fn ngspice_nmos3_id_vgs_sweep() {
    match NgspiceHarness::detect() {
        Some(harness) => {
            harness.sweep_case("nmos3_id_vgs", "vg", "d", "gnd", NgspiceHarness::ABSTOL_I)
        }
        None => eprintln!("SKIP nmos3_id_vgs: ngspice not on PATH"),
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

// ── Spectral analyses (.disto/.four/.pz) — spectral-analyses T15 ─────────

/// `.disto`: a hard-biased diode's HD2/HD3 — piperine's Volterra ratios
/// against ngspice's harmonic components normalized by its AC fundamental
/// (both sides run the same nonlinear-currents method on the
/// equation-identical diode, 1 % tolerance).
#[test]
fn ngspice_disto_diode() {
    match NgspiceHarness::detect() {
        Some(harness) => harness.disto_case("disto_diode", 1e6, 1e-2),
        None => eprintln!("SKIP disto_diode: ngspice not on PATH"),
    }
}

/// `.four`: the hard-driven diode stage's THD — piperine's transient +
/// `Waveform::fourier` against ngspice's `fourier` over the last period
/// (integrator-pair tolerance 2 %).
#[test]
fn ngspice_four_diode() {
    match NgspiceHarness::detect() {
        Some(harness) => harness.four_case("four_diode", 1e6, 3e-6, 2e-2),
        None => eprintln!("SKIP four_diode: ngspice not on PATH"),
    }
}

/// `.pz`: the RC network's single natural pole at −500 rad/s — piperine's
/// QZ poles against ngspice's pole-zero analysis (1e-3 relative).
#[test]
fn ngspice_pz_rc() {
    match NgspiceHarness::detect() {
        Some(harness) => harness.pz_case("pz_rc", 1e-3),
        None => eprintln!("SKIP pz_rc: ngspice not on PATH"),
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
    let va = op.v("a".to_string()).unwrap();
    let vb = op.v("b".to_string()).unwrap();
    let d1 = va - vb;
    let d2 = vb;
    assert!(
        (d1 - d2).abs() < 5e-4,
        "identical series diodes must drop equally: d1={d1:.6e} d2={d2:.6e} Δ={:.3e}",
        (d1 - d2).abs()
    );
}
