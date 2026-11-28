//! One body of a hand-built design compiled with `rustc` and called through the device ABI
//! (the evaluation harness of the printer tests of `piperine-transpile`'s `analog` and `digital` targets).

use std::sync::Arc;

use piperine_abi::caller::{CallError, Handle};
use piperine_abi::device::{Access, cell_lens};
use piperine_ir::{BodyId, Ir};
use piperine_plan::Modules;
use piperine_transpile::{Settings, Transpiler};

/// One body of a design compiled and instantiated at its defaults. The cache directory lives as
/// long as this does.
pub struct CompiledDevice {
    handle: Handle,
    _cache: tempfile::TempDir,
}

impl CompiledDevice {
    /// The body `name` of `ir`, compiled.
    pub fn of_body(ir: Ir, name: &str) -> CompiledDevice {
        let cache = tempfile::tempdir().expect("a temp dir");
        let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
        let ir = Arc::new(ir);
        let body: BodyId = ir.bodies().find(|b| b.name() == name).unwrap_or_else(|| panic!("no body {name}")).id();
        let compiled = Transpiler::new(ir).unwrap_or_else(|e| panic!("{e}")).compile(&settings).unwrap_or_else(|e| panic!("{e}"));
        let module = Modules::module(&compiled, body).unwrap_or_else(|| panic!("{name} has a module"));
        let handle = Handle::create(module, &[]).unwrap_or_else(|e| panic!("create: {e}"));
        CompiledDevice { handle, _cache: cache }
    }

    /// Runs function `name` on `reads`, one byte string per read cell, and returns each write as
    /// an `f64` (the first eight bytes of its cell).
    pub fn run(&self, name: &str, reads: &[&[u8]]) -> Result<Vec<f64>, CallError> {
        let (cells, _) = self.run_cells(name, reads)?;
        Ok(cells.iter().map(|c| f64::from_le_bytes(c[..8].try_into().expect("an f64 cell"))).collect())
    }

    /// `run`, and the partials the function declares, in the shape's order.
    pub fn run_with_partials(&self, name: &str, reads: &[&[u8]]) -> Result<(Vec<f64>, Vec<f64>), CallError> {
        let (cells, jac) = self.run_cells(name, reads)?;
        Ok((cells.iter().map(|c| f64::from_le_bytes(c[..8].try_into().expect("an f64 cell"))).collect(), jac))
    }

    /// `run`, with each write as the bytes of its cell, and the partials the function declares.
    pub fn run_cells(&self, name: &str, reads: &[&[u8]]) -> Result<(Vec<Vec<u8>>, Vec<f64>), CallError> {
        let shape = self.handle.shape();
        let k = shape.functions.iter().position(|f| f.name.as_str() == name).unwrap_or_else(|| panic!("no function `{name}`"));
        let f = shape.functions.get(k).expect("the function");
        let lens = cell_lens(shape, &f.writes, Access::FunctionWrite).expect("write lens");
        let mut cells: Vec<Vec<u8>> = lens.iter().map(|len| vec![0u8; *len]).collect();
        let mut writes: Vec<&mut [u8]> = cells.iter_mut().map(Vec::as_mut_slice).collect();
        let seed = vec![1u8; f.partials.len()];
        let mut jac = vec![0.0; f.partials.len()];
        self.handle.eval(k as u32, reads, &mut writes, &seed, &mut jac)?;
        Ok((cells, jac))
    }
}

/// FedBytes of the cells the tests feed.
pub struct FedBytes;

impl FedBytes {
    /// A known `f64` held by a continuous pin or an `F64` layout.
    pub fn real(v: f64) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }
}
