//! `Held`: the value of a param no device pin holds, printed from the host's table (HII-09 AC 1,
//! F4). A param that only placements read (a pass-through of a hierarchy) has no net in the
//! circuit, so a row cannot read it from the solver; the session knows its value at the start of
//! the run and every change a timed `set` makes to it, and each row reads the value in force at
//! its time.

use piperine_ir::ValueType;

use crate::table::Cell;

/// One param's value over a run: the value at the start and each change, in time order.
#[derive(Debug, Clone, PartialEq)]
pub struct Held {
    /// The param's full path, `<instance>.<param>`.
    pub path: String,
    /// The param's type.
    pub ty: ValueType,
    /// The value before any timed change.
    initial: f64,
    /// `(time, value)` of each change, the time ascending.
    changes: Vec<(f64, f64)>,
}

impl Held {
    /// A param of type `ty` at `path` holding `initial`, then each of `changes` (a time and the
    /// value taken after it), given in time order.
    pub fn new(path: String, ty: ValueType, initial: f64, changes: Vec<(f64, f64)>) -> Held {
        Held { path, ty, initial, changes }
    }

    /// The value in force at `time`: a change at `at` applies after the step to `at`, so the
    /// row at `at` still holds the value before it.
    pub fn value_at(&self, time: f64) -> f64 {
        self.changes.iter().take_while(|(at, _)| *at < time).last().map_or(self.initial, |(_, value)| *value)
    }

    /// The cell a row at `time` shows, of the param's type.
    pub fn cell_at(&self, time: f64) -> Cell {
        let value = self.value_at(time);
        match &self.ty {
            ValueType::Bool => Cell::Bool(value != 0.0),
            ValueType::UInt(_) | ValueType::Int(_) => Cell::Int(value as i128),
            _ => Cell::Real(value),
        }
    }

    /// The bytes of the value at `time` as a param's net holds them (what `Cell::decode_param`
    /// reads back): the value itself in the type's width, little endian.
    pub fn bytes_at(&self, time: f64) -> Vec<u8> {
        let value = self.value_at(time);
        match &self.ty {
            ValueType::Bool => vec![u8::from(value != 0.0)],
            ValueType::UInt(n) | ValueType::Int(n) => {
                let width = (*n as usize).div_ceil(8).next_power_of_two();
                (value as i64).to_le_bytes()[..width].to_vec()
            }
            _ => value.to_le_bytes().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_applies_after_the_step_to_its_time() {
        // HII-13 AC 2: the row at the time of the change holds the old value, the next holds the new
        let held = Held::new("Top.q".into(), ValueType::F64, 1.0, vec![(2e-6, 3.0), (4e-6, 5.0)]);
        let seen: Vec<f64> = [0.0, 2e-6, 3e-6, 4e-6, 5e-6].iter().map(|t| held.value_at(*t)).collect();
        assert_eq!(seen, [1.0, 1.0, 3.0, 3.0, 5.0]);
    }

    #[test]
    fn the_bytes_read_back_as_the_cell_of_the_type() {
        // the op and dc tables read a param by the bytes of its net: the encoding is decode_param's inverse
        for (ty, value, cell) in [(ValueType::F64, 2.5, Cell::Real(2.5)), (ValueType::Bool, 1.0, Cell::Bool(true)), (ValueType::UInt(8), 7.0, Cell::Int(7))] {
            let held = Held::new("Top.p".into(), ty.clone(), value, Vec::new());
            assert_eq!(Cell::decode_param(&ty, &held.bytes_at(0.0)).expect("decodes"), cell);
        }
    }
}
