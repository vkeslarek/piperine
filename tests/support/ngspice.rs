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
use std::path::{Path, PathBuf};

use num_complex::Complex64;
use piperine::{OpResult, Session, SolverConfig};
use piperine_lang::SourceMap;

/// The numeric agreement contract for one comparison domain (NGP-07).
///
/// Every golden target reads its bound from this table and never retypes a
/// number: a tolerance that lives in a test body is a constant nobody can
/// review, and widening one to make a run green is the exact failure mode this
/// feature exists to prevent. Each constant carries the reason its value is
/// what it is, and that reason is printed with the failure so a reader sees
/// *why* the bound is the bound at the moment it is exceeded.
pub(crate) struct ToleranceContract {
    reltol: f64,
    abstol: f64,
    rationale: &'static str,
}

impl ToleranceContract {
    /// DC node voltages — ngspice's own defaults, the contract the existing
    /// operating-point corpus was baselined against. Unchanged by this
    /// feature so no golden baseline drifts.
    pub(crate) const DC: Self = Self {
        reltol: 1e-3,
        abstol: 1e-6,
        rationale: "ngspice's own reltol/vntol defaults — the DC baseline every \
                    existing operating-point check was measured against",
    };

    /// DC branch currents — same relative agreement, floored at ngspice's
    /// `abstol` for currents so a reverse-biased junction near 0 A is not
    /// compared relatively against ~0.
    pub(crate) const DC_CURRENT: Self = Self {
        reltol: 1e-3,
        abstol: 1e-9,
        rationale: "ngspice's abstol for currents — floors the comparison where a \
                    swept branch current passes through zero",
    };

    /// AC magnitude — a linear solve at a frozen operating point, so agreement
    /// should be near machine level; the absolute floor governs the deep
    /// stopband where the magnitude collapses.
    pub(crate) const AC_MAG: Self = Self {
        reltol: 1e-3,
        abstol: 1e-12,
        rationale: "AC is a linear solve at a frozen OP — agreement is near \
                    machine level; the abstol floors the deep stopband",
    };

    /// AC phase — purely absolute, in radians. Phase carries no information
    /// where the magnitude is ~0, so the comparison is *gated* on magnitude
    /// clearing `AC_MAG`'s floor rather than being loosened here.
    pub(crate) const AC_PHASE: Self = Self {
        reltol: 0.0,
        abstol: 1e-4,
        rationale: "radians, absolute: phase is meaningless where magnitude ≈ 0, so \
                    the comparison is gated on magnitude rather than widened here",
    };

    /// Output-noise PSD, in **V²/Hz**.
    ///
    /// ⚠ Unit reconciliation — the single highest-risk detail of this
    /// comparison. ngspice's `onoise_spectrum` is an *amplitude* spectrum in
    /// **V/√Hz**; piperine reports a *power* spectral density in **V²/Hz**
    /// (Part VII §12). ngspice's value must therefore be **squared** before it
    /// is compared against piperine's. Verified against ngspice-46: a 1 kΩ
    /// resistor at 1 kHz gives ngspice 4.071e-9, and √(4kTR) =
    /// √(4·1.38e-23·300·1000) = 4.07e-9 V/√Hz. Comparing the two directly is
    /// wrong by a square and reads as a broken noise model rather than a unit
    /// mismatch. The abstol below is a squared amplitude floor, in V²/Hz.
    pub(crate) const NOISE_PSD: Self = Self {
        reltol: 1e-3,
        abstol: 1e-24,
        rationale: "V²/Hz — ngspice's onoise_spectrum is an amplitude spectrum in \
                    V/√Hz and must be SQUARED to reach piperine's PSD (Part VII \
                    §12); comparing them directly is wrong by a square. abstol is \
                    the squared amplitude floor",
    };

