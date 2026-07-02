//! Lower `Design` (PPR/PHDL) → `IrProgram`.

use std::collections::{HashMap, HashSet};

use crate::pom::Design;

use piperine_codegen::ir::*;

pub mod analog_ops;
pub mod event;
pub mod expr;
pub mod stmt;
pub mod structure;
pub mod syscalls;

use structure::{convert_fn, convert_mod, const_val_to_ir};
use stmt::lower_stmts;

// ─── Context ──────────────────────────────────────────────────────────────────

/// Lowering context carrying the current scope, state-variable counter, and
/// discovered noise-source list.
pub(crate) struct LowerCtx<'a> {
    /// Name → IR expression bindings for the current scope.
    pub env: HashMap<String, IrExpr>,
    /// The module's symbol table, populated during `convert_mod`.
    pub symbols: &'a mut SymbolTable,
    /// State variables (ddt, idt, etc.) allocated during this behavior lowering.
    pub states: Vec<StateId>,
    /// Noise sources discovered from contribution right-hand sides.
    pub noise_sources: Vec<IrNoiseSource>,
    /// Set to `true` while lowering a `digital` body.  Lets the Bind-Force
    /// arm pick the digital-drive form (`IrStmt::Assign`) instead of the
    /// analog-force form (`IrStmt::Force`).
    pub is_digital: bool,
    /// Names of the owning module's persistent `var`s (GAPS §I.15).
    pub module_vars: HashSet<String>,
    /// Map from `"instance_name.port_name"` → NodeId for named instance
    /// port access (SPEC §7.3: `I(load.p, gnd) <+ …`). The parent
    /// contributes to the child's port node, which is the parent-scope
    /// node the port is connected to.
    pub instance_ports: HashMap<String, NodeId>,
}

impl<'a> LowerCtx<'a> {
    /// Create a fresh lowering context.
    pub fn new(symbols: &'a mut SymbolTable, is_digital: bool, module_vars: HashSet<String>) -> Self {
        Self {
            env: HashMap::new(),
            symbols,
            states: vec![],
            noise_sources: vec![],
            is_digital,
            module_vars,
            instance_ports: HashMap::new(),
        }
    }

    /// Lookup a named instance port (e.g. `load.p`) → NodeId.
    pub fn lookup_instance_port(&self, qualified: &str) -> Option<NodeId> {
        self.instance_ports.get(qualified).copied()
    }

    /// Allocate a new state variable of `kind`, returning its `StateId`.
    pub fn alloc_state(&mut self, kind: IrStateKind, arg: IrExpr) -> StateId {
        let id = self.symbols.add_state(IrStateVar { kind, arg });
        self.states.push(id);
        id
    }

    /// Lookup a parameter by name.
    pub fn lookup_param(&self, name: &str) -> Option<ParamId> {
        self.symbols.params().find(|(_, p)| p.name == name).map(|(id, _)| id)
    }

    /// Lookup a variable by name.
    pub fn lookup_var(&self, name: &str) -> Option<VarId> {
        self.symbols.vars().find(|(_, v)| v.name == name).map(|(id, _)| id)
    }

    /// Lookup a node (net/port) by name. Also resolves named-instance
    /// port accesses (`load.p` → the parent NodeId the port connects to,
    /// SPEC §7.3).
    pub fn lookup_node(&self, name: &str) -> Option<NodeId> {
        if name == "gnd" || name == "GND" || name == "vss" || name == "VSS" || name == "0" {
            return Some(NodeId::GROUND);
        }
        // Check instance port map first (e.g. "load.p" or "rseg_0.n").
        if let Some(id) = self.instance_ports.get(name) {
            return Some(*id);
        }
        self.symbols.nodes().find(|(_, n)| n.name == name).map(|(id, _)| id)
    }
}

// ─── Entry point ──────────────────────────────────────────────────────────────

