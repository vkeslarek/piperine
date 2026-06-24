use crate::math::num::Scalar;

#[derive(Debug, Clone)]
pub enum Dynamic<T: Scalar> {
    Empty,
    Literal(T),
}

impl<T: Scalar> Dynamic<T> {
    pub fn eval_or(&self, default: T) -> T {
        match self {
            Dynamic::Empty => default,
            Dynamic::Literal(val) => *val,
        }
    }
}

impl<T: Scalar> From<T> for Dynamic<T> {
    fn from(val: T) -> Self {
        Dynamic::Literal(val)
    }
}

impl<T: Scalar> From<Option<T>> for Dynamic<T> {
    fn from(value: Option<T>) -> Self {
        if let Some(val) = value {
            Dynamic::Literal(val)
        } else {
            Dynamic::Empty
        }
    }
}
