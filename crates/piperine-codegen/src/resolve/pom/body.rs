//! [`LoweredBody`] and [`lower_bodies`]: the POM → resolved-form pass. Each
//! body covers one module's own behavior (symbol table, resolved ports,
//! analog/digital bodies); instance wiring (connections, param overrides) is
//! left to `device::circuit`, which reads the POM directly.
//!
//! Also home to the pass's failure surface (`LowerError`/`LowerErrors`) and
//! the `LowerCtx` scope the per-construct modules resolve against.

use std::collections::HashMap;

use piperine_lang::pom::Design;

use crate::resolve::*;

use piperine_lang::ast::{Expr as PomExpr, Stmt as PomStmt};
use super::structure::{build_symbols_and_ports, value_to_pom_expr};
use super::stmt::resolve_stmts;


/// A module's resolved lowering: its symbol table, resolved ports, and
/// analog/digital bodies. This is what `device::CompiledModule::compile`
/// consumes; the POM `Module`/`Instance` themselves are read directly by
/// `device::circuit` for structure (connections, param overrides).
///
/// `constraints`/`cover` are permanently `None` post-MD-45: PHDL's dv-core
/// `require`/`var`/`target`/`cover` constraint blocks (`pom::ConstraintBlock`)
/// were deleted at T21 along with the rest of the PHDL-only value layer —
/// Verilog-AMS's Annex A grammar has no constraint-block production, and a
/// future phase re-anchors this on VAMS `(* attribute *)` syntax rather
/// than this retarget inventing one. `kernel`'s constraint/coverage
/// capabilities stay in place (already `Option`-gated) and simply never
/// activate until that phase lands.
#[derive(Debug, Clone, Default)]
pub struct LoweredBody {
    /// The owning module's name — diagnostics only (`lower_bodies`'s
    /// returned map is already keyed by this same name).
    pub name: String,
    pub symbols: SymbolTable,
    pub ports: Vec<Port>,
    pub analog: Option<AnalogBody>,
    pub digital: Option<DigitalBody>,
    /// See the type doc above (MD-45) — always `None` until a future phase
    /// rebuilds the producer.
    pub constraints: Option<Vec<crate::resolve::pom::LoweredMargin>>,
    /// See the type doc above (MD-45) — always `None` until a future phase
    /// rebuilds the producer.
    pub cover: Option<(Vec<crate::resolve::pom::LoweredCoverPoint>, Vec<crate::resolve::pom::LoweredCoverCross>)>,
}

impl LoweredBody {
    /// An empty resolved body — used by hand-built test fixtures.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..Default::default() }
    }
}

// ─── Errors ───────────────────────────────────────────────────────────────────

/// One unresolved name found while lowering `Design` → IR (SIMPLIFICATION.md
/// P5). This phase used to paper over these with `ParamId(0)`/`GROUND`
/// placeholders — silently mis-wiring the circuit; now every failed
/// resolution is recorded and [`lower_bodies`] refuses to hand out the program.
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
    /// The module's symbol table, populated during `convert_mod`.
    pub symbols: &'a mut SymbolTable,
    /// The module being lowered — error context only.
    pub module_name: String,
    /// Unresolved names found so far; `lower_bodies` fails if any module's
    /// lowering left this non-empty. The `require_*` lookups push here and
    /// return a placeholder id that never escapes — the whole program is
    /// discarded on error.
    pub errors: Vec<LowerError>,
    /// Name → id maps built once from the symbol table (SIMPLIFICATION.md
    /// P12: resolve names once, ids afterwards). `nodes`/`params` are fixed
    /// before behavior lowering starts; `vars` gains entries only through
    /// [`LowerCtx::shadow_var_for`], which keeps its map in sync.
    nodes: HashMap<String, NodeId>,
    vars: HashMap<String, VarId>,
    /// State variables (ddt, idt, etc.) allocated during this behavior lowering.
    pub states: Vec<StateId>,
    /// Noise sources discovered from contribution right-hand sides.
    pub noise_sources: Vec<NoiseSource>,
    /// Whether this context is lowering a `digital` (`initial`/`always`)
    /// body, as opposed to an `analog`/`analog initial` one.
    pub is_digital: bool,
    /// Map from `"instance_name.port_name"` → NodeId for named instance
    /// port access (SPEC §7.3: `I(load.p, gnd) <+ …`). The parent
    /// contributes to the child's port node, which is the parent-scope
    /// node the port is connected to.
    pub instance_ports: HashMap<String, NodeId>,
    /// Global `const` values as POM expressions (`NG_K`, `M_PI`, …).
    pub consts: HashMap<String, PomExpr>,
    /// Digital-domain nodes read from *this* analog body (a port or wire
    /// whose value comes from the digital side, referenced by bare name —
    /// not through `V`/`I`), bridged through a synthetic module-level
    /// shadow `var`: the same D2A path (`AnalogInstance::sync_vars`) a
    /// real `var` read already uses, so this never falls back to a silent
    /// `Real(0.0)`. The caller (`lower_bodies`) merges these into the
    /// module's digital body (creating one if the module has none) after
    /// every behavior is lowered.
    pub digital_shadows: Vec<(NodeId, VarId)>,
    /// The in-force `` `default_transition ``'s `(rise, fall)` value
    /// (LRM §10.3, LRMC-C10-02, T56), if any — `analog_ops.rs::Transition`
    /// falls back to this instead of `0.0` for an omitted `rise`/`fall`
    /// argument. `None` when no such directive was seen.
    pub default_transition: Option<(f64, f64)>,
}