/// Lower a PHDL design into an [`IrProgram`] by converting every module and
/// attaching its analog/digital behavior blocks.
pub fn ppr_to_ir(prog: &Design) -> IrProgram {
    let mut modules: Vec<IrModule> = Vec::new();

    // Pass 1: Build the IrModule skeleton with SymbolTable for all modules.
    for m in prog.modules() {
        modules.push(convert_mod(m, prog));
    }

    // Pass 1.5: Add non-generic functions to each module's symbol table.
    // Generic functions (map, reduce, …) are elaboration-time generators:
    // they are monomorphized at call sites and never lowered as-is, so
    // adding their unresolved bodies would produce dangling references.
    for m in &mut modules {
        for f in prog.functions() {
            if f.is_generic() {
                continue;
            }
            let ir_f = convert_fn(f, prog, &mut m.symbols);
            m.symbols.add_fn(ir_f);
        }
    }

    // Pass 2: Lower behaviors using the built SymbolTables.
    for (i, m) in prog.modules().enumerate() {
        // Build the instance-port map (SPEC §7.3): for each named instance,
        // map `"label.port_name"` → parent NodeId. This lets the parent's
        // analog body reference child ports (`I(load.p, gnd) <+ …`).
        let mut instance_ports: HashMap<String, NodeId> = HashMap::new();
        for inst in m.instances() {
            if let Some(label) = inst.label() {
                // Look up the child module to get port names.
                if let Some(child) = prog.module(inst.module_name()) {
                    for (port_idx, port) in child.ports().iter().enumerate() {
                        let parent_node = inst.ports().get(port_idx)
                            .and_then(|nr| {
                                let name = nr.net();
                                modules[i].symbols.nodes()
                                    .find(|(_, n)| n.name == name)
                                    .map(|(id, _)| id)
                            })
                            .unwrap_or(NodeId::GROUND);
                        instance_ports.insert(format!("{label}.{}", port.name()), parent_node);
                    }
                }
            }
        }

        for behavior in m.behaviors() {
            let is_digital = behavior.is_digital();
            let module_vars: HashSet<String> = m.vars().iter().map(|v| v.name().to_string()).collect();
            let mut ctx = LowerCtx::new(&mut modules[i].symbols, is_digital, module_vars);
            ctx.instance_ports = instance_ports.clone();

            let stmts = lower_stmts(behavior.body(), &mut ctx);

            if is_digital {
                // Populate digital inputs/outputs from the module's ports:
                // a digital input is a Digital-domain In port, a digital
                // output is a Digital-domain Out port. Inout digital ports
                // are both (they appear in inputs so the evaluator can read
                // them and in outputs so the evaluator can drive them).
                let mut inputs = Vec::new();
                let mut outputs = Vec::new();
                for port in &modules[i].ports {
                    let node = modules[i].symbols.node(port.node);
                    if node.domain != Domain::Digital {
                        continue;
                    }
                    match port.direction {
                        IrDirection::In => inputs.push(port.node),
                        IrDirection::Out => outputs.push(port.node),
                        IrDirection::Inout => {
                            inputs.push(port.node);
                            outputs.push(port.node);
                        }
                    }
                }

                // Module-level vars are persistent digital state — they are
                // registers (SPEC §9: a `var` updated in a clocked `@` block
                // is an edge-triggered register). Collect their VarIds and
                // emit `VarDecl` statements with their initializers so the
                // digital compiler can extract `reg_inits`.
                let var_inits: Vec<(String, Option<&crate::elab::const_eval::ConstVal>)> = m
                    .vars()
                    .iter()
                    .map(|v| (v.name().to_string(), v.init()))
                    .collect();
                let mut regs = Vec::new();
                let mut reg_decls = Vec::new();
                for (vname, vinit) in var_inits {
                    let vid = match modules[i].symbols.vars().find(|(_, info)| info.name == vname).map(|(id, _)| id) {
                        Some(id) => id,
                        None => continue,
                    };
                    regs.push(vid);
                    if let Some(init) = vinit {
                        let init_expr = const_val_to_ir(init, &mut modules[i].symbols);
                        reg_decls.push(IrStmt::VarDecl { var: vid, init: Some(init_expr) });
                    }
                }

                let mut all_stmts = reg_decls;
                all_stmts.extend(stmts);

                modules[i].digital = Some(IrDigitalBody {
                    inputs,
                    outputs,
                    regs,
                    stmts: all_stmts,
                });
            } else {
                modules[i].analog = Some(IrAnalogBody {
                    states: ctx.states,
                    noise: ctx.noise_sources,
                    stmts,
                });
            }
        }
    }


    IrProgram {
        source: Source::Ppr,
        modules,
    }
}
