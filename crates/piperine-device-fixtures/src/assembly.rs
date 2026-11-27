//! Circuit and system helpers of the analog assembly's tests: a netlist of numbered nodes and branch
//! unknowns, an instance built from a fixture module, a `NonLinearSystem` over the device assembly so the
//! tests solve circuits with the real Newton engine of the analog crate, and a dense view of a stamp list
//! for comparing stamps against hand-computed values.

use ndarray::Array1;
use piperine_abi::caller::Handle;
use piperine_abi::raw::Module;
use piperine_analog_solver::convergence::NoDamping;
use piperine_analog_solver::device::{AnalogBinding, AnalogInstance, Ambient, Assembly, Stage};
use piperine_analog_solver::error::Result;
use piperine_analog_solver::math::circular_array::CircularArrayBuffer2;
use piperine_analog_solver::math::faer::FaerSparseLinearSystem;
use piperine_analog_solver::math::iv::InitialValue;
use piperine_analog_solver::math::linear::Stamp;
use piperine_analog_solver::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use piperine_analog_solver::netlist::{AnalogReference, BranchIdentifier, Netlist, NodeIdentifier};
use piperine_analog_solver::tolerances::{Policy, Tolerances};

/// The stamps of one assembly.
pub type Stamps = Vec<Stamp<AnalogReference, f64>>;
/// The Newton solver over the assembly's unknowns.
pub type Solver = NewtonRaphsonSolver<AnalogReference, f64>;

/// An equilibrium solve over an assembly.
pub struct Dc<'a> {
    /// The asm.
    pub assembly: &'a mut Assembly,
    /// The net.
    pub net: &'a Netlist,
    /// The amb.
    pub ambient: Ambient,
}

/// One transient stage over an assembly.
pub struct Tran<'a> {
    /// The asm.
    pub assembly: &'a mut Assembly,
    /// The net.
    pub net: &'a Netlist,
    /// The amb.
    pub ambient: Ambient,
    /// The stage.
    pub stage: Stage,
}

impl NonLinearSystem<AnalogReference, f64> for Tran<'_> {
    fn assemble(&mut self, state: &CircularArrayBuffer2<f64>) -> Result<Stamps> {
        self.assembly.assemble_tran(&AssemblySolves::iterate(state), self.ambient, self.stage)
    }
    fn netlist(&self) -> &Netlist {
        self.net
    }
    fn before_iter_callback(&mut self, _state: &CircularArrayBuffer2<f64>, iteration: usize) {
        if iteration > 0 {
            self.assembly.end_iteration();
        }
    }
    fn any_limiting_report(&self) -> bool {
        !self.assembly.limiting_active().is_empty()
    }
}

impl NonLinearSystem<AnalogReference, f64> for Dc<'_> {
    fn assemble(&mut self, state: &CircularArrayBuffer2<f64>) -> Result<Stamps> {
        self.assembly.assemble_dc(&AssemblySolves::iterate(state), self.ambient)
    }
    fn netlist(&self) -> &Netlist {
        self.net
    }
    /// Every iteration after the first starts by advancing the limits.
    fn before_iter_callback(&mut self, _state: &CircularArrayBuffer2<f64>, iteration: usize) {
        if iteration > 0 {
            self.assembly.end_iteration();
        }
    }
    /// An active limit vetoes convergence.
    fn any_limiting_report(&self) -> bool {
        !self.assembly.limiting_active().is_empty()
    }
}

/// The netlists and instances of the analog assembly's tests.
pub struct Circuits;

impl Circuits {
    /// A netlist holding `nodes` node unknowns, then one branch unknown per name
    /// of `branches`, in that order.
    pub fn netlist(nodes: usize, branches: &[&str]) -> (Netlist, Vec<AnalogReference>) {
        let mut net = Netlist::new();
        let mut refs: Vec<AnalogReference> =
            (0..nodes).map(|i| net.connect_node(NodeIdentifier::Anonymous(i))).collect();
        refs.extend(branches.iter().map(|b| net.connect_branch(BranchIdentifier::from(*b))));
        (net, refs)
    }

