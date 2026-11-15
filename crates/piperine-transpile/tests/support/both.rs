//! The five M1 device bodies built once on each backend: the interpreter's modules and the
//! compiled library's, in a cache directory of the test's own, with small readers that evaluate
//! one function of a body on either and give back what it wrote.

use std::sync::Arc;

use piperine_abi::caller::{CallError, Handle};
use piperine_abi::raw::Module;
use piperine_interp::Interpreter;
use piperine_ir::{Ir, IrBuilder};
use piperine_plan::Modules;
use piperine_transpile::{Compiled, Settings, Transpiler};

use crate::devices::Devices;

/// The five bodies on both backends. The cache directory lives as long as this does.
pub struct Both {
    pub compiled: Compiled,
    pub interp: Interpreter,
    _cache: tempfile::TempDir,
}

/// What one call wrote: each written cell's bytes, and the partials.
#[derive(Debug, Clone, PartialEq)]
pub struct Written {
    pub cells: Vec<Vec<u8>>,
    pub jac: Vec<f64>,
}

impl Written {
    /// The partials as bits, so two `-0.0`/`0.0` or two NaNs are told apart.
    pub fn jac_bits(&self) -> Vec<u64> {
        self.jac.iter().map(|v| v.to_bits()).collect()
    }

    /// Write `i` read as an `f64`.
    pub fn real(&self, i: usize) -> f64 {
        f64::from_le_bytes(self.cells[i][..8].try_into().expect("an 8-byte cell"))
    }
}

impl Both {
    /// `Resistor`, `Capacitor`, `Vsource`, `Isource`, `Diode`; the top is a resistor alone.
    pub fn five() -> Both {
        let mut b = IrBuilder::new();
        let r = Devices::resistor(&mut b);
        Devices::capacitor(&mut b);
        Devices::vsource(&mut b);
        Devices::isource(&mut b);
        Devices::diode(&mut b);
        let top = Devices::alone(&mut b, r, "R", 1, vec![1000.0]);
        Both::of(b.finish(top))
    }

    /// `ir` on both backends, the compiled one built in a fresh cache.
    pub fn of(ir: Ir) -> Both {
        let cache = tempfile::tempdir().expect("a temp dir");
        let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
        let ir = Arc::new(ir);
        let compiled = Transpiler::new(ir.clone())
            .unwrap_or_else(|e| panic!("{e}"))
            .compile(&settings)
            .unwrap_or_else(|e| panic!("{e}"));
        let interp = Interpreter::new(ir).unwrap_or_else(|r| panic!("{:?}", r.entries));
        Both { compiled, interp, _cache: cache }
    }

    /// The body names, in the design's order, that have a module.
    pub fn bodies(&self) -> Vec<String> {
        self.compiled.ir().bodies().filter(|b| Modules::module(&self.compiled, b.id()).is_some()).map(|b| b.name().to_string()).collect()
    }

    fn body(&self, name: &str) -> piperine_ir::BodyId {
        self.compiled.ir().bodies().find(|b| b.name() == name).unwrap_or_else(|| panic!("no body `{name}`")).id()
    }

    pub fn compiled_module(&self, name: &str) -> &'static Module {
        Modules::module(&self.compiled, self.body(name)).unwrap_or_else(|| panic!("`{name}` has no compiled module"))
    }

    pub fn interp_module(&self, name: &str) -> &'static Module {
        Modules::module(&self.interp, self.body(name)).unwrap_or_else(|| panic!("`{name}` has no interpreted module"))
    }

    /// An instance of each backend's module of `name`, at the defaults: `(interpreted, compiled)`.
    pub fn instances(&self, name: &str) -> (Handle, Handle) {
        let make = |m: &'static Module, what: &str| Handle::create(m, &[]).unwrap_or_else(|e| panic!("create {what} `{name}`: {e}"));
        (make(self.interp_module(name), "interpreted"), make(self.compiled_module(name), "compiled"))
    }
}

impl Written {
    /// Runs function `k` of `h` on `input`; every partial is seeded unless `seed` says otherwise.
    pub fn eval(h: &Handle, k: u32, input: &[Vec<u8>], seed: Option<&[u8]>) -> Result<Written, CallError> {
        let shape = h.shape();
        let f = shape.functions.get(k as usize).unwrap_or_else(|| panic!("no function {k}"));
        let lens = piperine_abi::device::cell_lens(shape, &f.writes, piperine_abi::device::Access::FunctionWrite).expect("write lens");
        let n = f.partials.len();
        let reads: Vec<&[u8]> = input.iter().map(Vec::as_slice).collect();
        let mut cells: Vec<Vec<u8>> = lens.iter().map(|len| vec![0u8; *len]).collect();
        let mut writes: Vec<&mut [u8]> = cells.iter_mut().map(Vec::as_mut_slice).collect();
        let all = vec![1u8; n];
        let mut jac = vec![0.0; n];
        h.eval(k, &reads, &mut writes, seed.unwrap_or(&all), &mut jac)?;
        Ok(Written { cells, jac })
    }
}
