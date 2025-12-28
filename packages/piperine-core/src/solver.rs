mod solver;

use faer::prelude::Solve;
use faer::sparse::linalg::solvers::SymbolicLu;
use faer::sparse::{SparseColMat, Triplet};
use faer::Col;
use std::collections::HashMap;

#[derive(Debug)]
pub struct PiperineError {
    pub description: String,
}

pub type PiperineResult<T> = Result<T, PiperineError>;

pub struct Variables<T> {
    pub values: Vec<T>,
}

#[derive(Debug, Clone)]
pub enum NodeRef {
    Gnd,
    Indexed(usize),
    Named(String),
}

pub struct TranOptions {
    pub tstep: f64,
    pub tstop: f64,
    pub ic: Vec<(NodeRef, f64)>,
}

pub(crate) enum NumericalIntegration {
    BE,
    TRAP,
}

pub struct AcOptions {
    pub fstart: usize,
    pub fstop: usize,
    pub npts: usize, // Total, not "per decade"
}

pub(crate) struct TranState {
    pub(crate) t: f64,
    pub(crate) dt: f64,
    pub(crate) vic: Vec<usize>,
    pub(crate) ric: Vec<usize>,
    pub(crate) ni: NumericalIntegration,
}

#[derive(Default)]
pub(crate) struct AcState {
    pub omega: f64,
}

pub(crate) enum AnalysisInfo<'a> {
    OP,
    TRAN(&'a TranOptions, &'a TranState),
    AC(&'a AcOptions, &'a AcState),
}

pub struct Options {
    pub temp: f64,
    pub tnom: f64,
    pub gmin: f64,
    pub iabstol: f64,
    pub reltol: f64,
    pub chgtol: f64,
    pub volt_tol: f64,
    pub trtol: usize,
    pub tran_max_iter: usize,
    pub dc_max_iter: usize,
    pub dc_trcv_max_iter: usize,
    pub integrate_method: usize,
    pub order: usize,
    pub max_order: usize,
    pub pivot_abs_tol: f64,
    pub pivot_rel_tol: f64,
    pub src_factor: f64,
    pub diag_gmin: f64,
}

impl Options {
    fn default() -> Self {
        Self {
            temp: 27.0,
            tnom: 27.0,
            gmin: 1e-12,
            iabstol: 1e-12,
            reltol: 1e-3,
            chgtol: 1e-14,
            volt_tol: 1e-6,
            trtol: 7,
            tran_max_iter: 150,
            dc_max_iter: 150,
            dc_trcv_max_iter: 500,
            integrate_method: 2,
            order: 2,
            max_order: 2,
            pivot_abs_tol: 1e-20,
            pivot_rel_tol: 1e-4,
            src_factor: 1.0,
            diag_gmin: 1e-12,
        }
    }
}

pub enum Stamp<T> {
    /// Contribution to the A matrix: (row, col, value)
    Matrix { r: usize, c: usize, value: T },
    /// Contribution to the RHS vector b: (row, value)
    Rhs { r: usize, value: T },
}

pub trait Matrix {
    fn get_variable_index(&self, node: &NodeRef) -> usize;
    fn allocate_internal_variable(&mut self) -> usize;
}

pub trait Component {
    /// Commit operating-point guesses to internal state
    fn commit(&mut self, guess: &Variables<f64>) {}
    /// Update values of single-valued components
    /// FIXME: prob not for every Component
    fn update(&mut self, _val: f64) {}
    /// Validation of parameter Values etc.
    fn validate(&self) -> PiperineResult<()> {
        Ok(())
    }

    /// Logic for DC/Transient: Look at current guesses, return Stamps
    fn load(
        &mut self,
        guess: &Variables<f64>,
        an: &AnalysisInfo,
        opts: &Options,
    ) -> Vec<Stamp<f64>>;

    /// This is where the component "remembers" its connections to the matrix
    fn setup(&mut self, mat: &mut dyn Matrix) -> PiperineResult<()>;
    /// Create matrix elements, adding them to mutable Matrix `mat`
    fn create_matrix_elems(&mut self, mat: &dyn Matrix) {}
}

