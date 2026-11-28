//! Pole-zero analysis (`.pz`) — poles and zeros of a circuit's linearized
//! input→output transfer function, via a generalized eigenvalue problem on
//! the `(G, C)` MNA pencil (`design.md` Algorithm 2).
//!
//! **Descriptor system.** The circuit linearized at the DC operating point is
//! `C·(dx/dt) = −G·x + b·u`, `y = lᵀ·x`, with `G` the real DC Jacobian and
//! `C` the reactive (charge/flux) matrix. Its transfer function is
//! `H(s) = lᵀ(sC + G)⁻¹b`.
//!
//! **`G` extraction.** Exactly the `tf.rs::assemble_dc_stamps` recipe: the
//! assembly's DC stamps at the operating point, matrix entries only (dropping
//! `Rhs`), into a dense `n×n` matrix.
//!
//! **`C` extraction.** Every Piperine AC stamp is affine in `jω`:
//! `Y(jω) = G + jω·C`. One AC probe at `ω0` gives `C = Im(Y(jω0))/ω0`.
//! A second probe at `ω1` guards the assumption: `Im(Y(jω1))/ω1` must match
//! `Im(Y(jω0))/ω0`, and `Re(Y(jω1))` must match `Re(Y(jω0))` — a device whose
//! AC stamp is *not* affine in `jω` (frequency-nonlinear) fails this
//! comparison, and the analysis fails loud (PZ-06) rather than silently
//! mis-extracting `C`.
//!
//! **Poles.** The finite generalized eigenvalues of `(−G, C)` (`s = α/β`
//! from the QZ decomposition's `S_a`/`S_b` factors); `|β| ≈ 0` roots are
//! "at infinity" (a singular `C` — fewer dynamic states than nodes) and are
//! dropped. A circuit with no reactive elements has no finite poles at all,
//! which fails loud (PZ-05) rather than returning an empty success.
//!
//! **Zeros.** The finite generalized eigenvalues of the bordered
//! `(n+1)×(n+1)` Rosenbrock system pencil `([−G, b; lᵀ, 0], [C, 0; 0, 0])` —
//! the textbook transmission-zero definition, exact (no root-search
//! heuristic). An empty zero set is a legitimate answer, unlike poles.

use piperine_analog_solver::netlist::{AnalogReference, AnalogVariable, BranchIdentifier, NodeIdentifier};
use crate::analyses::Context;
use crate::analyses::dc::DcSolver;
use crate::core::circuit::CircuitInstance;
use piperine_analog_solver::error::{Error, SolverDomain};
use piperine_analog_solver::math::linear::Stamp;
use crate::prelude::DcAnalysisResult;

use ndarray::Array2;
use num_complex::Complex;

// ── request/state ────────────────────────────────────────────────────────

/// Pole-zero analysis options: the input excitation and the output
/// measurement defining the transfer function whose poles/zeros are wanted.
/// Same shape as [`TransferFunctionAnalysisOptions`](crate::analyses::tf::TransferFunctionAnalysisOptions).
#[derive(Clone, Debug)]
pub struct PoleZeroOptions {
    /// Input source branch (e.g. a voltage source's branch — the excitation
    /// column `b` gets a unit entry here).
    pub input_source: BranchIdentifier,
    /// Output variable: `AnalogVariable::Node` for `V(out)` or
    /// `AnalogVariable::Branch` for a branch current.
    pub output: AnalogVariable,
    /// Reference node for a differential voltage output
    /// (`V(output) − V(output_ref)`); `None` measures single-ended (GND ref).
    pub output_ref: Option<NodeIdentifier>,
}

// ── driver ───────────────────────────────────────────────────────────────

/// Relative + absolute tolerance for the two-probe linearity guard (PZ-06):
/// `Im(Y(jω1))/ω1` must match `Im(Y(jω0))/ω0`, and likewise for `Re(Y)`,
/// within `guard_rel * scale + guard_abs`.
const GUARD_REL: f64 = 1e-6;
const GUARD_ABS: f64 = 1e-12;

