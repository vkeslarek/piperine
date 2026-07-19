//! Distortion analysis (`.disto`) — small-signal weakly-nonlinear
//! distortion by the **method of nonlinear currents** (Volterra series on
//! the AC-linearized circuit, `design.md` Algorithm 4).
//!
//! **First order.** The circuit linearized at the DC operating point is
//! `Y(jω) = G + jωC`; a stimulus tone at `F1` (the circuit's AC stimuli,
//! scaled by [`DistoOptions::amplitude`]) gives the first-order phasors
//! `X1 = Y(jω1)⁻¹·b`.
//!
//! **Second order (2·F1).** Every nonlinear contribution `f(v)` (resistive
//! `i(v)` and charge `q(v)`) expands to second order as
//! `½·Σ_{j,k} (∂²f/∂v_j∂v_k)·Δv_j·Δv_k`. With peak phasors
//! (`Δv(t) = Re{x·e^{jωt}}`), a same-tone product lands at `2·F1` with
//! phasor `x_j·x_k/2`, so each contribution injects a *nonlinear current*
//! whose `2·F1` phasor is `¼·Σ H_jk·x_j·x_k` — charge contributions carry
//! an extra `j·2ω1` (`i = dq/dt`). The Hessians `H_jk` come from the
//! devices' symbolic second derivatives ([`Disto2`], DISTO-03 — never
//! numeric perturbation). Solving `Y(j·2ω1)·X2 = −I2` yields the
//! second-order response; `HD2 = |X2(out)| / |X1(out)|` (DISTO-01).
//!
//! Devices whose nonlinearity cannot be differentiated fail loud at
//! compile time (`CodegenError::Unsupported`, DISTO-04) — never a silent
//! zero row.
#![allow(dead_code)]

use crate::analog::{AnalogReference, AnalogVariable, NodeIdentifier};
use crate::analyses::Context;
use crate::analyses::ac::AcAnalysisContext;
use crate::analyses::dc::DcSolver;
use crate::core::circuit::CircuitInstance;
use crate::error::{Error, SolverDomain};
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::faer::FaerSparseLinearSystem;
use crate::math::linear::Stamp;
use crate::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::prelude::DcAnalysisResult;
use crate::result::DistoResult;

use num_complex::{Complex, Complex64};

// ── element-facing vocabulary ────────────────────────────────────────────

/// A device's second derivatives at the DC operating point (DISTO-03): the
/// Hessian of every nonlinear contribution over every ordered
/// controlling-branch pair — the element-facing half of the `.disto`
/// contract (like [`Noise`](crate::analyses::noise::Noise) for `.noise`).
#[derive(Debug, Clone)]
pub struct Disto2 {
    /// Ordered controlling branch pairs `((j_plus, j_minus), (k_plus,
    /// k_minus))`, in `values` row order; a `None` terminal is ground.
    /// Only pairs with at least one nonzero Hessian row appear.
    pub pairs: Vec<(
        (Option<AnalogReference>, Option<AnalogReference>),
        (Option<AnalogReference>, Option<AnalogReference>),
    )>,
    /// Contribution terminals `(plus, minus)` (a `None` terminal is
    /// ground), in `values` column order: resistive contributions first,
    /// then charge contributions (the split is `charge_start`).
    pub contribs: Vec<(Option<AnalogReference>, Option<AnalogReference>)>,
    /// Index in `contribs` where charge contributions begin.
    pub charge_start: usize,
    /// Row-major `[pair][contrib]` Hessian values at the DC point.
    pub values: Vec<f64>,
}

// ── request/state ────────────────────────────────────────────────────────

/// Distortion analysis options: the stimulus tone and the output the
/// distortion ratios are measured at.
#[derive(Clone, Debug)]
pub struct DistoOptions {
    /// Single-tone stimulus frequency `F1` (Hz).
    pub f1: f64,
    /// Stimulus amplitude (peak): every AC stimulus magnitude in the
    /// circuit is scaled by this factor for the first-order solve.
    pub amplitude: f64,
    /// Output variable: `AnalogVariable::Node` for `V(out)`.
    pub output: AnalogVariable,
    /// Reference node for a differential voltage output
    /// (`V(output) − V(output_ref)`); `None` measures single-ended (GND ref).
    pub output_ref: Option<NodeIdentifier>,
}

// ── driver ───────────────────────────────────────────────────────────────

/// The linearized `.disto` system: device AC stamps with the stimulus RHS
/// scaled by `stim_scale` (0 for the higher-order solves — only the
/// nonlinear currents drive them), plus the nonlinear-current injections.
struct DistoSystem<'a> {
    circuit: &'a mut CircuitInstance,
    context: Context,
    dc_point: DcAnalysisResult,
    frequency: f64,
    stim_scale: f64,
    nonlinear_rhs: Vec<Stamp<AnalogReference, Complex64>>,
}

