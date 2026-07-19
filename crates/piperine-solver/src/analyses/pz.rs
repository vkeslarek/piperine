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
//! Poles/zeros themselves (generalized eigenvalues of the `(G, C)` pencil and
//! the Rosenbrock system pencil) are added in the next task; this module
//! currently exposes the `(G, C)` extraction the eigensolve consumes.
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
/// operating point, then (next task) computes poles/zeros as generalized
/// eigenvalues of that pencil.
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
}