/// Pole-zero solver: extracts the `(G, C)` descriptor-system pencil at the DC
/// operating point, then computes poles ([`Self::poles`]) and zeros
/// ([`Self::zeros`]) as generalized eigenvalues of that pencil.
pub struct PoleZeroSolver<'a> {
    // Held, never read: the exclusive borrow is the point. `new` extracts the
    // whole `(G, C)` pencil up front, so keeping the circuit here is what stops
    // a caller mutating it out from under a pole/zero computation.
    #[allow(dead_code)]
    circuit: &'a mut CircuitInstance,
    size: usize,
    g: Array2<f64>,
    c: Array2<f64>,
    input_ref: AnalogReference,
    output_ref: AnalogReference,
    output_ref_node: Option<AnalogReference>,
}

impl<'a> PoleZeroSolver<'a> {
    /// Builds the solver: solves the DC operating point, resolves the
    /// input/output references, and extracts the dense `(G, C)` pencil
    /// (with the PZ-06 linearity guard). Fails loud when the input/output
    /// are not found in the circuit, or when a device's AC stamp is not
    /// affine in `jω`.
    pub fn new(
        circuit: &'a mut CircuitInstance,
        options: PoleZeroOptions,
        context: Context,
    ) -> crate::core::result::Result<Self> {
        Context::init_global();
        circuit.setup_all(&context)?;

        let dc_point = DcSolver::new(circuit, context.clone())?.solve()?;

        let input_ref = circuit
            .netlist()
            .reference_for(&AnalogVariable::Branch(options.input_source.clone()))
            .ok_or_else(|| {
                Error::simple(
                    SolverDomain::Pz,
                    format!("input source branch '{}' not found", options.input_source.component),
                )
            })?
            .clone();

        let output_ref = circuit
            .netlist()
            .reference_for(&options.output)
            .ok_or_else(|| Error::simple(SolverDomain::Pz, "output variable not found in circuit"))?
            .clone();

        let output_ref_node = if let Some(ref_node) = &options.output_ref {
            Some(
                circuit
                    .netlist()
                    .reference_for(&AnalogVariable::Node(ref_node.clone()))
                    .ok_or_else(|| {
                        Error::simple(SolverDomain::Pz, "output reference node not found in circuit")
                    })?
                    .clone(),
            )
        } else {
            None
        };

        let size = circuit.netlist().max_index().map(|i| i + 1).unwrap_or(0);
        let g = Self::assemble_g(circuit, &dc_point, &context, size)?;
        let c = Self::assemble_c(circuit, &dc_point, &context, size)?;

        Ok(Self { circuit, size, g, c, input_ref, output_ref, output_ref_node })
    }

    /// The real DC Jacobian `G` (n×n), read-only — exposed for testing and
    /// diagnostics.
    pub fn g(&self) -> &Array2<f64> {
        &self.g
    }

    /// The reactive matrix `C` (n×n), read-only.
    pub fn c(&self) -> &Array2<f64> {
        &self.c
    }

    /// The system size `n` (number of MNA unknowns).
    pub fn size(&self) -> usize {
        self.size
    }

    /// Poles of `H(s) = lᵀ(sC + G)⁻¹b`: the finite generalized eigenvalues of
    /// `(−G, C)` (PZ-01), infinite eigenvalues filtered (singular `C` — fewer
    /// dynamic states than nodes), real/conjugate-paired (PZ-03). A circuit
    /// with no reactive elements has no finite poles at all — that is a
    /// fail-loud condition (PZ-05), not an empty success.
    pub fn poles(&self) -> crate::core::result::Result<Vec<Complex<f64>>> {
        let neg_g = self.g.mapv(|v| -v);
        let roots = Self::finite_generalized_eigenvalues(&neg_g, &self.c, self.size)?;
        if roots.is_empty() {
            return Err(Error::simple(
                SolverDomain::Pz,
                "circuit has no reactive elements (C is structurally zero) — no finite poles exist",
            ));
        }
        Ok(roots)
    }

