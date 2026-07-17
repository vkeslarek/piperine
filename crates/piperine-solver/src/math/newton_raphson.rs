use crate::error::Error;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::iv::{InitialValue, InitialValueApplyExt};
use crate::math::linear::{AsIndex, Stamp, SymbolicLinearSystem, SymbolicMatrix};
use crate::math::num::Scalar;
use crate::solver::convergence::NewtonStrategy;
use crate::solver::{Policy, Tolerances};
use ndarray::{Array1, ArrayView1, ArrayViewMut1};
use tracing::debug;

pub trait NonLinearSystem<A: AsIndex, E: Scalar> {
    fn assemble(
        &mut self,
        state: &CircularArrayBuffer2<E>,
    ) -> crate::result::Result<Vec<Stamp<A, E>>>;

    /// The netlist naming this system's unknowns. The strategy solve path
    /// reads it for the per-row convergence tests; routing it through the
    /// system (instead of a separate parameter) lets the solver reborrow it
    /// each iteration without aliasing the `&mut system` borrow.
    fn netlist(&self) -> &crate::analog::Netlist;

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

    /// Whether any device in this system reports active limiting. The
    /// Newton strategy calls this each iteration as part of convergence.
    fn any_limiting(&self) -> bool {
        false
    }

    /// Apply device convergence hints (structured limiting) to the Newton
    /// guess before the convergence test. Default: no hints.
    fn apply_convergence_hints(&self, _guess: ArrayViewMut1<E>) {}
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
    /// Per-row nonlinear residual `(A·v − b)`, rebuilt each iteration in
    /// place. Hoisted so the Newton inner loop allocates nothing (CP-06).
    residual: Vec<E>,
    /// Per-row absolute-contribution sum for the relative tolerance term.
    scale: Vec<f64>,
    last_iterations: usize,
    total_iterations: usize,
    /// Cumulative wall-clock time spent assembling + stamping (ns).
    assembly_ns: u64,
    /// Cumulative wall-clock time spent in the sparse LU solve (ns).
    solve_ns: u64,
    /// One-shot predictor ratio for the next `solve_with_strategy` call
    /// (CP-16): seed `x̂ = x₀ + (x₀ − x₁)·r` from the two newest history
    /// rows instead of plain `x₀`. Consumed (taken) by the solve; `None`
    /// seeds from the latest point as before.
    predictor_ratio: Option<f64>,
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
        let dry_run_stamps = system.assemble(&state)?;
        let symbolic = L::SymbolicType::new(size, dry_run_stamps)?;
        let linear_system = L::new(size);

