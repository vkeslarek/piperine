use crate::circuit::netlist::CircuitReference;
use crate::math::newton_raphson::SolverState;
use crate::math::num::Field;

pub type CircuitState<E: Field> = SolverState<CircuitReference, E>;