    /// An instance of `module` at `path` with these bindings.
    pub fn instance(path: &str, module: &'static Module, pins: Vec<AnalogBinding>) -> AnalogInstance {
        let handle = Handle::create(module, &[]).unwrap_or_else(|e| panic!("create {path}: {e}"));
        AnalogInstance { path: path.into(), handle, pins }
    }

    /// `Unknown(r)`.
    pub fn at(r: &AnalogReference) -> AnalogBinding {
        AnalogBinding::Unknown(r.clone())
    }

    /// An instance whose `tag` param (pin 0) is `tag`, for the recording fixtures.
    pub fn tagged(path: &str, module: &'static Module, tag: u64, pins: Vec<AnalogBinding>) -> AnalogInstance {
        let bytes = (tag as f64).to_le_bytes();
        let handle = Handle::create(module, &[(0, &bytes)]).unwrap_or_else(|e| panic!("create {path}: {e}"));
        AnalogInstance { path: path.into(), handle, pins }
    }

    /// The eight bytes of an `F64`.
    pub fn real_bytes(v: f64) -> [u8; 8] {
        v.to_le_bytes()
    }

    /// The `F64` of eight bytes.
    pub fn real_of(b: &[u8]) -> f64 {
        f64::from_le_bytes(b[..8].try_into().expect("8 bytes"))
    }
}

/// The ambients, solves and dense views of the analog assembly's tests.
pub struct AssemblySolves;

impl AssemblySolves {
    /// The transient ambient at time `t`.
    pub fn at_time(t: f64) -> Ambient {
        Ambient { time: t, freq: 0.0, is_static: false, gmin: 1e-12, source_scale: 1.0 }
    }

    /// The iterate a system assembles at: the buffer's latest row, else zeros.
    pub fn iterate(state: &CircularArrayBuffer2<f64>) -> Vec<f64> {
        state.latest().map_or_else(|| vec![0.0; state.size()], |r| r.to_vec())
    }

    /// The equilibrium ambient at rest.
    pub fn at_rest() -> Ambient {
        Ambient { time: 0.0, freq: 0.0, is_static: true, gmin: 1e-12, source_scale: 1.0 }
    }

    /// The unknown count of a netlist.
    pub fn size(net: &Netlist) -> usize {
        net.max_index().map_or(0, |i| i + 1)
    }

    /// Solves an equilibrium from `start` (one value per unknown) with the
    /// engine's own Newton loop and the default tolerances.
    pub fn solve_dc(assembly: &mut Assembly, net: &Netlist, start: &[f64]) -> Array1<f64> {
        AssemblySolves::solve_dc_with(assembly, net, start, &Tolerances::default())
    }

    /// Tolerances far tighter than the defaults, for comparing two solves of one
    /// equation that took different Newton paths: the defaults stop each within
    /// `1e-3` relative of the root, so two paths may differ by more than that.
    pub fn tight() -> Tolerances {
        Tolerances { reltol: 1e-10, vntol: 1e-12, abstol: 1e-14, ..Tolerances::default() }
    }

    /// [`solve_dc`] under `tol`.
    pub fn solve_dc_with(assembly: &mut Assembly, net: &Netlist, start: &[f64], tol: &Tolerances) -> Array1<f64> {
        let mut sys = Dc { assembly, net, ambient: AssemblySolves::at_rest() };
        let mut solver = Solver::new(&mut sys, AssemblySolves::size(net), 4).expect("dry run");
        let ivs = net
            .all_references()
            .into_iter()
            .filter_map(|r| r.idx().map(|i| InitialValue { reference: r.clone(), value: start[i] }))
            .collect();
        solver.push_initial_conditions(ivs);
        solver.solve_with_strategy(&mut sys, &NoDamping, tol, &Policy::default()).expect("the Newton solve converges")
    }

