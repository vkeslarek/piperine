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
/// `constraints`/`cover` carry the module's lowered constraint statements
/// (`constraint begin … end`, the sanctioned MD-49 extension): margins
/// (`require`/`target`/reduced vars) in `constraints`, the coverage
/// catalog in `cover`, each `Some` iff the module declares at least one.
#[derive(Debug, Clone, Default)]
pub struct LoweredBody {
    /// The owning module's name — diagnostics only (`lower_bodies`'s
    /// returned map is already keyed by this same name).
    pub name: String,
    pub symbols: SymbolTable,
    pub ports: Vec<Port>,
    pub analog: Option<AnalogBody>,
    pub digital: Option<DigitalBody>,
    /// The lowered constraint margins (`require`/`target`/reduced vars),
    /// `Some` iff the module declares at least one — filled by
    /// [`ConstraintLowering::lower`](crate::resolve::pom::ConstraintLowering).
    pub constraints: Option<Vec<crate::resolve::pom::LoweredMargin>>,
    /// The lowered coverage catalog (`cover` points + `cover cross`es),
    /// `Some` iff the module declares at least one — filled by
    /// [`ConstraintLowering::lower_cover`](crate::resolve::pom::ConstraintLowering).
    pub cover: Option<(Vec<crate::resolve::pom::LoweredCoverPoint>, Vec<crate::resolve::pom::LoweredCoverCross>)>,
    /// Variable initializers (Annex A.2.2.1): `(VarId, init expr)` for
    /// every module-level `var` declared with `= constant_expression`.
    /// The digital path's [`DigitalBody::reg_inits`] carries the same
    /// entries for its register power-on; the analog kernel keeps the
    /// shared copy here to seed its persistent-var bank (one source, two
    /// consumers — never two lowering rules).
    pub var_inits: Vec<(VarId, PomExpr)>,
    /// Access-function name → nature kind (LRM §4.4/§5.5.1), design-wide
    /// (disciplines are a design-level registry, not per-module) — the
    /// LRM's own always-legal `"V"`/`"potential"`/`"I"`/`"flow"` names are
    /// NOT in this map (handled directly wherever it's consulted,
    /// regardless of what any discipline declares); every OTHER entry is a
    /// custom discipline's own `access` name (§3.6.1.2), resolved once here
    /// from every `discipline { potential …; flow …; }` in the design —
    /// mirrors `piperine-lang`'s own `build_access_kind_map`
    /// (`elab/lower/vams.rs`, LRMC-C1-09), which resolves the identical
    /// rule for POM branch-kind bookkeeping; this is `flatten/analog.rs`'s
    /// copy for contribution-destination classification.
    pub access_kinds: HashMap<String, NatureKind>,
    /// T26/LRMW2-26 (LRM §8.4.3.3): the `` `timescale `` in force for this
    /// module (1364-2005), `None` when the module declares none — codegen's
    /// only consumer today is the digital event scheduler
    /// (`kernel/digital/abi.rs`'s `DigitalKernel::timescale`), which rounds
    /// every event time to the `precision_exponent` grid before scheduling
    /// it. `None` means no quantization (unchanged pre-T26 behavior) —
    /// most fixtures declare no `` `timescale `` at all.
    pub timescale: Option<piperine_lang::directives::Timescale>,
    /// Node pairs of every branch the elaborator marked
    /// [`piperine_lang::pom::BranchKind::Switch`] (§5.6.5, LRMW2-40, T45) —
    /// resolved from `Module::branches()` once here since the flattener
    /// (`flatten/analog.rs::AnalogFlattener`) only ever sees this resolved
    /// form, never the POM `Design`/`Module` directly. A guarded potential
    /// contribution (`if (cond) V(p,n) <+ expr;`) is only legal — §5.6.5's
    /// switch exception — when `(p,n)` (in either order) appears here.
    pub switch_branches: Vec<(NodeId, NodeId)>,
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
    /// Design-wide nature name → declared `abstol` (LRM §4.5.3's
    /// `abstol_expression ::= constant_expression | nature_identifier`):
    /// `ddt`/`idt`/`idtmod`'s optional tolerance argument resolves a bare
    /// nature identifier through this map instead of treating it as a
    /// (nonexistent) parameter reference. Only natures that declared an
    /// `abstol` attribute are present.
    pub nature_abstols: HashMap<String, f64>,
    /// User-function inlining recursion guard (§4.7): names currently
    /// being inlined, pushed on entry to `inline_user_fn` and popped on
    /// exit — a name already on the stack is a (possibly indirect)
    /// recursive call, which §4.7's straight-line-substitution model
    /// cannot terminate.
    pub inlining_stack: Vec<String>,
    /// `output`/`inout` function-argument write-backs (§4.7) discovered
    /// while resolving the current statement's expression(s) — drained by
    /// the enclosing `resolve_stmt` call and turned into extra `Assign`
    /// statements run immediately after it. A call whose write-backs are
    /// never drained (an `output`/`inout`-bearing call outside a
    /// statement's direct value position) is the fail-loud case
    /// `inline_user_fn` itself reports through `errors`.
    pub write_backs: Vec<piperine_lang::ast::Stmt>,
    /// The design's inferred top-level module name (`Design::top`'s own
    /// name), if any — the anchor a `$root`-rooted [`PomExpr::Hier`] path
    /// resolves its first segment against (§6.2.1). Cloned once at
    /// construction rather than borrowing all of `Design`, matching this
    /// context's existing "own copy of what it needs" shape (`nodes`/
    /// `vars` are the same kind of snapshot).
    pub top_module: Option<String>,
}

