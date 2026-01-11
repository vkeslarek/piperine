use crate::circuit::netlist::CircuitReference;
use crate::math::num::Field;

pub mod ac;
pub mod dc;
pub mod transient;

pub struct InitialValue<E: Field> {
    pub reference: CircuitReference,
    pub value: E,
}
