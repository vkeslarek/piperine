//! Lower a POM `Design` (PPR/PHDL) straight into each module's resolved
//! [`LoweredBody`] — no separate IR crate, no `IrModule`/`IrProgram`
//! structural twin. Instance wiring (connections, param overrides) is left
//! to `device::circuit`, which reads the POM directly.

use std::collections::{HashMap, HashSet};

use piperine_lang::pom::Design;

use crate::lower::*;

pub mod analog_ops;
pub mod event;
pub mod expr;
pub mod stmt;
pub mod structure;
pub mod syscalls;

use piperine_lang::parse::ast::{BindOp, Expr as PomExpr, Stmt as PomStmt};
use structure::{build_symbols_and_ports, convert_fn, value_to_pom_expr};
use stmt::lower_stmts;

/// A module's resolved lowering: its symbol table, resolved ports, and
/// analog/digital bodies. This is what `device::CompiledModule::compile`
/// consumes; the POM `Module`/`Instance` themselves are read directly by
/// `device::circuit` for structure (connections, param overrides).
#[derive(Debug, Clone, Default)]
pub struct LoweredBody {
    /// The owning module's name — diagnostics only (`lower_bodies`'s
    /// returned map is already keyed by this same name).
    pub name: String,
    pub symbols: SymbolTable,
    pub ports: Vec<Port>,
    pub analog: Option<AnalogBody>,
    pub digital: Option<DigitalBody>,
}

impl LoweredBody {
    /// An empty resolved body — used by hand-built test fixtures (the old
    /// `IrModule::new`).
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..Default::default() }
    }
}

// ─── Errors ───────────────────────────────────────────────────────────────────

/// One unresolved name found while lowering `Design` → IR (SIMPLIFICATION.md
/// P5). This phase used to paper over these with `ParamId(0)`/`GROUND`
/// placeholders — silently mis-wiring the circuit; now every failed
/// resolution is recorded and [`ppr_to_ir`] refuses to hand out the program.
#[derive(Debug, Clone, thiserror::Error)]
#[error("in module `{module}`: unresolved {what} `{name}`")]
pub struct LowerError {
    pub module: String,
    pub what: &'static str,
    pub name: String,
}

/// Every unresolved name of one lowering run, reported together (fixing a
/// batch beats fixing one per compile).
#[derive(Debug, thiserror::Error)]
pub struct LowerErrors(pub Vec<LowerError>);

impl std::fmt::Display for LowerErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{} unresolved name(s) while lowering to IR:", self.0.len())?;
        for e in &self.0 {
            writeln!(f, "  - {e}")?;
        }
        Ok(())
    }
}

// ─── Context ──────────────────────────────────────────────────────────────────