pub struct Resistor {
    pub p: NodeRef,
    pub n: NodeRef,
    pub value: f64,
    idx_p: Option<usize>,
    idx_n: Option<usize>,
}

impl Component for Resistor {
    fn setup(&mut self, mat: &mut dyn Matrix) -> PiperineResult<()> {
        self.idx_p = match self.p {
            NodeRef::Gnd => None,
            _ => Some(mat.get_variable_index(&self.p)),
        };
        self.idx_n = match self.n {
            NodeRef::Gnd => None,
            _ => Some(mat.get_variable_index(&self.n)),
        };
        Ok(())
    }

    fn load(&mut self, _: &Variables<f64>, _: &AnalysisInfo, _: &Options) -> Vec<Stamp<f64>> {
        let mut stamps = Vec::new();
        let g = 1.0 / self.value;
        let nodes = [
            (self.idx_p, self.idx_p, g),
            (self.idx_n, self.idx_n, g),
            (self.idx_p, self.idx_n, -g),
            (self.idx_n, self.idx_p, -g),
        ];
        for (r_opt, c_opt, val) in nodes {
            if let (Some(r), Some(c)) = (r_opt, c_opt) {
                stamps.push(Stamp::Matrix { r, c, value: val });
            }
        }
        stamps
    }
}

// --- Component: Voltage Source ---
//
// pub struct VoltageSource {
//     pub p: NodeRef,
//     pub n: NodeRef,
//     pub value: f64,
//     idx_p: Option<usize>,
//     idx_n: Option<usize>,
//     idx_ibr: usize, // The index for the current through this source
// }
//
// impl VoltageSource {
//     pub fn new(p: NodeRef, n: NodeRef, value: f64) -> Self {
//         Self {
//             p,
//             n,
//             value,
//             idx_p: None,
//             idx_n: None,
//             idx_ibr: 0,
//         }
//     }
// }
//
// impl Component for VoltageSource {
//     fn setup(&mut self, mat: &mut dyn Matrix) -> PiperineResult<()> {
//         self.idx_p = match self.p {
//             NodeRef::Gnd => None,
//             _ => Some(mat.get_variable_index(&self.p)),
//         };
//         self.idx_n = match self.n {
//             NodeRef::Gnd => None,
//             _ => Some(mat.get_variable_index(&self.n)),
//         };
//         // Voltage sources MUST allocate a new variable for their branch current
//         self.idx_ibr = mat.allocate_internal_variable();
//         Ok(())
//     }
//
//     fn load(&mut self, _: &Variables<f64>, _: &AnalysisInfo, _: &Options) -> Vec<Stamp<f64>> {
//         let mut stamps = Vec::new();
//         let k = self.idx_ibr;
//
//         // KCL stamps (current leaving P and entering N)
//         if let Some(p) = self.idx_p {
//             stamps.push(Stamp::Matrix {
//                 r: p,
//                 c: k,
//                 value: 1.0,
//             });
//             stamps.push(Stamp::Matrix {
//                 r: k,
//                 c: p,
//                 value: 1.0,
//             });
//         }
//         if let Some(n) = self.idx_n {
//             stamps.push(Stamp::Matrix {
//                 r: n,
//                 c: k,
//                 value: -1.0,
//             });
//             stamps.push(Stamp::Matrix {
//                 r: k,
//                 c: n,
//                 value: -1.0,
//             });
//         }
//
//         // RHS stamp for the voltage constraint
//         stamps.push(Stamp::Rhs {
//             r: k,
//             value: self.value,
//         });
//         stamps
//     }
// }

pub struct VoltageSource {
    pub p: NodeRef,
    pub n: NodeRef,
    pub amplitude: f64,
    pub freq: f64,
    pub phase: f64,
    pub dc_offset: f64,
    idx_p: Option<usize>,
    idx_n: Option<usize>,
    idx_ibr: usize,
}

impl VoltageSource {
    pub fn sine(p: NodeRef, n: NodeRef, amp: f64, freq: f64) -> Self {
        Self {
            p,
            n,
            amplitude: amp,
            freq,
            phase: 0.0,
            dc_offset: 0.0,
            idx_p: None,
            idx_n: None,
            idx_ibr: 0,
        }
    }
}

