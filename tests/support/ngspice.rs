//! The piperine-vs-ngspice comparison harness, shared by every golden target.
//!
//! Owns detection of the `ngspice` binary (and the SKIP seam that makes its
//! absence a pass, never a hard dependency), the fixture paths under
//! `tests/ngspice/`, the stdlib source map both sides elaborate against, the
//! strict `wrdata` parser, and the per-domain comparison cases. Extracted from
//! `tests/ngspice_validation.rs` unchanged so the DC oracle keeps its
//! behaviour while AC/noise/transient/temperature targets reuse the same
//! machinery.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

use piperine::{OpResult, Session, SolverConfig};
use piperine_lang::SourceMap;

/// The piperine-vs-ngspice comparison harness. Owns detection of the ngspice
/// binary, both simulation paths and the tolerance contract.
pub(crate) struct NgspiceHarness {
    pub(crate) exe: PathBuf,
}

impl NgspiceHarness {
    /// ngspice's own defaults for DC node voltages (validation contract).
    const RELTOL: f64 = 1e-3;
    pub(crate) const ABSTOL_V: f64 = 1e-6;
    /// Current abstol for sweep comparisons (A).
    pub(crate) const ABSTOL_I: f64 = 1e-9;

    /// The harness for the `ngspice` binary on PATH, or `None` (skip).
    pub(crate) fn detect() -> Option<Self> {
        Self::detect_with_path(std::env::var_os("PATH"))
    }

    /// PATH-injectable detection seam so the skip branch is testable
    /// without mutating the process environment.
    pub(crate) fn detect_with_path(path: Option<OsString>) -> Option<Self> {
        let path = path?;
        std::env::split_paths(&path)
            .map(|dir| dir.join("ngspice"))
            .find(|candidate| candidate.is_file())
            .map(|exe| Self { exe })
    }

    /// `|a − b| ≤ abstol + reltol·max(|a|, |b|)` — the run.py contract.
    pub(crate) fn within_tolerance(a: f64, b: f64, abstol: f64) -> bool {
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
    pub(crate) fn parse_op_output(
        circuit: &str,
        out: &str,
    ) -> Result<BTreeMap<String, f64>, String> {
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
    pub(crate) fn piperine_op(&self, circuit: &str) -> Result<OpResult, String> {
        let phdl = Self::circuits_dir().join(format!("{circuit}.phdl"));
        let src = std::fs::read_to_string(&phdl)
            .map_err(|e| format!("{circuit}: {}: {e}", phdl.display()))?;
        let design = piperine_lang::parse_and_elaborate(&src, &Self::headers_source_map())
            .map_err(|e| format!("{circuit}: elaboration failed: {e:?}"))?;
        let mut session = Session::compile(&design, "Top").expect("session compiles");
        session
            .op(&SolverConfig::default(), None)
            .map_err(|e| format!("{circuit}: piperine DC solve failed: {e}"))
    }

    /// Compare every ngspice-reported node against piperine. `0`/ground is
    /// dropped; zero *shared* nodes is a contract violation.
    pub(crate) fn compare_op(
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
    pub(crate) fn op_case(&self, circuit: &str) {
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
    pub(crate) fn parse_wrdata(circuit: &str, content: &str) -> Result<Vec<(f64, f64)>, String> {
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
    pub(crate) fn sweep_case(
        &self,
        circuit: &str,
        source: &str,
        branch_a: &str,
        branch_b: &str,
        abstol: f64,
    ) {
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
        let mut session = Session::compile(&design, "Top").expect("session compiles");

        let values: Vec<f64> = golden.iter().map(|(x, _)| *x).collect();
        // Compile-once sweep (MD-18): one build, restamped per point, with a
        // full `OpResult` at each — `Session::sweep` clones the build info per
        // point, which is what makes the device-internal current readback
        // below read that point's parameter value.
        let mut ops = Vec::with_capacity(values.len());
        {
            let mut sweep = session.sweep(source, "dc", &values);
            while let Some(point) = sweep.next() {
                let mut point = point
                    .unwrap_or_else(|e| panic!("{circuit}: piperine DC sweep failed: {e}"));
                ops.push(
                    point
                        .op(&SolverConfig::default(), None)
                        .unwrap_or_else(|e| panic!("{circuit}: piperine DC sweep failed: {e}")),
                );
            }
        }

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
    fn piperine_session(&self, circuit: &str) -> Result<Session, String> {
        let phdl = Self::circuits_dir().join(format!("{circuit}.phdl"));
        let src = std::fs::read_to_string(&phdl)
            .map_err(|e| format!("{circuit}: {}: {e}", phdl.display()))?;
        let design = piperine_lang::parse_and_elaborate(&src, &Self::headers_source_map())
            .map_err(|e| format!("{circuit}: elaboration failed: {e:?}"))?;
        // `.disto(true)`: the spectral cases include `.disto`, the one analysis
        // whose 2nd/3rd-derivative kernels are opt-in.
        Ok(Session::builder(&design, "Top").disto(true).compile().expect("session compiles"))
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
            if cols.len() >= 3 && cols[0] == "0"
                && let Ok(re) = cols[2].trim_end_matches(',').parse::<f64>()
            {
                match harmonic {
                    Some(2) if hd2.is_none() => hd2 = Some(re.abs()),
                    Some(3) if hd3.is_none() => hd3 = Some(re.abs()),
                    _ => {}
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
    pub(crate) fn disto_case(&self, circuit: &str, f1: f64, reltol: f64) {
        let (fund, ng_hd2, ng_hd3) =
            self.ngspice_disto(circuit).unwrap_or_else(|e| panic!("{e}"));
        assert!(fund > 0.0, "{circuit}: ngspice fundamental must be positive");
        let (ng_hd2, ng_hd3) = (ng_hd2 / fund, ng_hd3 / fund);

        let mut session = self.piperine_session(circuit).unwrap_or_else(|e| panic!("{e}"));
        let result = session
            .disto(f1, None, 1.0, "out", None, &SolverConfig::default())
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
    pub(crate) fn four_case(&self, circuit: &str, f0: f64, stop: f64, reltol: f64) {
        let (ng_thd, ng_norms) = self.ngspice_fourier(circuit).unwrap_or_else(|e| panic!("{e}"));

        let mut session = self.piperine_session(circuit).unwrap_or_else(|e| panic!("{e}"));
        let trace = session
            .tran(stop, None, 0.0, &SolverConfig::default(), None, false, &[])
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
    pub(crate) fn pz_case(&self, circuit: &str, reltol: f64) {
        let ng_poles = self.ngspice_poles(circuit).unwrap_or_else(|e| panic!("{e}"));

        let mut session = self.piperine_session(circuit).unwrap_or_else(|e| panic!("{e}"));
        let result = session
            .pz("v1", "out", None, &SolverConfig::default())
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