    /// Transmission zeros of `H(s) = lᵀ(sC + G)⁻¹b`: the finite generalized
    /// eigenvalues of the bordered `(n+1)×(n+1)` Rosenbrock system pencil
    /// (PZ-02)
    ///
    /// ```text
    /// A' = [ −G   b ]      B' = [ C   0 ]
    ///      [  lᵀ  0 ]           [ 0   0 ]
    /// ```
    ///
    /// `b` is a unit excitation at the input branch's row; `l` is a unit
    /// selector at the output's row (minus a unit at the differential
    /// reference's row, when one is given). Unlike [`Self::poles`], an empty
    /// zero set is a legitimate answer (e.g. a plain RC low-pass has no
    /// transmission zero) — not a fail-loud condition.
    pub fn zeros(&self) -> crate::core::result::Result<Vec<Complex<f64>>> {
        let n = self.size;
        let bordered = n + 1;
        let mut a = Array2::<f64>::zeros((bordered, bordered));
        let mut b = Array2::<f64>::zeros((bordered, bordered));
        for i in 0..n {
            for j in 0..n {
                a[[i, j]] = -self.g[[i, j]];
                b[[i, j]] = self.c[[i, j]];
            }
        }
        if let Some(idx) = self.input_ref.idx() {
            a[[idx, n]] = 1.0;
        }
        if let Some(idx) = self.output_ref.idx() {
            a[[n, idx]] = 1.0;
        }
        if let Some(ref_node) = &self.output_ref_node
            && let Some(idx) = ref_node.idx()
        {
            a[[n, idx]] -= 1.0;
        }
        Self::finite_generalized_eigenvalues(&a, &b, bordered)
    }

    /// Finite generalized eigenvalues of the pencil `(a, b)`: `s = α/β` for
    /// every `(α, β)` pair the QZ decomposition returns, dropping "at
    /// infinity" roots (`|β| ≈ 0`, PZ-01/edge case), snapping near-real
    /// roots to the real axis, and sorting for determinism (real matrices
    /// give conjugate pairs directly from the QZ solve, so a stable sort by
    /// `(Re, Im)` keeps each pair adjacent — PZ-03).
    fn finite_generalized_eigenvalues(
        a: &Array2<f64>,
        b: &Array2<f64>,
        size: usize,
    ) -> crate::core::result::Result<Vec<Complex<f64>>> {
        if size == 0 {
            return Ok(Vec::new());
        }
        const TOL_INFINITE: f64 = 1e-9;
        const TOL_REAL: f64 = 1e-9;

        if a.iter().chain(b.iter()).any(|entry| !entry.is_finite()) {
            return Err(Error::simple(
                SolverDomain::Pz,
                "the pencil holds an infinite or undefined entry, so its poles and zeros are undefined",
            ));
        }
        let a_mat = faer::Mat::from_fn(size, size, |i, j| a[[i, j]]);
        let b_mat = faer::Mat::from_fn(size, size, |i, j| b[[i, j]]);
        let evd = a_mat.generalized_eigen(&b_mat).map_err(|e| {
            Error::simple(SolverDomain::Pz, format!("QZ generalized eigenvalue solve failed: {e:?}"))
        })?;

        let mut roots = Vec::new();
        for (alpha, beta) in evd.S_a().column_vector().iter().zip(evd.S_b().column_vector().iter()) {
            if beta.norm() <= TOL_INFINITE * (alpha.norm() + beta.norm()) {
                continue; // root at infinity — singular C / algebraic constraint row
            }
            let mut s = alpha / beta;
            if !s.re.is_finite() || !s.im.is_finite() {
                continue;
            }
            if s.im.abs() < TOL_REAL * s.norm().max(1.0) {
                s.im = 0.0;
            }
            roots.push(s);
        }
        roots.sort_by(|x, y| x.re.total_cmp(&y.re).then(x.im.total_cmp(&y.im)));
        Ok(roots)
    }

    /// `G`: the assembly's DC matrix stamps at the operating point (dropping
    /// `Rhs`) into a dense matrix — exactly the `tf.rs::assemble_dc_stamps`
    /// recipe.
    fn assemble_g(
        circuit: &mut CircuitInstance,
        dc_point: &DcAnalysisResult,
        context: &Context,
        size: usize,
    ) -> crate::core::result::Result<Array2<f64>> {
        let netlist = circuit.netlist();
        let dc_values_iv = netlist.initial_values(dc_point.values());
        let mut dc_state_array = ndarray::Array1::zeros(size);
        for iv in dc_values_iv {
            if let Some(idx) = iv.reference.idx() {
                dc_state_array[idx] = iv.value;
            }
        }

        let mut g = Array2::<f64>::zeros((size, size));
        let bound = &mut circuit.abi;
        let amb = piperine_analog_solver::device::Ambient {
            time: 0.0,
            freq: 0.0,
            is_static: true,
            gmin: context.tolerances.gmin,
            source_scale: 1.0,
        };
        for stamp in bound.analog.assemble_dc(&dc_state_array.to_vec(), amb)? {
            if let Stamp::Matrix(r, c, val) = stamp
                && let (Some(ri), Some(ci)) = (r.idx(), c.idx())
            {
                g[[ri, ci]] += val;
            }
        }
        Ok(g)
    }

