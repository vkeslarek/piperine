//! Pole-zero analysis (`.pz`) — poles and zeros of a circuit's linearized
//! input→output transfer function, via a generalized eigenvalue problem on
//! the `(G, C)` MNA pencil (`design.md` Algorithm 2).
//!
//! **Descriptor system.** The circuit linearized at the DC operating point is
//! `C·(dx/dt) = −G·x + b·u`, `y = lᵀ·x`, with `G` the real DC Jacobian and
//! `C` the reactive (charge/flux) matrix. Its transfer function is
//! `H(s) = lᵀ(sC + G)⁻¹b`.
//!
//! **`G` extraction.** Exactly the `tf.rs::assemble_dc_stamps` recipe: update
//! every device at the DC point, collect `load_dc` matrix stamps (dropping
//! `Rhs`), into a dense `n×n` matrix.
//!
//! **`C` extraction.** Every Piperine AC stamp is affine in `jω`:
//! `Y(jω) = G + jω·C`. One `load_ac` probe at `ω0` gives `C = Im(Y(jω0))/ω0`.
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
#![allow(dead_code)]

use crate::analog::{AnalogReference, AnalogVariable, BranchIdentifier, NodeIdentifier};
use crate::analyses::Context;
use crate::analyses::ac::AcAnalysisContext;
use crate::analyses::dc::{DcAnalysisState, DcSolver};
use crate::core::circuit::CircuitInstance;
use crate::error::{Error, SolverDomain};
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::linear::Stamp;
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
    #[allow(dead_code)]
    circuit: &'a mut CircuitInstance,
    options: PoleZeroOptions,
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
    ) -> crate::result::Result<Self> {
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
        let g = Self::assemble_g(circuit, &dc_point, &context, size);
        let c = Self::assemble_c(circuit, &dc_point, &context, size)?;

        Ok(Self { circuit, options, size, g, c, input_ref, output_ref, output_ref_node })
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
    pub fn poles(&self) -> crate::result::Result<Vec<Complex<f64>>> {
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
    pub fn zeros(&self) -> crate::result::Result<Vec<Complex<f64>>> {
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
    ) -> crate::result::Result<Vec<Complex<f64>>> {
        if size == 0 {
            return Ok(Vec::new());
        }
        const TOL_INFINITE: f64 = 1e-9;
        const TOL_REAL: f64 = 1e-9;

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
        roots.sort_by(|x, y| x.re.partial_cmp(&y.re).unwrap().then(x.im.partial_cmp(&y.im).unwrap()));
        Ok(roots)
    }

    /// `G`: update every device at the DC point, collect `load_dc` matrix
    /// stamps (dropping `Rhs`) into a dense matrix — exactly the
    /// `tf.rs::assemble_dc_stamps` recipe.
    fn assemble_g(
        circuit: &mut CircuitInstance,
        dc_point: &DcAnalysisResult,
        context: &Context,
        size: usize,
    ) -> Array2<f64> {
        let netlist = circuit.netlist();
        let dc_values_iv = netlist.initial_values(dc_point.values());
        let mut dc_state_array = ndarray::Array1::zeros(size);
        for iv in dc_values_iv {
            if let Some(idx) = iv.reference.idx() {
                dc_state_array[idx] = iv.value;
            }
        }

        let mut state = CircularArrayBuffer2::new(1, size);
        state.push(&dc_state_array.view());
        circuit.update_all(&state, context);

        let mut g = Array2::<f64>::zeros((size, size));
        let CircuitInstance { devices, digital_state, .. } = &mut *circuit;
        let dc_state = DcAnalysisState::new(&state, &digital_state.nets, 1.0);
        for dev in devices.iter_mut() {
            for stamp in dev.load_dc(&dc_state, context) {
                if let Stamp::Matrix(r, c, val) = stamp
                    && let (Some(ri), Some(ci)) = (r.idx(), c.idx())
                {
                    g[[ri, ci]] += val;
                }
            }
        }
        g
    }

    /// One AC probe: collects every device's `load_ac` matrix stamps at
    /// angular frequency `omega` into a dense complex matrix `Y(jω)`.
    fn assemble_y(
        circuit: &mut CircuitInstance,
        dc_point: &DcAnalysisResult,
        omega: f64,
        context: &Context,
        size: usize,
    ) -> Array2<Complex<f64>> {
        let ac_ctx = AcAnalysisContext { frequency: omega / (2.0 * std::f64::consts::PI) };
        let mut y = Array2::<Complex<f64>>::zeros((size, size));
        for dev in circuit.devices.iter_mut() {
            for stamp in dev.load_ac(dc_point, &ac_ctx, context) {
                if let Stamp::Matrix(r, c, val) = stamp
                    && let (Some(ri), Some(ci)) = (r.idx(), c.idx())
                {
                    y[[ri, ci]] += val;
                }
            }
        }
        y
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
    ) -> crate::result::Result<Array2<f64>> {
        let omega0 = 1.0;
        let omega1 = 2.0;
        let y0 = Self::assemble_y(circuit, dc_point, omega0, context, size);
        let y1 = Self::assemble_y(circuit, dc_point, omega1, context, size);

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
    use crate::analog::{Netlist, NodeIdentifier};
    use crate::core::element::{AnalogDevice, DigitalDevice, Element, ElementCapabilities, Introspect};
    use crate::core::circuit::CircuitInstance;
    use num_complex::Complex64;

    // ── test doubles ─────────────────────────────────────────────────────

    struct TestResistor {
        n1: AnalogReference,
        n2: AnalogReference,
        r: f64,
    }
    impl AnalogDevice for TestResistor {
        fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
            let g = 1.0 / self.r;
            vec![
                Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
                Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
                Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
                Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
            ]
        }
    }
    impl DigitalDevice for TestResistor {}
    impl Introspect for TestResistor {}
    impl Element for TestResistor {
        fn name(&self) -> &str {
            "r"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
        }
    }

    struct TestCapacitor {
        n1: AnalogReference,
        n2: AnalogReference,
        c: f64,
    }
    impl AnalogDevice for TestCapacitor {
        fn load_ac(
            &mut self,
            _dc: &DcAnalysisResult,
            ac_ctx: &AcAnalysisContext,
            _ctx: &Context,
        ) -> Vec<Stamp<AnalogReference, Complex64>> {
            let omega = 2.0 * std::f64::consts::PI * ac_ctx.frequency;
            let y = Complex64::new(0.0, omega * self.c);
            vec![
                Stamp::Matrix(self.n1.clone(), self.n1.clone(), y),
                Stamp::Matrix(self.n2.clone(), self.n2.clone(), y),
                Stamp::Matrix(self.n1.clone(), self.n2.clone(), -y),
                Stamp::Matrix(self.n2.clone(), self.n1.clone(), -y),
            ]
        }
    }
    impl DigitalDevice for TestCapacitor {}
    impl Introspect for TestCapacitor {}
    impl Element for TestCapacitor {
        fn name(&self) -> &str {
            "c"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_AC
        }
    }

    /// A device whose AC stamp is quadratic in ω (`jω²C`, not `jωC`) —
    /// deliberately breaks the affine-in-jω assumption to exercise PZ-06.
    struct FreqNonlinearDevice {
        n1: AnalogReference,
        n2: AnalogReference,
        k: f64,
    }
    impl AnalogDevice for FreqNonlinearDevice {
        fn load_ac(
            &mut self,
            _dc: &DcAnalysisResult,
            ac_ctx: &AcAnalysisContext,
            _ctx: &Context,
        ) -> Vec<Stamp<AnalogReference, Complex64>> {
            let omega = 2.0 * std::f64::consts::PI * ac_ctx.frequency;
            let y = Complex64::new(0.0, omega * omega * self.k);
            vec![Stamp::Matrix(self.n1.clone(), self.n2.clone(), y)]
        }
    }
    impl DigitalDevice for FreqNonlinearDevice {}
    impl Introspect for FreqNonlinearDevice {}
    impl Element for FreqNonlinearDevice {
        fn name(&self) -> &str {
            "bad"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_AC
        }
    }

    /// An ideal DC voltage source between `p` and `n`, MNA branch-unknown
    /// style (mirrors `core::builder::tests::TestVsource`).
    struct TestVoltageSource {
        p: AnalogReference,
        n: AnalogReference,
        branch: AnalogReference,
        v: f64,
    }
    impl AnalogDevice for TestVoltageSource {
        fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
            let b = self.branch.clone();
            vec![
                Stamp::Matrix(self.p.clone(), b.clone(), 1.0),
                Stamp::Matrix(b.clone(), self.p.clone(), 1.0),
                Stamp::Matrix(self.n.clone(), b.clone(), -1.0),
                Stamp::Matrix(b.clone(), self.n.clone(), -1.0),
                Stamp::Rhs(b, self.v),
            ]
        }
    }
    impl DigitalDevice for TestVoltageSource {}
    impl Introspect for TestVoltageSource {}
    impl Element for TestVoltageSource {
        fn name(&self) -> &str {
            "v1"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
        }
    }

    /// An ideal inductor between `p` and `n`: a DC short (branch constraint
    /// `V(p) − V(n) = 0`) whose AC branch row adds `−jωL` on the branch
    /// unknown (`V(p) − V(n) − jωL·I = 0`).
    struct TestInductor {
        p: AnalogReference,
        n: AnalogReference,
        branch: AnalogReference,
        l: f64,
    }
    impl AnalogDevice for TestInductor {
        fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
            let b = self.branch.clone();
            vec![
                Stamp::Matrix(self.p.clone(), b.clone(), 1.0),
                Stamp::Matrix(b.clone(), self.p.clone(), 1.0),
                Stamp::Matrix(self.n.clone(), b.clone(), -1.0),
                Stamp::Matrix(b.clone(), self.n.clone(), -1.0),
            ]
        }
        fn load_ac(
            &mut self,
            _dc: &DcAnalysisResult,
            ac_ctx: &AcAnalysisContext,
            _ctx: &Context,
        ) -> Vec<Stamp<AnalogReference, Complex64>> {
            let omega = 2.0 * std::f64::consts::PI * ac_ctx.frequency;
            let b = self.branch.clone();
            let one = Complex64::new(1.0, 0.0);
            vec![
                Stamp::Matrix(self.p.clone(), b.clone(), one),
                Stamp::Matrix(b.clone(), self.p.clone(), one),
                Stamp::Matrix(self.n.clone(), b.clone(), -one),
                Stamp::Matrix(b.clone(), self.n.clone(), -one),
                Stamp::Matrix(b.clone(), b, Complex64::new(0.0, -omega * self.l)),
            ]
        }
    }
    impl DigitalDevice for TestInductor {}
    impl Introspect for TestInductor {}
    impl Element for TestInductor {
        fn name(&self) -> &str {
            "l1"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::LOADS_AC
        }
    }

    /// `v1(in,gnd) --R-- out --C-- gnd`: a source-driven single-pole RC
    /// low-pass — `.pz`'s canonical single-pole worked example. `V(in)` is
    /// pinned by the ideal source, so `H(s) = V(out)/V(in) = 1/(RCs + 1)`,
    /// pole at `s = −1/(RC)`.
    fn rc_circuit_with_source(r: f64, c: f64) -> (CircuitInstance, PoleZeroOptions) {
        let mut netlist = Netlist::new();
        let n_in = netlist.connect_node(NodeIdentifier::Anonymous(0));
        let n_out = netlist.connect_node(NodeIdentifier::Anonymous(1));
        let gnd = netlist.connect_node(NodeIdentifier::Gnd);
        let branch = netlist.connect_branch(BranchIdentifier::from_component("v1"));

        let devices: Vec<Box<dyn Element>> = vec![
            Box::new(TestVoltageSource { p: n_in.clone(), n: gnd.clone(), branch, v: 1.0 }),
            Box::new(TestResistor { n1: n_in, n2: n_out.clone(), r }),
            Box::new(TestCapacitor { n1: n_out, n2: gnd, c }),
        ];
        let circuit = CircuitInstance::from_devices_and_netlist("rc", devices, netlist);
        let options = PoleZeroOptions {
            input_source: BranchIdentifier::from_component("v1"),
            output: AnalogVariable::Node(NodeIdentifier::Anonymous(1)),
            output_ref: None,
        };
        (circuit, options)
    }

    /// `v1(in,gnd) --R-- a --L-- b --C-- gnd`: a source-driven series RLC —
    /// `.pz`'s canonical complex-conjugate-pair worked example, poles at
    /// `−R/(2L) ± j·sqrt(1/(LC) − (R/(2L))²)`.
    fn rlc_circuit_with_source(r: f64, l: f64, c: f64) -> (CircuitInstance, PoleZeroOptions) {
        let mut netlist = Netlist::new();
        let n_in = netlist.connect_node(NodeIdentifier::Anonymous(0));
        let n_a = netlist.connect_node(NodeIdentifier::Anonymous(1));
        let n_b = netlist.connect_node(NodeIdentifier::Anonymous(2));
        let gnd = netlist.connect_node(NodeIdentifier::Gnd);
        let v_branch = netlist.connect_branch(BranchIdentifier::from_component("v1"));
        let l_branch = netlist.connect_branch(BranchIdentifier::from_component("l1"));

        let devices: Vec<Box<dyn Element>> = vec![
            Box::new(TestVoltageSource { p: n_in.clone(), n: gnd.clone(), branch: v_branch, v: 1.0 }),
            Box::new(TestResistor { n1: n_in, n2: n_a.clone(), r }),
            Box::new(TestInductor { p: n_a, n: n_b.clone(), branch: l_branch, l }),
            Box::new(TestCapacitor { n1: n_b, n2: gnd, c }),
        ];
        let circuit = CircuitInstance::from_devices_and_netlist("rlc", devices, netlist);
        let options = PoleZeroOptions {
            input_source: BranchIdentifier::from_component("v1"),
            output: AnalogVariable::Node(NodeIdentifier::Anonymous(2)),
            output_ref: None,
        };
        (circuit, options)
    }

    /// `v1(in,gnd) --R-- gnd`: a purely resistive network (no reactive
    /// element anywhere) — `.pz`'s PZ-05 fail-loud worked example.
    fn resistor_only_circuit(r: f64) -> (CircuitInstance, PoleZeroOptions) {
        let mut netlist = Netlist::new();
        let n_in = netlist.connect_node(NodeIdentifier::Anonymous(0));
        let gnd = netlist.connect_node(NodeIdentifier::Gnd);
        let branch = netlist.connect_branch(BranchIdentifier::from_component("v1"));

        let devices: Vec<Box<dyn Element>> = vec![
            Box::new(TestVoltageSource { p: n_in.clone(), n: gnd.clone(), branch, v: 1.0 }),
            Box::new(TestResistor { n1: n_in.clone(), n2: gnd, r }),
        ];
        let circuit = CircuitInstance::from_devices_and_netlist("ronly", devices, netlist);
        let options = PoleZeroOptions {
            input_source: BranchIdentifier::from_component("v1"),
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
        let mut netlist = Netlist::new();
        let n_in = netlist.connect_node(NodeIdentifier::Anonymous(0));
        let n_out = netlist.connect_node(NodeIdentifier::Anonymous(1));
        let gnd = netlist.connect_node(NodeIdentifier::Gnd);
        let branch = netlist.connect_branch(BranchIdentifier::from_component("v1"));

        let devices: Vec<Box<dyn Element>> = vec![
            Box::new(TestVoltageSource { p: n_in.clone(), n: gnd.clone(), branch, v: 1.0 }),
            Box::new(TestResistor { n1: n_in.clone(), n2: n_out.clone(), r: r1 }),
            Box::new(TestCapacitor { n1: n_in, n2: n_out.clone(), c }),
            Box::new(TestResistor { n1: n_out.clone(), n2: gnd, r: r2 }),
        ];
        let circuit = CircuitInstance::from_devices_and_netlist("lag", devices, netlist);
        let options = PoleZeroOptions {
            input_source: BranchIdentifier::from_component("v1"),
            output: AnalogVariable::Node(NodeIdentifier::Anonymous(1)),
            output_ref: None,
        };
        (circuit, options)
    }

    /// `in --Rin-- gnd`, `in --R-- out`, `out --C-- gnd`: a 2-unknown RC
    /// network with a well-posed (nonsingular) DC point and a clean,
    /// hand-computable `(G, C)` pencil.
    fn rc_circuit(r_in: f64, r: f64, c: f64) -> (CircuitInstance, AnalogReference, AnalogReference) {
        let mut netlist = Netlist::new();
        let n_in = netlist.connect_node(NodeIdentifier::Anonymous(0));
        let n_out = netlist.connect_node(NodeIdentifier::Anonymous(1));
        let gnd = netlist.connect_node(NodeIdentifier::Gnd);

        let devices: Vec<Box<dyn Element>> = vec![
            Box::new(TestResistor { n1: n_in.clone(), n2: gnd.clone(), r: r_in }),
            Box::new(TestResistor { n1: n_in.clone(), n2: n_out.clone(), r }),
            Box::new(TestCapacitor { n1: n_out.clone(), n2: gnd, c }),
        ];
        let circuit = CircuitInstance::from_devices_and_netlist("rc", devices, netlist);
        (circuit, n_in, n_out)
    }

    #[test]
    fn assembles_known_2x2_g_and_c_on_an_rc_network() {
        let (mut circuit, n_in, n_out) = rc_circuit(2000.0, 1000.0, 1e-6);
        let context = Context::default();
        circuit.setup_all(&context).unwrap();
        let dc_point = DcSolver::new(&mut circuit, context.clone()).unwrap().solve().unwrap();
        let size = circuit.netlist().max_index().map(|i| i + 1).unwrap_or(0);
        assert_eq!(size, 2);

        let g = PoleZeroSolver::assemble_g(&mut circuit, &dc_point, &context, size);
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
        let mut netlist = Netlist::new();
        let n1 = netlist.connect_node(NodeIdentifier::Anonymous(0));
        let n2 = netlist.connect_node(NodeIdentifier::Anonymous(1));
        let gnd = netlist.connect_node(NodeIdentifier::Gnd);

        let devices: Vec<Box<dyn Element>> = vec![
            Box::new(TestResistor { n1: n1.clone(), n2: gnd.clone(), r: 1000.0 }),
            Box::new(TestResistor { n1: n2.clone(), n2: gnd, r: 1000.0 }),
            Box::new(FreqNonlinearDevice { n1, n2, k: 1.0 }),
        ];
        let mut circuit = CircuitInstance::from_devices_and_netlist("bad", devices, netlist);
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
        let (mut circuit, options) = rc_circuit_with_source(1000.0, 1e-6);
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
        let (mut circuit, options) = rlc_circuit_with_source(r, l, c);
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
        let (mut circuit, options) = resistor_only_circuit(1000.0);
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
        let (mut circuit, options) = lag_network_circuit(r1, r2, c);
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
        let (mut circuit, options) = rc_circuit_with_source(1000.0, 1e-6);
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
}
