//! The Newton-Raphson engine: the `NonLinearSystem` contract, the
//! strategy-driven damped iteration loop with predictor seed, and the
//! solution-history buffer it owns.
use crate::device::LimitState;
use crate::error::{Error, RunCode, SolverDomain};
use crate::math::singular::SingularityReport;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::iv::InitialValue;
use crate::math::faer::{FaerSparseLinearSystem, FaerSymbolicMatrix};
use crate::math::linear::{AsIndex, Stamp};
use crate::math::num::Scalar;
use crate::convergence::{DampContext, NewtonStrategy};
use crate::tolerances::{Policy, Tolerances};
use ndarray::{Array1, ArrayView1};
use tracing::debug;

pub trait NonLinearSystem<A: AsIndex, E: Scalar> {
    fn assemble(
        &mut self,
        state: &CircularArrayBuffer2<E>,
    ) -> crate::error::Result<Vec<Stamp<A, E>>>;

    /// [`assemble`](Self::assemble) into `out`, which the Newton solver keeps between iterations.
    /// A system that can fill a buffer without allocating overrides this; the default takes the
    /// stamps `assemble` returns.
    fn assemble_into(
        &mut self,
        state: &CircularArrayBuffer2<E>,
        out: &mut Vec<Stamp<A, E>>,
    ) -> crate::error::Result<()> {
        *out = self.assemble(state)?;
        Ok(())
    }

    /// The netlist naming this system's unknowns. The strategy solve path
    /// reads it for the per-row convergence tests; routing it through the
    /// system (instead of a separate parameter) lets the solver reborrow it
    /// each iteration without aliasing the `&mut system` borrow.
    fn netlist(&self) -> &crate::netlist::Netlist;

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

    /// Whether any device in this system reports active limiting (ABI-10).
    /// The Newton strategy calls this each iteration as part of convergence:
    /// a device whose `LIMIT` input is limiting vetoes convergence until the
    /// limited value settles.
    fn any_limiting_report(&self) -> bool {
        false
    }

    /// Whether this is an operating-point system (ngspice `MODEDCOP`/`MODETRANOP`), the only kind
    /// a node-damping strategy acts on. Default: no.
    fn static_solve(&self) -> bool {
        false
    }

    /// The limits that veto convergence of the iteration just assembled, for
    /// the error of a Newton exhaustion to name. Default: none.
    fn limiting_report(&self) -> Vec<LimitState> {
        Vec::new()
    }
}

/// The part of an iteration a [`SolveTimer`] reading adds to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimedPart {
    /// Assembling and stamping the system.
    Assembly,
    /// The sparse linear solve.
    Solve,
}

/// Cumulative wall-clock time of the two parts of a Newton iteration. It reads the clock only
/// when timing was asked for (`TraceFlags::timing`); otherwise both totals stay 0 and no clock
/// is read (a reading costs about 1.5 µs on a machine whose clock source is the HPET).
#[derive(Debug, Clone, Copy, Default)]
struct SolveTimer {
    enabled: bool,
    assembly_ns: u64,
    solve_ns: u64,
}

impl SolveTimer {
    /// A timer that reads the clock only when `enabled`.
    fn new(enabled: bool) -> Self {
        Self { enabled, ..Self::default() }
    }