impl Component for VoltageSource {
    fn setup(&mut self, mat: &mut dyn Matrix) -> PiperineResult<()> {
        self.idx_p = match self.p {
            NodeRef::Gnd => None,
            _ => Some(mat.get_variable_index(&self.p)),
        };
        self.idx_n = match self.n {
            NodeRef::Gnd => None,
            _ => Some(mat.get_variable_index(&self.n)),
        };
        self.idx_ibr = mat.allocate_internal_variable();
        Ok(())
    }

    fn load(&mut self, _: &Variables<f64>, an: &AnalysisInfo, _: &Options) -> Vec<Stamp<f64>> {
        let mut stamps = Vec::new();
        let k = self.idx_ibr;

        // Determine current voltage value based on analysis type
        let v_now = match an {
            AnalysisInfo::OP => self.dc_offset, // At DC, sine is just the offset
            AnalysisInfo::TRAN(_, state) => {
                // V(t) = Offset + Amp * sin(2*pi*f*t + phase)
                self.dc_offset
                    + self.amplitude
                    * (2.0 * std::f64::consts::PI * self.freq * state.t + self.phase).sin()
            }
            _ => self.dc_offset,
        };

        if let Some(p) = self.idx_p {
            stamps.push(Stamp::Matrix {
                r: p,
                c: k,
                value: 1.0,
            });
            stamps.push(Stamp::Matrix {
                r: k,
                c: p,
                value: 1.0,
            });
        }
        if let Some(n) = self.idx_n {
            stamps.push(Stamp::Matrix {
                r: n,
                c: k,
                value: -1.0,
            });
            stamps.push(Stamp::Matrix {
                r: k,
                c: n,
                value: -1.0,
            });
        }
        stamps.push(Stamp::Rhs { r: k, value: v_now });
        stamps
    }
}

pub struct Diode {
    pub p: NodeRef,
    pub n: NodeRef,
    pub is: f64, // Saturation current (e.g., 1e-14)
    idx_p: Option<usize>,
    idx_n: Option<usize>,
}

impl Component for Diode {
    fn setup(&mut self, mat: &mut dyn Matrix) -> PiperineResult<()> {
        self.idx_p = match self.p {
            NodeRef::Gnd => None,
            _ => Some(mat.get_variable_index(&self.p)),
        };
        self.idx_n = match self.n {
            NodeRef::Gnd => None,
            _ => Some(mat.get_variable_index(&self.n)),
        };
        Ok(())
    }

    fn load(
        &mut self,
        guess: &Variables<f64>,
        _: &AnalysisInfo,
        opts: &Options,
    ) -> Vec<Stamp<f64>> {
        let v_p = self.idx_p.map(|i| guess.values[i]).unwrap_or(0.0);
        let v_n = self.idx_n.map(|i| guess.values[i]).unwrap_or(0.0);
        let vd = v_p - v_n;

        let vt = (1.380649e-23 * (opts.temp + 273.15)) / 1.602176634e-19;

        // Use a limit to prevent exp(1000) from exploding
        let vd_limited = if vd > 0.8 { 0.8 } else { vd };
        let exp_term = (vd_limited / vt).exp();

        let id = self.is * (exp_term - 1.0);
        let gd = (self.is / vt) * exp_term + opts.gmin; // Use opts.gmin here!
        let ieq = id - gd * vd;

        let mut stamps = Vec::new();
        let nodes = [
            (self.idx_p, self.idx_p, gd),
            (self.idx_n, self.idx_n, gd),
            (self.idx_p, self.idx_n, -gd),
            (self.idx_n, self.idx_p, -gd),
        ];

        for (r_opt, c_opt, val) in nodes {
            if let (Some(r), Some(c)) = (r_opt, c_opt) {
                stamps.push(Stamp::Matrix { r, c, value: val });
            }
        }
        // Current flows from P to N, so P gets -Ieq on RHS, N gets +Ieq
        if let Some(p) = self.idx_p {
            stamps.push(Stamp::Rhs { r: p, value: -ieq });
        }
        if let Some(n) = self.idx_n {
            stamps.push(Stamp::Rhs { r: n, value: ieq });
        }

        stamps
    }
}

