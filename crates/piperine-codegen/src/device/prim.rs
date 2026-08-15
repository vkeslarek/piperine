//! Gate-primitive and UDP dispatch (LRMC-C7-13, T40 — the dispatch half;
//! T39 lowered `GateInstance`/UDP instances into the POM). Neither needs
//! Cranelift: a gate's truth table (`kernel::digital::gates::
//! GatePrimitive`) and a UDP's state table (`kernel::digital::udp::
//! UdpTable`) are closed, LRM-fixed lookups over a small input vector —
//! there is no user-authored expression to JIT, so [`GateElement`]/
//! [`UdpElement`] implement [`Element`]'s `DigitalDevice` half directly
//! in plain Rust, the same way `MockInverter` does in
//! `piperine-solver`'s own tests, rather than routing through
//! [`DigitalInstance`](super::DigitalInstance)'s JIT-kernel machinery.
//!
//! **Sequential UDP scope note:** a sequential UDP's edge/level matching
//! (`UdpTable::eval_sequential`) needs the input vector from *before* the
//! current transition. [`UdpElement`] tracks that internally
//! (`prev_inputs`) inside `comb_phase`, evaluated whenever any input
//! changes — this does not use the two-phase `seq_phase`/`comb_phase`
//! split registers get (`kernel/digital/compile.rs`'s pre-edge-snapshot
//! mechanism), so two UDPs racing to read each other's pre-edge state in
//! the *same* delta cycle is not modeled — the same LRM-semantics-only
//! scope `udp.rs`'s own module doc already draws around this module.

use std::sync::Arc;

use piperine_lang::ast::{GateKind, PrimitiveDecl};
use piperine_lang::pom::GateInstance as PomGateInstance;
use piperine_solver::abi::{
    AnalogDevice, DigitalDevice, DigitalNet, DigitalPorts, Element, ElementCapabilities, EvalCtx, EventSink,
    Introspect, LogicValue,
};

use crate::error::CodegenError;
use crate::kernel::digital::gates::GatePrimitive;
use crate::kernel::digital::udp::UdpTable;

fn gate_primitive_for(kind: GateKind) -> Result<GatePrimitive, CodegenError> {
    Ok(match kind {
        GateKind::And => GatePrimitive::And,
        GateKind::Nand => GatePrimitive::Nand,
        GateKind::Or => GatePrimitive::Or,
        GateKind::Nor => GatePrimitive::Nor,
        GateKind::Xor => GatePrimitive::Xor,
        GateKind::Xnor => GatePrimitive::Xnor,
        GateKind::Buf => GatePrimitive::Buf,
        GateKind::Not => GatePrimitive::Not,
        GateKind::Bufif0 => GatePrimitive::Bufif0,
        GateKind::Bufif1 => GatePrimitive::Bufif1,
        GateKind::Notif0 => GatePrimitive::Notif0,
        GateKind::Notif1 => GatePrimitive::Notif1,
        // T39 already rejects every switch-level kind at lowering — this
        // arm is defense in depth, never reached from a design that
        // elaborated successfully.
        other => {
            return Err(CodegenError::unsupported(format!("gate primitive `{other:?}` has no digital dispatch")));
        }
    })
}

/// One gate-primitive instance (Annex A.4.2), dispatched via
/// [`GatePrimitive::eval_n`]/`eval1` — no Cranelift kernel.
pub struct GateElement {
    name: String,
    primitive: GatePrimitive,
    output: DigitalNet,
    /// n-input gates: every input, in order. `buf`/`not`: the single
    /// data input. Tristate (`bufif*`/`notif*`): `[data, control]`.
    inputs: Vec<DigitalNet>,
    delay: f64,
}

impl GateElement {
    pub fn new(pom: &PomGateInstance, device_id: usize, output: DigitalNet, inputs: Vec<DigitalNet>) -> Result<Self, CodegenError> {
        let primitive = gate_primitive_for(pom.kind)?;
        Ok(Self {
            name: pom.name.clone().unwrap_or_else(|| format!("gate_{device_id}")),
            primitive,
            output,
            inputs,
            delay: pom.delay.unwrap_or(0.0),
        })
    }
}

impl AnalogDevice for GateElement {}
impl Introspect for GateElement {}

impl DigitalDevice for GateElement {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: &self.inputs, outputs: std::slice::from_ref(&self.output) }
    }

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        let vals: Vec<LogicValue> = self.inputs.iter().map(|n| ctx.nets[n.0]).collect();
        let out = match self.primitive {
            GatePrimitive::And | GatePrimitive::Nand | GatePrimitive::Or | GatePrimitive::Nor
            | GatePrimitive::Xor | GatePrimitive::Xnor => self.primitive.eval_n(&vals),
            GatePrimitive::Buf | GatePrimitive::Not => vals.first().and_then(|&v| self.primitive.eval1(v, None)),
            GatePrimitive::Bufif0 | GatePrimitive::Bufif1 | GatePrimitive::Notif0 | GatePrimitive::Notif1 => {
                match (vals.first(), vals.get(1)) {
                    (Some(&data), Some(&control)) => self.primitive.eval1(data, Some(control)),
                    _ => None,
                }
            }
        };
        if let Some(v) = out {
            sink.emit(self.output, v, self.delay);
        }
    }
}

impl Element for GateElement {
    fn name(&self) -> &str {
        &self.name
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::DIGITAL
    }
}

/// One UDP instance (Annex A.5), dispatched via [`UdpTable`] — no
/// Cranelift kernel. `decl` is `Arc`-shared so every instance of the same
/// UDP borrows one parsed table rather than cloning it.
pub struct UdpElement {
    name: String,
    decl: Arc<PrimitiveDecl>,
    output: DigitalNet,
    inputs: Vec<DigitalNet>,
    /// Sequential UDPs only: the register's current state and the input
    /// vector from the previous evaluation (edge matching needs both —
    /// see this module's own scope note on the two-phase split it does
    /// not use).
    state: LogicValue,
    prev_inputs: Option<Vec<LogicValue>>,
}

impl UdpElement {
    pub fn new(decl: Arc<PrimitiveDecl>, name: String, output: DigitalNet, inputs: Vec<DigitalNet>) -> Self {
        Self { name, decl, output, inputs, state: LogicValue::X, prev_inputs: None }
    }
}

impl AnalogDevice for UdpElement {}
impl Introspect for UdpElement {}

impl DigitalDevice for UdpElement {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: &self.inputs, outputs: std::slice::from_ref(&self.output) }
    }

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        let vals: Vec<LogicValue> = self.inputs.iter().map(|n| ctx.nets[n.0]).collect();
        let table = UdpTable::new(&self.decl);
        let next = if self.decl.is_sequential {
            let prev = self.prev_inputs.clone().unwrap_or_else(|| vals.clone());
            let result = table.eval_sequential(&prev, &vals, self.state).unwrap_or(self.state);
            self.prev_inputs = Some(vals);
            self.state = result;
            result
        } else {
            table.eval_combinational(&vals).unwrap_or(LogicValue::X)
        };
        sink.emit(self.output, next, 0.0);
    }
}

impl Element for UdpElement {
    fn name(&self) -> &str {
        &self.name
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::DIGITAL
    }
}