    /// Turn the readings on or off for the solves that follow; the totals are kept.
    fn enable(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// The instant a part starts, or `None` when timing is off.
    fn start(&self) -> Option<std::time::Instant> {
        self.enabled.then(std::time::Instant::now)
    }

    /// Add the time since `started` to `part`; nothing when timing is off.
    fn add(&mut self, started: Option<std::time::Instant>, part: TimedPart) {
        let Some(started) = started else { return };
        let elapsed = started.elapsed().as_nanos() as u64;
        match part {
            TimedPart::Assembly => self.assembly_ns += elapsed,
            TimedPart::Solve => self.solve_ns += elapsed,
        }
    }

    /// Zero both totals, keeping the on/off setting.
    fn reset(&mut self) {
        self.assembly_ns = 0;
        self.solve_ns = 0;
    }
}

pub struct NewtonRaphsonSolver<A, E>
where
    A: AsIndex,
    E: Scalar,
{
    linear_system: FaerSparseLinearSystem<E>,
    symbolic: FaerSymbolicMatrix,
    state: CircularArrayBuffer2<E>,
    /// Per-row nonlinear residual `(A·v − b)`, rebuilt each iteration in
    /// place. Hoisted so the Newton inner loop allocates nothing (CP-06).
    residual: Vec<E>,
    /// Per-row absolute-contribution sum for the relative tolerance term.
    scale: Vec<f64>,
    /// The stamps of the iteration, refilled by `assemble_into` and kept between iterations.
    stamps: Vec<Stamp<A, E>>,
    /// The unknowns that are nodes (the ones a damping strategy acts on), listed once.
    nodes: Vec<usize>,
    last_iterations: usize,
    total_iterations: usize,
    /// Cumulative wall-clock time of assembly + stamping and of the sparse LU solve.
    timer: SolveTimer,
    /// One-shot predictor ratio for the next `solve_with_strategy` call
    /// (CP-16): seed `x̂ = x₀ + (x₀ − x₁)·r` from the two newest history
    /// rows instead of plain `x₀`. Consumed (taken) by the solve; `None`
    /// seeds from the latest point as before.
    predictor_ratio: Option<f64>,
    _marker: std::marker::PhantomData<A>,
}

impl<A, E> NewtonRaphsonSolver<A, E>
where
    A: AsIndex,
    E: Scalar,
{
    pub fn new(
        system: &mut dyn NonLinearSystem<A, E>,
        size: usize,
        history_depth: usize,
    ) -> crate::error::Result<Self> {
        let state = CircularArrayBuffer2::new(history_depth, size);
        let dry_run_stamps = system.assemble(&state)?;
        let entries: Vec<Stamp<usize, E>> = dry_run_stamps
            .iter()
            .filter_map(|stamp| match stamp {
                Stamp::Matrix(r, c, v) => Some(Stamp::Matrix(r.as_index()?, c.as_index()?, *v)),
                Stamp::Rhs(..) => None,
            })
            .collect();
        let symbolic = FaerSymbolicMatrix::new(size, dry_run_stamps).map_err(|cause| {
            SingularityReport::error(
                SolverDomain::Newton,
                size,
                &entries,
                &format!("the Jacobian is singular ({})", Self::cause_text(&cause)),
            )
        })?;
        let linear_system = FaerSparseLinearSystem::new(size);
        let nodes = system
            .netlist()
            .all_references()
            .iter()
            .filter(|reference| reference.is_node())
            .filter_map(|reference| reference.idx())
            .collect();

        Ok(Self {
            linear_system,
            symbolic,
            state,
            residual: vec![E::zero(); size],
            scale: vec![0.0; size],
            stamps: Vec::new(),
            nodes,
            last_iterations: 0,
            total_iterations: 0,
            timer: SolveTimer::new(false),
            predictor_ratio: None,
            _marker: std::marker::PhantomData,
        })
    }

    /// The library's own text of a failed factorization (the `Caused by` of
    /// the error, or the error itself).
    fn cause_text(error: &Error) -> String {
        match error {
            Error::WithCause { cause, .. } => cause.to_string(),
            other => other.to_string(),
        }
    }

    /// Stores `guess` as the latest row of the iterate buffer; a buffer with no row is the
    /// solver's error, not a panic.
    fn commit(&mut self, guess: &ArrayView1<E>) -> Result<(), Error> {
        match self.state.latest_mut() {
            Some(mut latest) => {
                latest.assign(guess);
                Ok(())
            }
            None => Err(self.singular("the iterate buffer holds no row to commit to")),
        }
    }

    /// The `E0503`/`E0507` of the system just stamped, which `what` failed to
    /// solve: reads the entries still held by the linear system.
    fn singular(&self, what: &str) -> Error {
        let entries: Vec<Stamp<usize, E>> = self
            .linear_system
            .matrix_entries()
            .into_iter()
            .map(|(r, c, v)| Stamp::Matrix(r, c, v))
            .collect();
        SingularityReport::error(SolverDomain::Newton, self.state.size(), &entries, what)
    }

    /// The exhaustion of `max_iter` iterations: names the unknowns of the last
    /// iteration that failed the update or residual test and the limits still
    /// vetoing. The caller's driver re-codes it (`E0505`, `E0508`, `E0504`).
    fn exhausted(max_iter: usize, failing: Vec<usize>, limits: &[LimitState]) -> Error {
        let mut detail = format!("Newton failed to converge after {max_iter} iterations");
        if !limits.is_empty() {
            let named: Vec<String> =
                limits.iter().map(|l| format!("{} (limit {})", l.instance, l.input)).collect();
            detail.push_str(&format!(", limiting: {}", named.join(", ")));
        }
        Error::run(RunCode::E0505, SolverDomain::Newton, detail).with_unknowns(failing)
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
    ) -> crate::error::Result<Array1<E>> {
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

            let current_guess = self.iterate(system, iter)?;

            debug!("New guess: {:?}", current_guess);

            if system.converged(&self.state, &current_guess.view())
                && system.residual_converged(&self.residual, &self.scale)
            {
                // Commit the converged solution — the buffer's latest row is
                // what snapshots read and what the next timestep's companion
                // models treat as the accepted point.
                self.commit(&current_guess.view())?;
                system.convergence_success_callback(&self.state, &current_guess.view());
                debug!("Converged in {} iterations", iter + 1);
                self.last_iterations = iter + 1;
                self.total_iterations += iter + 1;
                return Ok(current_guess);
            }

            self.commit(&current_guess.view())?;
        }

        Err(self.exhausted_after(system, max_iter, Vec::new()))
    }