    /// Integrated output noise — ngspice's `onoise_total` against
    /// `NoiseTrace::total()`, both an RMS in volts (no squaring here: the
    /// integrated figure is already an amplitude on both sides).
    pub(crate) const NOISE_TOTAL: Self = Self {
        reltol: 1e-3,
        abstol: 1e-15,
        rationale: "volts RMS — onoise_total and NoiseTrace::total() are both \
                    amplitudes, so unlike the PSD this one needs no squaring",
    };

    /// Transient node voltages on ngspice's print grid. Piperine's adaptive
    /// samples are interpolated onto those times, which adds an error the LTE
    /// contract bounds — hence the same relative bound as DC rather than a
    /// tighter one.
    pub(crate) const TRAN: Self = Self {
        reltol: 1e-3,
        abstol: 1e-6,
        rationale: "interpolating piperine's adaptive grid onto ngspice's print \
                    times adds an error bounded by the LTE contract",
    };

    /// A temperature-swept operating point — a DC comparison with a different
    /// `SolverConfig.temperature`, so it inherits the DC contract.
    pub(crate) const TEMP: Self = Self {
        reltol: 1e-3,
        abstol: 1e-6,
        rationale: "a DC comparison under a non-nominal temperature — same bound \
                    as DC, different SolverConfig.temperature",
    };

    /// `.disto` harmonic ratios. Both sides run the nonlinear-currents method
    /// on the equation-identical device, but the Volterra kernels are summed
    /// in a different order, so agreement is percent-level rather than tight.
    pub(crate) const DISTO_HD: Self = Self {
        reltol: 1e-2,
        abstol: 0.0,
        rationale: "HD2/HD3 ratios: the same nonlinear-currents method summed in a \
                    different order on both sides — percent-level agreement",
    };

    /// `.four` THD and normalized harmonic magnitudes — the two sides
    /// integrate the transient with different steppers before transforming,
    /// so the bound is the integrator pair's, not the transform's.
    pub(crate) const FOUR_THD: Self = Self {
        reltol: 2e-2,
        abstol: 0.0,
        rationale: "THD over a transient integrated by two different steppers \
                    before the transform — the integrator pair sets the bound",
    };

    /// `.pz` pole locations — a generalized eigenproblem on both sides, so
    /// the agreement is as tight as DC.
    pub(crate) const PZ_POLE: Self = Self {
        reltol: 1e-3,
        abstol: 0.0,
        rationale: "poles from a generalized eigenproblem on both sides — as tight \
                    as the DC bound",
    };

    /// `|golden − actual| ≤ abstol + reltol·max(|golden|, |actual|)`.
    pub(crate) fn holds(&self, golden: f64, actual: f64) -> bool {
        (golden - actual).abs() <= self.abstol + self.reltol * golden.abs().max(actual.abs())
    }

    /// Why this domain's bound is what it is — printed with every failure so
    /// the reader can judge the bound, not just the miss.
    pub(crate) fn rationale(&self) -> &'static str {
        self.rationale
    }
}

/// The working directory one ngspice run writes its `wrdata` exports into.
///
/// ngspice writes exports relative to its working directory, so each run needs
/// its own. The key names the circuit and the domain so a run under inspection
/// is findable, but what makes it *unique* is the trailing serial: two tests in
/// one binary may run the same fixture in the same domain at the same time (a
/// golden comparison and an analytic cross-check of one RC, say), and a shared
/// directory means one run's `Drop` removes the other's working directory out
/// from under it — which surfaces as ngspice failing to spawn with `No such
/// file or directory`, not as a comparison failure. Removal is `Drop`'s job, so
/// the success path and every early return clean up identically.
struct ScratchDir {
    path: PathBuf,
}

/// Serial source for [`ScratchDir::key`] — one directory per run, never per
/// (circuit, domain) pair.
static SCRATCH_SERIAL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

impl ScratchDir {
    /// The directory name for one ngspice run of this process.
    fn key(circuit: &str, domain: &str) -> String {
        let serial = SCRATCH_SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        format!("piperine-ngspice-{circuit}-{domain}-{}-{serial}", std::process::id())
    }