    /// One AC probe: the assembly's AC matrix stamps at angular frequency
    /// `omega` into a dense complex matrix `Y(jω)`.
    fn assemble_y(
        circuit: &mut CircuitInstance,
        dc_point: &DcAnalysisResult,
        omega: f64,
        context: &Context,
        size: usize,
    ) -> crate::core::result::Result<Array2<Complex<f64>>> {
        let mut y = Array2::<Complex<f64>>::zeros((size, size));
        let op = crate::analyses::ac::AcSystem::operating_vector(circuit, dc_point);
        let amb = piperine_analog_solver::device::Ambient {
            time: 0.0,
            freq: omega / (2.0 * std::f64::consts::PI),
            is_static: false,
            gmin: context.tolerances.gmin,
            source_scale: 1.0,
        };
        let bound = &mut circuit.abi;
        for stamp in bound.analog.assemble_ac(&op, amb)?.0 {
            if let Stamp::Matrix(r, c, val) = stamp
                && let (Some(ri), Some(ci)) = (r.idx(), c.idx())
            {
                y[[ri, ci]] += val;
            }
        }
        Ok(y)
    }

    /// `C = Im(Y(jω0))/ω0`, guarded by a second probe at `ω1` (PZ-06): every
    /// entry of `Im(Y)/ω` and `Re(Y)` must agree between the two probes
    /// within tolerance, or the AC stamp is not affine in `jω` and the
    /// pencil cannot be trusted — fails loud, naming the offending entry.
    fn assemble_c(
        circuit: &mut CircuitInstance,
        dc_point: &DcAnalysisResult,
        context: &Context,
        size: usize,
    ) -> crate::core::result::Result<Array2<f64>> {
        let omega0 = 1.0;
        let omega1 = 2.0;
        let y0 = Self::assemble_y(circuit, dc_point, omega0, context, size)?;
        let y1 = Self::assemble_y(circuit, dc_point, omega1, context, size)?;

        let mut c = Array2::<f64>::zeros((size, size));
        for i in 0..size {
            for j in 0..size {
                let c0 = y0[[i, j]].im / omega0;
                let c1 = y1[[i, j]].im / omega1;
                let c_scale = c0.abs().max(c1.abs());
                if (c0 - c1).abs() > GUARD_REL * c_scale + GUARD_ABS {
                    return Err(Error::simple(
                        SolverDomain::Pz,
                        format!(
                            "AC stamp at MNA entry ({i},{j}) is not affine in jω \
                             (Im(Y)/ω differs between probe frequencies) — cannot \
                             extract a (G,C) pencil for .pz"
                        ),
                    ));
                }
                let g0 = y0[[i, j]].re;
                let g1 = y1[[i, j]].re;
                let g_scale = g0.abs().max(g1.abs());
                if (g0 - g1).abs() > GUARD_REL * g_scale + GUARD_ABS {
                    return Err(Error::simple(
                        SolverDomain::Pz,
                        format!(
                            "AC stamp at MNA entry ({i},{j}) is not affine in jω \
                             (Re(Y) differs between probe frequencies) — cannot \
                             extract a (G,C) pencil for .pz"
                        ),
                    ));
                }
                c[[i, j]] = c0;
            }
        }
        Ok(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use piperine_analog_solver::netlist::NodeIdentifier;
    use crate::analyses::test_devices::AnalysisDevices;
    use crate::core::builder::CircuitBuilder;
    use crate::core::circuit::CircuitInstance;

    

    /// The circuits the pole-zero tests solve.
    struct PoleZeroCircuits;

    impl PoleZeroCircuits {
        /// `v1(in,gnd) --R-- out --C-- gnd`: a source-driven single-pole RC
        /// low-pass — `.pz`'s canonical single-pole worked example. `V(in)` is
        /// pinned by the ideal source, so `H(s) = V(out)/V(in) = 1/(RCs + 1)`,
        /// pole at `s = −1/(RC)`.
        fn rc_circuit_with_source(r: f64, c: f64) -> (CircuitInstance, PoleZeroOptions) {
            let mut b = CircuitBuilder::new("rc");
            let n_in = b.node("n0");
            let n_out = b.node("n1");
            let gnd = b.ground();
            AnalysisDevices::vsrc(&mut b, "v1", &n_in, &gnd, 1.0);
            AnalysisDevices::resistor(&mut b, "r", &n_in, &n_out, r);
            AnalysisDevices::capacitor(&mut b, "c", &n_out, &gnd, c);
            let circuit = b.build().unwrap();
            let options = PoleZeroOptions {
                input_source: BranchIdentifier::new("v1", "f"),
                output: AnalogVariable::Node(NodeIdentifier::Anonymous(1)),
                output_ref: None,
            };
            (circuit, options)
        }

        /// `v1(in,gnd) --R-- a --L-- b --C-- gnd`: a source-driven series RLC —
        /// `.pz`'s canonical complex-conjugate-pair worked example, poles at
        /// `−R/(2L) ± j·sqrt(1/(LC) − (R/(2L))²)`.
        fn rlc_circuit_with_source(r: f64, l: f64, c: f64) -> (CircuitInstance, PoleZeroOptions) {
            let mut b = CircuitBuilder::new("rlc");
            let n_in = b.node("n0");
            let n_a = b.node("n1");
            let n_b = b.node("n2");
            let gnd = b.ground();
            AnalysisDevices::vsrc(&mut b, "v1", &n_in, &gnd, 1.0);
            AnalysisDevices::resistor(&mut b, "r", &n_in, &n_a, r);
            AnalysisDevices::inductor(&mut b, "l1", &n_a, &n_b, l);
            AnalysisDevices::capacitor(&mut b, "c", &n_b, &gnd, c);
            let circuit = b.build().unwrap();
            let options = PoleZeroOptions {
                input_source: BranchIdentifier::new("v1", "f"),
                output: AnalogVariable::Node(NodeIdentifier::Anonymous(2)),
                output_ref: None,
            };
            (circuit, options)
        }

        /// `v1(in,gnd) --R-- gnd`: a purely resistive network (no reactive
        /// element anywhere) — `.pz`'s PZ-05 fail-loud worked example.
        fn resistor_only_circuit(r: f64) -> (CircuitInstance, PoleZeroOptions) {
            let mut b = CircuitBuilder::new("ronly");
            let n_in = b.node("n0");
            let gnd = b.ground();
            AnalysisDevices::vsrc(&mut b, "v1", &n_in, &gnd, 1.0);
            AnalysisDevices::resistor(&mut b, "r", &n_in, &gnd, r);
            let circuit = b.build().unwrap();
            let options = PoleZeroOptions {
                input_source: BranchIdentifier::new("v1", "f"),
                output: AnalogVariable::Node(NodeIdentifier::Anonymous(0)),
                output_ref: None,
            };
            (circuit, options)
        }

        /// `v1(in,gnd) --R1‖C-- out --R2-- gnd`: a lead/lag network — `R1` and
        /// `C` in parallel between `in` and `out`, `R2` from `out` to gnd.
        /// `H(s) = (1/R1 + sC) / (1/R1 + 1/R2 + sC)`: zero at `s = −1/(R1·C)`,
        /// pole at `s = −1/((R1‖R2)·C)` — `.pz`'s canonical worked example with
        /// a known finite zero (PZ-02/04).
        fn lag_network_circuit(r1: f64, r2: f64, c: f64) -> (CircuitInstance, PoleZeroOptions) {
            let mut b = CircuitBuilder::new("lag");
            let n_in = b.node("n0");
            let n_out = b.node("n1");
            let gnd = b.ground();
            AnalysisDevices::vsrc(&mut b, "v1", &n_in, &gnd, 1.0);
            AnalysisDevices::resistor(&mut b, "r1", &n_in, &n_out, r1);
            AnalysisDevices::capacitor(&mut b, "c", &n_in, &n_out, c);
            AnalysisDevices::resistor(&mut b, "r2", &n_out, &gnd, r2);
            let circuit = b.build().unwrap();
            let options = PoleZeroOptions {
                input_source: BranchIdentifier::new("v1", "f"),
                output: AnalogVariable::Node(NodeIdentifier::Anonymous(1)),
                output_ref: None,
            };
            (circuit, options)
        }

        /// `in --Rin-- gnd`, `in --R-- out`, `out --C-- gnd`: a 2-unknown RC
        /// network with a well-posed (nonsingular) DC point and a clean,
        /// hand-computable `(G, C)` pencil.
        fn rc_circuit(r_in: f64, r: f64, c: f64) -> (CircuitInstance, AnalogReference, AnalogReference) {
            let mut b = CircuitBuilder::new("rc");
            let n_in = b.node("n0");
            let n_out = b.node("n1");
            let gnd = b.ground();
            AnalysisDevices::resistor(&mut b, "rin", &n_in, &gnd, r_in);
            AnalysisDevices::resistor(&mut b, "r", &n_in, &n_out, r);
            AnalysisDevices::capacitor(&mut b, "c", &n_out, &gnd, c);
            let circuit = b.build().unwrap();
            (circuit, n_in, n_out)
        }
    }





    #[test]
    fn assembles_known_2x2_g_and_c_on_an_rc_network() {
        let (mut circuit, n_in, n_out) = PoleZeroCircuits::rc_circuit(2000.0, 1000.0, 1e-6);
        let context = Context::default();
        circuit.setup_all(&context).unwrap();
        let dc_point = DcSolver::new(&mut circuit, context.clone()).unwrap().solve().unwrap();
        let size = circuit.netlist().max_index().map(|i| i + 1).unwrap_or(0);
        assert_eq!(size, 2);

        let g = PoleZeroSolver::assemble_g(&mut circuit, &dc_point, &context, size).expect("devices evaluate");
        let c = PoleZeroSolver::assemble_c(&mut circuit, &dc_point, &context, size).expect("linear AC stamps");

        let i_in = n_in.idx().unwrap();
        let i_out = n_out.idx().unwrap();

        let g_in_in = 1.0 / 2000.0 + 1.0 / 1000.0;
        let g_cross = -1.0 / 1000.0;
        let g_out_out = 1.0 / 1000.0;
        assert!((g[[i_in, i_in]] - g_in_in).abs() < 1e-9, "G[in,in] = {}", g[[i_in, i_in]]);
        assert!((g[[i_in, i_out]] - g_cross).abs() < 1e-9);
        assert!((g[[i_out, i_in]] - g_cross).abs() < 1e-9);
        assert!((g[[i_out, i_out]] - g_out_out).abs() < 1e-9, "G[out,out] = {}", g[[i_out, i_out]]);

        assert!((c[[i_out, i_out]] - 1e-6).abs() < 1e-15, "C[out,out] = {}", c[[i_out, i_out]]);
        assert!(c[[i_in, i_in]].abs() < 1e-15, "C[in,in] should be 0 (no reactance there)");
        assert!(c[[i_in, i_out]].abs() < 1e-15);
    }

    #[test]
    fn freq_nonlinear_ac_stamp_fails_loud_pz06() {
        let mut b = CircuitBuilder::new("bad");
        let n1 = b.node("n0");
        let n2 = b.node("n1");
        let gnd = b.ground();
        AnalysisDevices::resistor(&mut b, "r1", &n1, &gnd, 1000.0);
        AnalysisDevices::resistor(&mut b, "r2", &n2, &gnd, 1000.0);
        AnalysisDevices::freq_nonlinear(&mut b, "bad", &n1, &n2, 1.0);
        let mut circuit = b.build().unwrap();
        let context = Context::default();
        circuit.setup_all(&context).unwrap();
        let dc_point = DcSolver::new(&mut circuit, context.clone()).unwrap().solve().unwrap();
        let size = circuit.netlist().max_index().map(|i| i + 1).unwrap_or(0);

        let result = PoleZeroSolver::assemble_c(&mut circuit, &dc_point, &context, size);
        assert!(result.is_err(), "frequency-nonlinear AC stamp must fail loud (PZ-06)");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("not affine in jω"), "error should name the linearity guard: {msg}");
    }

    // ── T4: poles via QZ ────────────────────────────────────────────────

    #[test]
    fn rc_low_pass_has_one_real_pole_at_minus_one_over_rc() {
        let (mut circuit, options) = PoleZeroCircuits::rc_circuit_with_source(1000.0, 1e-6);
        let solver = PoleZeroSolver::new(&mut circuit, options, Context::default()).unwrap();
        let poles = solver.poles().expect("RC has one finite pole");

        assert_eq!(poles.len(), 1, "RC low-pass should have exactly one finite pole: {poles:?}");
        let expected = -1.0 / (1000.0 * 1e-6); // -1000 rad/s
        assert!(poles[0].im == 0.0, "pole should be snapped real: {:?}", poles[0]);
        let rel_err = (poles[0].re - expected).abs() / expected.abs();
        assert!(rel_err < 1e-6, "pole = {:?}, expected {expected}, rel_err = {rel_err}", poles[0]);
    }

    #[test]
    fn series_rlc_has_complex_conjugate_pole_pair() {
        let (r, l, c) = (10.0, 1e-3, 1e-6);
        let (mut circuit, options) = PoleZeroCircuits::rlc_circuit_with_source(r, l, c);
        let solver = PoleZeroSolver::new(&mut circuit, options, Context::default()).unwrap();
        let poles = solver.poles().expect("series RLC has a conjugate pole pair");

        assert_eq!(poles.len(), 2, "series RLC should have exactly two finite poles: {poles:?}");
        let sigma = -r / (2.0 * l);
        let omega_d = (1.0 / (l * c) - (r / (2.0 * l)).powi(2)).sqrt();

        // Sorted by (Re, Im): the negative-imaginary root comes first.
        let (p_minus, p_plus) = (poles[0], poles[1]);
        assert!(p_minus.im < 0.0 && p_plus.im > 0.0, "expected a conjugate pair: {poles:?}");
        assert!((p_minus.re - sigma).abs() / sigma.abs() < 1e-6, "Re(pole) = {}, expected {sigma}", p_minus.re);
        assert!((p_plus.re - sigma).abs() / sigma.abs() < 1e-6);
        assert!(
            (p_plus.im - omega_d).abs() / omega_d < 1e-6,
            "Im(pole) = {}, expected {omega_d}",
            p_plus.im
        );
        assert!((p_minus.im + omega_d).abs() / omega_d < 1e-6, "conjugate: Im = {}", p_minus.im);
    }

    #[test]
    fn resistor_only_circuit_fails_loud_pz05() {
        let (mut circuit, options) = PoleZeroCircuits::resistor_only_circuit(1000.0);
        let solver = PoleZeroSolver::new(&mut circuit, options, Context::default()).unwrap();
        let result = solver.poles();
        assert!(result.is_err(), "a purely resistive network has no finite poles (PZ-05)");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("no reactive elements") || msg.contains("no finite poles"), "{msg}");
    }