// --- Component: Capacitor ---
pub struct Capacitor {
    pub p: NodeRef,
    pub n: NodeRef,
    pub value: f64,
    idx_p: Option<usize>,
    idx_n: Option<usize>,
    // Capacitors must store the voltage from the PREVIOUS time step
    v_prev: f64,
}

impl Capacitor {
    pub fn new(p: NodeRef, n: NodeRef, value: f64) -> Self {
        Self {
            p,
            n,
            value,
            idx_p: None,
            idx_n: None,
            v_prev: 0.0,
        }
    }
}

impl Component for Capacitor {
    fn setup(&mut self, mat: &mut dyn Matrix) -> PiperineResult<()> {
        self.idx_p = match self.p {
            NodeRef::Gnd => None,
            _ => Some(mat.get_variable_index(&self.p)),
        };
        self.idx_n = match self.n {
            NodeRef::Gnd => None,
            _ => Some(mat.get_variable_index(&self.n)),
        };
        Ok(())
    }

    fn commit(&mut self, guess: &Variables<f64>) {
        // After a time-step converges, save the voltage for the next step
        let v_p = self.idx_p.map(|i| guess.values[i]).unwrap_or(0.0);
        let v_n = self.idx_n.map(|i| guess.values[i]).unwrap_or(0.0);
        self.v_prev = v_p - v_n;
    }

    fn load(&mut self, _: &Variables<f64>, an: &AnalysisInfo, _: &Options) -> Vec<Stamp<f64>> {
        let mut stamps = Vec::new();

        match an {
            AnalysisInfo::OP => {
                // Add a tiny path to ground so the node isn't floating during the first guess
                let mut stamps = Vec::new();
                if let Some(p) = self.idx_p {
                    stamps.push(Stamp::Matrix { r: p, c: p, value: 1e-12 });
                }
                if let Some(n) = self.idx_n {
                    stamps.push(Stamp::Matrix { r: n, c: n, value: 1e-12 });
                }
            }
            AnalysisInfo::TRAN(_, state) => {
                let g_eq = self.value / state.dt;
                let i_eq = g_eq * self.v_prev;

                let nodes = [
                    (self.idx_p, self.idx_p, g_eq),
                    (self.idx_n, self.idx_n, g_eq),
                    (self.idx_p, self.idx_n, -g_eq),
                    (self.idx_n, self.idx_p, -g_eq),
                ];

                for (r_opt, c_opt, val) in nodes {
                    if let (Some(r), Some(c)) = (r_opt, c_opt) {
                        stamps.push(Stamp::Matrix { r, c, value: val });
                    }
                }
                // Current source portion of the companion model
                if let Some(p) = self.idx_p {
                    stamps.push(Stamp::Rhs { r: p, value: i_eq });
                }
                if let Some(n) = self.idx_n {
                    stamps.push(Stamp::Rhs { r: n, value: -i_eq });
                }
            }
            _ => {}
        }
        stamps
    }
}

// --- Solver Implementation ---

pub struct FaerSolver {
    pub next_idx: usize,
    pub nodes: HashMap<String, usize>,
}

impl Matrix for FaerSolver {
    fn get_variable_index(&self, node: &NodeRef) -> usize {
        match node {
            NodeRef::Indexed(i) => *i,
            NodeRef::Named(s) => *self
                .nodes
                .get(s)
                .expect(format!("Could not find node {:?}", node).as_str()),
            NodeRef::Gnd => panic!("Ground has no index"),
        }
    }
    fn allocate_internal_variable(&mut self) -> usize {
        let idx = self.next_idx;
        self.next_idx += 1;
        idx
    }
}

impl FaerSolver {
    pub fn new() -> Self {
        Self {
            next_idx: 0,
            nodes: HashMap::new(),
        }
    }

