//! `View`: an ABI shape read as plain values a test compares.

use piperine_abi::raw::{Cell, Partial, Shape};

/// A shape read as plain values.
pub struct View<'s> {
    /// The shape read.
    pub shape: &'s Shape<'s>,
}

impl<'s> View<'s> {
    /// The pin names, in declaration order.
    pub fn pin_names(&self) -> Vec<&str> {
        self.shape.pins.iter().map(|p| p.name.as_str()).collect()
    }

    /// The index of the pin `name`.
    pub fn pin_index(&self, name: &str) -> u32 {
        self.shape.pins.iter().position(|p| p.name.as_str() == name).unwrap_or_else(|| panic!("no pin `{name}`")) as u32
    }

    /// `(name, op, plus, minus)` of each term, ends by pin name (`GROUND` as "GROUND").
    pub fn terms(&self) -> Vec<(String, u32, String, String)> {
        let end = |i: u32| self.shape.pins.get(i as usize).map_or("GROUND".to_string(), |p| p.name.as_str().to_string());
        self.shape.terms.iter().map(|t| (t.name.as_str().to_string(), t.op, end(t.plus), end(t.minus))).collect()
    }

    /// `(name, kind)` of each input.
    pub fn inputs(&self) -> Vec<(String, u32)> {
        self.shape.inputs.iter().map(|x| (x.name.as_str().to_string(), x.kind)).collect()
    }

    /// The names of the shape's functions.
    pub fn function_names(&self) -> Vec<&str> {
        self.shape.functions.iter().map(|f| f.name.as_str()).collect()
    }

    /// The `(reads, writes, partials)` cells of the function `name`.
    pub fn function(&self, name: &str) -> (Vec<Cell>, Vec<Cell>, Vec<Partial>) {
        let f = self.shape.functions.iter().find(|f| f.name.as_str() == name).unwrap_or_else(|| panic!("no function `{name}`"));
        (f.reads.iter().copied().collect(), f.writes.iter().copied().collect(), f.partials.iter().copied().collect())
    }
}