impl<'a> LowerCtx<'a> {
    /// Create a fresh lowering context. Snapshots the symbol table's
    /// name → id maps once — every later lookup is a hash probe, not a
    /// table scan.
    pub fn new(symbols: &'a mut SymbolTable, module_name: String, is_digital: bool) -> Self {
        let nodes = symbols.nodes().map(|(id, n)| (n.name.clone(), id)).collect();
        let vars = symbols.vars().map(|(id, v)| (v.name.clone(), id)).collect();
        Self {
            symbols,
            module_name,
            errors: Vec::new(),
            nodes,
            vars,
            states: vec![],
            noise_sources: vec![],
            is_digital,
            instance_ports: HashMap::new(),
            consts: HashMap::new(),
            digital_shadows: Vec::new(),
            default_transition: None,
        }
    }

    /// Global-const POM expressions for `prog`, shared by every context.
    pub fn const_irs(prog: &Design) -> HashMap<String, PomExpr> {
        prog.consts().map(|(name, v)| (name.clone(), value_to_pom_expr(v))).collect()
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

    /// Allocate a new state variable of `kind`, returning its `StateId`.
    pub fn alloc_state(&mut self, kind: StateKind, arg: PomExpr) -> StateId {
        let id = self.symbols.add_state(StateVar { kind, arg });
        self.states.push(id);
        id
    }

    /// Lookup a node (net/port) by name. Also resolves named-instance
    /// port accesses (`load.p` → the parent NodeId the port connects to,
    /// SPEC §7.3).
    pub fn lookup_node(&self, name: &str) -> Option<NodeId> {
        if piperine_lang::pom::is_ground(name) {
            return Some(NodeId::GROUND);
        }
        // Check instance port map first (e.g. "load.p" or "rseg[0].n").
        if let Some(id) = self.instance_ports.get(name) {
            return Some(*id);
        }
        self.nodes.get(name).copied()
    }

    // ── Fail-loud lookups (SIMPLIFICATION.md P5) ──────────────────────────
    //
    // Each returns a placeholder id on failure *after* recording the error;
    // `lower_bodies` discards the whole program when any error was
    // recorded, so a placeholder can never mis-wire a circuit that gets
    // simulated.

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

    // Snapshot module names once — both passes and the final zip use the
    // same iteration order. Each name resolves to the module's FLAT form
    // (codegen-facing): `Design::flat_module` falls back to the authored
    // module for leaves, so leaf bodies are unchanged. A flattened root's
    // body carries nodes for the lifted wires and spliced instance ports
    // that codegen reads via `CircuitCompiler::flat_module` (FLAT-01).
    let names: Vec<String> = prog.modules().map(|m| m.name().to_string()).collect();

    // Pass 1: build the symbol table + resolved ports for every module.
    for name in &names {
        let m = prog.flat_module(name).expect(
            "flat_module falls back to modules; name came from modules() so this is an internal bug",
        );
        let (symbols, ports) = build_symbols_and_ports(m, prog);
        bodies.push(LoweredBody {
            name: name.clone(),
            symbols,
            ports,
            analog: None,
            digital: None,
            constraints: None,
            cover: None,
        });
    }

    // Pass 1.5 (functions): PHDL used to register every non-generic
    // `Design::functions()`/`impls()` entry into each module's symbol
    // table here. Both collections were deleted at T21 (MD-45) along with
    // the rest of the value layer, and VAMS's own `analog function`/
    // `function`/`task` module items aren't lowered into the POM at all
    // yet (a documented pre-existing gap from T22, not this retarget's
    // scope — see `elab/lower/vams.rs`'s no-op match arms). Nothing to do
    // here until that lands; `resolve/pom/expr.rs`'s `fn_by_name` lookup
    // stays in place as the (currently always-empty) landing spot.

    // Pass 2: Lower behaviors using the built SymbolTables — flat module
    // form (lifted wires + spliced instance ports are already in the symbol
    // table from Pass 1; the authored module's behaviors are unchanged).
    for (i, name) in names.iter().enumerate() {
        let m = prog.flat_module(name).expect(
            "flat_module falls back to modules; name came from modules() so this is an internal bug",
        );
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
                        // An indexed net ref (`tap[i]`, lifted from an
                        // array wire by the flatten pass or authored as
                        // `tap[i]` in a structural `for`) resolves through
                        // its full Display form — Fix A registers one node
                        // per element (`tap[0]`..`tap[N-1]`), so the bare
                        // parent name no longer identifies a node.
                        let bare = net_ref.net();
                        let full = net_ref.to_string();
                        let lookup = if net_ref.index().is_some() { &full } else { bare };
                        let parent_node = if piperine_lang::pom::is_ground(bare) {
                            NodeId::GROUND
                        } else if let Some(&id) = node_by_name.get(lookup) {
                            id
                        } else {
                            errors.push(LowerError {
                                module: m.name().to_string(),
                                what: "net (instance connection)",
                                name: full.clone(),
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
            let mut ctx = LowerCtx::new(&mut bodies[i].symbols, m.name().to_string(), is_digital);
            ctx.instance_ports = instance_ports.clone();
            ctx.consts = LowerCtx::const_irs(prog);
            ctx.default_transition = prog.directives().default_transition;

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
                // initial values (see `DigitalBody::reg_inits`'s doc — VAMS
                // has no `VarDecl`-with-default statement to smuggle these
                // through, so they're a first-class parallel list here).
                let var_inits: Vec<(String, Option<&piperine_lang::value::Value>)> = m
                    .vars()
                    .iter()
                    .map(|v| (v.name().to_string(), v.init()))
                    .collect();
                let mut regs = Vec::new();
                let mut reg_inits: Vec<(VarId, PomExpr)> = Vec::new();
                for (vname, vinit) in var_inits {
                    let vid = match bodies[i].symbols.vars().find(|(_, info)| info.name == vname).map(|(id, _)| id) {
                        Some(id) => id,
                        None => continue,
                    };
                    regs.push(vid);
                    if let Some(init) = vinit {
                        reg_inits.push((vid, value_to_pom_expr(init)));
                    }
                }

                let stmts: Vec<PomStmt> = behavior.body().to_vec();

                // A module may declare more than one digital behavior —
                // two `always` blocks are independent concurrent processes
                // in Verilog, and BOTH run. Their statements therefore
                // *append* to one `DigitalBody` rather than replacing it:
                // assigning unconditionally here kept only the last
                // behavior and dropped every earlier one silently, with no
                // diagnostic — the exact "silently emit a no-op" failure
                // the crate's fail-loud contract forbids.
                //
                // Appending is the faithful lowering, not a workaround:
                // `inputs`/`outputs` derive from the module's ports and
                // `regs`/`reg_inits` from its vars, so every behavior of a
                // given module computes the identical values for them —
                // only `stmts` differ. Downstream,
                // `kernel::digital::compile` splits `stmts` per statement
                // (each `EventControl` routed to its clocked or
                // combinational list), so concatenated behaviors split
                // exactly as separately-compiled ones would.
                match &mut bodies[i].digital {
                    Some(body) => body.stmts.extend(stmts),
                    None => {
                        bodies[i].digital = Some(DigitalBody {
                            inputs,
                            outputs,
                            regs,
                            reg_inits,
                            stmts,
                        });
                    }
                }
            } else {
                // Analog path: resolve POM `Stmt` (keep POM structure, mark
                // analog operators with state ids). The flattener and Builder
                // dispatch on POM `Expr`/`Stmt` directly.
                let stmts = resolve_stmts(behavior.body(), &mut ctx);
                digital_shadows.append(&mut ctx.digital_shadows);
                errors.append(&mut ctx.errors);
                bodies[i].analog = Some(AnalogBody {
                    states: ctx.states,
                    noise: ctx.noise_sources,
                    stmts,
                });
            }
        }

        // Constraint margins (constraints-on-vams): a module's authored
        // `require`s lower to signed margins through the same
        // name-resolution path its analog body takes. `Some` exactly when
        // at least one require lowered; a module without the block pays
        // exactly what it always paid (`None`, nothing allocated). A
        // predicate form with no margin lands in `errors` — the run fails
        // loud, never a silent `0.0`.
        if !m.constraints().is_empty() {
            let mut ctx = LowerCtx::new(&mut bodies[i].symbols, m.name().to_string(), false);
            ctx.instance_ports = instance_ports.clone();
            ctx.consts = LowerCtx::const_irs(prog);
            ctx.default_transition = prog.directives().default_transition;
            let margins = crate::resolve::pom::ConstraintLowering::lower(m.constraints(), &mut ctx);
            errors.append(&mut ctx.errors);
            bodies[i].constraints = Some(margins);
        }

        if !digital_shadows.is_empty() {
            let assigns: Vec<PomStmt> = digital_shadows
                .iter()
                .map(|(node, var)| {
                    let var_name = bodies[i].symbols.var(*var).name.clone();
                    let node_name = bodies[i].symbols.node(*node).name.clone();
                    let nospan = piperine_lang::error::PreprocessedSpan::new(0, 0);
                    PomStmt::Assign {
                        span: nospan,
                        target: PomExpr::Ident { span: nospan, name: var_name },
                        value: PomExpr::Ident { span: nospan, name: node_name },
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
                    bodies[i].digital = Some(DigitalBody {
                        inputs,
                        outputs: Vec::new(),
                        regs: Vec::new(),
                        reg_inits: Vec::new(),
                        stmts: assigns,
                    });
                }
            }
        }
    }

    if !errors.is_empty() {
        return Err(LowerErrors(errors));
    }

    Ok(names.into_iter().zip(bodies).collect())
}
