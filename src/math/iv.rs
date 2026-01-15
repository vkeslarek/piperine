use crate::math::num::Field;
use crate::math::Symbol;

pub struct InitialValue<S: Symbol, E: Field> {
    pub reference: S,
    pub value: E,
}