    /// One iteration's assembly and linear solve: stamps the system at the latest row, folds the residual and returns the
    /// new guess; a singular Jacobian or a non-finite solution is the solver's error.
    fn iterate(
        &mut self,
        system: &mut dyn NonLinearSystem<A, E>,
        iter: usize,
    ) -> crate::error::Result<Array1<E>> {
        let stamps = system.assemble(&self.state)?;
        self.compute_residual(&stamps);

        self.linear_system.reset();
        self.linear_system.apply_stamps(stamps);
        let current_guess = self
            .linear_system
            .solve_with_backend(&self.symbolic)
            .map_err(|cause| {
                self.singular(&format!("the Jacobian is singular ({})", Self::cause_text(&cause)))
            })?;

        if current_guess.iter().any(|x| !x.is_finite()) {
            system.convergence_failed_callback(&self.state, iter, &current_guess.view());

            return Err(self.singular("the linear solve returned a non-finite value"));
        }
        Ok(current_guess)
    }

    /// The solver's error after `max_iter` iterations that did not converge: counts them, tells the system and names
    /// the unknowns `failing` and the devices still limiting.
    fn exhausted_after(
        &mut self,
        system: &mut dyn NonLinearSystem<A, E>,
        max_iter: usize,
        failing: Vec<usize>,
    ) -> Error {
        self.last_iterations = max_iter;
        self.total_iterations += max_iter;
        if let Some(latest) = self.state.latest() {
            system.convergence_failed_callback(&self.state, max_iter, &latest);
        }
        Self::exhausted(max_iter, failing, &system.limiting_report())
    }

    pub fn current_guess(&self) -> Option<ArrayView1<'_, E>> {
        self.state.latest()
    }

    pub fn state(&self) -> &CircularArrayBuffer2<E> {
        &self.state
    }
}

// ── f64-only path: NewtonStrategy integration ──────────────────────────────