    // ── T5: zeros via Rosenbrock pencil ─────────────────────────────────

    #[test]
    fn lag_network_has_the_known_pole_and_zero() {
        let (r1, r2, c) = (1000.0, 2000.0, 1e-6);
        let (mut circuit, options) = PoleZeroCircuits::lag_network_circuit(r1, r2, c);
        let solver = PoleZeroSolver::new(&mut circuit, options, Context::default()).unwrap();

        let poles = solver.poles().expect("lag network has one finite pole");
        let r_parallel = 1.0 / (1.0 / r1 + 1.0 / r2);
        let expected_pole = -1.0 / (r_parallel * c);
        assert_eq!(poles.len(), 1, "expected exactly one finite pole: {poles:?}");
        assert!(poles[0].im == 0.0, "pole should be real: {:?}", poles[0]);
        assert!(
            (poles[0].re - expected_pole).abs() / expected_pole.abs() < 1e-6,
            "pole = {:?}, expected {expected_pole}",
            poles[0]
        );

        let zeros = solver.zeros().expect("Rosenbrock pencil solves");
        let expected_zero = -1.0 / (r1 * c);
        assert_eq!(zeros.len(), 1, "expected exactly one finite zero: {zeros:?}");
        assert!(zeros[0].im == 0.0, "zero should be real: {:?}", zeros[0]);
        assert!(
            (zeros[0].re - expected_zero).abs() / expected_zero.abs() < 1e-6,
            "zero = {:?}, expected {expected_zero}",
            zeros[0]
        );
    }

