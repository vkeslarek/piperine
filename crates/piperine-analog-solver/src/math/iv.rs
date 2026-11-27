//! Initial values: the `InitialValue` seed type and its application to
//! the solution history buffer.
use crate::math::linear::AsIndex;
use crate::math::num::Scalar;

#[derive(Clone)]
pub struct InitialValue<A: AsIndex, E: Scalar> {
    pub reference: A,
    pub value: E,
}
