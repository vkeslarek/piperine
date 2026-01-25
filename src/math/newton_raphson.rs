use crate::error::Error;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::iv::{InitialValue, InitialValueApplyExt};
use crate::math::linear::{AsIndex, Stamp, SymbolicLinearSystem, SymbolicMatrix};
use crate::math::num::Field;
use ndarray::{Array1, ArrayView1, ArrayViewMut1};
use tracing::debug;

pub trait NonLinearSystem<A: AsIndex, E: Field> {
    fn assemble(
        &mut self,
        state: &CircularArrayBuffer2<E>,
        alpha: E,
    ) -> crate::result::Result<Vec<Stamp<A, E>>>;

    fn converged(&self, state: &CircularArrayBuffer2<E>, delta: &ArrayView1<E>) -> bool {
        true
    }

    fn apply_limit(&mut self, state: &CircularArrayBuffer2<E>, current_guess: ArrayViewMut1<E>) {}
    fn update_sources(&mut self, state: &mut CircularArrayBuffer2<E>) {}
    fn before_iter_callback(&mut self, state: &CircularArrayBuffer2<E>, iteration_number: usize) {}

    fn convergence_failed_callback(
        &mut self,
        state: &CircularArrayBuffer2<E>,
        iteration_number: usize,
        current_guess: &ArrayView1<E>,
    ) {
    }

    fn convergence_success_callback(
        &mut self,
        state: &CircularArrayBuffer2<E>,
        converged_guess: &ArrayView1<E>,
    ) {
    }
}

pub struct NewtonRaphsonSolver<A, E, L>
where
    A: AsIndex,
    E: Field,
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
    E: Field,
    L: SymbolicLinearSystem<E>,
{
    pub fn new(
        system: &mut dyn NonLinearSystem<A, E>,
        size: usize,
        history_depth: usize,
    ) -> crate::result::Result<Self> {
        let mut state = CircularArrayBuffer2::new(history_depth, size);
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

            if system.converged(&self.state, &current_guess.view()) {
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

    pub fn current_guess(&self) -> Option<ArrayView1<E>> {
        self.state.latest()
    }
}