    #[test]
    fn rc_low_pass_has_no_transmission_zero() {
        // A plain RC low-pass has one pole and no finite zero — zeros() must
        // return an empty (not an error) result, unlike poles().
        let (mut circuit, options) = PoleZeroCircuits::rc_circuit_with_source(1000.0, 1e-6);
        let solver = PoleZeroSolver::new(&mut circuit, options, Context::default()).unwrap();
        let zeros = solver.zeros().expect("empty zero set is a legitimate answer, not an error");
        assert!(zeros.is_empty(), "RC low-pass should have no finite transmission zero: {zeros:?}");
    }

    #[test]
    fn finite_generalized_eigenvalues_pairs_conjugates() {
        // Shared by both poles() and zeros() (PZ-03): a hand-built pencil
        // with B = I reduces to a plain eigenvalue problem. Char. poly of
        // [[0,1],[-1,-1]] is λ² + λ + 1 = 0 -> λ = -1/2 ± j·√3/2, the
        // textbook complex-conjugate pair. Padded to 3×3 (an extra decoupled
        // real root) — faer 0.23.2's real-GEVD path has a scratch-sizing
        // edge case at bare n=2 with a complex pair; n=3 sidesteps it and is
        // just as valid a probe of the shared post-processing.
        Context::init_global();
        let a = ndarray::arr2(&[[0.0, 1.0, 0.0], [-1.0, -1.0, 0.0], [0.0, 0.0, -5.0]]);
        let b = ndarray::arr2(&[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        let roots = PoleZeroSolver::finite_generalized_eigenvalues(&a, &b, 3).unwrap();

        // Sorted by (Re, Im): the decoupled real root (-5) comes first, then
        // the conjugate pair (negative-imaginary member first).
        assert_eq!(roots.len(), 3, "{roots:?}");
        assert!((roots[0].re - (-5.0)).abs() < 1e-9 && roots[0].im == 0.0, "real root: {:?}", roots[0]);

        let (p_minus, p_plus) = (roots[1], roots[2]);
        assert!(p_minus.im < 0.0 && p_plus.im > 0.0, "expected a sorted conjugate pair: {roots:?}");
        let (expected_re, expected_im) = (-0.5, (3.0_f64).sqrt() / 2.0);
        assert!((p_minus.re - expected_re).abs() < 1e-9, "Re(root) = {}, expected {expected_re}", p_minus.re);
        assert!((p_plus.re - expected_re).abs() < 1e-9, "Re(root) = {}, expected {expected_re}", p_plus.re);
        assert!((p_plus.im - expected_im).abs() < 1e-9);
        assert!((p_minus.im + expected_im).abs() < 1e-9, "not a conjugate pair: {roots:?}");
    }

    #[test]
    fn finite_generalized_eigenvalues_snaps_real_roots() {
        // Shared by both poles() and zeros() (PZ-03): a real diagonal
        // pencil's roots must land exactly on the real axis (im == 0.0),
        // not with QZ-solver floating-point noise in the imaginary part.
        Context::init_global();
        let a = ndarray::arr2(&[[-2.0, 0.0], [0.0, -3.0]]);
        let b = ndarray::arr2(&[[1.0, 0.0], [0.0, 1.0]]);
        let roots = PoleZeroSolver::finite_generalized_eigenvalues(&a, &b, 2).unwrap();

        assert_eq!(roots.len(), 2, "{roots:?}");
        for r in &roots {
            assert_eq!(r.im, 0.0, "expected exact real snap: {r:?}");
        }
        let mut re: Vec<f64> = roots.iter().map(|r| r.re).collect();
        re.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!((re[0] - (-3.0)).abs() < 1e-9);
        assert!((re[1] - (-2.0)).abs() < 1e-9);
    }

    #[test]
    fn a_matrix_with_an_infinite_entry_is_refused() {
        Context::init_global();
        let a = ndarray::arr2(&[[-2.0, 0.0], [0.0, f64::INFINITY]]);
        let b = ndarray::arr2(&[[1.0, 0.0], [0.0, 1.0]]);
        match PoleZeroSolver::finite_generalized_eigenvalues(&a, &b, 2) {
            Err(Error::Simple { domain: SolverDomain::Pz, detail }) => assert!(detail.contains("infinite"), "{detail}"),
            other => panic!("an infinite entry is refused in the Pz domain: {other:?}"),
        }
    }
}