impl<'a> NonLinearSystem<AnalogReference, Complex<f64>> for DistoSystem<'a> {
    fn assemble(
        &mut self,
        _state: &CircularArrayBuffer2<Complex<f64>>,
    ) -> crate::result::Result<Vec<Stamp<AnalogReference, Complex<f64>>>> {
        let ac_ctx = AcAnalysisContext { frequency: self.frequency };
        let mut stamps = Vec::new();
        for dev in &mut self.circuit.devices {
            for stamp in dev.load_ac(&self.dc_point, &ac_ctx, &self.context) {
                match stamp {
                    Stamp::Rhs(reference, value) => {
                        if self.stim_scale != 0.0 {
                            stamps.push(Stamp::Rhs(reference, value * self.stim_scale));
                        }
                    }
                    other => stamps.push(other),
                }
            }
        }
        stamps.extend(self.nonlinear_rhs.iter().cloned());
        Ok(stamps)
    }

    fn netlist(&self) -> &crate::analog::Netlist {
        self.circuit.netlist()
    }
}

/// `.disto` solver: solves the DC operating point, then drives the
/// single-tone Volterra recursion (first order at `F1`, second order at
/// `2·F1` from the devices' [`Disto2`] Hessians).
pub struct DistoSolver<'a> {
    system: DistoSystem<'a>,
    solver: NewtonRaphsonSolver<AnalogReference, Complex<f64>, FaerSparseLinearSystem<Complex<f64>>>,
    options: DistoOptions,
    output_ref: AnalogReference,
    output_ref_node: Option<AnalogReference>,
    policy: crate::analyses::Policy,
}

impl<'a> DistoSolver<'a> {
    /// Builds the solver: validates the options, solves the DC operating
    /// point (a DC failure surfaces as-is — no distortion is attempted on
    /// an unconverged bias), and resolves the output reference.
    pub fn new(
        circuit: &'a mut CircuitInstance,
        options: DistoOptions,
        context: Context,
    ) -> crate::result::Result<Self> {
        Context::init_global();
        circuit.setup_all(&context)?;

        if options.f1 <= 0.0 {
            return Err(Error::simple(
                SolverDomain::Disto,
                format!("`.disto` requires a positive stimulus frequency, got f1 = {}", options.f1),
            ));
        }
        if options.amplitude <= 0.0 {
            return Err(Error::simple(
                SolverDomain::Disto,
                format!("`.disto` requires a positive stimulus amplitude, got {}", options.amplitude),
            ));
        }

        let size = circuit.netlist().max_index().map(|i| i + 1).unwrap_or(0);
        if size == 0 {
            return Err(Error::simple(SolverDomain::Disto, "no analog network"));
        }

        let dc_point = DcSolver::new(circuit, context.clone())?.solve()?;

        let output_ref = circuit
            .netlist()
            .reference_for(&options.output)
            .ok_or_else(|| Error::simple(SolverDomain::Disto, "output variable not found in circuit"))?
            .clone();
        let output_ref_node = options
            .output_ref
            .as_ref()
            .map(|node| {
                circuit
                    .netlist()
                    .reference_for(&AnalogVariable::Node(node.clone()))
                    .cloned()
                    .ok_or_else(|| Error::simple(SolverDomain::Disto, "output reference node not found in circuit"))
            })
            .transpose()?;

        let mut system = DistoSystem {
            circuit,
            context,
            dc_point,
            frequency: 0.0,
            stim_scale: 0.0,
            nonlinear_rhs: Vec::new(),
        };
        let solver = NewtonRaphsonSolver::new(&mut system, size, 1)?;

        Ok(Self { system, solver, options, output_ref, output_ref_node, policy: crate::analyses::Policy::default() })
    }

    /// Single-tone distortion: first order at `F1`, second order at
    /// `2·F1`, returning `HD2` (DISTO-01).
    pub fn solve(&mut self) -> crate::result::Result<DistoResult> {
        let f1 = self.options.f1;
        let omega1 = 2.0 * std::f64::consts::PI * f1;

        let x1 = self.solve_at(f1, self.options.amplitude, Vec::new())?;
        let i2 = self.nonlinear_currents(&x1, omega1);
        let rhs: Vec<Stamp<AnalogReference, Complex64>> = i2
            .into_iter()
            .map(|(reference, value)| Stamp::Rhs(reference, value))
            .collect();
        let x2 = self.solve_at(2.0 * f1, 0.0, rhs)?;

        let out1 = self.output_phasor(&x1);
        let out2 = self.output_phasor(&x2);
        if out1.norm() == 0.0 {
            return Err(Error::simple(
                SolverDomain::Disto,
                "no first-order response at the output — distortion ratios are undefined",
            ));
        }
        Ok(DistoResult { hd2: Some(out2.norm() / out1.norm()), ..DistoResult::default() })
    }