    pub fn solve_op(
        &self,
        components: &mut [Box<dyn Component>],
    ) -> PiperineResult<Variables<f64>> {
        let opts = Options::default();
        let mut guess = Variables {
            values: vec![0.0; self.next_idx],
        };

        // --- 1. Symbolic Analysis (Topology remains same) ---
        // We build a "dummy" matrix just to get the sparsity pattern
        let mut triplets = Vec::new();
        for comp in components.iter_mut() {
            for s in comp.load(&guess, &AnalysisInfo::OP, &opts) {
                if let Stamp::Matrix { r, c, .. } = s {
                    triplets.push(Triplet::new(r, c, 1.0));
                }
            }
        }
        let a_dummy = SparseColMat::<usize, f64>::try_new_from_triplets(
            self.next_idx,
            self.next_idx,
            &triplets,
        )
            .unwrap();
        let symbolic = SymbolicLu::try_new(a_dummy.symbolic()).unwrap();

        // --- 2. Newton-Raphson Loop ---
        for i in 0..255 {
            let mut triplets = Vec::new();
            let mut b_vec = vec![0.0; self.next_idx];

            for comp in components.iter_mut() {
                for stamp in comp.load(&guess, &AnalysisInfo::OP, &opts) {
                    match stamp {
                        Stamp::Matrix { r, c, value } => triplets.push(Triplet::new(r, c, value)),
                        Stamp::Rhs { r, value } => b_vec[r] += value,
                    }
                }
            }

            let a = SparseColMat::<usize, f64>::try_new_from_triplets(
                self.next_idx,
                self.next_idx,
                &triplets,
            )
                .unwrap();
            let b = Col::from_fn(self.next_idx, |i| b_vec[i]);
            let lu = faer::sparse::linalg::solvers::Lu::try_new_with_symbolic(
                symbolic.clone(),
                a.as_ref(),
            )
                .unwrap();
            let next_values_col = lu.solve(&b);
            let next_values: Vec<f64> = next_values_col.iter().copied().collect();

            // --- 3. Convergence Check ---
            let mut converged = true;
            for (v_new, v_old) in next_values.iter().zip(guess.values.iter()) {
                let diff = (v_new - v_old).abs();
                let limit = opts.reltol * v_new.abs().max(v_old.abs()) + opts.volt_tol;
                if diff > limit {
                    converged = false;
                    break;
                }
            }

            guess.values = next_values;
            if converged {
                println!("Converged in {} iterations.", i + 1);
                return Ok(guess);
            }
        }

        Err(PiperineError {
            description: "Newton-Raphson failed to converge".into(),
        })
    }

