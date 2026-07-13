use crate::error::Error;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::iv::{InitialValue, InitialValueApplyExt};
use crate::math::linear::{AsIndex, Stamp, SymbolicLinearSystem, SymbolicMatrix};
use crate::math::num::Scalar;
use ndarray::{Array1, ArrayView1, ArrayViewMut1};
use tracing::debug;

pub trait NonLinearSystem<A: AsIndex, E: Scalar> {
    fn assemble(
        &mut self,
        state: &CircularArrayBuffer2<E>,
        alpha: E,
    ) -> crate::result::Result<Vec<Stamp<A, E>>>;

    fn converged(&self, _state: &CircularArrayBuffer2<E>, _delta: &ArrayView1<E>) -> bool {
        true
    }

    /// Current/branch-residual convergence test (ngspice `NIconvTest`), ANDed
    /// with [`converged`](Self::converged). `residual[i]` is the row-`i`
    /// imbalance `(A·v − b)[i]` of the just-assembled system at the current
    /// point (for a node row: the KCL current mismatch; for a branch row: the
    /// branch-equation residual). `scale[i]` is the sum of the absolute
    /// contributions into that row (the local current/voltage magnitude). A
    /// system applies its own per-row tolerances. Default: no residual gate.
    ///
    /// This is why stiff exponential devices need it: the damped Newton
    /// *step* can go small while this residual is still large (a big residual
    /// divided by a large device conductance is a tiny `Δv`), so the step
    /// test alone accepts non-solutions.
    fn residual_converged(&self, _residual: &[E], _scale: &[f64]) -> bool {
        true
    }

    fn apply_limit(&mut self, _state: &CircularArrayBuffer2<E>, _current_guess: ArrayViewMut1<E>) {}
    fn update_sources(&mut self, _state: &mut CircularArrayBuffer2<E>) {}
    fn before_iter_callback(&mut self, _state: &CircularArrayBuffer2<E>, _iteration_number: usize) {
    }

    fn convergence_failed_callback(
        &mut self,
        _state: &CircularArrayBuffer2<E>,
        _iteration_number: usize,
        _current_guess: &ArrayView1<E>,
    ) {
    }

    fn convergence_success_callback(
        &mut self,
        _state: &CircularArrayBuffer2<E>,
        _converged_guess: &ArrayView1<E>,
    ) {
    }
}

pub struct NewtonRaphsonSolver<A, E, L>
where
    A: AsIndex,
    E: Scalar,
    L: SymbolicLinearSystem<E>,
{
    linear_system: L,
    symbolic: L::SymbolicType,
    state: CircularArrayBuffer2<E>,
    _marker: std::marker::PhantomData<A>,
}

impl<A, E, L> NewtonRaphsonSolver<A, E, L>
where
    A: AsIndex,
    E: Scalar,
    L: SymbolicLinearSystem<E>,
{
    pub fn new(
        system: &mut dyn NonLinearSystem<A, E>,
        size: usize,
        history_depth: usize,
    ) -> crate::result::Result<Self> {
        let state = CircularArrayBuffer2::new(history_depth, size);
        let dry_run_stamps = system.assemble(&state, E::zero())?;
        let symbolic = L::SymbolicType::new(size, dry_run_stamps)?;
        let linear_system = L::new(size);

        Ok(Self {
            linear_system,
            symbolic,
            state,
            _marker: std::marker::PhantomData,
        })
    }

    pub fn push_initial_conditions(&mut self, ivs: Vec<InitialValue<A, E>>) {
        self.state.apply_iv(ivs);
    }

    pub fn solve(
        &mut self,
        system: &mut dyn NonLinearSystem<A, E>,
        alpha: E,
        max_iter: usize,
    ) -> crate::result::Result<Array1<E>> {
        let guess = if let Some(prev) = self.state.latest() {
            prev.to_owned()
        } else {
            Array1::zeros(self.state.size())
        };
        self.state.push(&guess.view());

        system.update_sources(&mut self.state);

        for iter in 0..max_iter {
            system.before_iter_callback(&self.state, iter);
            debug!("Newton Iteration {}", iter + 1);

            let stamps = system.assemble(&self.state, alpha)?;

            // Nonlinear residual at the assembled point v_old: for a companion
            // (Norton) stamp set, `(A·v_old − b)[r]` equals the node's current
            // imbalance `I_r(v_old)` (or a branch row's equation residual).
            // `scale[r]` accumulates the absolute contributions for the
            // relative tolerance. Computed before the stamps are consumed.
            let size = self.symbolic.size();
            let mut residual = vec![E::zero(); size];
            let mut scale = vec![0.0_f64; size];
            if let Some(v_old) = self.state.latest() {
                for stamp in &stamps {
                    match stamp {
                        Stamp::Matrix(r, c, g) => {
                            if let (Some(ri), Some(ci)) = (r.as_index(), c.as_index()) {
                                let term = *g * v_old[ci];
                                residual[ri] += term;
                                scale[ri] += term.abs();
                            }
                        }
                        Stamp::Rhs(r, val) => {
                            if let Some(ri) = r.as_index() {
                                residual[ri] -= *val;
                                scale[ri] += val.abs();
                            }
                        }
                    }
                }
            }

            self.linear_system = L::new(self.symbolic.size());
            self.linear_system.apply_stamps(stamps);
            let mut current_guess = self.linear_system.solve_with_backend(&self.symbolic)?;

            if current_guess.iter().any(|x| !x.is_finite()) {
                system.convergence_failed_callback(&self.state, iter, &current_guess.view());

                return Err(Error::simple(
                    "Convergence Failure",
                    "Linear solver returned NaN/Inf",
                ));
            }

            debug!("New guess: {:?}", current_guess);

            system.apply_limit(&self.state, current_guess.view_mut());

            if system.converged(&self.state, &current_guess.view())
                && system.residual_converged(&residual, &scale)
            {
                // Commit the converged solution — the buffer's latest row is
                // what snapshots read and what the next timestep's companion
                // models treat as the accepted point.
                self.state
                    .latest_mut()
                    .unwrap()
                    .assign(&current_guess.view());
                system.convergence_success_callback(&self.state, &current_guess.view());
                debug!("Converged in {} iterations", iter + 1);
                return Ok(current_guess);
            }

            self.state
                .latest_mut()
                .unwrap()
                .assign(&current_guess.view());
        }

        system.convergence_failed_callback(&self.state, max_iter, &self.state.latest().unwrap());
        Err(Error::simple(
            "Convergence Failure",
            format!("Failed to converge after {} iterations", max_iter),
        ))
    }

    pub fn current_guess(&self) -> Option<ArrayView1<'_, E>> {
        self.state.latest()
    }

    pub fn state(&self) -> &CircularArrayBuffer2<E> {
        &self.state
    }
}