    /// Solve the linearized system at `f_hz` with the stimulus scaled by
    /// `stim_scale` and `extra` added to the RHS.
    fn solve_at(
        &mut self,
        f_hz: f64,
        stim_scale: f64,
        extra: Vec<Stamp<AnalogReference, Complex64>>,
    ) -> crate::result::Result<ndarray::Array1<Complex64>> {
        self.system.frequency = f_hz;
        self.system.stim_scale = stim_scale;
        self.system.nonlinear_rhs = extra;
        Ok(self.solver.solve(&mut self.system, self.policy.max_iter)?)
    }

    /// The output phasor from a solution vector (single-ended or
    /// differential).
    fn output_phasor(&self, x: &ndarray::Array1<Complex64>) -> Complex64 {
        let base = self.output_ref.idx().map(|i| x[i]).unwrap_or(Complex64::ZERO);
        let reference = self
            .output_ref_node
            .as_ref()
            .and_then(|r| r.idx())
            .map(|i| x[i])
            .unwrap_or(Complex64::ZERO);
        base - reference
    }

    /// The second-order nonlinear-current injections at `2·F1`: per device,
    /// per contribution, `¼·Σ H_jk·x_j·x_k` (charge rows scaled by
    /// `j·2ω1`), stamped as `−I2` on the RHS (`Y·X2 = −I2`).
    fn nonlinear_currents(
        &mut self,
        x1: &ndarray::Array1<Complex64>,
        omega1: f64,
    ) -> Vec<(AnalogReference, Complex64)> {
        let mut i2: Vec<(AnalogReference, Complex64)> = Vec::new();
        let DistoSystem { circuit, dc_point, context, .. } = &mut self.system;
        for dev in &mut circuit.devices {
            let Some(d2) = dev.load_disto2(dc_point, context) else { continue };
            let nc = d2.contribs.len();
            let mut sums = vec![Complex64::ZERO; nc];
            for (pi, ((jp, jm), (kp, km))) in d2.pairs.iter().enumerate() {
                let xj = Self::branch_phasor(x1, jp) - Self::branch_phasor(x1, jm);
                let xk = Self::branch_phasor(x1, kp) - Self::branch_phasor(x1, km);
                if xj == Complex64::ZERO || xk == Complex64::ZERO {
                    continue;
                }
                for (ci, sum) in sums.iter_mut().enumerate() {
                    let h = d2.values[pi * nc + ci];
                    if h != 0.0 {
                        *sum += 0.25 * h * xj * xk;
                    }
                }
            }
            for (ci, (plus, minus)) in d2.contribs.iter().enumerate() {
                let mut cur = sums[ci];
                if cur == Complex64::ZERO {
                    continue;
                }
                if ci >= d2.charge_start {
                    cur *= Complex64::new(0.0, 2.0 * omega1);
                }
                // The contribution's current leaves `plus` through the
                // device, so the RHS (`−I2`) takes the negation there.
                if let Some(p) = plus {
                    i2.push((p.clone(), -cur));
                }
                if let Some(m) = minus {
                    i2.push((m.clone(), cur));
                }
            }
        }
        i2
    }

