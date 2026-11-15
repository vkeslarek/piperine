//! The five M1 device bodies of `devices.rs` in one checked `Ir`, behind an `Interpreter`, and
//! small readers of a shape: the module of a body by name, its pins, terms, inputs and functions
//! as plain values a test compares.

use std::sync::Arc;

use piperine_abi::caller::{CallError, Handle};
use piperine_abi::raw::{Cell, Module, Partial, Shape};
use piperine_interp::Interpreter;
use piperine_ir::{BodyId, Ir, IrBuilder};

use crate::devices::Devices;

/// An `Interpreter` over a design that holds every M1 device body.
pub struct Models {
    pub interp: Interpreter,
}

impl Models {
    /// `Resistor`, `Capacitor`, `Vsource`, `Isource`, `Diode`; the top is a resistor alone.
    pub fn all() -> Models {
        let mut b = IrBuilder::new();
        let r = Devices::resistor(&mut b);
        Devices::capacitor(&mut b);
        Devices::vsource(&mut b);
        Devices::isource(&mut b);
        Devices::diode(&mut b);
        let top = Devices::alone(&mut b, r, "R", 1, vec![1000.0]);
        Models::of(b.finish(top))
    }

    /// An `Interpreter` over `ir`, which must pass `check()`.
    pub fn of(ir: Ir) -> Models {
        let interp = Interpreter::new(Arc::new(ir)).unwrap_or_else(|r| panic!("the fixture fails check(): {:?}", r.entries));
        Models { interp }
    }

    pub fn body(&self, name: &str) -> BodyId {
        let ir = self.interp.ir();
        ir.bodies().find(|b| b.name() == name).unwrap_or_else(|| panic!("no body `{name}`")).id()
    }

    pub fn module(&self, name: &str) -> &'static Module {
        self.interp.module(self.body(name)).unwrap_or_else(|| panic!("body `{name}` has no module"))
    }

    /// An instance of `name` at its defaults.
    pub fn instance(&self, name: &str) -> Handle {
        Handle::create(self.module(name), &[]).unwrap_or_else(|e| panic!("create `{name}`: {e}"))
    }
}

/// What one call wrote: each write as an `F64`, and the partials.
#[derive(Debug, Clone, PartialEq)]
pub struct Out {
    pub values: Vec<f64>,
    pub jac: Vec<f64>,
}

/// One read cell's bytes.
pub enum In {
    F64(f64),
    Byte(u8),
}

impl Models {
    /// Runs function `k` of `h` on `input`, every partial seeded unless `seed` is given. Every
    /// write is an `F64` (terms, `PROPOSED`/`LIMITED`, `RE`/`IM`).
    pub fn eval(h: &Handle, k: u32, input: &[In], seed: Option<&[u8]>) -> Result<Out, CallError> {
        let f = h.shape().functions.get(k as usize).unwrap_or_else(|| panic!("no function {k}"));
        let (writes, partials) = (f.writes.len(), f.partials.len());
        let bytes: Vec<Vec<u8>> = input
            .iter()
            .map(|x| match x {
                In::F64(v) => v.to_le_bytes().to_vec(),
                In::Byte(b) => vec![*b],
            })
            .collect();
        let input: Vec<&[u8]> = bytes.iter().map(|b| b.as_slice()).collect();
        let mut out = vec![[0u8; 8]; writes];
        let mut output: Vec<&mut [u8]> = out.iter_mut().map(|b| &mut b[..]).collect();
        let all = vec![1u8; partials];
        let seed = seed.unwrap_or(&all);
        let mut jac = vec![0.0; partials];
        h.eval(k, &input, &mut output, seed, &mut jac)?;
        Ok(Out { values: out.iter().map(|b| f64::from_le_bytes(*b)).collect(), jac })
    }

    /// The bytes of `eval`'s writes and partials, for bit-for-bit comparison.
    pub fn bits(out: &Out) -> (Vec<u64>, Vec<u64>) {
        (out.values.iter().map(|v| v.to_bits()).collect(), out.jac.iter().map(|v| v.to_bits()).collect())
    }
}

/// A shape read as plain values.
pub struct View<'s> {
    pub shape: &'s Shape<'s>,
}

impl<'s> View<'s> {
    pub fn pin_names(&self) -> Vec<&str> {
        self.shape.pins.iter().map(|p| p.name.as_str()).collect()
    }

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

    pub fn function_names(&self) -> Vec<&str> {
        self.shape.functions.iter().map(|f| f.name.as_str()).collect()
    }

    pub fn function(&self, name: &str) -> (Vec<Cell>, Vec<Cell>, Vec<Partial>) {
        let f = self.shape.functions.iter().find(|f| f.name.as_str() == name).unwrap_or_else(|| panic!("no function `{name}`"));
        (f.reads.iter().copied().collect(), f.writes.iter().copied().collect(), f.partials.iter().copied().collect())
    }
}