    fn create(circuit: &str, domain: &str) -> Result<Self, String> {
        let path = std::env::temp_dir().join(Self::key(circuit, domain));
        std::fs::create_dir_all(&path)
            .map_err(|e| format!("{circuit} ({domain}): scratch dir {}: {e}", path.display()))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// One parsed `wrdata` export — N columns of floats, strictly parsed.
///
/// `wrdata` is ngspice's general vector export and the mechanism behind every
/// cross-checked domain, but the column count is domain-dependent: a DC sweep,
/// a transient and a noise spectrum each render two columns
/// (`sweep|time|freq  value`), while **AC renders three** — ngspice expands a
/// complex vector into `freq  real  imag`. The caller states the shape it
/// expects and any deviation fails loud, so a version or locale change can
/// never degrade into a silently truncated comparison.
#[derive(Debug)]
pub(crate) struct WrData {
    columns: usize,
    rows: Vec<Vec<f64>>,
}

impl WrData {
    /// Strict parser: every non-empty line must hold exactly `columns` floats;
    /// anything else — a wrong column count, an unparseable number, or an
    /// export with no rows at all — is a loud error naming the circuit.
    pub(crate) fn parse(circuit: &str, content: &str, columns: usize) -> Result<Self, String> {
        let mut rows = Vec::new();
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() != columns {
                return Err(format!(
                    "{circuit}: malformed wrdata line (expected {columns} columns): `{line}`"
                ));
            }
            let mut row = Vec::with_capacity(columns);
            for col in cols {
                row.push(col.parse::<f64>().map_err(|e| {
                    format!("{circuit}: unparseable wrdata number `{col}`: {e}")
                })?);
            }
            rows.push(row);
        }
        if rows.is_empty() {
            return Err(format!("{circuit}: empty wrdata export — contract violation"));
        }
        Ok(Self { columns, rows })
    }

    /// Read one export back out of a finished run's directory. An export
    /// ngspice never wrote is a loud failure naming the circuit and the path
    /// it was expected at — never an empty comparison.
    fn read(circuit: &str, dir: &Path, file: &str, columns: usize) -> Result<Self, String> {
        let path = dir.join(file);
        let content = std::fs::read_to_string(&path).map_err(|e| {
            format!("{circuit}: ngspice wrote no wrdata export {}: {e}", path.display())
        })?;
        Self::parse(circuit, &content, columns)
    }

    /// The two-column form: `(coordinate, value)` per row.
    pub(crate) fn pairs(&self) -> Vec<(f64, f64)> {
        assert_eq!(self.columns, 2, "pairs() reads a 2-column export");
        self.rows.iter().map(|row| (row[0], row[1])).collect()
    }

    /// The three-column AC form: `(freq, real + j·imag)` per row — ngspice
    /// writes a complex vector as two adjacent real columns.
    pub(crate) fn complex(&self) -> Vec<(f64, Complex64)> {
        assert_eq!(self.columns, 3, "complex() reads a 3-column AC export");
        self.rows.iter().map(|row| (row[0], Complex64::new(row[1], row[2]))).collect()
    }
}

/// The piperine-vs-ngspice comparison harness. Owns detection of the ngspice
/// binary, both simulation paths and the tolerance contract.
pub(crate) struct NgspiceHarness {
    pub(crate) exe: PathBuf,
}

impl NgspiceHarness {
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

    /// Elaborate `<circuit>.phdl` against the real stdlib headers.
    fn elaborate(circuit: &str) -> Result<piperine_lang::Design, String> {
        let phdl = Self::circuits_dir().join(format!("{circuit}.phdl"));
        let src = std::fs::read_to_string(&phdl)
            .map_err(|e| format!("{circuit}: {}: {e}", phdl.display()))?;
        piperine_lang::parse_and_elaborate(&src, &Self::headers_source_map())
            .map_err(|e| format!("{circuit}: elaboration failed: {e:?}"))
    }

