//! IR → CircuitInstance adapter.
//!
//! Phase 1.6: the final glue step.  Given an [`IrProgram`] and the name of a
//! top module, walks the top's `instances`, dispatches each to the
//! analog or digital IR-to-device adapter, attaches nets, and returns a
//! `CircuitInstance` ready for the solver.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use piperine_solver::analog::{NodeIdentifier, Netlist};
use piperine_solver::circuit::CircuitInstance;
use piperine_solver::device::Device;
use piperine_solver::digital::DigitalNet;

use crate::codegen::CodegenError;
use crate::ir::IrProgram;
use crate::ir_analog_to_device::ir_analog_to_device;
use crate::ir_digital_to_interp::ir_digital_to_interp;
use crate::phdl_device::PhdlDevice;

static NODE_CTR: AtomicUsize = AtomicUsize::new(100_000);
static DNET_CTR: AtomicUsize = AtomicUsize::new(0);

/// Build a [`CircuitInstance`] directly from an [`IrProgram`].
///
/// `top` names the module whose instances form the netlist.  The top
/// module itself is **not** instantiated as a device; its children are.
pub fn from_ir(program: &IrProgram, top: &str) -> Result<CircuitInstance, String> {
    let top_module = program
        .modules
        .iter()
        .find(|m| m.name == top)
        .ok_or_else(|| format!("top module '{top}' not found"))?;

    // net_name → NodeIdentifier for analog nets.
    let mut net_to_node: HashMap<String, NodeIdentifier> = HashMap::new();
    // wire_name → DigitalNet for digital nets.
    let mut wire_to_dnet: HashMap<String, DigitalNet> = HashMap::new();

    let gnd_names = ["gnd", "GND", "vss", "VSS"];
    let is_gnd = |name: &str| gnd_names.contains(&name);
    let assign_node = |name: &str, map: &mut HashMap<String, NodeIdentifier>| {
        map.entry(name.to_string()).or_insert_with(|| {
            if is_gnd(name) {
                NodeIdentifier::Gnd
            } else {
                NodeIdentifier::Anonymous(NODE_CTR.fetch_add(1, Ordering::Relaxed))
            }
        }).clone()
    };
    let assign_dnet = |name: &str, map: &mut HashMap<String, DigitalNet>| -> DigitalNet {
        *map.entry(name.to_string()).or_insert_with(|| {
            DigitalNet(DNET_CTR.fetch_add(1, Ordering::Relaxed))
        })
    };

    // Register every port/wire of the top as a known net so child instances
    // can resolve their connections to either Gnd or an anonymous node.
    for p in &top_module.ports {
        assign_node(&p.name, &mut net_to_node);
    }
    for w in &top_module.wires {
        assign_node(&w.name, &mut net_to_node);
    }
    for ic in &top_module.connections {
        // lhs = rhs: treat them as the same node.
        if let (Some(left), Some(right)) = (
            Some(ic.lhs.clone()),
            Some(ic.rhs.clone()),
        ) {
            let right_node = assign_node(&right, &mut net_to_node);
            net_to_node.insert(left, right_node);
        }
    }

    let mut netlist = Netlist::new();
    let mut devices: Vec<Box<dyn Device>> = Vec::new();

    for inst in &top_module.instances {
        let child = program
            .modules
            .iter()
            .find(|m| m.name == inst.module)
            .ok_or_else(|| {
                format!(
                    "module '{}' not found (instance '{}')",
                    inst.module, inst.label
                )
            })?;

        // Map each port connection (positional or named) to the net name.
        // IR connections are positional with `port: Option<String>`;
        // when `port` is None, ports are matched in declaration order.
        let mut terminal_for_port: HashMap<String, NodeIdentifier> = HashMap::new();
        for (idx, conn) in inst.connections.iter().enumerate() {
            let port_name = conn
                .port
                .clone()
                .or_else(|| child.ports.get(idx).map(|p| p.name.clone()))
                .ok_or_else(|| format!("missing port for connection {idx} on instance '{}'", inst.label))?;
            let net_id = assign_node(&conn.net, &mut net_to_node);
            terminal_for_port.insert(port_name, net_id);
        }

        // Build the port-order terminal list once; re-use the same Netlist
        // for both analog and digital.
        let terminal_list: Vec<NodeIdentifier> = (0..child.ports.len())
            .map(|i| {
                let port_name = &child.ports[i].name;
                terminal_for_port
                    .get(port_name)
                    .cloned()
                    .unwrap_or(NodeIdentifier::Gnd)
            })
            .collect();

        // Resolve parameters.  Defaults are evaluated in declaration order so
        // a later default can reference an earlier param (`real b = 2*a;`).
        // Instance overrides are applied afterwards and may reference any
        // already-resolved param.  Anything that is not a compile-time
        // constant falls back to 0.0.
        let mut resolved: HashMap<String, f64> = HashMap::new();
        for p in &child.params {
            let v = p
                .default
                .as_ref()
                .and_then(|d| eval_ir_const(d, &resolved))
                .unwrap_or(0.0);
            resolved.insert(p.name.clone(), v);
        }
        for (pname, pval) in &inst.params {
            if let Some(v) = eval_ir_const(pval, &resolved) {
                resolved.insert(pname.clone(), v);
            }
        }
        let params: Vec<f64> = child
            .params
            .iter()
            .map(|p| resolved.get(&p.name).copied().unwrap_or(0.0))
            .collect();

        // Compile body (analog & digital).
        let analog_dev = if child.analog.is_some() {
            ir_analog_to_device(program, &child.name)
                .ok()
                .map(Arc::new)
        } else {
            None
        };

        let digital_interp = ir_digital_to_interp(program, &child.name).ok().map(
            |mut interp| {
                let port_net_map: HashMap<String, DigitalNet> = child
                    .ports
                    .iter()
                    .filter_map(|port| {
                        let dnet = assign_dnet(
                            &inst.connections
                                .iter()
                                .find(|c| c.port.as_deref() == Some(&port.name))
                                .map(|c| c.net.clone())
                                .unwrap_or_else(|| port.name.clone()),
                            &mut wire_to_dnet,
                        );
                        Some((port.name.clone(), dnet))
                    })
                    .collect();
                interp.set_port_nets(port_net_map);
                interp
            },
        );

        if analog_dev.is_some() || digital_interp.is_some() {
            let mut dev = PhdlDevice::new(
                &inst.label,
                analog_dev,
                digital_interp,
                Vec::new(),
                params,
            );
            dev.allocate_nodes(&terminal_list, &mut netlist);
            devices.push(Box::new(dev));
        }
    }

    Ok(CircuitInstance::from_devices_and_netlist(top, devices, netlist))
}

