//! N-port scattering-parameter analysis (`.sp`) — per-frequency Thévenin-
//! source-behind-`z0` port excitation over the AC-linearized circuit, with
//! Kurokawa power-wave normalization (`design.md` Algorithm 3).
//!
//! **Ports are analysis-time, not authored (SP-01).** Each port (a node, a
//! reference impedance `z0`, a 1-based `num`) is declared in PHDL via the
//! `@rfport(num, z0)` attribute (`piperine-lang`) — no stdlib device, no
//! `IS_PORT` capability. The `.sp` driver itself adds the `z0` termination
//! and the switchable excitation for the duration of the analysis.
//!
//! **Per-frequency algorithm.** For every driven port `j` (all others left
//! matched by their own `z0` termination):
//! 1. Assemble `Y(jω)` from every device's `load_ac` stamp, plus a `1/z0`
//!    conductance stamp at each port's node (the termination).
//! 2. Drive port `j` with a 1V Thévenin source behind `z0_j`. Its Norton
//!    equivalent — a `1/z0_j` current injection at the port's node — needs
//!    no new branch unknown, so this reuses the plain AC complex solve
//!    (`Y·V = I`) unchanged.
//! 3. Power waves (Kurokawa, real `z0`):
//!    `a_i = (V_i + z0_i·I_i) / (2·√z0_i)`,
//!    `b_i = (V_i − z0_i·I_i) / (2·√z0_i)`,
//!    with the current *into* the network at port `i`, `I_i = (E_i − V_i)/z0_i`
//!    (`E_j = 1`, `E_{i≠j} = 0`). With only port `j` driven, `a_j` is the
//!    constant `1/(2√z0_j)` and `a_{i≠j} = 0`, so `S_ij = b_i / a_j`
//!    (SP-02) — no fragile current bookkeeping, only node voltages already
//!    solved for.
//! 4. Repeat for every `j`, filling column `j` of `S`.
#![allow(dead_code)]

use crate::analog::{AnalogReference, AnalogVariable, NodeIdentifier};
use crate::analyses::Context;
use crate::analyses::ac::{AcAnalysisContext, AcSweepAnalysisOptions};
use crate::analyses::dc::DcSolver;
use crate::core::circuit::CircuitInstance;
use crate::error::{Error, SolverDomain};
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::faer::FaerSparseLinearSystem;
use crate::math::linear::Stamp;
use crate::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::prelude::DcAnalysisResult;
use crate::result::SpResult;

use ndarray::Array2;
use num_complex::Complex;

// ── request/state ────────────────────────────────────────────────────────

/// One `.sp` port declaration: the node it sits on, its reference impedance,
/// and its 1-based port number (from `@rfport(num, z0)`).
#[derive(Clone, Debug)]
pub struct SpPort {
    pub num: usize,
    pub node: NodeIdentifier,
    pub z0: f64,
}

/// `.sp` analysis options: the ports (in the order the `S` matrix indexes
/// them) and the frequency sweep.
#[derive(Clone, Debug)]
pub struct SpOptions {
    pub ports: Vec<SpPort>,
    pub sweep: AcSweepAnalysisOptions,
}

/// One resolved port: its declared spec plus the netlist reference its node
/// maps to.
struct ResolvedPort {
    spec: SpPort,
    reference: AnalogReference,
}

// ── driver ───────────────────────────────────────────────────────────────

/// The linearized `.sp` system: device AC stamps plus every port's `z0`
/// termination, with a `1/z0` current injection at whichever port is
/// currently driven (the Norton equivalent of a 1V Thévenin source behind
/// `z0`, SP-02's "practical stamping").
struct SpSystem<'a> {
    circuit: &'a mut CircuitInstance,
    context: Context,
    dc_point: DcAnalysisResult,
    frequency: f64,
    ports: Vec<AnalogReference>,
    z0: Vec<f64>,
    driven: usize,
}