impl<'a> LowerCtx<'a> {
    /// Create a fresh lowering context. Snapshots the symbol table's
    /// name → id maps once — every later lookup is a hash probe, not a
    /// table scan.
    pub fn new(symbols: &'a mut SymbolTable, module_name: String, is_digital: bool, top_module: Option<String>) -> Self {
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
            nature_abstols: HashMap::new(),
            inlining_stack: Vec::new(),
            write_backs: Vec::new(),
            top_module,
        }
    }

    /// Global-const POM expressions for `prog`, shared by every context.
    pub fn const_irs(prog: &Design) -> HashMap<String, PomExpr> {
        prog.consts().map(|(name, v)| (name.clone(), value_to_pom_expr(v))).collect()
    }

    /// The shadow `var` bridging digital-domain node `id` (named `name`,
    /// for a readable IR symbol) into this analog body. Reuses the same
    /// shadow if `id` was already read earlier in this behavior. `wreal`
    /// (T52, LRMW2-45, §3.7) never reaches this path at all — it classifies
    /// `Domain::Analog` (`resolve/pom/structure.rs::domain_of`'s own doc
    /// comment), so every other digital node this bridges is one of §7.3.1
    /// Table 7-1's "bit" types (`wire`/`reg`/a discrete-discipline port),
    /// which `Type::Bool` already represents correctly (the LRM's own
    /// `onebit_dac` example only ever compares one against `0`).
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
        if self.symbols.is_ground(name) {
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

    /// Resolve a [`PomExpr::Hier`] path (LRM §6.7 `hierarchical_identifier`,
    /// §6.2.1 `$root`) to a plain node-name `Ident`, or record a fail-loud
    /// error and return a harmless `0` placeholder (SIMPLIFICATION.md P5 —
    /// never a silently mis-wired node).
    ///
    /// A local (`root == false`) path resolves against *this* module's own
    /// flattened instance tree: `FlattenHierarchy` already names every
    /// nested net by its dotted instance path (`elab/lower/flatten.rs`'s
    /// `"{inst}.{net}"` rename), and `build_symbols_and_ports` builds this
    /// module's node table from exactly that flattened form — so a local
    /// path is already a registered node name, for *any* module, not only
    /// the design's top (LRM's own words: a local path "starts at the
    /// level where the path is being used").
    ///
    /// A `$root`-rooted path's first segment must name the design's
    /// inferred top-level module (§6.2.1); once that segment is stripped,
    /// resolution is identical to the local form — but only when *this*
    /// module IS that top module. `$root` used from a non-top module would
    /// need to reach into a different module's separately-JIT'd analog
    /// kernel (each module compiles once and is instantiated many times,
    /// CLAUDE.md's "no re-JIT" pipeline — there is no cross-kernel channel
    /// for that today), so it fails loud rather than wiring something
    /// wrong. (T50/LRMW2-42 SPEC_DEVIATION: `$root` reads are scoped to the
    /// design's own top-level module; deeper cross-module probing is a
    /// follow-up.)
    pub fn resolve_hier(&mut self, root: bool, path: &[String]) -> PomExpr {
        let placeholder = || PomExpr::Number { span: super::expr::nospan(), value: piperine_lang::lexer::NumberLit::Integer(0) };
        let joined = path.join(".");
        let local = if root {
            let Some(top) = self.top_module.clone() else {
                self.errors.push(LowerError {
                    module: self.module_name.clone(),
                    what: "hierarchical reference (§6.2.1): design has no inferred top-level module",
                    name: joined,
                });
                return placeholder();
            };
            // Grammar guarantees at least one segment after `$root.`.
            let first = &path[0];
            if *first != top {
                self.errors.push(LowerError {
                    module: self.module_name.clone(),
                    what: "hierarchical reference (§6.2.1): `$root`'s first segment must name the design's top-level module",
                    name: format!("{joined} (failing segment: `{first}`, top module is `{top}`)"),
                });
                return placeholder();
            }
            if self.module_name != top {
                self.errors.push(LowerError {
                    module: self.module_name.clone(),
                    what: "hierarchical reference (§6.2.1): `$root` read from a non-top-level module is not yet supported",
                    name: joined,
                });
                return placeholder();
            }
            path[1..].join(".")
        } else {
            joined.clone()
        };
        if self.lookup_node(&local).is_some() {
            PomExpr::Ident { span: super::expr::nospan(), name: local }
        } else {
            let failing = local.rsplit('.').next().unwrap_or(&local).to_string();
            self.errors.push(LowerError {
                module: self.module_name.clone(),
                what: "hierarchical net (§6.7)",
                name: format!("{local} (failing segment: `{failing}`)"),
            });
            placeholder()
        }
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
/// Design-wide access-function name → nature kind (LRM §4.4/§5.5.1),
/// resolved from every `discipline { potential …; flow …; }` in the
/// design: for each discipline's `potential`/`flow` nature, the nature's
/// declared `access` name maps to [`NatureKind::Potential`]/`Flow`
/// respectively. The LRM's own always-legal `"V"`/`"potential"`/`"I"`/
/// `"flow"` names are NOT entered here — callers (`flatten/analog.rs`'s
/// `parse_dest`) check those literals first, regardless of what any
/// discipline declares, exactly as `piperine-lang`'s own
/// `build_access_kind_map` (`elab/lower/vams.rs`) does for POM branch-kind
/// bookkeeping — this is the identical rule, resolved again here because
/// `LoweredBody` carries no reference back to the source `Design`.
/// Design-wide nature name → declared `abstol`, for `ddt`/`idt`/`idtmod`'s
/// optional tolerance argument (§4.5.3's `abstol_expression`) resolving a
/// bare nature identifier. Only natures with an `abstol` attribute set are
/// entered — a nature with none is not a valid `abstol_expression` value
/// (its identifier is left to the ordinary parameter/const resolution path,
/// which fails loud with "unresolved name" the same way a typo would).
fn build_nature_abstols(design: &Design) -> HashMap<String, f64> {
    design
        .natures()
        .filter_map(|(name, nat)| nat.abstol().map(|tol| (name.clone(), tol)))
        .collect()
}

fn build_access_kinds(design: &Design) -> HashMap<String, NatureKind> {
    let mut map = HashMap::new();
    for (_, disc) in design.disciplines() {
        if let Some(nat_name) = &disc.potential
            && let Some(access) = design.nature(nat_name).and_then(piperine_lang::pom::Nature::access) {
                map.entry(access.to_string()).or_insert(NatureKind::Potential);
            }
        if let Some(nat_name) = &disc.flow
            && let Some(access) = design.nature(nat_name).and_then(piperine_lang::pom::Nature::access) {
                map.entry(access.to_string()).or_insert(NatureKind::Flow);
            }
    }
    map
}

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
    // Disciplines are a design-wide registry, not per-module — resolved once.
    let access_kinds = build_access_kinds(prog);
    let nature_abstols = build_nature_abstols(prog);
    // The design's inferred top-level module name (LRM §6.2.1's `$root`
    // anchor, T50) — `None` when the host never called `Design::set_top`
    // (e.g. a bare `parse_and_elaborate` with no host-level top inference),
    // in which case a `$root`-rooted hierarchical reference fails loud
    // rather than guessing a module.
    let top_module_name: Option<String> = prog.top().map(|m| m.name().to_string());

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
            var_inits: Vec::new(),
            access_kinds: access_kinds.clone(),
            timescale: m.timescale,
            switch_branches: Vec::new(),
        });
    }

    // Pass 1.5 (functions): `analog function`/`function` declarations
    // (§4.7, LRMW2-44, T44) register into each module's `SymbolTable`
    // inside `build_symbols_and_ports` itself (Pass 1, above) — a formal
    // becomes a `VarId` in the module's own scope, and the function
    // registers by name so `resolve/pom/expr.rs`'s `fn_by_name` lookup
    // (and the digital `Codegen` path's `Resolver::new`, which snapshots
    // `symbols.fns()`) both find it. No separate pass needed.

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
                        let width = port.net_type().width();
                        if width > 1 {
                            // T13/LRMW3-05: a vector port's `net_ref`
                            // names the whole connected net (its own
                            // width already matched against the port's by
                            // `CheckPortSizes`, §6.5.7.1) — there is no
                            // single node a bare `label.port` could name,
                            // so this registers one `label.port[i]` entry
                            // per bit instead, resolved the same way
                            // `device::builder::InstanceBuilder::
                            // resolve_connections_by_bit` resolves the
                            // circuit-build-time connection for the same
                            // bit.
                            for bit in 0..width {
                                let elem_name = crate::resolve::pom::structure::element_node_name(net_ref.net(), bit);
                                let parent_node = if let Some(&id) = node_by_name.get(&elem_name) {
                                    id
                                } else {
                                    errors.push(LowerError {
                                        module: m.name().to_string(),
                                        what: "net (instance connection)",
                                        name: elem_name.clone(),
                                    });
                                    NodeId::GROUND
                                };
                                instance_ports.insert(
                                    crate::resolve::pom::structure::element_node_name(
                                        &format!("{label}.{}", port.name()),
                                        bit,
                                    ),
                                    parent_node,
                                );
                            }
                            continue;
                        }
                        // An indexed net ref (`tap[i]`, lifted from an
                        // array wire by the flatten pass or authored as
                        // `tap[i]` in a structural `for`) resolves through
                        // its full Display form — Fix A registers one node
                        // per element (`tap[0]`..`tap[N-1]`), so the bare
                        // parent name no longer identifies a node.
                        let bare = net_ref.net();
                        let full = net_ref.to_string();
                        let lookup = if net_ref.index().is_some() { &full } else { bare };
                        let parent_node = if bodies[i].symbols.is_ground(bare) {
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

        // Switch branches (§5.6.5, LRMW2-40, T45): resolve every branch
        // the elaborator marked `BranchKind::Switch` to its `(NodeId,
        // NodeId)` pair, using the same `node_by_name` map instance-port
        // resolution just built above.
        bodies[i].switch_branches = m
            .branches()
            .iter()
            .filter(|b| b.kind() == piperine_lang::pom::BranchKind::Switch)
            .filter_map(|b| {
                let resolve_net = |net_ref: Option<&piperine_lang::pom::NetRef>| -> Option<NodeId> {
                    match net_ref {
                        None => Some(NodeId::GROUND),
                        Some(nr) if bodies[i].symbols.is_ground(nr.net()) => Some(NodeId::GROUND),
                        Some(nr) => node_by_name.get(nr.net()).copied(),
                    }
                };
                let plus = resolve_net(Some(b.pos()))?;
                let minus = resolve_net(b.neg())?;
                Some((plus, minus))
            })
            .collect();

        // Variable initializers (Annex A.2.2.1): every module-level `var`
        // declared with `= constant_expression`, as `(VarId, init expr)`
        // pairs resolved against the module's symbols — the shared
        // collection the digital `reg_inits` and the analog var-bank seed
        // both draw from (one lowering rule, two consumers).
        bodies[i].var_inits = m
            .vars()
            .iter()
            .filter_map(|v| v.init().map(|init| (v.name().to_string(), init)))
            .filter_map(|(vname, init)| {
                bodies[i]
                    .symbols
                    .vars()
                    .find(|(_, info)| info.name == vname)
                    .map(|(id, _)| (id, value_to_pom_expr(init)))
            })
            .collect();

        // Digital-domain nodes read from an analog body (§ shadow-var
        // bridge, `LowerCtx::shadow_var_for`), collected across every
        // analog behavior and merged into the module's digital body below
        // — creating one if the module declares no `digital` block at all
        // (a module can be purely analog but still read a digital port).
        let mut digital_shadows: Vec<(NodeId, VarId)> = Vec::new();

        for behavior in m.behaviors() {
            let is_digital = behavior.is_digital();
            let mut ctx = LowerCtx::new(&mut bodies[i].symbols, m.name().to_string(), is_digital, top_module_name.clone());
            ctx.instance_ports = instance_ports.clone();
            ctx.consts = LowerCtx::const_irs(prog);
            ctx.default_transition = prog.directives().default_transition;
            ctx.nature_abstols = nature_abstols.clone();

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
                    // T13/LRMW3-05: a vector port contributes one entry
                    // per bit — same loop body as the scalar (1-element)
                    // case, never a special-cased branch.
                    for &node_id in &port.nodes {
                        let node = bodies[i].symbols.node(node_id);
                        if node.domain != Domain::Digital {
                            continue;
                        }
                        match port.direction {
                            Direction::In => inputs.push(node_id),
                            Direction::Out => outputs.push(node_id),
                            Direction::Inout => {
                                inputs.push(node_id);
                                outputs.push(node_id);
                            }
                        }
                    }
                }
                // T52 (LRMW2-45, §3.7): an internal (non-port) net
                // declared with one of the four 1364 net-type keywords is
                // a legitimate digital write target — `reg` by procedural
                // assignment (`initial`/`always`), `wire`/`tri`/`wreal` by
                // continuous assign (`assign w = expr;`, which
                // `elab/lower/vams.rs` lowers to a synthesized `always
                // @(*)` — the *same* procedural-assignment shape a `reg`
                // write takes by the time this pass sees it). Registered
                // as a digital output here so that assignment passes IR
                // validation instead of failing "not a digital output" (a
                // check that, before this task, only ever considered
                // ports).
                for w in m.wires() {
                    if matches!(w.net_type(), piperine_lang::pom::NetType::Digital1364(_))
                        && let Some((id, _)) = bodies[i].symbols.nodes().find(|(_, n)| n.name == w.name())
                    {
                        outputs.push(id);
                    }
                }

                // Module-level vars are persistent digital state — they are
                // registers (SPEC §9: a `var` updated in a clocked `@` block
                // is an edge-triggered register). Their VarIds and initial
                // values come from the shared `var_inits` collection above
                // (A.2.2.1 initializers); vars without an initializer start
                // at the kernel's power-on default.
                let mut regs = Vec::new();
                for (vname, _) in m.vars().iter().map(|v| (v.name().to_string(), v.init())) {
                    if let Some((id, _)) = bodies[i].symbols.vars().find(|(_, info)| info.name == vname) {
                        regs.push(id);
                    }
                }
                let reg_inits: Vec<(VarId, PomExpr)> = bodies[i]
                    .var_inits
                    .iter()
                    .filter(|(id, _)| regs.contains(id))
                    .cloned()
                    .collect();

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

                // A module may declare more than one `analog` block — LRM
                // §6.9.1 "Concatenation of analog blocks": the simulator
                // combines them into one, in source order. Mirrors the
                // digital arm above (`f75beb4d`): assigning unconditionally
                // here kept only the *last* block and silently dropped every
                // earlier one — the same "silent no-op" failure the crate's
                // fail-loud contract forbids, except here it produced a
                // wrong numeric answer instead of a missing log line.
                //
                // `ctx.states`/`ctx.noise_sources` are just *this* behavior's
                // allocations — `LowerCtx::alloc_state` (`:186` above) always
                // calls `self.symbols.add_state`, and `symbols` is the one
                // `SymbolTable` shared by every behavior of this module
                // (`bodies[i].symbols`, re-borrowed fresh per behavior at
                // `LowerCtx::new`, never reset). So `StateId`s are already
                // globally unique across the whole module regardless of
                // which block's `ctx` allocated them — extending the vectors
                // needs no id offset; only `stmts` order matters (preserved
                // by `extend`, so a var declared in block 1 is visible when
                // block 2 reads it).
                let new_states = ctx.states;
                let new_noise = ctx.noise_sources;
                match &mut bodies[i].analog {
                    Some(body) => {
                        body.states.extend(new_states);
                        body.noise.extend(new_noise);
                        body.stmts.extend(stmts);
                    }
                    None => {
                        bodies[i].analog = Some(AnalogBody {
                            states: new_states,
                            noise: new_noise,
                            stmts,
                        });
                    }
                }
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
            let mut ctx = LowerCtx::new(&mut bodies[i].symbols, m.name().to_string(), false, top_module_name.clone());
            ctx.instance_ports = instance_ports.clone();
            ctx.consts = LowerCtx::const_irs(prog);
            ctx.default_transition = prog.directives().default_transition;
            let margins = crate::resolve::pom::ConstraintLowering::lower(m.constraints(), &mut ctx);
            let (points, crosses) =
                crate::resolve::pom::ConstraintLowering::lower_cover(m.constraints(), &mut ctx);
            errors.append(&mut ctx.errors);
            if !margins.is_empty() {
                bodies[i].constraints = Some(margins);
            }
            if !points.is_empty() || !crosses.is_empty() {
                bodies[i].cover = Some((points, crosses));
            }
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
