use crate::math::num::Field;

#[derive(Debug, Clone)]
pub enum Dynamic<T: Field> {
    Empty,
    Literal(T),
}

impl<T: Field> Dynamic<T> {
    pub fn eval_or(&self, default: T) -> T {
        match self {
            Dynamic::Empty => default,
            Dynamic::Literal(val) => *val,
        }
    }
}

impl<T: Field> From<T> for Dynamic<T> {
    fn from(val: T) -> Self {
        Dynamic::Literal(val)
    }
}

impl<T: Field> From<Option<T>> for Dynamic<T> {
    fn from(value: Option<T>) -> Self {
        if let Some(val) = value {
            Dynamic::Literal(val)
        } else {
            Dynamic::Empty
        }
    }
}