    /// Solves one transient stage with the engine's own Newton loop, then
    /// records the converged solution as the stage's history row.
    pub fn solve_stage(solver: &mut Solver, assembly: &mut Assembly, net: &Netlist, ambient: Ambient, stage: Stage) -> Array1<f64> {
        let mut sys = Tran { assembly, net, ambient, stage };
        let x = solver
            .solve_with_strategy(&mut sys, &NoDamping, &Tolerances::default(), &Policy::default())
            .expect("the stage converges");
        sys.assembly.push_stage(x.as_slice().expect("contiguous"), ambient).expect("push the stage");
        x
    }

    /// The dense matrix and right side a stamp list sums to.
    pub fn dense(stamps: &Stamps, n: usize) -> (Vec<Vec<f64>>, Vec<f64>) {
        let (mut a, mut b) = (vec![vec![0.0; n]; n], vec![0.0; n]);
        for s in stamps {
            match s {
                Stamp::Matrix(r, c, v) => {
                    if let (Some(r), Some(c)) = (r.idx(), c.idx()) {
                        a[r][c] += v;
                    }
                }
                Stamp::Rhs(r, v) => {
                    if let Some(r) = r.idx() {
                        b[r] += v;
                    }
                }
            }
        }
        (a, b)
    }

    /// The `(row, column)` positions of the matrix stamps, each once.
    pub fn matrix_positions(stamps: &Stamps) -> std::collections::HashSet<(usize, usize)> {
        stamps
            .iter()
            .filter_map(|s| match s {
                Stamp::Matrix(r, c, _) => Some((r.idx()?, c.idx()?)),
                Stamp::Rhs(..) => None,
            })
            .collect()
    }

    /// `|a − b| ≤ rel·max(|a|, |b|)`, reported with both values.
    pub fn close(a: f64, b: f64, rel: f64, what: &str) {
        assert!((a - b).abs() <= rel * a.abs().max(b.abs()), "{what}: {a} vs {b} (rel {rel})");
    }

    /// The ambient of a small-signal sweep point at `freq` hertz.
    pub fn at_frequency(freq: f64) -> Ambient {
        Ambient { time: 0.0, freq, is_static: false, gmin: 1e-12, source_scale: 1.0 }
    }

    /// Solves a complex system with this crate's sparse LU.
    pub fn solve_complex(
        n: usize,
        matrix: Vec<Stamp<AnalogReference, num_complex::Complex64>>,
        rhs: Vec<Stamp<AnalogReference, num_complex::Complex64>>,
    ) -> Array1<num_complex::Complex64> {
        use piperine_analog_solver::math::faer::FaerSymbolicMatrix;
        let symbolic = FaerSymbolicMatrix::new(n, matrix.clone()).expect("symbolic");
        let mut sys = FaerSparseLinearSystem::<num_complex::Complex64>::new(n);
        sys.apply_stamps(matrix);
        sys.apply_stamps(rhs);
        sys.solve_with_backend(&symbolic).expect("solve")
    }

    /// The dense complex matrix and right side of a small-signal system.
    pub fn dense_complex(
        matrix: &[Stamp<AnalogReference, num_complex::Complex64>],
        rhs: &[Stamp<AnalogReference, num_complex::Complex64>],
        n: usize,
    ) -> (Vec<Vec<num_complex::Complex64>>, Vec<num_complex::Complex64>) {
        let zero = num_complex::Complex64::new(0.0, 0.0);
        let (mut a, mut b) = (vec![vec![zero; n]; n], vec![zero; n]);
        for s in matrix.iter().chain(rhs) {
            match s {
                Stamp::Matrix(r, c, v) => {
                    if let (Some(r), Some(c)) = (r.idx(), c.idx()) {
                        a[r][c] += v;
                    }
                }
                Stamp::Rhs(r, v) => {
                    if let Some(r) = r.idx() {
                        b[r] += v;
                    }
                }
            }
        }
        (a, b)
    }
}