        Ok(Self {
            linear_system,
            symbolic,
            state,
            residual: vec![E::zero(); size],
            scale: vec![0.0; size],
            last_iterations: 0,
            total_iterations: 0,
            assembly_ns: 0,
            solve_ns: 0,
            predictor_ratio: None,
            _marker: std::marker::PhantomData,
        })
    }

    /// Rebuild the nonlinear residual + scale vectors in place from the
    /// just-assembled stamps, evaluated at the current point. For a companion
    /// (Norton) stamp set, `(A·v_old − b)[r]` equals the node's current
    /// imbalance `I_r(v_old)` (or a branch row's equation residual);
    /// `scale[r]` accumulates the absolute contributions for the relative
    /// tolerance. Must run before the stamps are consumed by the linear system.
    fn compute_residual(&mut self, stamps: &[Stamp<A, E>]) {
        self.residual.fill(E::zero());
        self.scale.fill(0.0);
        let Some(v_old) = self.state.latest() else { return };
        for stamp in stamps {
            match stamp {
                Stamp::Matrix(r, c, g) => {
                    if let (Some(ri), Some(ci)) = (r.as_index(), c.as_index()) {
                        let term = *g * v_old[ci];
                        self.residual[ri] += term;
                        self.scale[ri] += term.abs();
                    }
                }
                Stamp::Rhs(r, val) => {
                    if let Some(ri) = r.as_index() {
                        self.residual[ri] -= *val;
                        self.scale[ri] += val.abs();
                    }
                }
            }
        }
    }

    pub fn push_initial_conditions(&mut self, ivs: Vec<InitialValue<A, E>>) {
        self.state.apply_iv(ivs);
    }

    pub fn solve(
        &mut self,
        system: &mut dyn NonLinearSystem<A, E>,
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

            let stamps = system.assemble(&self.state)?;
            self.compute_residual(&stamps);

            self.linear_system.reset();
            self.linear_system.apply_stamps(stamps);
            let mut current_guess = self.linear_system.solve_with_backend(&self.symbolic)?;

            if current_guess.iter().any(|x| !x.is_finite()) {
                system.convergence_failed_callback(&self.state, iter, &current_guess.view());

                return Err(Error::simple(
                    crate::error::SolverDomain::Newton,
                    "Linear solver returned NaN/Inf",
                ));
            }

            debug!("New guess: {:?}", current_guess);

            system.apply_limit(&self.state, current_guess.view_mut());

            if system.converged(&self.state, &current_guess.view())
                && system.residual_converged(&self.residual, &self.scale)
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
                self.last_iterations = iter + 1;
                self.total_iterations += iter + 1;
                return Ok(current_guess);
            }

            self.state
                .latest_mut()
                .unwrap()
                .assign(&current_guess.view());
        }

        self.last_iterations = max_iter;
        self.total_iterations += max_iter;
        system.convergence_failed_callback(&self.state, max_iter, &self.state.latest().unwrap());
        Err(Error::simple(
            crate::error::SolverDomain::Newton,
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

// ── f64-only path: NewtonStrategy integration ──────────────────────────────

impl<A, L> NewtonRaphsonSolver<A, f64, L>
where
    A: AsIndex,
    L: SymbolicLinearSystem<f64>,
{
    /// Iterations taken by the last `solve` / `solve_with_strategy` call.
    /// Drivers read this to populate `SolverStats`.
    pub fn last_iterations(&self) -> usize {
        self.last_iterations
    }

    /// Total iterations across all solves since the last reset. Transient
    /// drivers reset before the step loop and read the total after.
    pub fn total_iterations(&self) -> usize {
        self.total_iterations
    }

    /// Reset the cumulative iteration + timing counters (call before a solve
    /// or step loop whose stats will be reported).
    pub fn reset_iteration_counter(&mut self) {
        self.total_iterations = 0;
        self.assembly_ns = 0;
        self.solve_ns = 0;
    }

    /// Cumulative assembly+stamping wall time (ns) since the last reset.
    pub fn assembly_time_ns(&self) -> u64 {
        self.assembly_ns
    }

    /// Cumulative sparse-solve wall time (ns) since the last reset.
    pub fn solve_time_ns(&self) -> u64 {
        self.solve_ns
    }

    /// Arm the first-order predictor for the next `solve_with_strategy`
    /// call: the Newton seed becomes `x̂ = x₀ + (x₀ − x₁)·ratio` over the two
    /// newest history rows. One-shot — consumed by the solve. The transient
    /// driver arms it per step with `ratio = γ·dt / ((1−γ)·prev_h)` and skips
    /// arming after breakpoints/rejections (no valid history).
    pub fn set_predictor_ratio(&mut self, ratio: f64) {
        self.predictor_ratio = Some(ratio);
    }

    /// Snapshot the solution-history buffer before a candidate step. Each
    /// solve pushes a row, so a rejected TR-BDF2 attempt leaves its two
    /// rejected rows where the retry's charge-history views (`x_n`,
    /// `x_{n−γ}`, …) expect accepted points — the companion then integrates
    /// off the rejected trajectory. The transient driver snapshots before
    /// each attempt and [`restore_state`](Self::restore_state)s on rejection.
    pub fn state_snapshot(&self) -> CircularArrayBuffer2<f64> {
        self.state.clone()
    }

    /// Unwind the solution history to a [`state_snapshot`](Self::state_snapshot)
    /// taken before a rejected candidate step.
    pub fn restore_state(&mut self, snapshot: CircularArrayBuffer2<f64>) {
        self.state = snapshot;
    }

    /// Newton solve with damping and convergence delegated to a
    /// [`NewtonStrategy`]. The DC and transient drivers call this; AC/Noise/TF
    /// (Complex) continue to use the generic [`solve`] path.
    pub fn solve_with_strategy(
        &mut self,
        system: &mut dyn NonLinearSystem<A, f64>,
        strategy: &dyn NewtonStrategy,
        tolerances: &Tolerances,
        policy: &Policy,
    ) -> crate::result::Result<Array1<f64>> {
        // Seed: first-order extrapolation when the driver armed the
        // predictor and two history rows exist (CP-16); else the latest
        // accepted point; else zeros (cold start).
        let predictor = self.predictor_ratio.take();
        let guess = match (predictor, self.state.view(0), self.state.view(1)) {
            (Some(r), Some(x0), Some(x1)) if r.is_finite() => {
                &x0 + &((&x0 - &x1) * r)
            }
            (_, Some(x0), _) => x0.to_owned(),
            _ => Array1::zeros(self.state.size()),
        };
        self.state.push(&guess.view());

        system.update_sources(&mut self.state);

        let max_iter = strategy.max_iter(policy);

        for iter in 0..max_iter {
            system.before_iter_callback(&self.state, iter);
            debug!("Newton Iteration {}", iter + 1);

            let t_assembly = std::time::Instant::now();
            let stamps = system.assemble(&self.state)?;
            self.compute_residual(&stamps);

            self.linear_system.reset();
            self.linear_system.apply_stamps(stamps);
            self.assembly_ns += t_assembly.elapsed().as_nanos() as u64;

            let t_solve = std::time::Instant::now();
            let mut current_guess = self.linear_system.solve_with_backend(&self.symbolic)?;
            self.solve_ns += t_solve.elapsed().as_nanos() as u64;

            if current_guess.iter().any(|x| !x.is_finite()) {
                system.convergence_failed_callback(&self.state, iter, &current_guess.view());
                return Err(Error::simple(
                    crate::error::SolverDomain::Newton,
                    "Linear solver returned NaN/Inf",
                ));
            }

            debug!("New guess: {:?}", current_guess);

            // Damping via strategy (replaces system.apply_limit)
            if let Some(prev) = self.state.latest() {
                strategy.damp_update(prev, current_guess.view_mut(), policy);
            }

            // Structured limiting: devices that know *what* they clamped
            // steer the guess to the limited value before the convergence
            // test (CP-12) — instead of only vetoing via any_limiting.
            system.apply_convergence_hints(current_guess.view_mut());

            // Device limiting gate: the strategy checks update+residual;
            // limiting_active is a system-level check done per iteration.
            if !system.any_limiting()
                && strategy.is_converged(
                    &self.state,
                    &current_guess.view(),
                    &self.residual,
                    &self.scale,
                    system.netlist(),
                    tolerances,
                ) {
                self.state
                    .latest_mut()
                    .unwrap()
                    .assign(&current_guess.view());
                system.convergence_success_callback(&self.state, &current_guess.view());
                debug!("Converged in {} iterations", iter + 1);
                self.last_iterations = iter + 1;
                self.total_iterations += iter + 1;
                return Ok(current_guess);
            }

            self.state
                .latest_mut()
                .unwrap()
                .assign(&current_guess.view());
        }

        self.last_iterations = max_iter;
        self.total_iterations += max_iter;
        system.convergence_failed_callback(&self.state, max_iter, &self.state.latest().unwrap());
        Err(Error::simple(
            crate::error::SolverDomain::Newton,
            format!("Failed to converge after {} iterations", max_iter),
        ))
    }
}
