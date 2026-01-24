use crate::error::Error;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::iv::{InitialValue2, InitialValueApplyExt};
use crate::math::linear::{AsIndex, SparseLinearSystem2, Stamp2, SymbolicMatrix2};
use crate::math::num::Field;
use crate::solver::Context;
use ndarray::{Array1, ArrayView1, ArrayViewMut1};
use num_traits::Zero;
use tracing::debug;

pub trait NonLinearSystem<A: AsIndex, E: Field> {
    fn assemble(
        &mut self,
        state: &CircularArrayBuffer2<E>,
        alpha: E,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp2<A, E>>>;

    fn converged(
        &self,
        state: &CircularArrayBuffer2<E>,
        delta: &ArrayView1<E>,
        context: &Context,
    ) -> bool;

    fn apply_limit(
        &mut self,
        state: &CircularArrayBuffer2<E>,
        current_guess: ArrayViewMut1<E>,
        context: &Context,
    );

    fn update_sources(&mut self, state: &mut CircularArrayBuffer2<E>, context: &Context);
}

pub struct NewtonRaphsonSolver2<A, E, L>
where
    A: AsIndex,
    E: Field,
    L: SparseLinearSystem2<E>,
{
    linear_system: L,
    symbolic: L::SymbolicType,
    pub state: CircularArrayBuffer2<E>,
    context: Context,
    _marker: std::marker::PhantomData<A>,
}

impl<A, E, L> NewtonRaphsonSolver2<A, E, L>
where
    A: AsIndex + Clone,
    E: Field + 'static + Zero + PartialOrd + Copy,
    L: SparseLinearSystem2<E>,
{
    pub fn new(
        system: &mut dyn NonLinearSystem<A, E>,
        size: usize,
        history_depth: usize,
        context: Context,
    ) -> crate::result::Result<Self> {
        let mut state = CircularArrayBuffer2::new(history_depth, size);
        let dry_run_stamps = system.assemble(&state, E::zero(), &context)?;
        let symbolic = L::SymbolicType::new(size, dry_run_stamps)?;
        let linear_system = L::new(size);

        Ok(Self {
            linear_system,
            symbolic,
            state,
            context,
            _marker: std::marker::PhantomData,
        })
    }

    pub fn set_initial_conditions(&mut self, ivs: Vec<InitialValue2<A, E>>) {
        self.state.apply_iv(ivs);
    }

    pub fn solve(
        &mut self,
        system: &mut dyn NonLinearSystem<A, E>,
        alpha: E,
    ) -> crate::result::Result<Array1<E>> {
        let guess = if let Some(prev) = self.state.latest() {
            prev.to_owned()
        } else {
            Array1::zeros(self.state.size())
        };
        self.state.push(&guess.view());

        system.update_sources(&mut self.state, &self.context);

        for iter in 0..self.context.max_iter {
            debug!("Newton Iteration {}", iter + 1);

            let stamps = system.assemble(&self.state, alpha, &self.context)?;

            self.linear_system = L::new(self.symbolic.size());
            self.linear_system.apply_stamps(stamps);
            let mut current_guess = self.linear_system.solve_with_backend(&self.symbolic)?;

            if current_guess.iter().any(|x| !x.is_finite()) {
                return Err(Error::simple(
                    "Convergence Failure",
                    "Linear solver returned NaN/Inf",
                ));
            }

            debug!("New guess: {:?}", current_guess);

            system.apply_limit(&self.state, current_guess.view_mut(), &self.context);

            if system.converged(&self.state, &current_guess.view(), &self.context) {
                debug!("Converged in {} iterations", iter + 1);
                return Ok(current_guess);
            }

            self.state
                .latest_mut()
                .unwrap()
                .assign(&current_guess.view());
        }

        Err(Error::simple(
            "Convergence Failure",
            format!(
                "Failed to converge after {} iterations",
                self.context.max_iter
            ),
        ))
    }
}