    /// A solution phasor at a (possibly ground) terminal reference.
    fn branch_phasor(x: &ndarray::Array1<Complex64>, terminal: &Option<AnalogReference>) -> Complex64 {
        terminal
            .as_ref()
            .and_then(|r| r.idx())
            .map(|i| x[i])
            .unwrap_or(Complex64::ZERO)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analog::{BranchIdentifier, Netlist, NodeIdentifier};
    use crate::analyses::dc::DcAnalysisState;
    use crate::core::element::{AnalogDevice, DigitalDevice, Element, ElementCapabilities, Introspect};
    use num_complex::Complex64;

    // ── test doubles ─────────────────────────────────────────────────────

    /// Ideal DC voltage source with a unit AC stimulus (mag 1, phase 0) on
    /// its branch equation.
    struct TestAcVsource {
        p: AnalogReference,
        n: AnalogReference,
        branch: AnalogReference,
        v: f64,
    }
    impl TestAcVsource {
        fn topology(&self) -> Vec<Stamp<AnalogReference, Complex64>> {
            let b = self.branch.clone();
            let one = Complex64::new(1.0, 0.0);
            vec![
                Stamp::Matrix(self.p.clone(), b.clone(), one),
                Stamp::Matrix(b.clone(), self.p.clone(), one),
                Stamp::Matrix(self.n.clone(), b.clone(), -one),
                Stamp::Matrix(b.clone(), self.n.clone(), -one),
            ]
        }
    }
    impl AnalogDevice for TestAcVsource {
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
        fn load_ac(
            &mut self,
            _dc: &DcAnalysisResult,
            _ac: &AcAnalysisContext,
            _ctx: &Context,
        ) -> Vec<Stamp<AnalogReference, Complex64>> {
            let mut stamps = self.topology();
            stamps.push(Stamp::Rhs(self.branch.clone(), Complex64::new(1.0, 0.0)));
            stamps
        }
    }
    impl DigitalDevice for TestAcVsource {}
    impl Introspect for TestAcVsource {}
    impl Element for TestAcVsource {
        fn name(&self) -> &str {
            "v1"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::LOADS_AC
        }
    }

    struct TestResistor {
        n1: AnalogReference,
        n2: AnalogReference,
        r: f64,
    }
    impl TestResistor {
        fn g(&self) -> f64 {
            1.0 / self.r
        }
    }
    impl AnalogDevice for TestResistor {
        fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
            let g = self.g();
            vec![
                Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
                Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
                Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
                Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
            ]
        }
        fn load_ac(
            &mut self,
            _dc: &DcAnalysisResult,
            _ac: &AcAnalysisContext,
            _ctx: &Context,
        ) -> Vec<Stamp<AnalogReference, Complex64>> {
            let g = Complex64::new(self.g(), 0.0);
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
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::LOADS_AC
        }
    }

    /// Polynomial VCCS `i = g1·v + g2·v² + g3·v³` from `out` to ground,
    /// controlled by `v = V(in)`: the DISTO-05 closed-form stage. DC/AC
    /// stamp the exact Norton linearization; `load_disto2` reports the
    /// symbolic Hessian `f''(v_dc) = 2·g2 + 6·g3·v_dc` on the single
    /// controlling branch.
    struct TestPolyVccs {
        input: AnalogReference,
        output: AnalogReference,
        g1: f64,
        g2: f64,
        g3: f64,
    }
    impl TestPolyVccs {
        fn conductance(&self, v: f64) -> f64 {
            self.g1 + 2.0 * self.g2 * v + 3.0 * self.g3 * v * v
        }
        fn current(&self, v: f64) -> f64 {
            self.g1 * v + self.g2 * v * v + self.g3 * v * v * v
        }
        fn dc_voltage(&self, dc: &DcAnalysisResult) -> f64 {
            dc.get(AnalogVariable::Node(NodeIdentifier::Anonymous(0))).unwrap_or(0.0)
        }
    }
    impl AnalogDevice for TestPolyVccs {
        fn load_dc(&mut self, s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
            let v = s
                .latest()
                .and_then(|x| self.input.idx().and_then(|i| x.get(i)).copied())
                .unwrap_or(0.0);
            let g = self.conductance(v);
            let i_eq = self.current(v) - g * v;
            vec![
                Stamp::Matrix(self.output.clone(), self.input.clone(), g),
                Stamp::Rhs(self.output.clone(), -i_eq),
            ]
        }
        fn load_ac(
            &mut self,
            dc: &DcAnalysisResult,
            _ac: &AcAnalysisContext,
            _ctx: &Context,
        ) -> Vec<Stamp<AnalogReference, Complex64>> {
            let g = Complex64::new(self.conductance(self.dc_voltage(dc)), 0.0);
            vec![Stamp::Matrix(self.output.clone(), self.input.clone(), g)]
        }
        fn load_disto2(&mut self, dc_op: &DcAnalysisResult, _context: &Context) -> Option<Disto2> {
            let v = self.dc_voltage(dc_op);
            let hessian = 2.0 * self.g2 + 6.0 * self.g3 * v;
            Some(Disto2 {
                pairs: vec![(
                    (Some(self.input.clone()), None),
                    (Some(self.input.clone()), None),
                )],
                contribs: vec![(Some(self.output.clone()), None)],
                charge_start: 1,
                values: vec![hessian],
            })
        }
    }
    impl DigitalDevice for TestPolyVccs {}
    impl Introspect for TestPolyVccs {}
    impl Element for TestPolyVccs {
        fn name(&self) -> &str {
            "n1"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::LOADS_AC
        }
    }

