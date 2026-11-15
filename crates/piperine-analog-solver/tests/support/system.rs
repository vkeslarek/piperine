//! A `NonLinearSystem` over the device assembly, so the analog targets solve
//! circuits with the real Newton engine of this crate (`solve_with_strategy`,
//! `DampedNewton`, the default tolerances and policy), and a dense view of a
//! stamp list for comparing stamps against hand-computed values.

use ndarray::Array1;
use piperine_analog_solver::convergence::DampedNewton;
use piperine_analog_solver::device::{Ambient, Assembly, Stage};
use piperine_analog_solver::error::Result;
use piperine_analog_solver::math::circular_array::CircularArrayBuffer2;
use piperine_analog_solver::math::faer::FaerSparseLinearSystem;
use piperine_analog_solver::math::iv::InitialValue;
use piperine_analog_solver::math::linear::Stamp;
use piperine_analog_solver::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use piperine_analog_solver::netlist::{AnalogReference, Netlist};
use piperine_analog_solver::tolerances::{Policy, Tolerances};

pub type Stamps = Vec<Stamp<AnalogReference, f64>>;
pub type Solver = NewtonRaphsonSolver<AnalogReference, f64, FaerSparseLinearSystem<f64>>;

/// An equilibrium solve over an assembly.
pub struct Dc<'a> {
    pub asm: &'a mut Assembly,
    pub net: &'a Netlist,
    pub amb: Ambient,
}

/// One transient stage over an assembly.
pub struct Tran<'a> {
    pub asm: &'a mut Assembly,
    pub net: &'a Netlist,
    pub amb: Ambient,
    pub stage: Stage,
}

impl NonLinearSystem<AnalogReference, f64> for Tran<'_> {
    fn assemble(&mut self, state: &CircularArrayBuffer2<f64>) -> Result<Stamps> {
        self.asm.assemble_tran(&iterate(state), self.amb, self.stage)
    }
    fn netlist(&self) -> &Netlist {
        self.net
    }
}

/// The transient ambient at time `t`.
pub fn at_time(t: f64) -> Ambient {
    Ambient { time: t, freq: 0.0, is_static: false, gmin: 1e-12, source_scale: 1.0 }
}

/// The iterate a system assembles at: the buffer's latest row, else zeros.
pub fn iterate(state: &CircularArrayBuffer2<f64>) -> Vec<f64> {
    state.latest().map_or_else(|| vec![0.0; state.size()], |r| r.to_vec())
}

impl NonLinearSystem<AnalogReference, f64> for Dc<'_> {
    fn assemble(&mut self, state: &CircularArrayBuffer2<f64>) -> Result<Stamps> {
        self.asm.assemble_dc(&iterate(state), self.amb)
    }
    fn netlist(&self) -> &Netlist {
        self.net
    }
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
/// engine's own Newton loop.
pub fn solve_dc(asm: &mut Assembly, net: &Netlist, start: &[f64]) -> Array1<f64> {
    let mut sys = Dc { asm, net, amb: at_rest() };
    let mut solver = Solver::new(&mut sys, size(net), 4).expect("dry run");
    let ivs = net
        .all_references()
        .into_iter()
        .filter_map(|r| r.idx().map(|i| InitialValue { reference: r.clone(), value: start[i] }))
        .collect();
    solver.push_initial_conditions(ivs);
    solver
        .solve_with_strategy(&mut sys, &DampedNewton, &Tolerances::default(), &Policy::default())
        .expect("the Newton solve converges")
}

/// Solves one transient stage with the engine's own Newton loop, then
/// records the converged solution as the stage's history row.
pub fn solve_stage(solver: &mut Solver, asm: &mut Assembly, net: &Netlist, amb: Ambient, stage: Stage) -> Array1<f64> {
    let mut sys = Tran { asm, net, amb, stage };
    let x = solver
        .solve_with_strategy(&mut sys, &DampedNewton, &Tolerances::default(), &Policy::default())
        .expect("the stage converges");
    sys.asm.push_stage(x.as_slice().expect("contiguous"), amb).expect("push the stage");
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