/// Lowering context carrying the current scope, state-variable counter, and
/// discovered noise-source list.
pub(crate) struct LowerCtx<'a> {
    /// Name → IR expression bindings for the current scope.
    pub env: HashMap<String, IrExpr>,
    /// The module's symbol table, populated during `convert_mod`.
    pub symbols: &'a mut SymbolTable,
    /// The module being lowered — error context only.
    pub module_name: String,
    /// Unresolved names found so far; `ppr_to_ir` fails if any module's
    /// lowering left this non-empty. The `require_*` lookups push here and
    /// return a placeholder id that never escapes — the whole program is
    /// discarded on error.
    pub errors: Vec<LowerError>,
    /// Name → id maps built once from the symbol table (SIMPLIFICATION.md
    /// P12: resolve names once, ids afterwards). `nodes`/`params` are fixed
    /// before behavior lowering starts; `vars` gains entries only through
    /// [`LowerCtx::shadow_var_for`], which keeps its map in sync.
    nodes: HashMap<String, NodeId>,
    params: HashMap<String, ParamId>,
    vars: HashMap<String, VarId>,
    /// State variables (ddt, idt, etc.) allocated during this behavior lowering.
    pub states: Vec<StateId>,
    /// Noise sources discovered from contribution right-hand sides.
    pub noise_sources: Vec<NoiseSource>,
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
    /// Enum variant discriminants, keyed bare (`Idle`) and qualified
    /// (`SarState::Idle`). SPEC §6.4: a variant is an integer constant.
    pub enum_values: HashMap<String, i64>,
    /// Global `const` values as IR literals (`NG_K`, `M_PI`, …). Before
    /// SIMPLIFICATION.md P5 these silently fell through to `ParamId(0)` —
    /// a physics constant read as "whatever param 0 is".
    pub consts: HashMap<String, IrExpr>,
    /// Bundle-typed value bindings in scope (module bundle params and
    /// flattened fn params): logical name → (bundle type, field names).
    /// What `model.method(...)` and bundle-valued call arguments resolve
    /// against (GAPS §I.14 extended to fns/methods).
    pub bundle_bindings: HashMap<String, (String, Vec<String>)>,
    /// Per-fn bundle-typed parameter positions (fn name → one entry per
    /// declared param, `Some((bundle, fields))` for bundle-typed ones) —
    /// call sites expand a bundle argument into its per-field scalars.
    pub fn_bundle_sigs: HashMap<String, Vec<Option<(String, Vec<String>)>>>,
    /// Digital-domain nodes read from *this* analog body (a port or wire
    /// whose value comes from the digital side, referenced by bare name —
    /// not through `V`/`I`), bridged through a synthetic module-level
    /// shadow `var`: the same D2A path (`AnalogInstance::sync_vars`) a
    /// real `var` read already uses, so this never falls back to a silent
    /// `Real(0.0)`. The caller (`ppr_to_ir`) merges these into the
    /// module's digital body (creating one if the module has none) after
    /// every behavior is lowered.
    pub digital_shadows: Vec<(NodeId, VarId)>,
}

/// The ground-node aliases every net namespace accepts (SPEC: gnd-family).
pub(crate) const GROUND_NAMES: &[&str] = &["gnd", "GND", "vss", "VSS", "0"];

impl<'a> LowerCtx<'a> {
    /// Create a fresh lowering context. Snapshots the symbol table's
    /// name → id maps once — every later lookup is a hash probe, not a
    /// table scan.
    pub fn new(
        symbols: &'a mut SymbolTable,
        module_name: String,
        is_digital: bool,
        module_vars: HashSet<String>,
    ) -> Self {
        let nodes = symbols.nodes().map(|(id, n)| (n.name.clone(), id)).collect();
        let params = symbols.params().map(|(id, p)| (p.name.clone(), id)).collect();
        let vars = symbols.vars().map(|(id, v)| (v.name.clone(), id)).collect();
        Self {
            env: HashMap::new(),
            symbols,
            module_name,
            errors: Vec::new(),
            nodes,
            params,
            vars,
            states: vec![],
            noise_sources: vec![],
            is_digital,
            module_vars,
            instance_ports: HashMap::new(),
            enum_values: HashMap::new(),
            consts: HashMap::new(),
            bundle_bindings: HashMap::new(),
            fn_bundle_sigs: HashMap::new(),
            digital_shadows: Vec::new(),
        }
    }

    /// Global-const IR literals for `prog`, shared by every context.
    pub fn const_irs(prog: &Design) -> HashMap<String, IrExpr> {
        prog.consts().map(|(name, v)| (name.clone(), structure::value_to_ir(v))).collect()
    }

    /// `$param_given("name")` resolution: exact param name first, then a
    /// unique flattened bundle field (`narrow` → `model_narrow`) — the
    /// syscall's argument predates bundle flattening (GAPS §I.14).
    pub fn require_param_given(&mut self, name: &str) -> ParamId {
        if let Some(id) = self.lookup_param(name) {
            return id;
        }
        let suffix = format!("_{name}");
        let mut matches = self.params.iter().filter(|(n, _)| n.ends_with(&suffix));
        if let (Some((_, &id)), None) = (matches.next(), matches.next()) {
            return id;
        }
        self.errors.push(LowerError {
            module: self.module_name.clone(),
            what: "parameter ($param_given)",
            name: name.to_string(),
        });
        ParamId(0)
    }