    /// `v1(in,gnd)` → poly VCCS `in→out` → `R(out,gnd)`: single nonlinear
    /// stage with the analytic Volterra prediction
    /// `HD2 = f''(v_dc)·A / (4·f'(v_dc))`.
    fn poly_stage(v_dc: f64, r: f64, g1: f64, g2: f64, g3: f64) -> (CircuitInstance, AnalogReference, AnalogReference) {
        let mut netlist = Netlist::new();
        let n_in = netlist.connect_node(NodeIdentifier::Anonymous(0));
        let n_out = netlist.connect_node(NodeIdentifier::Anonymous(1));
        let gnd = netlist.connect_node(NodeIdentifier::Gnd);
        let branch = netlist.connect_branch(BranchIdentifier::from_component("v1"));

        let devices: Vec<Box<dyn Element>> = vec![
            Box::new(TestAcVsource { p: n_in.clone(), n: gnd.clone(), branch, v: v_dc }),
            Box::new(TestPolyVccs { input: n_in.clone(), output: n_out.clone(), g1, g2, g3 }),
            Box::new(TestResistor { n1: n_out.clone(), n2: gnd, r }),
        ];
        (CircuitInstance::from_devices_and_netlist("stage", devices, netlist), n_in, n_out)
    }

    #[test]
    fn single_tone_hd2_matches_closed_form_volterra() {
        let (v_dc, r, g1, g2, g3) = (0.5, 50.0, 0.1, 0.02, 0.003);
        let amplitude = 0.1;
        let (mut circuit, _in, out) = poly_stage(v_dc, r, g1, g2, g3);
        let options = DistoOptions {
            f1: 1e6,
            amplitude,
            output: AnalogVariable::Node(NodeIdentifier::Anonymous(1)),
            output_ref: None,
        };
        let mut solver = DistoSolver::new(&mut circuit, options, Context::default()).unwrap();
        let result = solver.solve().expect("disto solves");

        // Exact Volterra prediction at this bias: f'(v) = g1+2·g2·v+3·g3·v²,
        // f''(v) = 2·g2+6·g3·v, HD2 = f''·A / (4·f').
        let f1p = g1 + 2.0 * g2 * v_dc + 3.0 * g3 * v_dc * v_dc;
        let f2p = 2.0 * g2 + 6.0 * g3 * v_dc;
        let expected = f2p * amplitude / (4.0 * f1p);
        let hd2 = result.hd2.expect("single-tone run reports HD2");
        let rel = (hd2 - expected).abs() / expected;
        assert!(rel < 1e-3, "HD2 = {hd2}, closed form {expected} (rel {rel}), out = {out:?}");
    }

    #[test]
    fn linear_circuit_has_zero_hd2() {
        // No device reports Disto2: no nonlinear currents, X2 == 0, HD2 == 0.
        let mut netlist = Netlist::new();
        let n_in = netlist.connect_node(NodeIdentifier::Anonymous(0));
        let n_out = netlist.connect_node(NodeIdentifier::Anonymous(1));
        let gnd = netlist.connect_node(NodeIdentifier::Gnd);
        let branch = netlist.connect_branch(BranchIdentifier::from_component("v1"));
        let devices: Vec<Box<dyn Element>> = vec![
            Box::new(TestAcVsource { p: n_in.clone(), n: gnd.clone(), branch, v: 1.0 }),
            Box::new(TestResistor { n1: n_in, n2: n_out.clone(), r: 1000.0 }),
            Box::new(TestResistor { n1: n_out, n2: gnd, r: 1000.0 }),
        ];
        let mut circuit = CircuitInstance::from_devices_and_netlist("linear", devices, netlist);
        let options = DistoOptions {
            f1: 1e6,
            amplitude: 0.5,
            output: AnalogVariable::Node(NodeIdentifier::Anonymous(1)),
            output_ref: None,
        };
        let mut solver = DistoSolver::new(&mut circuit, options, Context::default()).unwrap();
        let result = solver.solve().expect("disto solves");
        assert_eq!(result.hd2, Some(0.0), "a linear circuit has no 2nd-order response");
    }

    #[test]
    fn bad_options_fail_loud() {
        let (mut circuit, _, _) = poly_stage(0.5, 50.0, 0.1, 0.02, 0.003);
        let options = DistoOptions {
            f1: 0.0,
            amplitude: 0.1,
            output: AnalogVariable::Node(NodeIdentifier::Anonymous(1)),
            output_ref: None,
        };
        let Err(err) = DistoSolver::new(&mut circuit, options, Context::default()) else {
            panic!("f1 = 0 must fail loud");
        };
        assert!(err.to_string().contains("positive stimulus frequency"), "{err}");
    }
}
