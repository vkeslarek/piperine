use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use piperine_solver::analog::{NodeIdentifier, Netlist};
use piperine_solver::circuit::CircuitInstance;
use piperine_solver::device::Device;
use piperine_solver::digital::DigitalNet;

use crate::codegen::{compile_analog_module, compile_digital_module};
use crate::phdl_device::PhdlDevice;
use piperine_lang::elab::const_eval::ConstVal;
use piperine_lang::elab::ir::{Instance, Module, Design};

static NODE_CTR:  AtomicUsize = AtomicUsize::new(100_000);
static DNET_CTR:  AtomicUsize = AtomicUsize::new(0);
static DEVICE_CTR: AtomicUsize = AtomicUsize::new(0);

pub fn from_elab(prog: &Design, top_module: &str) -> Result<CircuitInstance, String> {
    let top = prog.module(top_module)
        .ok_or_else(|| format!("top module '{}' not found", top_module))?;

    let mut net_to_node: HashMap<String, NodeIdentifier> = HashMap::new();
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

    for port in top.ports() { assign_node(port.name(), &mut net_to_node); }
    for wire in top.wires() { assign_node(wire.name(), &mut net_to_node); }

    let mut netlist = Netlist::new();
    let mut devices: Vec<Box<dyn Device>> = Vec::new();

    for inst in top.instances() {
        let mod_def = prog.module(inst.module_name()).ok_or_else(|| {
            format!("module '{}' not found (instance '{:?}')", inst.module_name(), inst.label())
        })?;

        let terminals: Vec<NodeIdentifier> = inst.ports().iter().map(|net_ref| {
            net_to_node.get(net_ref.net())
                .cloned()
                .unwrap_or(NodeIdentifier::Gnd)
        }).collect();

        let params = resolve_params(mod_def, inst);

        let instance_name = inst.label()
            .unwrap_or_else(|| inst.module_name())
            .to_string();

        let analog = compile_analog_module(prog, inst.module_name()).ok().map(Arc::new);

        let device_id = DEVICE_CTR.fetch_add(1, Ordering::Relaxed);
        let digital = compile_digital_module(prog, inst.module_name(), device_id).ok()
            .map(|mut interp| {
                let port_net_map: HashMap<String, DigitalNet> = mod_def.ports().iter()
                    .enumerate()
                    .filter_map(|(i, port)| {
                        let wire_name = inst.ports().get(i).map(|r| r.net())?;
                        let dnet = assign_dnet(wire_name, &mut wire_to_dnet);
                        Some((port.name().to_string(), dnet))
                    })
                    .collect();
                interp.set_port_nets(port_net_map);
                interp
            });

        if analog.is_some() || digital.is_some() {
            let mut dev = PhdlDevice::new(
                &instance_name,
                analog,
                digital,
                Vec::new(),
                params,
            );
            dev.allocate_nodes(&terminals, &mut netlist);
            devices.push(Box::new(dev));
        }
    }

    Ok(CircuitInstance::from_devices_and_netlist(top_module, devices, netlist))
}

fn const_to_f64(cv: &ConstVal) -> f64 {
    match cv {
        ConstVal::Real(v)  => *v,
        ConstVal::Int(n)   => *n as f64,
        ConstVal::Nat(n)   => *n as f64,
        ConstVal::Bool(b)  => if *b { 1.0 } else { 0.0 },
        _                  => 0.0,
    }
}

fn resolve_params(mod_def: &Module, inst: &Instance) -> Vec<f64> {
    mod_def.params().iter().map(|ep| {
        if let Some((_, cv)) = inst.params().iter().find(|(n, _)| n == ep.name()) {
            return const_to_f64(cv);
        }
        ep.default().map(const_to_f64).unwrap_or(0.0)
    }).collect()
}