    /// The piperine side of a domain cross-check: `<circuit>.phdl` compiled
    /// into a session. Deliberately **not** `.disto(true)` — the 2nd/3rd
    /// derivative kernels are opt-in, costly, and overrun Cranelift on
    /// MOS2/MOS3, so only the spectral target (which needs them) asks.
    pub(crate) fn session(&self, circuit: &str) -> Result<Session, String> {
        let design = Self::elaborate(circuit)?;
        Ok(Session::compile(&design, "Top").expect("session compiles"))
    }

    /// Piperine side: elaborate `<circuit>.phdl` and solve the DC operating
    /// point in-process, returning the result object.
    pub(crate) fn piperine_op(&self, circuit: &str) -> Result<OpResult, String> {
        let design = Self::elaborate(circuit)?;
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
            if !ToleranceContract::DC.holds(*ng, pp) {
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
            return Err(format!(
                "{circuit}: {} node(s) out of tolerance:\n{}\n  tolerance: {}",
                mismatches.len(),
                mismatches.join("\n"),
                ToleranceContract::DC.rationale()
            ));
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
        let exports = self.run_export(circuit, "sweep", &[(&format!("{circuit}_sweep"), 2)])?;
        Ok(exports[0].pairs())
    }

    /// Run `<circuit>.cir` in a scratch directory of its own and read back the
    /// `wrdata` exports its `.control` block wrote, one `(file, columns)` pair
    /// per export. `domain` keys the scratch directory, so cross-checking one
    /// fixture in several domains — in parallel — cannot collide.
    pub(crate) fn run_export(
        &self,
        circuit: &str,
        domain: &str,
        exports: &[(&str, usize)],
    ) -> Result<Vec<WrData>, String> {
        let cir = Self::circuits_dir().join(format!("{circuit}.cir"));
        // Removal is the scratch dir's own `Drop`: every `?` below leaves by
        // the same path as success, so a failed run never leaks a directory.
        let scratch = ScratchDir::create(circuit, domain)?;
        std::process::Command::new(&self.exe)
            .arg("-b")
            .arg(&cir)
            .current_dir(scratch.path())
            .output()
            .map_err(|e| format!("{circuit}: failed to run ngspice: {e}"))?;
        let mut parsed = Vec::with_capacity(exports.len());
        for (file, columns) in exports {
            parsed.push(WrData::read(circuit, scratch.path(), file, *columns)?);
        }
        Ok(parsed)
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
        tolerance: &ToleranceContract,
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
            if !tolerance.holds(*i_golden, i_piperine) {
                mismatches.push(format!(
                    "    {source}={x:+.4e}: i ngspice={i_golden:+.6e}  piperine={i_piperine:+.6e}  Δ={:.3e}",
                    (i_golden - i_piperine).abs()
                ));
            }
        }
        assert!(
            mismatches.is_empty(),
            "{circuit}: {}/{} sweep point(s) out of tolerance:\n{}\n  tolerance: {}",
            mismatches.len(),
            golden.len(),
            mismatches.join("\n"),
            tolerance.rationale()
        );
        eprintln!("PASS {circuit} ({} sweep points)", golden.len());
    }

