//! IR → CircuitInstance adapter.
//!
//! Given an [`IrProgram`] and the name of a top module, walks the top's
//! `instances`, dispatches each to the analog or digital IR-to-device
//! adapter, attaches nets, and returns a `CircuitInstance` ready for the solver.

pub mod device;
pub mod digital;
pub mod digital_lower;

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use piperine_solver::analog::{NodeIdentifier, Netlist};
use piperine_solver::circuit::CircuitInstance;
use piperine_solver::device::Device;
use piperine_solver::digital::DigitalNet;

use piperine_codegen::CodegenError;
use piperine_codegen::ir::{IrProgram, NodeId, ParamId, VarId};
use crate::runtime::digital_lower::ir_digital_to_interp;
use crate::runtime::device::PhdlDevice;

pub fn from_ir(program: &IrProgram, top: &str) -> Result<CircuitInstance, String> {
    let mut compiler = piperine_codegen::CircuitCompiler::new(program);
    compiler.build_circuit(top).map_err(|e| e.to_string())
}
