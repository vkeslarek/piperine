use crate::analysis::ac::AcAnalysisContext;
use crate::analysis::transient::TransientAnalysisContext;
use crate::circuit::{Circuit, CircuitReference, Netlist};
use crate::component::{Component, Components, Context};
use crate::error::{Error, Problem};
use crate::state::CircuitStates;
use faer::prelude::{Solve, SparseColMat};
use faer::sparse::linalg::solvers::SymbolicLu;
use faer::sparse::Triplet;
use faer::traits::ComplexField;
use faer::Col;
use num_complex::Complex;
use num_traits::Zero;
use std::collections::HashMap;
use std::ops::AddAssign;

pub enum Stamp<T> {
    Matrix(CircuitReference, CircuitReference, T),
    Rhs(CircuitReference, T),
}

pub struct LinearSystem<D: Copy + Zero + AddAssign + ComplexField> {
    pub triplets: Vec<Triplet<usize, usize, D>>,
    pub b_vec: Vec<D>,
    pub size: usize,
}

impl<D: Copy + Zero + AddAssign + ComplexField> LinearSystem<D> {
    pub fn new(size: usize) -> Self {
        Self {
            triplets: Vec::with_capacity(size * 4),
            b_vec: vec![D::zero(); size],
            size,
        }
    }

    pub fn apply_stamps(&mut self, symbolic: &SymbolicMatrix, stamps: Vec<Stamp<D>>) {
        for stamp in stamps {
            match stamp {
                Stamp::Matrix(r, c, val) => {
                    if let (Some(&ri), Some(&ci)) =
                        (symbolic.mapping.get(&r), symbolic.mapping.get(&c))
                    {
                        self.triplets.push(Triplet::new(ri, ci, val));
                    }
                }
                Stamp::Rhs(r, val) => {
                    if let Some(&ri) = symbolic.mapping.get(&r) {
                        self.b_vec[ri] += val;
                    }
                }
            }
        }
    }

    /// This is where the Backend lives.
    /// To swap faer, you only change the implementation of this method.
    pub fn solve_with_backend(self, symbolic: &SymbolicMatrix) -> crate::error::Result<Vec<D>> {
        let a = SparseColMat::try_new_from_triplets(self.size, self.size, &self.triplets).map_err(
            |err| Error {
                title: "Problem assembling the space matrix".to_string(),
                detail:
                    "The library threw an error while trying to create the LHS of the sparse matrix"
                        .to_string(),
                problems: vec![Problem::FaerCreationProblem(err)],
            },
        )?;

        let b = Col::from_fn(self.size, |i| self.b_vec[i]);

        // REUSE Symbolic
        let lu = faer::sparse::linalg::solvers::Lu::try_new_with_symbolic(
            symbolic.pattern.clone(),
            a.as_ref(),
        )
        .map_err(|err| Error {
            title: "Problem assembling the space matrix".to_string(),
            detail:
                "The library threw an error while trying to create the RHS of the sparse matrix"
                    .to_string(),
            problems: vec![Problem::FaerLuError(err)],
        })?;

        let sol = lu.solve(&b);
        Ok(sol.iter().copied().collect())
    }
}

pub struct SymbolicMatrix {
    pub mapping: HashMap<CircuitReference, usize>,
    pub size: usize,
    // We wrap the backend here. If you swap faer, you only change this field.
    pub pattern: SymbolicLu<usize>,
}

impl SymbolicMatrix {
    pub fn new(
        netlist: &Netlist,
        components: &Components,
        context: &Context,
    ) -> crate::error::Result<Self> {
        let mut mapping = HashMap::new();
        let mut index = 0;

        // 1. Build Indexing (Logic remains the same)
        for (_, res) in netlist.all_nodes() {
            if !res.is_ground() {
                mapping.insert(res, index);
                index += 1;
            }
        }
        for (_, res) in netlist.all_branches() {
            mapping.insert(res, index);
            index += 1;
        }

        // 2. Build Sparsity Pattern (using 1.0 placeholders)
        let mut triplets = Vec::new();
        for comp in components.get_all() {
            comp.as_dc()
                .map(|dc_comp| dc_comp.load_dc(context))
                .unwrap_or(vec![])
                .iter()
                .for_each(|stamp| {
                    if let Stamp::Matrix(r, c, _) = stamp {
                        if let (Some(&ri), Some(&ci)) = (mapping.get(&r), mapping.get(&c)) {
                            triplets.push(Triplet::new(ri, ci, 1.0));
                        }
                    }
                });
        }

        let size = mapping.len();
        let mat =
            SparseColMat::try_new_from_triplets(size, size, &triplets).map_err(|err| Error {
                title: "Problem assembling the space matrix".to_string(),
                detail: "The library threw an error while trying to create the symbolic matrix"
                    .to_string(),
                problems: vec![Problem::FaerCreationProblem(err)],
            })?;

        let pattern = SymbolicLu::try_new(mat.symbolic()).map_err(|err| Error {
            title: "Problem assembling the space matrix".to_string(),
            detail: "The library threw an error while trying to create the symbolic matrix"
                .to_string(),
            problems: vec![Problem::FaerGenericError(err)],
        })?;

        Ok(Self {
            mapping,
            size,
            pattern,
        })
    }
}

pub struct CircuitSolver {
    pub circuit: Circuit,
    pub symbolic: SymbolicMatrix,
}

impl CircuitSolver {
    pub fn new(circuit: Circuit, context: &Context) -> crate::error::Result<Self> {
        let symbolic = SymbolicMatrix::new(&circuit.netlist, &circuit.components, context)?;
        Ok(Self { circuit, symbolic })
    }