    pub fn solve_tran(
        &self,
        components: &mut [Box<dyn Component>],
        tran_options: TranOptions,
    ) -> Vec<(f64, Vec<f64>)> {
        let mut results = Vec::new();
        let mut current_time = 0.0;
        let opts = Options::default();

        // Start with an Initial Guess (usually the results of an OP analysis)
        let mut current_guess = Variables {
            values: vec![0.0; self.next_idx],
        };

        // Pre-analyze topology (assuming it doesn't change)
        let mut triplets = Vec::new();
        // Use a dummy state for pattern analysis
        let dummy_state = TranState {
            t: 0.0,
            dt: tran_options.tstep,
            vic: vec![],
            ric: vec![],
            ni: NumericalIntegration::BE,
        };
        let dummy_info = AnalysisInfo::TRAN(&tran_options, &dummy_state);
        for comp in components.iter_mut() {
            for s in comp.load(&current_guess, &dummy_info, &opts) {
                if let Stamp::Matrix { r, c, .. } = s {
                    triplets.push(Triplet::new(r, c, 1.0));
                }
            }
        }
        let a_dummy = SparseColMat::<usize, f64>::try_new_from_triplets(
            self.next_idx,
            self.next_idx,
            &triplets,
        )
            .unwrap();
        let symbolic = SymbolicLu::try_new(a_dummy.symbolic()).unwrap();

        println!("Starting Transient Simulation...");
        println!("Time (ms) | V(node1) | V(node2)");
        println!("--------------------------------");

        while current_time <= tran_options.tstop {
            let state = TranState {
                t: current_time,
                dt: tran_options.tstep,
                vic: vec![],
                ric: vec![],
                ni: NumericalIntegration::BE,
            };
            let info = AnalysisInfo::TRAN(&tran_options, &state);

            // --- Newton-Raphson Loop for this time point ---
            let mut converged = false;
            for _iter in 0..opts.tran_max_iter {
                let mut triplets = Vec::new();
                let mut b_vec = vec![0.0; self.next_idx];

                for comp in components.iter_mut() {
                    for stamp in comp.load(&current_guess, &info, &opts) {
                        match stamp {
                            Stamp::Matrix { r, c, value } => {
                                triplets.push(Triplet::new(r, c, value))
                            }
                            Stamp::Rhs { r, value } => b_vec[r] += value,
                        }
                    }
                }

                let a = SparseColMat::<usize, f64>::try_new_from_triplets(
                    self.next_idx,
                    self.next_idx,
                    &triplets,
                )
                    .unwrap();
                let b = Col::from_fn(self.next_idx, |i| b_vec[i]);
                let lu = faer::sparse::linalg::solvers::Lu::try_new_with_symbolic(
                    symbolic.clone(),
                    a.as_ref(),
                )
                    .unwrap();
                let next_values_col = lu.solve(&b);
                let next_values: Vec<f64> = next_values_col.iter().copied().collect();

                // Check Convergence
                let mut local_converged = true;
                for (v_new, v_old) in next_values.iter().zip(current_guess.values.iter()) {
                    if (v_new - v_old).abs()
                        > (opts.reltol * v_new.abs().max(v_old.abs()) + opts.volt_tol)
                    {
                        local_converged = false;
                        break;
                    }
                }

                current_guess.values = next_values;
                if local_converged {
                    converged = true;
                    break;
                }
            }

            if !converged {
                eprintln!("Warning: Timestep at {}s failed to converge!", current_time);
            }

            // SUCCESS: Commit this time point and move forward
            for comp in components.iter_mut() {
                comp.commit(&current_guess);
            }

            results.push((current_time, current_guess.values.clone()));

            // Print progress every 1ms
            if (current_time * 1000.0).round() % 1.0 == 0.0 {
                println!(
                    "{:9.3} {:8.2}",
                    current_time * 1000.0,
                    current_guess.values[0],
                    // current_guess.values[1]
                );
            }

            current_time += tran_options.tstep;
        }
        results
    }
}

fn solve() {
    let mut solver = FaerSolver::new();
    let vcc = solver.allocate_internal_variable(); // Vcc node
    let n1 = solver.allocate_internal_variable(); // Node 1
    let n2 = solver.allocate_internal_variable(); // Node 2
    solver.nodes.insert("vcc".into(), vcc);
    solver.nodes.insert("1".into(), n1);
    solver.nodes.insert("2".into(), n2);

    // Circuit: 5V Source -> 100 Ohm Resistor -> Diode -> 10uF Capacitor to Ground
    // Node 2 is the capacitor voltage
    let mut components: Vec<Box<dyn Component>> = vec![
        Box::new(VoltageSource::sine(
            NodeRef::Named("vcc".into()),
            NodeRef::Gnd,
            10.0,
            50.0,
        )),
        // Series Diode (Rectifier)
        Box::new(Diode {
            p: NodeRef::Named("vcc".into()),
            n: NodeRef::Named("2".into()),
            is: 1e-14,
            idx_p: None,
            idx_n: None,
        }),
        // RC Filter
        Box::new(Resistor {
            p: NodeRef::Named("2".into()),
            n: NodeRef::Gnd,
            value: 100.0,
            idx_p: None,
            idx_n: None,
        }),
        Box::new(Capacitor::new(
            NodeRef::Named("2".into()),
            NodeRef::Gnd,
            100e-6,
        )), // 100uF
    ];

    for comp in &mut components {
        comp.setup(&mut solver).unwrap();
    }

    // Run Transient analysis: 10ms stop time, 0.1ms steps
    let tran_opts = TranOptions {
        tstep: 0.0005, // 0.5ms steps
        tstop: 0.040,
        ic: vec![],
    };

    let _results = solver.solve_tran(&mut components, tran_opts);
}