    /// Piperine side shared by the spectral cross-checks: elaborate
    /// `<circuit>.phdl` into a session.
    fn piperine_session(&self, circuit: &str) -> Result<Session, String> {
        let design = Self::elaborate(circuit)?;
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
    pub(crate) fn disto_case(&self, circuit: &str, f1: f64, tolerance: &ToleranceContract) {
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
                tolerance.holds(ng, pp),
                "{circuit}: {name} ngspice={ng:.6e} piperine={pp:.6e} Δ={:.3e}\n  tolerance: {}",
                (ng - pp).abs(),
                tolerance.rationale()
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
    pub(crate) fn four_case(
        &self,
        circuit: &str,
        f0: f64,
        stop: f64,
        tolerance: &ToleranceContract,
    ) {
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
            tolerance.holds(ng_thd, result.thd),
            "{circuit}: THD ngspice={ng_thd:.6e} piperine={:.6e} Δ={:.3e}\n  tolerance: {}",
            result.thd,
            (result.thd - ng_thd).abs(),
            tolerance.rationale()
        );
        // The dominant harmonics (2 and 3) track individually.
        for k in [2usize, 3] {
            let ng = ng_norms[k - 1];
            let pp = result.harmonics[k].norm_magnitude;
            assert!(
                tolerance.holds(ng, pp),
                "{circuit}: norm mag H{k} ngspice={ng:.6e} piperine={pp:.6e} Δ={:.3e}\n  tolerance: {}",
                (ng - pp).abs(),
                tolerance.rationale()
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
    pub(crate) fn pz_case(&self, circuit: &str, tolerance: &ToleranceContract) {
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
                tolerance.holds(ng_re, pp.re),
                "{circuit}: pole Re ngspice={ng_re:.6e} piperine={:.6e}\n  tolerance: {}",
                pp.re,
                tolerance.rationale()
            );
            assert_eq!(pp.im, ng_im, "{circuit}: pole must be real");
        }
        eprintln!("PASS {circuit} (.pz pole {:.6e} vs {ng_re:.6e})", result.poles[0].re, ng_re = ng_poles[0].0);
    }
}

/// Unit coverage for the harness's own parsing layer. These live beside the
/// machinery they exercise (MD-28) and run inside every target that declares
/// `mod support;`.
#[cfg(test)]
mod tests {
    use super::{ScratchDir, ToleranceContract, WrData};

    /// Two domains of one circuit must not share a working directory — they
    /// would overwrite each other's exports under a parallel run.
    #[test]
    fn the_scratch_key_separates_two_domains_of_one_circuit() {
        let ac = ScratchDir::key("rc", "ac");
        let noise = ScratchDir::key("rc", "noise");
        assert_ne!(ac, noise, "the domain must be part of the key");
        for key in [&ac, &noise] {
            assert!(key.contains("rc"), "the key names the circuit: {key}");
            assert!(
                key.contains(&std::process::id().to_string()),
                "the key stays per-process: {key}"
            );
        }
        assert!(ac.contains("ac") && noise.contains("noise"), "{ac} / {noise}");
    }

    /// Two runs of the *same* circuit in the *same* domain must not share a
    /// directory either. They do overlap in practice — one fixture backs both a
    /// golden comparison and an analytic cross-check — and a shared directory
    /// makes the first `Drop` delete the second run's working directory, which
    /// ngspice reports as a spawn failure rather than a comparison one.
    #[test]
    fn two_runs_of_one_domain_do_not_share_a_scratch_dir() {
        let first = ScratchDir::create("rc", "ac").unwrap();
        let second = ScratchDir::create("rc", "ac").unwrap();
        assert_ne!(
            first.path(),
            second.path(),
            "concurrent runs of one (circuit, domain) share a working directory"
        );
        let survivor = second.path().to_path_buf();
        drop(first);
        assert!(
            survivor.is_dir(),
            "one run's cleanup removed another's working directory: {}",
            survivor.display()
        );
    }

    /// An export ngspice never wrote fails loud naming the circuit and the
    /// path it was expected at — never a silent empty comparison.
    #[test]
    fn a_missing_export_fails_loud_naming_circuit_and_path() {
        let scratch = ScratchDir::create("ac_rc", "ac").unwrap();
        let err = WrData::read("ac_rc", scratch.path(), "ac_rc_ac", 3)
            .expect_err("a missing export must fail");
        assert!(err.contains("ac_rc"), "names the circuit: {err}");
        assert!(err.contains("ngspice wrote no wrdata export"), "{err}");
        assert!(
            err.contains(&scratch.path().join("ac_rc_ac").display().to_string()),
            "names the expected path: {err}"
        );
    }

    /// The directory exists while the run needs it and is gone afterwards —
    /// including when the read-back failed, which is the path a hand-written
    /// cleanup call is most likely to miss.
    #[test]
    fn the_scratch_dir_is_removed_on_the_failure_path_too() {
        let path = {
            let scratch = ScratchDir::create("temp_diode", "temp").unwrap();
            let path = scratch.path().to_path_buf();
            assert!(path.is_dir(), "the run needs its directory: {}", path.display());
            assert!(
                WrData::read("temp_diode", &path, "temp_diode_op", 2).is_err(),
                "the read-back must fail so this exercises the error path"
            );
            path
        };
        assert!(!path.exists(), "scratch dir leaked: {}", path.display());
    }

    /// Every domain of the contract, so the exhaustiveness checks below cannot
    /// pass by simply forgetting one.
    const EVERY_DOMAIN: [(&str, &ToleranceContract); 11] = [
        ("DC", &ToleranceContract::DC),
        ("DC_CURRENT", &ToleranceContract::DC_CURRENT),
        ("AC_MAG", &ToleranceContract::AC_MAG),
        ("AC_PHASE", &ToleranceContract::AC_PHASE),
        ("NOISE_PSD", &ToleranceContract::NOISE_PSD),
        ("NOISE_TOTAL", &ToleranceContract::NOISE_TOTAL),
        ("TRAN", &ToleranceContract::TRAN),
        ("TEMP", &ToleranceContract::TEMP),
        ("DISTO_HD", &ToleranceContract::DISTO_HD),
        ("FOUR_THD", &ToleranceContract::FOUR_THD),
        ("PZ_POLE", &ToleranceContract::PZ_POLE),
    ];

    /// NGP-07: every domain states why its bound is what it is. A tolerance
    /// with no stated reason is exactly the undocumented constant the
    /// requirement forbids.
    #[test]
    fn every_domain_states_a_rationale() {
        for (name, contract) in EVERY_DOMAIN {
            assert!(
                !contract.rationale().trim().is_empty(),
                "{name} must state why its bound is what it is"
            );
        }
    }

    /// The noise reconciliation must survive in writing, not only in a comment
    /// someone can drop: ngspice's `onoise_spectrum` is an amplitude spectrum
    /// in V/√Hz and has to be squared to reach piperine's V²/Hz PSD. If this
    /// text goes missing, the next reader compares the two directly and reads
    /// a square-order unit error as a broken noise model.
    #[test]
    fn the_noise_psd_rationale_records_the_squaring() {
        let rationale = ToleranceContract::NOISE_PSD.rationale();
        assert!(rationale.contains("V/√Hz"), "names ngspice's unit: {rationale}");
        assert!(rationale.contains("V²/Hz"), "names piperine's unit: {rationale}");
        assert!(
            rationale.to_lowercase().contains("squar"),
            "states that ngspice's spectrum is squared: {rationale}"
        );
    }

    /// No baseline drift: the DC bounds are the values the existing golden
    /// corpus was measured against, and this feature must not move them.
    #[test]
    fn the_dc_bounds_have_not_drifted() {
        assert_eq!(ToleranceContract::DC.reltol, 1e-3);
        assert_eq!(ToleranceContract::DC.abstol, 1e-6);
        assert_eq!(ToleranceContract::DC_CURRENT.reltol, 1e-3);
        assert_eq!(ToleranceContract::DC_CURRENT.abstol, 1e-9);
    }

    /// The contract is `|Δ| ≤ abstol + reltol·max(|a|,|b|)` — inclusive at the
    /// boundary. `AC_PHASE` is the pure-abstol domain, so the boundary is
    /// exactly representable and the check is not a floating-point accident.
    #[test]
    fn tolerance_is_inclusive_exactly_at_the_boundary() {
        let phase = &ToleranceContract::AC_PHASE;
        assert!(phase.holds(0.0, 1e-4), "Δ exactly equal to abstol must hold");
        assert!(
            !phase.holds(0.0, 1e-4 * (1.0 + f64::EPSILON)),
            "one ulp beyond the boundary must fail"
        );
    }

    /// Just inside and just outside the DC bound at a working voltage.
    #[test]
    fn tolerance_separates_just_inside_from_just_outside() {
        // 7.5 V vs 7.51 V: Δ=1e-2 > 1e-6 + 1e-3·7.51 → out.
        assert!(!ToleranceContract::DC.holds(7.5, 7.51));
        // 7.5 V vs 7.507: Δ=7e-3 ≤ 1e-6 + 1e-3·7.507 ≈ 7.508e-3 → in.
        assert!(ToleranceContract::DC.holds(7.5, 7.507));
    }

    /// Near zero the absolute floor governs, so a relative test never divides
    /// by ~0.
    #[test]
    fn the_absolute_floor_governs_near_zero() {
        assert!(ToleranceContract::DC.holds(0.0, 9.9e-7));
        assert!(!ToleranceContract::DC.holds(0.0, 1.1e-6));
    }

    /// The two-column shape shared by DC sweeps, transients and noise spectra.
    #[test]
    fn wrdata_parses_a_two_column_export() {
        let data = WrData::parse("s", "-1.0e0  1.0e-12\n0.5 2e-3\n", 2).unwrap();
        assert_eq!(data.pairs(), vec![(-1.0, 1.0e-12), (0.5, 2e-3)]);
    }

    /// AC is the three-column case: ngspice renders a complex vector as
    /// `freq real imag`, and `complex()` recombines the pair.
    #[test]
    fn wrdata_parses_a_three_column_ac_export() {
        let data = WrData::parse("ac_rc", "1.0e3 0.5 -0.25\n1.0e4 0.1 -0.9\n", 3).unwrap();
        let points = data.complex();
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].0, 1.0e3);
        assert_eq!(points[0].1.re, 0.5);
        assert_eq!(points[0].1.im, -0.25);
        assert_eq!(points[1].1.im, -0.9);
    }