/// Best-effort compile-time evaluation of an [`IrExpr`] to an `f64`.
///
/// Handles literals, references to already-resolved params (`env`), the usual
/// arithmetic/comparison/logical operators, ternary `Select`, and built-in
/// math calls.  Returns `None` for anything that is not a compile-time
/// constant (branch accesses, simulator queries, unknown calls, …), letting
/// the caller fall back to a default.
fn eval_ir_const(e: &crate::ir::IrExpr, env: &HashMap<String, f64>) -> Option<f64> {
    use crate::ir::{IrBinOp, IrExpr, IrUnOp};

    let b2f = |b: bool| if b { 1.0 } else { 0.0 };
    match e {
        IrExpr::Real(v) => Some(*v),
        IrExpr::Int(v) => Some(*v as f64),
        IrExpr::Bool(b) => Some(b2f(*b)),
        IrExpr::Param(name) | IrExpr::Var(name) => {
            if name == "inf" {
                Some(f64::INFINITY)
            } else {
                env.get(name).copied()
            }
        }
        IrExpr::Unary(op, x) => {
            let v = eval_ir_const(x, env)?;
            match op {
                IrUnOp::Neg => Some(-v),
                IrUnOp::Not => Some(b2f(v == 0.0)),
                _ => None,
            }
        }
        IrExpr::Binary(op, a, b) => {
            let l = eval_ir_const(a, env)?;
            let r = eval_ir_const(b, env)?;
            Some(match op {
                IrBinOp::Add => l + r,
                IrBinOp::Sub => l - r,
                IrBinOp::Mul => l * r,
                IrBinOp::Div => l / r,
                IrBinOp::Rem => l % r,
                IrBinOp::Pow => l.powf(r),
                IrBinOp::Eq => b2f(l == r),
                IrBinOp::Ne => b2f(l != r),
                IrBinOp::Lt => b2f(l < r),
                IrBinOp::Le => b2f(l <= r),
                IrBinOp::Gt => b2f(l > r),
                IrBinOp::Ge => b2f(l >= r),
                IrBinOp::And => b2f(l != 0.0 && r != 0.0),
                IrBinOp::Or => b2f(l != 0.0 || r != 0.0),
                _ => return None,
            })
        }
        IrExpr::Select(c, t, f) => {
            if eval_ir_const(c, env)? != 0.0 {
                eval_ir_const(t, env)
            } else {
                eval_ir_const(f, env)
            }
        }
        IrExpr::Call(name, args) => {
            let a0 = args.first().and_then(|a| eval_ir_const(a, env));
            let a1 = args.get(1).and_then(|a| eval_ir_const(a, env));
            match (name.as_str(), a0, a1) {
                ("exp", Some(x), _) => Some(x.exp()),
                ("ln", Some(x), _) | ("log", Some(x), _) => Some(x.ln()),
                ("log10", Some(x), _) => Some(x.log10()),
                ("sqrt", Some(x), _) => Some(x.sqrt()),
                ("abs", Some(x), _) => Some(x.abs()),
                ("sin", Some(x), _) => Some(x.sin()),
                ("cos", Some(x), _) => Some(x.cos()),
                ("tan", Some(x), _) => Some(x.tan()),
                ("floor", Some(x), _) => Some(x.floor()),
                ("ceil", Some(x), _) => Some(x.ceil()),
                ("pow", Some(x), Some(y)) => Some(x.powf(y)),
                ("min", Some(x), Some(y)) => Some(x.min(y)),
                ("max", Some(x), Some(y)) => Some(x.max(y)),
                _ => None,
            }
        }
        _ => None,
    }
}

// `CodegenError` is used by the *inner* adapters; we surface their
// `Result::Err` here as a plain `String` so the public API matches the
// older `from_elab` shape and tests can use `?` ergonomics.
#[allow(dead_code)]
fn _mark_used(_e: CodegenError) {}
