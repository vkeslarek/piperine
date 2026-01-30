use crate::expression::Expr;
use crate::num::Scalar;

mod behavioral_source;
mod capacitor;
mod inductor;
mod linear_source;
mod resistor;
mod source;
mod switch;
mod diode;
mod bjt;
mod jfet;
mod mesfet;

pub trait Component {
    fn name(&self) -> &String;
}

pub trait Model {
    type ComponentType: Component;
}

#[derive(Debug, Clone)]
pub enum Dynamic<T: Scalar> {
    Literal(T),
    Expression(Expr),
}

impl<T: Scalar> From<T> for Dynamic<T> {
    fn from(val: T) -> Self {
        Dynamic::Literal(val)
    }
}

impl<T: Scalar> From<Expr> for Dynamic<T> {
    fn from(expr: Expr) -> Self {
        Dynamic::Expression(expr)
    }
}