    /// The shadow `var` bridging digital-domain node `id` (named `name`,
    /// for a readable IR symbol) into this analog body. Reuses the same
    /// shadow if `id` was already read earlier in this behavior.
    pub fn shadow_var_for(&mut self, id: NodeId, name: &str) -> VarId {
        if let Some((_, var)) = self.digital_shadows.iter().find(|(node, _)| *node == id) {
            return *var;
        }
        let shadow_name = format!("__shadow_{name}");
        let var = self.symbols.add_var(shadow_name.clone(), Type::Bool);
        self.vars.insert(shadow_name, var);
        self.digital_shadows.push((id, var));
        var
    }

    /// Lookup an enum variant's discriminant by bare or qualified name.
    pub fn lookup_enum_value(&self, name: &str) -> Option<i64> {
        self.enum_values.get(name).copied()
    }


    /// Allocate a new state variable of `kind`, returning its `StateId`.
    pub fn alloc_state(&mut self, kind: StateKind, arg: IrExpr) -> StateId {
        let id = self.symbols.add_state(StateVar { kind, arg });
        self.states.push(id);
        id
    }

    /// Lookup a parameter by name.
    pub fn lookup_param(&self, name: &str) -> Option<ParamId> {
        self.params.get(name).copied()
    }

    /// Lookup a variable by name.
    pub fn lookup_var(&self, name: &str) -> Option<VarId> {
        self.vars.get(name).copied()
    }

    /// Lookup a node (net/port) by name. Also resolves named-instance
    /// port accesses (`load.p` → the parent NodeId the port connects to,
    /// SPEC §7.3).
    pub fn lookup_node(&self, name: &str) -> Option<NodeId> {
        if GROUND_NAMES.contains(&name) {
            return Some(NodeId::GROUND);
        }
        // Check instance port map first (e.g. "load.p" or "rseg_0.n").
        if let Some(id) = self.instance_ports.get(name) {
            return Some(*id);
        }
        if let Some(id) = self.nodes.get(name) {
            return Some(*id);
        }
        // A net-capable bundle port (`out : Differential`) was expanded to
        // flat scalar ports `out_p`/`out_n` at elaboration (SPEC §7 bundle
        // expansion) — `out.p` in the behavior body names the flat form.
        if name.contains('.') {
            return self.nodes.get(&name.replace('.', "_")).copied();
        }
        None
    }

    // ── Fail-loud lookups (SIMPLIFICATION.md P5) ──────────────────────────
    //
    // Each returns a placeholder id on failure *after* recording the error;
    // `ppr_to_ir` discards the whole program when any error was recorded,
    // so a placeholder can never mis-wire a circuit that gets simulated.

    /// Resolve a node name or record an "unresolved net" error.
    pub fn require_node(&mut self, name: &str) -> NodeId {
        self.lookup_node(name).unwrap_or_else(|| {
            self.errors.push(LowerError {
                module: self.module_name.clone(),
                what: "net",
                name: name.to_string(),
            });
            NodeId::GROUND
        })
    }


    /// Resolve a bare identifier that fell through every other namespace
    /// (env binding, module var, node, enum variant): it must be a
    /// parameter, or it is an unresolved name.
    pub fn require_ident_as_param(&mut self, name: &str) -> ParamId {
        self.lookup_param(name).unwrap_or_else(|| {
            self.errors.push(LowerError {
                module: self.module_name.clone(),
                what: "name",
                name: name.to_string(),
            });
            ParamId(0)
        })
    }

    /// Resolve a var name or record an "unresolved variable" error.
    pub fn require_var(&mut self, name: &str) -> VarId {
        self.lookup_var(name).unwrap_or_else(|| {
            self.errors.push(LowerError {
                module: self.module_name.clone(),
                what: "variable",
                name: name.to_string(),
            });
            VarId(0)
        })
    }
}

// ─── Entry point ──────────────────────────────────────────────────────────────