impl<'a> NonLinearSystem<AnalogReference, Complex<f64>> for SpSystem<'a> {
    fn assemble(
        &mut self,
        _state: &CircularArrayBuffer2<Complex<f64>>,
    ) -> crate::result::Result<Vec<Stamp<AnalogReference, Complex<f64>>>> {
        let ac_ctx = AcAnalysisContext { frequency: self.frequency };
        let mut stamps = Vec::new();
        for dev in &mut self.circuit.devices {
            stamps.extend(dev.load_ac(&self.dc_point, &ac_ctx, &self.context));
        }
        for (i, reference) in self.ports.iter().enumerate() {
            let g0 = Complex::new(1.0 / self.z0[i], 0.0);
            stamps.push(Stamp::Matrix(reference.clone(), reference.clone(), g0));
            if i == self.driven {
                stamps.push(Stamp::Rhs(reference.clone(), g0));
            }
        }
        Ok(stamps)
    }

    fn netlist(&self) -> &crate::analog::Netlist {
        self.circuit.netlist()
    }
}

/// `.sp` solver: resolves every port to a netlist node, then sweeps the
/// frequency list, driving one port at a time and filling the `S` matrix.
pub struct SpSolver<'a> {
    system: SpSystem<'a>,
    solver: NewtonRaphsonSolver<AnalogReference, Complex<f64>, FaerSparseLinearSystem<Complex<f64>>>,
    ports: Vec<ResolvedPort>,
    sweep: AcSweepAnalysisOptions,
    policy: crate::analyses::Policy,
}

impl<'a> SpSolver<'a> {
    /// Builds the solver: solves the DC operating point, resolves every
    /// port's node to a netlist reference, and validates the port list
    /// (SP-05): at least one port, every `z0 > 0`, no duplicate `num`, and
    /// no port sitting on ground (a zero-length port — a `z0` termination
    /// to ground of a node that already *is* ground is degenerate).
    pub fn new(
        circuit: &'a mut CircuitInstance,
        options: SpOptions,
        context: Context,
    ) -> crate::result::Result<Self> {
        Context::init_global();
        circuit.setup_all(&context)?;

        if options.ports.is_empty() {
            return Err(Error::simple(SolverDomain::Sp, "`.sp` requires at least one port"));
        }

        let mut seen_nums = std::collections::HashSet::new();
        let mut seen_nodes = std::collections::HashSet::new();
        for port in &options.ports {
            if port.z0 <= 0.0 {
                return Err(Error::simple(
                    SolverDomain::Sp,
                    format!("port {} has non-positive z0 = {}", port.num, port.z0),
                ));
            }
            if port.node.is_ground() {
                return Err(Error::simple(
                    SolverDomain::Sp,
                    format!("port {} sits on ground — a degenerate zero-length port", port.num),
                ));
            }
            if !seen_nums.insert(port.num) {
                return Err(Error::simple(SolverDomain::Sp, format!("duplicate port num {}", port.num)));
            }
            if !seen_nodes.insert(port.node.clone()) {
                return Err(Error::simple(
                    SolverDomain::Sp,
                    format!("port {} coincides with another port's node '{}' — a degenerate zero-length port", port.num, port.node),
                ));
            }
        }

        let dc_point = DcSolver::new(circuit, context.clone())?.solve()?;

        let mut resolved = Vec::with_capacity(options.ports.len());
        for spec in &options.ports {
            let reference = circuit
                .netlist()
                .reference_for(&AnalogVariable::Node(spec.node.clone()))
                .ok_or_else(|| {
                    Error::simple(SolverDomain::Sp, format!("port {}: unknown node '{}'", spec.num, spec.node))
                })?
                .clone();
            resolved.push(ResolvedPort { spec: spec.clone(), reference });
        }

        let size = circuit.netlist().max_index().map(|i| i + 1).unwrap_or(0);
        let mut system = SpSystem {
            circuit,
            context,
            dc_point,
            frequency: 0.0,
            ports: resolved.iter().map(|p| p.reference.clone()).collect(),
            z0: resolved.iter().map(|p| p.spec.z0).collect(),
            driven: 0,
        };
        let solver = NewtonRaphsonSolver::new(&mut system, size, 1)?;

        Ok(Self { system, solver, ports: resolved, sweep: options.sweep, policy: crate::analyses::Policy::default() })
    }

