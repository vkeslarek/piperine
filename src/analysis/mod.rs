use crate::circuit::netlist::CircuitReference;

pub mod ac;
pub mod dc;
pub mod transient;

pub struct InitialValue {
    pub reference: CircuitReference,
    pub value: f64,
}