/// Lower every module of a POM `Design` into its [`LoweredBody`] (symbol
/// table + resolved analog/digital bodies), keyed by module name.
///
/// Fallible (SIMPLIFICATION.md P5): any name that fails to resolve — a
/// typo'd net in an instance connection, an unknown parameter in a
/// contribution — is an error naming the module and symbol, never a
/// silently grounded node or `ParamId(0)`.
pub fn lower_bodies(prog: &Design) -> Result<HashMap<String, LoweredBody>, LowerErrors> {
    let mut bodies: Vec<LoweredBody> = Vec::new();
    let mut errors: Vec<LowerError> = Vec::new();

    // Pass 1: build the symbol table + resolved ports for every module.
    for m in prog.modules() {
        let (symbols, ports) = build_symbols_and_ports(m, prog);
        bodies.push(LoweredBody { name: m.name().to_string(), symbols, ports, analog: None, digital: None });
    }

    // Pass 1.5: Add non-generic functions to each module's symbol table.
    // Generic functions (map, reduce, …) are elaboration-time generators:
    // they are monomorphized at call sites and never lowered as-is, so
    // adding their unresolved bodies would produce dangling references.
    for body in &mut bodies {
        for f in prog.functions() {
            if f.is_generic() {
                continue;
            }
            let ir_f = convert_fn(f, prog, &mut body.symbols, &mut errors);
            body.symbols.add_fn(ir_f);
        }
        // Impl methods register as `Type::method` with `self` prepended as
        // a bundle-typed param (flattened per-field like any other) —
        // `model.conductance()` lowers to a plain fn call (SPEC §6.5/§6.6).
        for ib in prog.impls() {
            for method in &ib.methods {
                let mut synth = method.clone();
                synth.params.insert(
                    0,
                    (
                        "self".to_string(),
                        piperine_lang::pom::TypeRef::Value(piperine_lang::pom::ValueType::Bundle(ib.ty.clone())),
                    ),
                );
                synth.defaults.insert(0, None);
                let mangled = format!("{}::{}", ib.ty, method.name);
                let ir_f =
                    structure::convert_fn_named(&mangled, &synth, prog, &mut body.symbols, &mut errors);
                body.symbols.add_fn(ir_f);
            }
        }
    }

    // Pass 2: Lower behaviors using the built SymbolTables.
    for (i, m) in prog.modules().enumerate() {
        // Build the instance-port map (SPEC §7.3): for each named instance,
        // map `"label.port_name"` → parent NodeId. This lets the parent's
        // analog body reference child ports (`I(load.p, gnd) <+ …`).
        let node_by_name: HashMap<String, NodeId> =
            bodies[i].symbols.nodes().map(|(id, n)| (n.name.clone(), id)).collect();
        let mut instance_ports: HashMap<String, NodeId> = HashMap::new();
        for inst in m.instances() {
            if let Some(label) = inst.label() {
                // Look up the child module to get port names.
                if let Some(child) = prog.module(inst.module_name()) {
                    for (port_idx, port) in child.ports().iter().enumerate() {
                        let Some(net_ref) = inst.ports().get(port_idx) else {
                            // Unconnected trailing port: legal, the device
                            // gets a fresh internal node at codegen.
                            continue;
                        };
                        let name = net_ref.net();
                        let parent_node = if GROUND_NAMES.contains(&name) {
                            NodeId::GROUND
                        } else if let Some(&id) = node_by_name.get(name) {
                            id
                        } else {
                            errors.push(LowerError {
                                module: m.name().to_string(),
                                what: "net (instance connection)",
                                name: name.to_string(),
                            });
                            NodeId::GROUND
                        };
                        instance_ports.insert(format!("{label}.{}", port.name()), parent_node);
                    }
                }
            }
        }

        // Digital-domain nodes read from an analog body (§ shadow-var
        // bridge, `LowerCtx::shadow_var_for`), collected across every
        // analog behavior and merged into the module's digital body below
        // — creating one if the module declares no `digital` block at all
        // (a module can be purely analog but still read a digital port).
        let mut digital_shadows: Vec<(NodeId, VarId)> = Vec::new();

        for behavior in m.behaviors() {
            let is_digital = behavior.is_digital();
            let module_vars: HashSet<String> = m.vars().iter().map(|v| v.name().to_string()).collect();
            let mut ctx =
                LowerCtx::new(&mut bodies[i].symbols, m.name().to_string(), is_digital, module_vars);
            ctx.instance_ports = instance_ports.clone();
            ctx.enum_values = prog.enum_value_map();
            ctx.consts = LowerCtx::const_irs(prog);
            ctx.fn_bundle_sigs = structure::fn_bundle_signatures(prog);
            // Module bundle params (flattened at elaboration) — resolvable
            // as method receivers and bundle-valued call arguments.
            for p in m.params() {
                if let Some((logical, bundle)) = &p.bundle_origin {
                    ctx.bundle_bindings
                        .entry(logical.clone())
                        .or_insert_with(|| {
                            (bundle.clone(), structure::bundle_field_names(prog, bundle))
                        });
                }
            }

            if is_digital {
                // Digital path: keep the POM `Stmt` tree directly — the
                // `Codegen` trait + `Builder` emit it to Cranelift without
                // the `IrStmt` intermediate. Name resolution happens at
                // codegen time via the `Resolver`.
                //
                // Populate digital inputs/outputs from the module's ports:
                // a digital input is a Digital-domain In port, a digital
                // output is a Digital-domain Out port. Inout digital ports
                // are both (they appear in inputs so the evaluator can read
                // them and in outputs so the evaluator can drive them).
                let mut inputs = Vec::new();
                let mut outputs = Vec::new();
                for port in &bodies[i].ports {
                    let node = bodies[i].symbols.node(port.node);
                    if node.domain != Domain::Digital {
                        continue;
                    }
                    match port.direction {
                        Direction::In => inputs.push(port.node),
                        Direction::Out => outputs.push(port.node),
                        Direction::Inout => {
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
                let var_inits: Vec<(String, Option<&piperine_lang::value::Value>)> = m
                    .vars()
                    .iter()
                    .map(|v| (v.name().to_string(), v.init()))
                    .collect();
                let mut regs = Vec::new();
                let mut reg_decls: Vec<PomStmt> = Vec::new();
                for (vname, vinit) in var_inits {
                    let vid = match bodies[i].symbols.vars().find(|(_, info)| info.name == vname).map(|(id, _)| id) {
                        Some(id) => id,
                        None => continue,
                    };
                    regs.push(vid);
                    if let Some(init) = vinit {
                        let init_expr = value_to_pom_expr(init);
                        reg_decls.push(PomStmt::VarDecl { name: vname, ty: None, default: Some(init_expr) });
                    }
                }

                let mut all_stmts = reg_decls;
                all_stmts.extend(behavior.body().iter().cloned());

                bodies[i].digital = Some(DigitalBody {
                    inputs,
                    outputs,
                    regs,
                    stmts: all_stmts,
                });
            } else {
                // Analog path: lower POM `Stmt` → `IrStmt` (the analog JIT
                // still dispatches on `IrExpr`/`IrStmt`).
                let stmts = lower_stmts(behavior.body(), &mut ctx);
                digital_shadows.append(&mut ctx.digital_shadows);
                errors.append(&mut ctx.errors);
                bodies[i].analog = Some(AnalogBody {
                    states: ctx.states,
                    noise: ctx.noise_sources,
                    stmts,
                });
            }
        }

        if !digital_shadows.is_empty() {
            let assigns: Vec<PomStmt> = digital_shadows
                .iter()
                .map(|(node, var)| {
                    let var_name = bodies[i].symbols.var(*var).name.clone();
                    let node_name = bodies[i].symbols.node(*node).name.clone();
                    PomStmt::Bind {
                        dest: PomExpr::Ident(var_name),
                        op: BindOp::Assign,
                        src: PomExpr::Ident(node_name),
                    }
                })
                .collect();
            match &mut bodies[i].digital {
                Some(body) => body.stmts.extend(assigns),
                None => {
                    // No `digital` block at all: synthesize a body whose
                    // only job is exporting these nodes' values into the
                    // shadow vars every step (SPEC §9 combinational
                    // semantics — a bare top-level `Assign`, re-evaluated
                    // each digital eval, never a one-shot).
                    let inputs = digital_shadows.iter().map(|(node, _)| *node).collect();
                    bodies[i].digital =
                        Some(DigitalBody { inputs, outputs: Vec::new(), regs: Vec::new(), stmts: assigns });
                }
            }
        }
    }

    if !errors.is_empty() {
        return Err(LowerErrors(errors));
    }

    Ok(prog.modules().map(|m| m.name().to_string()).zip(bodies).collect())
}