    /// Sweeps every frequency, driving one port at a time, filling the `S`
    /// matrix column by column (SP-02).
    pub fn solve_sweep(&mut self) -> crate::result::Result<SpResult> {
        let n_ports = self.ports.len();
        let frequencies = self.sweep.generate_frequencies();
        let max_iter = self.policy.max_iter;

        let mut s_list = Vec::with_capacity(frequencies.len());
        for &f_hz in &frequencies {
            self.system.frequency = f_hz;
            let mut s = Array2::<Complex<f64>>::zeros((n_ports, n_ports));
            for j in 0..n_ports {
                self.system.driven = j;
                let solution = self.solver.solve(&mut self.system, max_iter)?;
                let z0_j = self.ports[j].spec.z0;
                let a_j = Complex::new(1.0 / (2.0 * z0_j.sqrt()), 0.0);
                for i in 0..n_ports {
                    let idx = self.ports[i].reference.idx().ok_or_else(|| {
                        Error::simple(SolverDomain::Sp, format!("port {} has no MNA index", self.ports[i].spec.num))
                    })?;
                    let v_i = solution[idx];
                    let e_i = if i == j { Complex::new(1.0, 0.0) } else { Complex::new(0.0, 0.0) };
                    let z0_i = self.ports[i].spec.z0;
                    let i_i = (e_i - v_i) / z0_i;
                    let b_i = (v_i - z0_i * i_i) / (2.0 * z0_i.sqrt());
                    s[[i, j]] = b_i / a_j;
                }
            }
            s_list.push(s);
        }

        Ok(SpResult {
            frequencies,
            s: s_list,
            z0: self.ports.iter().map(|p| p.spec.z0).collect(),
            n_ports,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analog::{BranchIdentifier, Netlist, NodeIdentifier};
    use crate::analyses::dc::DcAnalysisState;
    use crate::core::element::{AnalogDevice, DigitalDevice, Element, ElementCapabilities, Introspect};
    use crate::math::linear::Stamp as S;
    use num_complex::Complex64;

    // ── test doubles ─────────────────────────────────────────────────────

    /// A plain resistor between two nodes — `load_dc` (DC point) and
    /// `load_ac` (frequency-independent real conductance) both stamped.
    struct TestResistor {
        n1: AnalogReference,
        n2: AnalogReference,
        r: f64,
    }
    impl AnalogDevice for TestResistor {
        fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<S<AnalogReference, f64>> {
            let g = 1.0 / self.r;
            vec![
                S::Matrix(self.n1.clone(), self.n1.clone(), g),
                S::Matrix(self.n2.clone(), self.n2.clone(), g),
                S::Matrix(self.n1.clone(), self.n2.clone(), -g),
                S::Matrix(self.n2.clone(), self.n1.clone(), -g),
            ]
        }
        fn load_ac(
            &mut self,
            _dc: &DcAnalysisResult,
            _ac_ctx: &AcAnalysisContext,
            _c: &Context,
        ) -> Vec<S<AnalogReference, Complex64>> {
            let g = Complex64::new(1.0 / self.r, 0.0);
            vec![
                S::Matrix(self.n1.clone(), self.n1.clone(), g),
                S::Matrix(self.n2.clone(), self.n2.clone(), g),
                S::Matrix(self.n1.clone(), self.n2.clone(), -g),
                S::Matrix(self.n2.clone(), self.n1.clone(), -g),
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

    /// A shunt capacitor to ground — pure `load_ac` (`jωC`), no DC stamp
    /// (DC point sees it as an open).
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
            _c: &Context,
        ) -> Vec<S<AnalogReference, Complex64>> {
            let omega = 2.0 * std::f64::consts::PI * ac_ctx.frequency;
            let y = Complex64::new(0.0, omega * self.c);
            vec![
                S::Matrix(self.n1.clone(), self.n1.clone(), y),
                S::Matrix(self.n2.clone(), self.n2.clone(), y),
                S::Matrix(self.n1.clone(), self.n2.clone(), -y),
                S::Matrix(self.n2.clone(), self.n1.clone(), -y),
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

    /// A huge (1 GΩ) DC-only bias resistor to ground — gives a floating
    /// test node a well-defined DC operating point (real `.sp` fixtures
    /// always have *some* DC continuity; a bare two-terminal passive test
    /// double otherwise has none) without perturbing the AC/S-parameter
    /// result (no `load_ac` stamp at all).
    struct TestDcBias {
        n: AnalogReference,
    }
    impl AnalogDevice for TestDcBias {
        fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<S<AnalogReference, f64>> {
            vec![S::Matrix(self.n.clone(), self.n.clone(), 1e-9)]
        }
    }
    impl DigitalDevice for TestDcBias {}
    impl Introspect for TestDcBias {}
    impl Element for TestDcBias {
        fn name(&self) -> &str {
            "rbias"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
        }
    }

    /// An ideal wire (zero-impedance short) between two *distinct* netlist
    /// nodes, MNA branch-unknown style: `V(p) − V(n) − 0·I = 0` in both DC
    /// and AC. Used to build a T-junction test fixture (two ports tied to
    /// the same electrical point through separate, non-coincident port
    /// nodes) — the physically correct way to test a shunt element, as
    /// opposed to declaring two ports on the literal same node (SP-05's
    /// "port nodes coincide" fail-loud case).
    struct TestWire {
        p: AnalogReference,
        n: AnalogReference,
        branch: AnalogReference,
    }
    impl AnalogDevice for TestWire {
        fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<S<AnalogReference, f64>> {
            let b = self.branch.clone();
            vec![
                S::Matrix(self.p.clone(), b.clone(), 1.0),
                S::Matrix(b.clone(), self.p.clone(), 1.0),
                S::Matrix(self.n.clone(), b.clone(), -1.0),
                S::Matrix(b.clone(), self.n.clone(), -1.0),
            ]
        }
        fn load_ac(
            &mut self,
            _dc: &DcAnalysisResult,
            _ac_ctx: &AcAnalysisContext,
            _c: &Context,
        ) -> Vec<S<AnalogReference, Complex64>> {
            let b = self.branch.clone();
            let one = Complex64::new(1.0, 0.0);
            vec![
                S::Matrix(self.p.clone(), b.clone(), one),
                S::Matrix(b.clone(), self.p.clone(), one),
                S::Matrix(self.n.clone(), b.clone(), -one),
                S::Matrix(b.clone(), self.n.clone(), -one),
            ]
        }
    }
    impl DigitalDevice for TestWire {}
    impl Introspect for TestWire {}
    impl Element for TestWire {
        fn name(&self) -> &str {
            "wire"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::LOADS_AC
        }
    }

    /// Series-`R` attenuator: `port1 --Rs-- port2`, both ports referenced to
    /// ground. A textbook 2-port with a known closed-form `S` at any `z0`.
    fn series_r_attenuator(rs: f64, z0: f64) -> (CircuitInstance, SpOptions) {
        let mut netlist = Netlist::new();
        let n1 = netlist.connect_node(NodeIdentifier::Anonymous(0));
        let n2 = netlist.connect_node(NodeIdentifier::Anonymous(1));
        let gnd = netlist.connect_node(NodeIdentifier::Gnd);
        let _ = gnd;

        let devices: Vec<Box<dyn Element>> = vec![
            Box::new(TestResistor { n1: n1.clone(), n2: n2.clone(), r: rs }),
            Box::new(TestDcBias { n: n1.clone() }),
            Box::new(TestDcBias { n: n2.clone() }),
        ];
        let circuit = CircuitInstance::from_devices_and_netlist("atten", devices, netlist);
        let options = SpOptions {
            ports: vec![
                SpPort { num: 1, node: NodeIdentifier::Anonymous(0), z0 },
                SpPort { num: 2, node: NodeIdentifier::Anonymous(1), z0 },
            ],
            sweep: AcSweepAnalysisOptions { start_frequency: 1e6, stop_frequency: 1e6, steps: 1, logarithmic: false },
        };
        (circuit, options)
    }

    /// `port1 --wire-- n --wire-- port2`, shunt `C` from `n` to gnd: a
    /// T-junction 2-port (port1/port2 tied to the same electrical point via
    /// ideal wires on *distinct* port nodes — not the coincident-node
    /// SP-05 case) — a first-order low-pass in the port impedance.
    fn shunt_c_lowpass(cap: f64, z0: f64) -> (CircuitInstance, SpOptions) {
        let mut netlist = Netlist::new();
        let n1 = netlist.connect_node(NodeIdentifier::Anonymous(0));
        let n2 = netlist.connect_node(NodeIdentifier::Anonymous(1));
        let n_mid = netlist.connect_node(NodeIdentifier::Anonymous(2));
        let gnd = netlist.connect_node(NodeIdentifier::Gnd);
        let b1 = netlist.connect_branch(BranchIdentifier::from_component("w1"));
        let b2 = netlist.connect_branch(BranchIdentifier::from_component("w2"));

        let devices: Vec<Box<dyn Element>> = vec![
            Box::new(TestWire { p: n1.clone(), n: n_mid.clone(), branch: b1 }),
            Box::new(TestWire { p: n2.clone(), n: n_mid.clone(), branch: b2 }),
            Box::new(TestCapacitor { n1: n_mid.clone(), n2: gnd, c: cap }),
            Box::new(TestDcBias { n: n_mid }),
        ];
        let circuit = CircuitInstance::from_devices_and_netlist("lp", devices, netlist);
        let options = SpOptions {
            ports: vec![
                SpPort { num: 1, node: NodeIdentifier::Anonymous(0), z0 },
                SpPort { num: 2, node: NodeIdentifier::Anonymous(1), z0 },
            ],
            sweep: AcSweepAnalysisOptions {
                start_frequency: 1e3,
                stop_frequency: 1e9,
                steps: 5,
                logarithmic: true,
            },
        };
        (circuit, options)
    }

    /// A reciprocal L-C matching section: `port1 -- L -- n -- C to gnd -- port2`
    /// is out of reach without a branch-unknown inductor test double; instead
    /// use a symmetric shunt-C pi-like network: two equal shunt capacitors at
    /// each port node plus a series resistor between them — reciprocal,
    /// passive, and buildable from the existing test doubles.
    fn reciprocal_rc_network(rs: f64, cap: f64, z0: f64) -> (CircuitInstance, SpOptions) {
        let mut netlist = Netlist::new();
        let n1 = netlist.connect_node(NodeIdentifier::Anonymous(0));
        let n2 = netlist.connect_node(NodeIdentifier::Anonymous(1));
        let gnd = netlist.connect_node(NodeIdentifier::Gnd);

        let devices: Vec<Box<dyn Element>> = vec![
            Box::new(TestResistor { n1: n1.clone(), n2: n2.clone(), r: rs }),
            Box::new(TestCapacitor { n1: n1.clone(), n2: gnd.clone(), c: cap }),
            Box::new(TestCapacitor { n1: n2.clone(), n2: gnd, c: cap }),
            Box::new(TestDcBias { n: n1.clone() }),
            Box::new(TestDcBias { n: n2.clone() }),
        ];
        let circuit = CircuitInstance::from_devices_and_netlist("pi", devices, netlist);
        let options = SpOptions {
            ports: vec![
                SpPort { num: 1, node: NodeIdentifier::Anonymous(0), z0 },
                SpPort { num: 2, node: NodeIdentifier::Anonymous(1), z0 },
            ],
            sweep: AcSweepAnalysisOptions { start_frequency: 1e6, stop_frequency: 1e6, steps: 1, logarithmic: false },
        };
        (circuit, options)
    }

    // ── SP-02/SP-03: matched series-R attenuator ────────────────────────

    #[test]
    fn series_r_attenuator_matches_analytic_s11_s21() {
        // A series resistor Rs between two z0-referenced ports has the
        // textbook closed form (matched attenuator when Rs is chosen so
        // S11=0 is not required here — general Rs):
        //   S11 = S22 = Rs / (Rs + 2*z0)
        //   S21 = S12 = 2*z0 / (Rs + 2*z0)
        let (z0, rs) = (50.0, 50.0);
        let (mut circuit, options) = series_r_attenuator(rs, z0);
        let mut solver = SpSolver::new(&mut circuit, options, Context::default()).unwrap();
        let result = solver.solve_sweep().unwrap();

        assert_eq!(result.frequencies.len(), 1);
        let s = &result.s[0];
        let expected_s11 = rs / (rs + 2.0 * z0);
        let expected_s21 = 2.0 * z0 / (rs + 2.0 * z0);

        assert!((s[[0, 0]].re - expected_s11).abs() < 1e-6, "S11 = {:?}, expected {expected_s11}", s[[0, 0]]);
        assert!(s[[0, 0]].im.abs() < 1e-9, "S11 should be real: {:?}", s[[0, 0]]);
        assert!((s[[1, 1]].re - expected_s11).abs() < 1e-6, "S22 = {:?}, expected {expected_s11}", s[[1, 1]]);
        assert!((s[[1, 0]].re - expected_s21).abs() < 1e-6, "S21 = {:?}, expected {expected_s21}", s[[1, 0]]);
        assert!((s[[0, 1]].re - expected_s21).abs() < 1e-6, "S12 = {:?}, expected {expected_s21}", s[[0, 1]]);
    }

    #[test]
    fn reciprocal_network_has_s12_eq_s21_and_passive_sii() {
        let (mut circuit, options) = reciprocal_rc_network(200.0, 1e-9, 50.0);
        let mut solver = SpSolver::new(&mut circuit, options, Context::default()).unwrap();
        let result = solver.solve_sweep().unwrap();

        let s = &result.s[0];
        let diff = (s[[0, 1]] - s[[1, 0]]).norm();
        assert!(diff < 1e-9, "reciprocity: S12={:?} S21={:?} diff={diff}", s[[0, 1]], s[[1, 0]]);
        assert!(s[[0, 0]].norm() <= 1.0 + 1e-9, "passive: |S11| = {}", s[[0, 0]].norm());
        assert!(s[[1, 1]].norm() <= 1.0 + 1e-9, "passive: |S22| = {}", s[[1, 1]].norm());
    }

    // ── SP-04: shunt-C low-pass roll-off ────────────────────────────────

    #[test]
    fn shunt_c_lowpass_s21_rolls_off_with_frequency() {
        let (mut circuit, options) = shunt_c_lowpass(1e-9, 50.0);
        let mut solver = SpSolver::new(&mut circuit, options.clone(), Context::default()).unwrap();
        let result = solver.solve_sweep().unwrap();

        assert_eq!(result.frequencies.len(), 5);
        let mags: Vec<f64> = result.s.iter().map(|s| s[[1, 0]].norm()).collect();
        for w in mags.windows(2) {
            assert!(w[1] <= w[0] + 1e-9, "|S21| should be non-increasing with frequency: {mags:?}");
        }
        // Closed form (ABCD -> S for a shunt admittance Y, A=1,B=0,C=Y,D=1,
        // equal z0 both ports): S21 = 2 / (2 + z0*Y).
        for (k, &f) in options.sweep.generate_frequencies().iter().enumerate() {
            let omega = 2.0 * std::f64::consts::PI * f;
            let y = Complex64::new(0.0, omega * 1e-9);
            let expected = Complex64::new(2.0, 0.0) / (Complex64::new(2.0, 0.0) + 50.0 * y);
            let got = result.s[k][[1, 0]];
            assert!((got - expected).norm() < 1e-6, "S21 at f={f}: got {got:?}, expected {expected:?}");
        }
    }

    // ── SP-05: fail-loud paths ───────────────────────────────────────────

    #[test]
    fn zero_ports_fails_loud() {
        let mut netlist = Netlist::new();
        let _gnd = netlist.connect_node(NodeIdentifier::Gnd);
        let devices: Vec<Box<dyn Element>> = vec![];
        let mut circuit = CircuitInstance::from_devices_and_netlist("empty", devices, netlist);
        let options = SpOptions {
            ports: vec![],
            sweep: AcSweepAnalysisOptions { start_frequency: 1e6, stop_frequency: 1e6, steps: 1, logarithmic: false },
        };
        let result = SpSolver::new(&mut circuit, options, Context::default());
        assert!(result.is_err(), "zero ports must fail loud (SP-05)");
        assert!(result.err().unwrap().to_string().contains("at least one port"));
    }

    #[test]
    fn non_positive_z0_fails_loud() {
        let (mut circuit, mut options) = series_r_attenuator(50.0, 50.0);
        options.ports[0].z0 = 0.0;
        let result = SpSolver::new(&mut circuit, options, Context::default());
        assert!(result.is_err(), "non-positive z0 must fail loud (SP-05)");
        assert!(result.err().unwrap().to_string().contains("non-positive z0"));
    }

    #[test]
    fn degenerate_ground_port_fails_loud() {
        let (mut circuit, mut options) = series_r_attenuator(50.0, 50.0);
        options.ports[0].node = NodeIdentifier::Gnd;
        let result = SpSolver::new(&mut circuit, options, Context::default());
        assert!(result.is_err(), "a port on ground must fail loud (SP-05)");
        assert!(result.err().unwrap().to_string().contains("ground"));
    }

    #[test]
    fn duplicate_port_num_fails_loud() {
        let (mut circuit, mut options) = series_r_attenuator(50.0, 50.0);
        options.ports[1].num = options.ports[0].num;
        let result = SpSolver::new(&mut circuit, options, Context::default());
        assert!(result.is_err(), "duplicate port num must fail loud (SP-05)");
        assert!(result.err().unwrap().to_string().contains("duplicate"));
    }

    #[test]
    fn coincident_port_nodes_fail_loud() {
        // Edge case (spec.md): "WHEN .sp port nodes coincide (degenerate
        // zero-length port) -> fail loud." Two *different* port numbers
        // declared on the same node is a distinct failure from duplicate
        // num (which reuses the same `num`, not the same node).
        let (mut circuit, mut options) = series_r_attenuator(50.0, 50.0);
        options.ports[1].node = options.ports[0].node.clone();
        let result = SpSolver::new(&mut circuit, options, Context::default());
        assert!(result.is_err(), "coincident port nodes must fail loud (SP-05)");
        assert!(result.err().unwrap().to_string().contains("coincides"));
    }

    #[test]
    fn unknown_port_node_fails_loud() {
        let (mut circuit, mut options) = series_r_attenuator(50.0, 50.0);
        options.ports[0].node = NodeIdentifier::Anonymous(99);
        let result = SpSolver::new(&mut circuit, options, Context::default());
        assert!(result.is_err(), "a port on a node not in the circuit must fail loud (SP-05)");
        assert!(result.err().unwrap().to_string().contains("unknown node"));
    }
}