impl<A> NewtonRaphsonSolver<A, f64>
where
    A: AsIndex,
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
        self.timer.reset();
    }

    /// Cumulative assembly+stamping wall time (ns) since the last reset.
    pub fn assembly_time_ns(&self) -> u64 {
        self.timer.assembly_ns
    }

    /// Cumulative sparse-solve wall time (ns) since the last reset.
    pub fn solve_time_ns(&self) -> u64 {
        self.timer.solve_ns
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
    ) -> crate::error::Result<Array1<f64>> {
        let guess = self.seed_guess();
        self.state.push(&guess.view());
        self.timer.enable(policy.trace.timing);

        system.update_sources(&mut self.state);

        let max_iter = strategy.max_iter(policy);
        let mut failing = Vec::new();

        for iter in 0..max_iter {
            system.before_iter_callback(&self.state, iter);
            debug!("Newton Iteration {}", iter + 1);

            let mut current_guess = self.iterate_timed(system, iter)?;

            debug!("New guess: {:?}", current_guess);

            self.damp(system, strategy, iter, &mut current_guess);

            // Device limiting gate: the strategy checks update+residual;
            // limiting_report is a system-level check done per iteration.
            if !system.any_limiting_report()
                && strategy.is_converged(
                    &self.state,
                    &current_guess.view(),
                    &self.residual,
                    &self.scale,
                    system.netlist(),
                    tolerances,
                ) {
                self.commit(&current_guess.view())?;
                system.convergence_success_callback(&self.state, &current_guess.view());
                debug!("Converged in {} iterations", iter + 1);
                self.last_iterations = iter + 1;
                self.total_iterations += iter + 1;
                return Ok(current_guess);
            }

            if iter + 1 == max_iter {
                failing = self.failing_unknowns(system, tolerances, &current_guess);
            }

            self.commit(&current_guess.view())?;
        }

        Err(self.exhausted_after(system, max_iter, failing))
    }

    /// Damping through the strategy: moves `guess` from the latest row by what the strategy allows at `iter`.
    fn damp(&self, system: &mut dyn NonLinearSystem<A, f64>, strategy: &dyn NewtonStrategy, iter: usize, guess: &mut Array1<f64>) {
        if let Some(prev) = self.state.latest() {
            let context = DampContext {
                static_solve: system.static_solve(),
                iteration: iter,
                limiting: system.any_limiting_report(),
                nodes: &self.nodes,
            };
            strategy.damp_update(prev, guess.view_mut(), &context);
        }
    }

    /// The unknowns of `guess` that fail the tolerances against the latest row, for the report of an exhausted solve.
    fn failing_unknowns(&self, system: &dyn NonLinearSystem<A, f64>, tolerances: &Tolerances, guess: &Array1<f64>) -> Vec<usize> {
        match self.state.latest() {
            Some(previous) => tolerances.failing(previous, &guess.view(), system.netlist(), &self.residual, &self.scale),
            None => Vec::new(),
        }
    }

    /// The seed of a strategy solve: first-order extrapolation when the driver armed the
    /// predictor and two history rows exist (CP-16); else the latest
    /// accepted point; else zeros (cold start).
    fn seed_guess(&mut self) -> Array1<f64> {
        let predictor = self.predictor_ratio.take();
        match (predictor, self.state.view(0), self.state.view(1)) {
            (Some(r), Some(x0), Some(x1)) if r.is_finite() => {
                &x0 + &((&x0 - &x1) * r)
            }
            (_, Some(x0), _) => x0.to_owned(),
            _ => Array1::zeros(self.state.size()),
        }
    }

    /// [`iterate`](Self::iterate) with the assembly and the sparse solve timed apart (and the stamp buffer reused).
    fn iterate_timed(
        &mut self,
        system: &mut dyn NonLinearSystem<A, f64>,
        iter: usize,
    ) -> crate::error::Result<Array1<f64>> {
        let assembly_started = self.timer.start();
        let mut stamps = std::mem::take(&mut self.stamps);
        system.assemble_into(&self.state, &mut stamps)?;
        self.compute_residual(&stamps);

        self.linear_system.reset();
        self.linear_system.apply_stamps_from(&stamps);
        self.stamps = stamps;
        self.timer.add(assembly_started, TimedPart::Assembly);

        let solve_started = self.timer.start();
        let solved = self.linear_system.solve_with_backend(&self.symbolic);
        self.timer.add(solve_started, TimedPart::Solve);
        let current_guess = solved.map_err(|cause| {
            self.singular(&format!("the Jacobian is singular ({})", Self::cause_text(&cause)))
        })?;

        if current_guess.iter().any(|x| !x.is_finite()) {
            system.convergence_failed_callback(&self.state, iter, &current_guess.view());
            return Err(self.singular("the linear solve returned a non-finite value"));
        }
        Ok(current_guess)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convergence::NoDamping;
    use crate::netlist::{AnalogReference, Netlist, NodeIdentifier};

    type Solver = NewtonRaphsonSolver<AnalogReference, f64>;

    /// A diagonal system `g_i · x_i = rhs_i(x)` over anonymous nodes.
    struct Diagonal {
        netlist: Netlist,
        nodes: Vec<AnalogReference>,
        conductance: Vec<f64>,
        /// Row `i`'s right-hand side as a function of the previous iterate.
        rhs: fn(usize, f64) -> f64,
    }

    impl Diagonal {
        fn new(conductance: Vec<f64>, rhs: fn(usize, f64) -> f64) -> Self {
            let mut netlist = Netlist::new();
            let nodes = (0..conductance.len())
                .map(|i| netlist.connect_node(NodeIdentifier::Anonymous(i + 1)))
                .collect();
            Self { netlist, nodes, conductance, rhs }
        }
    }

    impl NonLinearSystem<AnalogReference, f64> for Diagonal {
        fn assemble(
            &mut self,
            state: &CircularArrayBuffer2<f64>,
        ) -> crate::error::Result<Vec<Stamp<AnalogReference, f64>>> {
            let mut stamps = Vec::new();
            for (i, node) in self.nodes.iter().enumerate() {
                let previous = state.latest().map_or(0.0, |row| row[i]);
                stamps.push(Stamp::Matrix(node.clone(), node.clone(), self.conductance[i]));
                stamps.push(Stamp::Rhs(node.clone(), (self.rhs)(i, previous)));
            }
            Ok(stamps)
        }

        fn netlist(&self) -> &Netlist {
            &self.netlist
        }
    }

    #[test]
    fn a_non_finite_solution_of_finite_stamps_is_e0503() {
        // 1e-320 S against a 1e300 A source: every stamp is finite, the
        // solution 1e620 is not.
        let mut system = Diagonal::new(vec![1e-320], |_, _| 1e300);
        let mut solver = Solver::new(&mut system, 1, 3).expect("a pattern");
        let err = solver
            .solve_with_strategy(&mut system, &NoDamping, &Tolerances::default(), &Policy::default())
            .expect_err("the solution is not finite");
        assert_eq!(err.code(), Some(RunCode::E0503), "{err}");
        assert!(!err.to_string().contains("returned NaN"), "{err}");
    }

    #[test]
    fn exhaustion_names_the_unknowns_that_failed() {
        // Unknown 0 solves x = x + 1 (it never settles); unknown 1 solves x = 0 (settled from the zero seed).
        let mut system = Diagonal::new(vec![1.0, 1.0], |i, previous| if i == 0 { previous + 1.0 } else { 0.0 });
        let mut solver = Solver::new(&mut system, 2, 3).expect("a pattern");
        let policy = Policy { max_iter: 6, ..Policy::default() };
        let err = solver
            .solve_with_strategy(&mut system, &NoDamping, &Tolerances::default(), &policy)
            .expect_err("unknown 0 never settles");
        let Error::Run(run) = &err else { panic!("a run error, got {err}") };
        assert_eq!(run.code, RunCode::E0505);
        assert_eq!(run.unknowns, vec![0], "only the diverging unknown failed: {err}");
        assert!(run.detail.contains("6 iterations"), "{err}");
    }

    /// Builds the solver the timing tests inspect.
    struct TimingCases;

    impl TimingCases {
        /// One converging solve of a unit diagonal system, with timing on or off.
        fn solver_after_one_solve(timing: bool) -> Solver {
            let mut system = Diagonal::new(vec![1.0], |_, _| 1.0);
            let mut solver = Solver::new(&mut system, 1, 3).expect("a pattern");
            let policy = Policy {
                trace: crate::config::TraceFlags { timing, ..Default::default() },
                ..Policy::default()
            };
            solver
                .solve_with_strategy(&mut system, &NoDamping, &Tolerances::default(), &policy)
                .expect("the system converges");
            solver
        }
    }

    #[test]
    fn without_timing_a_solve_reads_no_clock() {
        let solver = TimingCases::solver_after_one_solve(false);
        assert_eq!(solver.assembly_time_ns(), 0);
        assert_eq!(solver.solve_time_ns(), 0);
    }

    #[test]
    fn with_timing_a_solve_reports_both_totals() {
        let solver = TimingCases::solver_after_one_solve(true);
        assert!(solver.assembly_time_ns() > 0, "assembly time");
        assert!(solver.solve_time_ns() > 0, "solve time");
    }
}