    pub fn solve_ac(
        &self,
        dc_state: &CircuitStates,
        omega: f64,
        context: &Context,
    ) -> crate::error::Result<Vec<Complex<f64>>> {
        let ac_analysis_context = AcAnalysisContext { omega };
        let mut system = LinearSystem::new(self.symbolic.size);

        // 1. Collect AC stamps from all components
        // Note: You may need to cast your components to a trait that supports load_ac
        let stamps: Vec<Stamp<Complex<f64>>> = self
            .circuit
            .components
            .get_all()
            .iter()
            .flat_map(|comp| {
                // Assuming components implement an AcAnalysis trait you defined
                if let Some(ac_interface) = comp.as_ac() {
                    ac_interface.load_ac(dc_state, &ac_analysis_context, context)
                } else {
                    vec![]
                }
            })
            .collect();

        // 2. Apply and Solve
        system.apply_stamps(&self.symbolic, stamps);
        system.solve_with_backend(&self.symbolic)
    }

    pub fn solve_ac_sweep(
        &mut self,
        start_freq: f64,
        stop_freq: f64,
        steps: usize,
        logarithmic: bool,
        context: &Context,
    ) -> crate::error::Result<Vec<(f64, Vec<Complex<f64>>)>> {
        // 1. Always find the DC operating point first
        let dc_state = self.solve_dc(context)?;

        let mut results = Vec::with_capacity(steps);

        for i in 0..steps {
            let freq = if logarithmic {
                let log_start = start_freq.log10();
                let log_stop = stop_freq.log10();
                10.0f64.powf(log_start + (log_stop - log_start) * (i as f64 / (steps - 1) as f64))
            } else {
                start_freq + (stop_freq - start_freq) * (i as f64 / (steps - 1) as f64)
            };

            let omega = 2.0 * std::f64::consts::PI * freq;
            let solution = self.solve_ac(&dc_state, omega, context)?;
            results.push((freq, solution));
        }

        Ok(results)
    }

    pub fn solve_dc(&mut self, context: &Context) -> crate::error::Result<CircuitStates> {
        let mut state = CircuitStates::new(self.symbolic.mapping.clone(), 2);
        let transient_analysis_context = TransientAnalysisContext { time: 0.0, dt: 0.0 };

        self.solve_nr(&mut state, &transient_analysis_context, &context)?;
        Ok(state)
    }

    pub fn solve_transient(
        &mut self,
        stop_time: f64,
        dt: f64,
        context: &Context,
    ) -> crate::error::Result<Vec<Vec<f64>>> {
        // Initiate at rest
        let mut state = CircuitStates::new(self.symbolic.mapping.clone(), 2);
        state.push_commited(vec![0.0; self.symbolic.size], 0.0);
        let mut all_states = Vec::new();

        let mut current_time = dt;

        while current_time <= stop_time {
            let transient_analysis_context = TransientAnalysisContext {
                time: current_time,
                dt,
            };

            // Run NR for this specific time step
            self.solve_nr(&mut state, &transient_analysis_context, &context)?;
            all_states.push(state.history.values.get(0).unwrap().clone());

            current_time += dt;
        }

        println!("{:?}", state);

        Ok(all_states)
    }

    fn solve_nr(
        &mut self,
        state: &mut CircuitStates,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> crate::error::Result<()> {
        let max_iterations = 1000;

        // Start with the last converged state as our first guess
        let current_guess = state.history.values.get(0).cloned().unwrap();

        // Insert the guess into the buffer so components can read it at lookback 0
        state.history.values.push_front(current_guess);

        for i in 0..max_iterations {
            self.update_components(state, context)?;

            let stamps = self.stamp_components(|c| {
                if transient_analysis_context.dt == 0.0 {
                    c.as_dc().unwrap().load_dc(context)
                } else {
                    c.as_transient().unwrap().load_transient(
                        state,
                        transient_analysis_context,
                        context,
                    )
                }
            });

            let mut system = LinearSystem::new(self.symbolic.size);
            system.apply_stamps(&self.symbolic, stamps);
            let next_solution = system.solve_with_backend(&self.symbolic)?;

            state.history.values.push_front(next_solution);

            let converged = self.circuit.components.get_all().iter().all(|c| {
                c.as_transient().unwrap().check_convergence(
                    state,
                    transient_analysis_context,
                    context,
                )
            });

            if converged {
                // Success! Remove the iteration history, keeping only the winner at index 0
                let winner = state.history.values.pop_front().unwrap();
                state.history.values.pop_front(); // Remove the old guess
                state.history.values.push_front(winner);

                state.commit_step(transient_analysis_context.time, context.numerical_method);
                return Ok(());
            }

            // 4. Not converged: Remove the "oldest" guess to keep buffer size managed for next iter
            // index 0 is the new solution (will be the guess for next iter)
            // index 1 is the guess we just used
            state.history.values.remove(1);
        }

        Err(Error {
            title: "Convergence Failure".to_string(),
            detail: format!(
                "Newton-Raphson failed to converge in {} iterations at t={}",
                max_iterations, transient_analysis_context.time
            ),
            problems: vec![],
        })
    }

    fn update_components(
        &mut self,
        state: &mut CircuitStates,
        context: &Context,
    ) -> crate::error::Result<()> {
        for comp in self.circuit.components_mut().components.values_mut() {
            comp.update(state, context)?;
        }

        Ok(())
    }

    fn stamp_components<F>(&self, mapper_fn: F) -> Vec<Stamp<f64>>
    where
        F: Fn(&dyn Component) -> Vec<Stamp<f64>>,
    {
        self.circuit
            .components
            .get_all()
            .iter()
            .flat_map(|c| mapper_fn(c.as_ref())) // c is Box<dyn Component>, as_ref() gets &dyn Component
            .collect()
    }
}