    /// A line whose column count differs from the requested shape fails loud
    /// and quotes the offending line.
    #[test]
    fn wrdata_malformed_line_fails_loud_naming_the_line() {
        let err = WrData::parse("s", "-1.0e0 1.0e-12 3.0\n", 2)
            .expect_err("3 columns must fail a 2-column read");
        assert!(err.contains("malformed wrdata line"), "{err}");
        assert!(err.contains("-1.0e0 1.0e-12 3.0"), "quotes the line: {err}");
    }

    /// The symmetric case: a 2-column line read as AC is equally loud, so a
    /// domain wired to the wrong shape cannot silently compare nothing.
    #[test]
    fn wrdata_wrong_column_count_fails_loud() {
        let err = WrData::parse("ac_rc", "1.0e3 0.5\n", 3)
            .expect_err("2 columns must fail a 3-column read");
        assert!(err.contains("malformed wrdata line (expected 3 columns)"), "{err}");
    }

    /// Numbers are parsed, never skipped.
    #[test]
    fn wrdata_unparseable_number_fails_loud() {
        let err = WrData::parse("s", "-1.0e0 bogus\n", 2).expect_err("non-numeric must fail");
        assert!(err.contains("unparseable wrdata number"), "{err}");
    }

    /// An export with no rows is a contract violation, never an empty pass.
    #[test]
    fn wrdata_empty_export_fails_loud() {
        let err = WrData::parse("s", "\n \n", 2).expect_err("empty must fail");
        assert!(err.contains("empty wrdata export"), "{err}");
    }
}
