use std::collections::HashMap;

use cranelift_codegen::ir::{types, AbiParam, InstBuilder, MemFlags, Signature, Value};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};

use crate::emit::{Builder, Resolver};
use crate::resolve::{Domain, LoweredBody, StateKind, NodeId};

use crate::flatten::analog::{visit_all, FlatAnalog, FlatContrib, FlatEventTrigger, FlatForce};
use crate::error::CodegenError;
use piperine_lang::math;

use piperine_lang::parse::ast::Expr as PomExpr;

use super::{
    AnalogFn, AnalogKernel, CompiledEvent, CompiledTrigger, RuntimeState, RuntimeStateSpec,
};

/// Collect the unique `$limit` expressions across every flattened expression,
/// in a stable order. Each becomes a `vold` slot appended to the state bank.
fn collect_limits(flat: &FlatAnalog) -> Vec<PomExpr> {
    let mut limits: Vec<PomExpr> = Vec::new();
    let mut scan = |e: &PomExpr| {
        use piperine_lang::parse::ast::Walk;
        e.walk(&mut |node| {
            if let PomExpr::SysCall(name, _) = node
                && name.trim_start_matches('$') == "limit" && !limits.iter().any(|l| expr_eq(l, node)) {
                    limits.push(node.clone());
                }
            Walk::Continue
        });
    };
    for c in &flat.resistive { scan(&c.expr); }
    for c in &flat.charge { scan(&c.expr); }
    for f in &flat.forces { scan(&f.expr); }
    // `$limit` most often lives inside a `var` (e.g. `vd = $limit(…)`), so
    // scan the temp tape too.
    for t in &flat.temps { scan(t); }
    limits
}


/// The junction branch `(plus, minus)` a `$limit` acts on.
fn limit_branch(limit: &PomExpr, module: &LoweredBody, temps: &[PomExpr]) -> Option<(NodeId, NodeId)> {
    let PomExpr::SysCall(name, args) = limit else { return None };
    if name.trim_start_matches('$') != "limit" { return None; }
    let vnew = args.get(1)?;
    let resolve = |n: &str| -> NodeId {
        if piperine_lang::pom::is_ground(n) { return NodeId::GROUND; }
        module.symbols.nodes().find(|(_, info)| info.name == n).map(|(id, _)| id).unwrap_or(NodeId::GROUND)
    };
    // Collect the unique `V`/`I` branch the limited voltage acts on, walking
    // `__temp` leaves through the tape by *id* (memoized) — never rebuilding
    // the inlined tree, which would re-expand param-only chains (`tBrkdwnV`)
    // exponentially.
    let mut branches: Vec<(NodeId, NodeId)> = Vec::new();
    let mut seen_temps = std::collections::HashSet::new();
    limit_branches_into(vnew, temps, &resolve, &mut seen_temps, &mut branches);
    if branches.len() == 1 { Some(branches[0]) } else { None }
}

fn limit_branches_into(
    expr: &PomExpr,
    temps: &[PomExpr],
    resolve: &impl Fn(&str) -> NodeId,
    seen_temps: &mut std::collections::HashSet<u64>,
    out: &mut Vec<(NodeId, NodeId)>,
) {
    use piperine_lang::parse::ast::{Literal, Walk};
    expr.walk(&mut |node| {
        if let PomExpr::Call(func, call_args) = node
            && let PomExpr::Ident(fname) = func.as_ref()
        {
            if fname == "V" || fname == "I" {
                let plus = ident_of(call_args.first()).unwrap_or_default();
                let minus = ident_of(call_args.get(1)).unwrap_or_else(|| "0".into());
                let branch = (resolve(&plus), resolve(&minus));
                if !out.contains(&branch) {
                    out.push(branch);
                }
                return Walk::SkipChildren;
            }
            if fname == "__temp"
                && let Some(PomExpr::Literal(Literal::Int(id))) = call_args.first()
            {
                if seen_temps.insert(*id) {
                    limit_branches_into(&temps[*id as usize], temps, resolve, seen_temps, out);
                }
                return Walk::SkipChildren;
            }
        }
        Walk::Continue
    });
}

fn ident_of(e: Option<&PomExpr>) -> Option<String> {
    match e {
        Some(PomExpr::Ident(s)) => Some(s.clone()),
        _ => None,
    }
}

/// Structural equality for POM `Expr`.
fn expr_eq(a: &PomExpr, b: &PomExpr) -> bool {
    crate::emit::expr_structural_eq(a, b)
}

// ─── Compiler ─────────────────────────────────────────────────────────────────

/// Builds every kernel function inside one Cranelift JIT module.
pub(super) struct AnalogCompiler<'m> {
    module: &'m LoweredBody,
    flat: FlatAnalog,
    terminals: Vec<NodeId>,
    num_ports: usize,
    slot: HashMap<NodeId, usize>,
    jit: JITModule,
    math_ids: HashMap<&'static str, FuncId>,
    fb_ctx: FunctionBuilderContext,
    /// Unique `$limit` expressions, in slot order.
    limits: Vec<PomExpr>,
    /// State-bank slot where `$limit` vold slots begin (= module state count).
    limit_base: usize,
}

impl<'m> AnalogCompiler<'m> {
    pub(super) fn new(module: &'m LoweredBody, flat: FlatAnalog) -> Result<Self, CodegenError> {
        let mut jit_builder = JITBuilder::new(cranelift_module::default_libcall_names())
            .map_err(|e| CodegenError::Module(e.to_string()))?;
        for f in math::MATH_FNS {
            jit_builder.symbol(f.name, f.symbol);
        }
        let mut jit = JITModule::new(jit_builder);

        let mut math_ids = HashMap::new();
        for f in math::MATH_FNS {
            let mut sig = jit.make_signature();
            for _ in 0..f.arity {
                sig.params.push(AbiParam::new(types::F64));
            }
            sig.returns.push(AbiParam::new(types::F64));
            let id = jit
                .declare_function(f.name, Linkage::Import, &sig)
                .map_err(|e| CodegenError::Module(e.to_string()))?;
            math_ids.insert(f.name, id);
        }

        let (terminals, num_ports) = Self::terminal_order(module, &flat);
        let slot = terminals
            .iter()
            .enumerate()
            .map(|(i, &n)| (n, i))
            .collect();

        let limits = collect_limits(&flat);
        let limit_base = module.symbols.num_states();

        Ok(Self {
            module,
            flat,
            terminals,
            num_ports,
            slot,
            jit,
            math_ids,
            fb_ctx: FunctionBuilderContext::new(),
            limits,
            limit_base,
        })
    }

    /// Ports in declaration order, then every non-ground internal node the
    /// flattened body touches.
    fn terminal_order(module: &LoweredBody, flat: &FlatAnalog) -> (Vec<NodeId>, usize) {
        let mut terminals: Vec<NodeId> = module.ports.iter().map(|p| p.node).collect();
        let num_ports = terminals.len();
        let mut add = |node: NodeId| {
            if !node.is_ground() && !terminals.contains(&node) {
                terminals.push(node);
            }
        };
        let mut pairs = Vec::new();
        let resolve_node = |name: &str| -> Option<NodeId> {
            if piperine_lang::pom::is_ground(name) { return Some(NodeId::GROUND); }
            module.symbols.nodes().find(|(_, info)| info.name == name).map(|(id, _)| id)
        };
        for expr in flat.exprs() {
            crate::resolve::diff::collect_branches(expr, &mut pairs, &resolve_node);
        }
        for contrib in flat.resistive.iter().chain(&flat.charge) {
            pairs.push((contrib.plus, contrib.minus));
        }
        for force in &flat.forces {
            pairs.push((force.plus, force.minus));
        }
        for &(p, m) in &pairs {
            add(p);
            add(m);
        }
        for &(plus, minus, _, _) in &flat.noise {
            add(plus);
            add(minus);
        }
        for &(plus, minus, _) in &flat.initial_conditions {
            add(plus);
            add(minus);
        }
        (terminals, num_ports)
    }

    pub(super) fn compile(mut self, compile_disto: bool) -> Result<AnalogKernel, CodegenError> {
        let read_bounds = self.flat.read_bounds(self.module);
        // Presence-queried params (`$param_given`, the optional `T?`
        // machinery) — collected before the flat body is drained below.
        // Resolution mirrors the emit-time `Resolver::param_given` rule:
        // exact param name first, then a unique flattened bundle-field
        // suffix (`ns` → `model_ns`).
        let mut presence_mask = 0u64;
        {
            let names: Vec<(&str, u32)> = self
                .module
                .symbols
                .params()
                .map(|(id, p)| (p.name.as_str(), id.0))
                .collect();
            let resolve = |pname: &str| -> Option<u32> {
                if let Some(&(_, id)) = names.iter().find(|(n, _)| *n == pname) {
                    return Some(id);
                }
                let suffix = format!("_{pname}");
                let mut matches = names.iter().filter(|(n, _)| n.ends_with(&suffix));
                match (matches.next(), matches.next()) {
                    (Some(&(_, id)), None) => Some(id),
                    _ => None,
                }
            };
            for expr in self.flat.exprs() {
                visit_all(expr, &mut |e| {
                    if let PomExpr::SysCall(name, args) = e
                        && name.trim_start_matches('$') == "param_given"
                        && let Some(PomExpr::Literal(
                            piperine_lang::parse::ast::Literal::String(pname),
                        )) = args.first()
                        && let Some(id) = resolve(pname)
                    {
                        presence_mask |= 1 << id.min(63);
                    }
                });
            }
        }
        let resistive = std::mem::take(&mut self.flat.resistive);
        let charge = std::mem::take(&mut self.flat.charge);
        let forces = std::mem::take(&mut self.flat.forces);
        let noise = std::mem::take(&mut self.flat.noise);
        let bound_steps = std::mem::take(&mut self.flat.bound_steps);
        let runtime_inputs = self.flat.runtime_states.clone();
        let initial_conditions = std::mem::take(&mut self.flat.initial_conditions);

        let temps = self.flat.temps.clone();
        let residual_id = self.compile_residual("residual", &resistive)?;
        let jacobian_id = self.compile_jacobian("jacobian", &resistive, &temps)?;

        // AC `idt` stamp: re-diff the resistive tape with every integrator
        // state's `__state_load` replaced by its input expression — the
        // linear-operator view of `idt(x)` (the device scales by 1/(jω) at
        // stamp time). Other runtime states stay frozen in AC, as before.
        let ac_idt_jacobian_id = {
            let idt_inputs: Vec<(u64, &PomExpr)> = runtime_inputs
                .iter()
                .filter(|(id, _)| {
                    matches!(
                        self.module.symbols.state(*id).kind,
                        StateKind::Idt { .. } | StateKind::IdtMod { .. }
                    )
                })
                .map(|(id, x)| (id.0 as u64, x))
                .collect();
            if idt_inputs.is_empty() {
                None
            } else {
                let subst = |e: &PomExpr| -> PomExpr {
                    crate::flatten::analog::rewrite_expr(e, &mut |ex| {
                        if let PomExpr::Call(func, args) = ex
                            && let PomExpr::Ident(name) = func.as_ref()
                            && name == "__state_load"
                            && let Some(PomExpr::Literal(piperine_lang::parse::ast::Literal::Int(k))) = args.first()
                            && let Some((_, x)) = idt_inputs.iter().find(|(slot, _)| slot == k)
                        {
                            return (*x).clone();
                        }
                        ex.clone()
                    })
                };
                let temps_ac: Vec<PomExpr> = temps.iter().map(|t| subst(t)).collect();
                let contribs_ac: Vec<FlatContrib> = resistive
                    .iter()
                    .map(|c| FlatContrib { plus: c.plus, minus: c.minus, expr: subst(&c.expr) })
                    .collect();
                Some(self.compile_jacobian("ac_idt_jacobian", &contribs_ac, &temps_ac)?)
            }
        };

        // `@initial` UIC seeds: one param-only row per condition (its value),
        // plus the terminal pair it seeds.
        let ic_terminals: Vec<(NodeId, NodeId)> =
            initial_conditions.iter().map(|(p, m, _)| (*p, *m)).collect();
        let ic_values_id = if initial_conditions.is_empty() {
            None
        } else {
            let vals: Vec<PomExpr> = initial_conditions.iter().map(|(_, _, v)| v.clone()).collect();
            Some(self.compile_rows("initial_conditions", &vals)?)
        };

        let (charge_id, charge_jac_id) = if charge.is_empty() {
            (None, None)
        } else {
            (
                Some(self.compile_residual("charge", &charge)?),
                Some(self.compile_jacobian("charge_jacobian", &charge, &temps)?),
            )
        };

        // `.disto` second/third derivatives over the resistive + charge
        // contributions (DISTO-03); skipped entirely (empty, zero compile
        // cost) unless the caller requested `.disto` support — a
        // many-branch device pays a real Cranelift compile cost for these
        // kernels (one function per branch combination), wasted on every
        // analysis but `.disto` itself.
        let (disto2_ids, disto2_pairs) = if compile_disto {
            match self.compile_disto2("disto2", &resistive, &charge, &temps)? {
                Some((ids, pairs)) => (ids, pairs),
                None => (Vec::new(), Vec::new()),
            }
        } else {
            (Vec::new(), Vec::new())
        };
        let (disto3_ids, disto3_triples) = if compile_disto {
            match self.compile_disto3("disto3", &resistive, &charge, &temps)? {
                Some((ids, triples)) => (ids, triples),
                None => (Vec::new(), Vec::new()),
            }
        } else {
            (Vec::new(), Vec::new())
        };

        let (force_id, force_jac_id) = if forces.is_empty() {
            (None, None)
        } else {
            (
                Some(self.compile_rows("force", &forces.iter().map(|f| f.expr.clone()).collect::<Vec<_>>())?),
                Some(self.compile_force_jacobian("force_jacobian", &forces)?),
            )
        };

        // Inductor flux terms flattened across forces: each is
        // `(force_idx, target_plus, target_minus)` + a coefficient row. The
        // transient companion adds `dΦ/dt` (`Φ = Σ coeff·I(target)`) onto
        // force `force_idx`'s branch equation, coupling to `target`'s current.
        let flux_meta: Vec<(usize, NodeId, NodeId)> = forces
            .iter()
            .enumerate()
            .flat_map(|(i, f)| f.flux_terms.iter().map(move |(tp, tm, _)| (i, *tp, *tm)))
            .collect();
        let force_flux_id = if flux_meta.is_empty() {
            None
        } else {
            let coeffs: Vec<PomExpr> = forces
                .iter()
                .flat_map(|f| f.flux_terms.iter().map(|(_, _, c)| c.clone()))
                .collect();
            Some(self.compile_rows("force_flux", &coeffs)?)
        };

        // Series-impedance terms flattened across forces, mirroring the flux
        // layout: `(force_idx, target_plus, target_minus)` + a coefficient
        // row. The runtime stamps `−coeff` on the target branch-current
        // column of force `force_idx`'s branch equation (DC, transient, AC).
        let current_meta: Vec<(usize, NodeId, NodeId)> = forces
            .iter()
            .enumerate()
            .flat_map(|(i, f)| f.current_terms.iter().map(move |(tp, tm, _)| (i, *tp, *tm)))
            .collect();
        let force_current_id = if current_meta.is_empty() {
            None
        } else {
            let coeffs: Vec<PomExpr> = forces
                .iter()
                .flat_map(|f| f.current_terms.iter().map(|(_, _, c)| c.clone()))
                .collect();
            Some(self.compile_rows("force_current", &coeffs)?)
        };

        // AC drive attached to force branches (ideal AC voltage stimulus). One
        // row per force; branches without a stimulus contribute 0. Compiled
        // only when at least one force carries an `ac_stim`.
        let (force_ac_mag_id, force_ac_phase_id) = if forces.iter().any(|f| f.ac_stim.is_some()) {
            let mags: Vec<PomExpr> = forces
                .iter()
                .map(|f| f.ac_stim.as_ref().map_or(PomExpr::Literal(piperine_lang::parse::ast::Literal::Real(0.0)), |(m, _)| m.clone()))
                .collect();
            let phases: Vec<PomExpr> = forces
                .iter()
                .map(|f| f.ac_stim.as_ref().map_or(PomExpr::Literal(piperine_lang::parse::ast::Literal::Real(0.0)), |(_, p)| p.clone()))
                .collect();
            (
                Some(self.compile_rows("force_ac_mag", &mags)?),
                Some(self.compile_rows("force_ac_phase", &phases)?),
            )
        } else {
            (None, None)
        };

        let noise_id = if noise.is_empty() {
            None
        } else {
            let psds: Vec<PomExpr> = noise.iter().map(|(_, _, psd, _)| psd.clone()).collect();
            Some(self.compile_rows("noise", &psds)?)
        };
        // `$limit` update: one row per slot yielding the limited value `vlim`,
        // which the device writes back into the state bank each Newton
        // iteration to become the next iteration's `vold`.
        let limit_update_id = if self.limits.is_empty() {
            None
        } else {
            let rows = self.limits.clone();
            Some(self.compile_rows("limit_update", &rows)?)
        };
        // `$limit` seed: the critical voltage `vcrit` (arg 3) per slot. Junctions
        // start limiting from vcrit — ngspice's MODEINITJCT — so a diode begins
        // near turn-on instead of at 0 V (which floats the node to the supply
        // and makes vold crawl up chasing a runaway node).
        let limit_seed_id = if self.limits.is_empty() {
            None
        } else {
            let seeds: Vec<PomExpr> = self
                .limits
                .iter()
                .map(|l| match l {
                    PomExpr::SysCall(name, args) if name.trim_start_matches('$') == "limit" && args.len() >= 5 => {
                        args[4].clone()
                    }
                    _ => PomExpr::Literal(piperine_lang::parse::ast::Literal::Real(0.0)),
                })
                .collect();
            Some(self.compile_rows("limit_seed", &seeds)?)
        };
        // Raw `vnew` (arg 0) per slot, for branch-polarity detection.
        let limit_vnew_id = if self.limits.is_empty() {
            None
        } else {
            let vnews: Vec<PomExpr> = self
                .limits
                .iter()
                .map(|l| match l {
                    PomExpr::SysCall(name, args) if name.trim_start_matches('$') == "limit" && args.len() >= 2 => {
                        args[1].clone()
                    }
                    _ => PomExpr::Literal(piperine_lang::parse::ast::Literal::Real(0.0)),
                })
                .collect();
            Some(self.compile_rows("limit_vnew", &vnews)?)
        };
        // Junction branch (as terminal slots) per limit.
        let limit_branches: Vec<Option<(Option<usize>, Option<usize>)>> = self
            .limits
            .iter()
            .map(|l| {
                // The $limit's `vnew` is a `__temp` leaf; `limit_branch`
                // searches the tape by id to find the `V`/`I` branch it acts on.
                limit_branch(l, self.module, &self.flat.temps).map(|(p, m)| {
                    (self.slot.get(&p).copied(), self.slot.get(&m).copied())
                })
            })
            .collect();
        let ac_stims = std::mem::take(&mut self.flat.ac_stims);
        let (ac_stim_mag_id, ac_stim_phase_id) = if ac_stims.is_empty() {
            (None, None)
        } else {
            let mags: Vec<PomExpr> = ac_stims.iter().map(|s| s.mag.clone()).collect();
            let phases: Vec<PomExpr> = ac_stims.iter().map(|s| s.phase.clone()).collect();
            (
                Some(self.compile_rows("ac_stim_mag", &mags)?),
                Some(self.compile_rows("ac_stim_phase", &phases)?),
            )
        };
        // Flicker exponent rows (0 for white sources) — only compiled when
        // at least one source is flicker.
        let noise_exp_id = if noise.iter().any(|(_, _, _, exp)| exp.is_some()) {
            let rows: Vec<PomExpr> = noise
                .iter()
                .map(|(_, _, _, exp)| exp.clone().unwrap_or(PomExpr::Literal(piperine_lang::parse::ast::Literal::Real(0.0))))
                .collect();
            Some(self.compile_rows("noise_exponents", &rows)?)
        } else {
            None
        };

        let state_inputs_id = if runtime_inputs.is_empty() {
            None
        } else {
            // One row per state *slot*; slots without a runtime input write 0.
            let mut rows = vec![PomExpr::Literal(piperine_lang::parse::ast::Literal::Real(0.0)); self.module.symbols.num_states()];
            for (id, input) in &runtime_inputs {
                rows[id.0 as usize] = input.clone();
            }
            Some(self.compile_rows("state_inputs", &rows)?)
        };

        let events = std::mem::take(&mut self.flat.events);
        let (event_triggers_id, event_actions_id) = if events.is_empty() {
            (None, None)
        } else {
            // Trigger rows: the watched expression (0 for initial/timer —
            // those fire on lifecycle/clock, not on a value transition).
            let trigger_rows: Vec<PomExpr> = events
                .iter()
                .map(|e| match &e.trigger {
                    FlatEventTrigger::Cross { expr, .. } | FlatEventTrigger::Above { expr } => {
                        expr.clone()
                    }
                    FlatEventTrigger::Initial | FlatEventTrigger::Timer { .. } => PomExpr::Literal(piperine_lang::parse::ast::Literal::Real(0.0)),
                })
                .collect();
            let action_rows: Vec<PomExpr> = events
                .iter()
                .flat_map(|e| e.actions.iter().map(|a| a.value.clone()))
                .collect();
            let actions_id = if action_rows.is_empty() {
                None
            } else {
                Some(self.compile_rows("event_actions", &action_rows)?)
            };
            (Some(self.compile_rows("event_triggers", &trigger_rows)?), actions_id)
        };
        let num_event_actions = events.iter().map(|e| e.actions.len()).sum();
        let compiled_events: Vec<CompiledEvent> = events
            .iter()
            .map(|e| CompiledEvent {
                trigger: match &e.trigger {
                    FlatEventTrigger::Initial => CompiledTrigger::Initial,
                    FlatEventTrigger::Cross { dir, .. } => CompiledTrigger::Cross(*dir),
                    FlatEventTrigger::Above { .. } => CompiledTrigger::Above,
                    FlatEventTrigger::Timer { period, phase } => {
                        CompiledTrigger::Timer { period: period.clone(), phase: phase.clone() }
                    }
                },
                action_vars: e.actions.iter().map(|a| a.var).collect(),
            })
            .collect();

        let bound_step_id = if bound_steps.is_empty() {
            None
        } else {
            let min = bound_steps
                .into_iter()
                .reduce(|a, b| PomExpr::Call(Box::new(PomExpr::Ident("min".to_string())), vec![a, b]))
                .expect("non-empty");
            Some(self.compile_rows("bound_step", &[min])?)
        };

        self.jit
            .finalize_definitions()
            .map_err(|e| CodegenError::Module(e.to_string()))?;

        let get = |jit: &JITModule, id: FuncId| -> AnalogFn {
            // SAFETY: every function is compiled with the shared 5-pointer
            // signature built by `begin_fn`.
            unsafe { std::mem::transmute(jit.get_finalized_function(id)) }
        };

        let runtime_states = runtime_inputs
            .iter()
            .map(|(id, _)| {
                let kind = match &self.module.symbols.state(*id).kind {
                    StateKind::Delay { delay } => RuntimeState::Delay { delay: delay.clone() },
                    StateKind::Slew { rise, fall } => {
                        RuntimeState::Slew { rise: rise.clone(), fall: fall.clone() }
                    }
                    StateKind::Transition { delay, rise, fall, .. } => RuntimeState::Transition {
                        delay: delay.clone(),
                        rise: rise.clone(),
                        fall: fall.clone(),
                    },
                    StateKind::Idt { ic } => {
                        RuntimeState::Integrator { ic: ic.clone(), modulus: None }
                    }
                    StateKind::IdtMod { ic, modulus } => RuntimeState::Integrator {
                        ic: ic.clone(),
                        modulus: Some(modulus.clone()),
                    },
                    other => {
                        return Err(CodegenError::Invalid(format!(
                            "`{}` is not a runtime-serviced state",
                            other.name()
                        )))
                    }
                };
                Ok(RuntimeStateSpec { id: *id, kind })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(AnalogKernel {
            name: self.module.name.clone(),
            num_ports: self.num_ports,
            presence_mask,
            num_params: self.module.symbols.num_params(),
            num_state_slots: self.module.symbols.num_states() + self.limits.len(),
            num_limits: self.limits.len(),
            limit_update: limit_update_id.map(|id| get(&self.jit, id)),
            limit_seed: limit_seed_id.map(|id| get(&self.jit, id)),
            limit_vnew: limit_vnew_id.map(|id| get(&self.jit, id)),
            limit_branches,
            num_vars: self.module.symbols.vars().count(),
            num_forces: forces.len(),
            num_noise: noise.len(),
            num_ac_stims: ac_stims.len(),
            force_terminals: forces.iter().map(|f| (f.plus, f.minus)).collect(),
            ac_stim_terminals: ac_stims.iter().map(|s| (s.plus, s.minus)).collect(),
            noise_terminals: noise.iter().map(|&(p, m, _, _)| (p, m)).collect(),
            noise_exponents: noise_exp_id.map(|id| get(&self.jit, id)),
            runtime_states,
            events: compiled_events,
            num_event_actions,
            diagnostics: std::mem::take(&mut self.flat.diagnostics),
            residual: get(&self.jit, residual_id),
            jacobian: get(&self.jit, jacobian_id),
            charge: charge_id.map(|id| get(&self.jit, id)),
            charge_jacobian: charge_jac_id.map(|id| get(&self.jit, id)),
            disto2: disto2_ids.iter().map(|&id| get(&self.jit, id)).collect(),
            disto2_pairs,
            disto3: disto3_ids.iter().map(|&id| get(&self.jit, id)).collect(),
            disto3_triples,
            disto2_contribs: resistive
                .iter()
                .chain(&charge)
                .map(|c| (c.plus, c.minus))
                .collect(),
            disto2_charge_start: resistive.len(),
            ac_idt_jacobian: ac_idt_jacobian_id.map(|id| get(&self.jit, id)),
            force: force_id.map(|id| get(&self.jit, id)),
            force_jacobian: force_jac_id.map(|id| get(&self.jit, id)),
            force_ac_mag: force_ac_mag_id.map(|id| get(&self.jit, id)),
            force_ac_phase: force_ac_phase_id.map(|id| get(&self.jit, id)),
            force_flux: force_flux_id.map(|id| get(&self.jit, id)),
            flux_meta,
            force_current: force_current_id.map(|id| get(&self.jit, id)),
            current_meta,
            noise: noise_id.map(|id| get(&self.jit, id)),
            ac_stim_mag: ac_stim_mag_id.map(|id| get(&self.jit, id)),
            ac_stim_phase: ac_stim_phase_id.map(|id| get(&self.jit, id)),
            state_inputs: state_inputs_id.map(|id| get(&self.jit, id)),
            event_triggers: event_triggers_id.map(|id| get(&self.jit, id)),
            event_actions: event_actions_id.map(|id| get(&self.jit, id)),
            bound_step: bound_step_id.map(|id| get(&self.jit, id)),
            initial_condition_terminals: ic_terminals,
            initial_conditions: ic_values_id.map(|id| get(&self.jit, id)),
            digital_terminals: self
                .terminals
                .iter()
                .map(|&id| self.module.symbols.node(id).domain == Domain::Digital)
                .collect(),
            read_bounds,
            param_names: self.module.symbols.params().map(|(_, p)| p.name.clone()).collect(),
            terminals: std::mem::take(&mut self.terminals),
            _jit: self.jit,
        })
    }

    // ── Function skeletons ──

    /// Residual shape: `out[plus] += expr; out[minus] -= expr` per contribution.
    fn compile_residual(&mut self, name: &str, contribs: &[FlatContrib]) -> Result<FuncId, CodegenError> {
        let exprs: Vec<&PomExpr> = contribs.iter().map(|c| &c.expr).collect();
        self.build_fn(name, &exprs, |b, slot, out_ptr| {
            for contrib in contribs {
                let current = b.emit_analog(&contrib.expr)?;
                if let Some(&p) = slot.get(&contrib.plus) {
                    b.accumulate_f64(current, out_ptr, p);
                }
                if let Some(&m) = slot.get(&contrib.minus) {
                    let negated = b.builder.ins().fneg(current);
                    b.accumulate_f64(negated, out_ptr, m);
                }
            }
            Ok(())
        })
    }

    /// Jacobian shape: `out[row·n + col] += ∂I/∂V` stamps per contribution.
    ///
    /// Contributions hold only `__temp` leaves, so the voltage dependence
    /// lives in the temp tape — passed explicitly so callers can substitute
    /// an adjusted tape (the AC `idt` Jacobian swaps integrator state loads
    /// for their input expressions). For each voltage branch `(a,b)` we build
    /// the derivative tape `dtemps[k] = d(temps[k])/dV(a,b)` once, then each
    /// contribution's derivative — which references `__dtemp` leaves — is
    /// emitted against it. Every temp/dtemp is emitted once per branch,
    /// keeping the Jacobian linear in body size.
    fn compile_jacobian(&mut self, name: &str, contribs: &[FlatContrib], temps: &[PomExpr]) -> Result<FuncId, CodegenError> {
        let n = self.terminals.len();
        let exprs: Vec<&PomExpr> = contribs.iter().map(|c| &c.expr).collect();
        let module = self.module;
        let resolve_node = |name: &str| -> Option<NodeId> {
            if piperine_lang::pom::is_ground(name) { return Some(NodeId::GROUND); }
            module.symbols.nodes().find(|(_, info)| info.name == name).map(|(id, _)| id)
        };
        // Global branch set: every V/I branch read anywhere in the body
        // (contributions carry none directly — they're inside the temps).
        let mut seen = std::collections::HashSet::new();
        let mut branches: Vec<(NodeId, NodeId)> = Vec::new();
        let mut collect = |e: &PomExpr| {
            let mut pairs = Vec::new();
            crate::resolve::diff::collect_branches(e, &mut pairs, &resolve_node);
            for pair in pairs {
                if seen.insert(pair) {
                    branches.push(pair);
                }
            }
        };
        for c in contribs { collect(&c.expr); }
        for t in temps { collect(t); }
        self.build_fn(name, &exprs, move |b, slot, out_ptr| {
            for (a, bb) in branches {
                // Derivative tape for this branch.
                let dtemps: Vec<PomExpr> = temps
                    .iter()
                    .map(|t| crate::resolve::diff::d_dv(t, a, bb, &resolve_node))
                    .collect();
                b.set_deriv_tape(dtemps);
                let col_a = slot.get(&a).copied();
                let col_b = slot.get(&bb).copied();
                for contrib in contribs {
                    let derivative = crate::resolve::diff::d_dv(&contrib.expr, a, bb, &resolve_node);
                    let g = b.emit_analog(&derivative)?;
                    let plus = slot.get(&contrib.plus).copied();
                    let minus = slot.get(&contrib.minus).copied();
                    let stamp = |b: &mut Builder, row: Option<usize>, col: Option<usize>, negate: bool| {
                        if let (Some(r), Some(c)) = (row, col) {
                            let v = if negate { b.builder.ins().fneg(g) } else { g };
                            b.accumulate_f64(v, out_ptr, r * n + c);
                        }
                    };
                    stamp(b, plus, col_a, false);
                    stamp(b, plus, col_b, true);
                    stamp(b, minus, col_a, true);
                    stamp(b, minus, col_b, false);
                }
            }
            Ok(())
        })
    }

    /// DISTO-04 guard shared by the `.disto` kernels: fail loud, naming the
    /// device, when any contribution or temp reads a branch current
    /// `I(...)` — the Volterra bookkeeping is over controlling *voltages*;
    /// a current-controlled nonlinearity has no voltage-pair higher
    /// derivative.
    fn guard_voltage_controlled(
        &self,
        kernel: &str,
        contribs: &[&FlatContrib],
        temps: &[PomExpr],
    ) -> Result<(), CodegenError> {
        let mut reads_i = false;
        let mut scan = |e: &PomExpr| {
            visit_all(e, &mut |node| {
                if let PomExpr::Call(func, _) = node
                    && let PomExpr::Ident(fname) = func.as_ref()
                    && fname == "I"
                {
                    reads_i = true;
                }
            });
        };
        for c in contribs { scan(&c.expr); }
        for t in temps { scan(t); }
        if reads_i {
            return Err(CodegenError::unsupported(format!(
                "{kernel}: device `{}` reads a branch current `I(...)`; \
                 current-controlled nonlinearities have no voltage-pair higher derivative",
                self.module.name
            )));
        }
        Ok(())
    }

    /// The ordered set of V/I branches read anywhere in the contributions
    /// or the temp tape (first-seen order).
    fn contrib_branches(&self, contribs: &[&FlatContrib], temps: &[PomExpr]) -> Vec<(NodeId, NodeId)> {
        let module = self.module;
        let resolve_node = |name: &str| -> Option<NodeId> {
            if piperine_lang::pom::is_ground(name) { return Some(NodeId::GROUND); }
            module.symbols.nodes().find(|(_, info)| info.name == name).map(|(id, _)| id)
        };
        let mut seen = std::collections::HashSet::new();
        let mut branches: Vec<(NodeId, NodeId)> = Vec::new();
        let mut collect = |e: &PomExpr| {
            let mut branch_pairs = Vec::new();
            crate::resolve::diff::collect_branches(e, &mut branch_pairs, &resolve_node);
            for pair in branch_pairs {
                if seen.insert(pair) {
                    branches.push(pair);
                }
            }
        };
        for c in contribs { collect(&c.expr); }
        for t in temps { collect(t); }
        branches
    }

    /// Disto3 shape: one compiled function per ordered branch triple
    /// `(j, k, l)`, each writing `out[ci] = ∂³(contrib_ci)/∂V(j)∂V(k)∂V(l)`
    /// for its own triple over the resistive contributions followed by the
    /// charge ones (DISTO-03). A separate function per triple — rather
    /// than one function unrolling every triple — keeps each Cranelift
    /// function's instruction count bounded (see [`Self::compile_disto2`]).
    ///
    /// Each triple's third derivatives reference seven tapes — the three
    /// branches' first-derivative tapes, the three pairwise cross tapes
    /// (each completing its branch's first-derivative tape with one more
    /// differentiate pass, shared across every third branch), and the
    /// third-derivative tape — all over the shared value tape, never an
    /// inlined tree. Literal-zero rows and empty triples are skipped;
    /// `None` when no contribution has a third derivative (degree ≤ 2).
    #[allow(clippy::type_complexity)]
    fn compile_disto3(
        &mut self,
        name: &str,
        resistive: &[FlatContrib],
        charge: &[FlatContrib],
        temps: &[PomExpr],
    ) -> Result<Option<(Vec<FuncId>, Vec<((NodeId, NodeId), (NodeId, NodeId), (NodeId, NodeId))>)>, CodegenError> {
        let contribs: Vec<&FlatContrib> = resistive.iter().chain(charge).collect();
        if contribs.is_empty() {
            return Ok(None);
        }
        self.guard_voltage_controlled("disto3", &contribs, temps)?;
        let branches = self.contrib_branches(&contribs, temps);
        let module = self.module;
        let resolve_node = |name: &str| -> Option<NodeId> {
            if piperine_lang::pom::is_ground(name) { return Some(NodeId::GROUND); }
            module.symbols.nodes().find(|(_, info)| info.name == name).map(|(id, _)| id)
        };

        // Every tape is built with the marker names of its role
        // (`d_dv_named`/`d_dv_twice_named`), so tape entries only reference
        // markers the emission installs: `__dtemp1/2/3` for the three
        // branches, `__ddtemp12/13/23` for the three pairwise crosses.
        let first_tapes = |marker: &'static str| -> Vec<Vec<PomExpr>> {
            branches
                .iter()
                .map(|&(a, b)| {
                    temps
                        .iter()
                        .map(|t| crate::resolve::diff::d_dv_named(t, a, b, &resolve_node, marker))
                        .collect()
                })
                .collect()
        };
        let dtemps1 = first_tapes("__dtemp1");
        let dtemps2 = first_tapes("__dtemp2");
        let dtemps3 = first_tapes("__dtemp3");
        let cross_tapes = |d1: &'static str, d2: &'static str, d12: &'static str| -> Vec<Vec<Vec<PomExpr>>> {
            branches
                .iter()
                .map(|&(a, b)| {
                    branches
                        .iter()
                        .map(|&(c, d)| {
                            temps
                                .iter()
                                .map(|t| crate::resolve::diff::d_dv_twice_named(t, a, b, c, d, &resolve_node, d1, d2, d12))
                                .collect()
                        })
                        .collect()
                })
                .collect()
        };
        let ddtemps12 = cross_tapes("__dtemp1", "__dtemp2", "__ddtemp12");
        let ddtemps13 = cross_tapes("__dtemp1", "__dtemp3", "__ddtemp13");
        let ddtemps23 = cross_tapes("__dtemp2", "__dtemp3", "__ddtemp23");

        let exprs: Vec<&PomExpr> = contribs.iter().map(|c| &c.expr).collect();
        let mut triples: Vec<((NodeId, NodeId), (NodeId, NodeId), (NodeId, NodeId))> = Vec::new();
        let mut func_ids: Vec<FuncId> = Vec::new();
        for (j_idx, &(a, b)) in branches.iter().enumerate() {
            for (k_idx, &(c, d)) in branches.iter().enumerate() {
                for (l_idx, &(e, f)) in branches.iter().enumerate() {
                    // Cheap check first (a handful of contribs, not the
                    // whole temp tape): skip the triple entirely — and the
                    // expensive per-temp third-derivative tape below — when
                    // every contribution's third derivative is literal zero.
                    let rows: Vec<Option<PomExpr>> = contribs
                        .iter()
                        .map(|contrib| {
                            let row = crate::resolve::diff::d_dv_thrice(&contrib.expr, a, b, c, d, e, f, &resolve_node);
                            match &row {
                                PomExpr::Literal(piperine_lang::parse::ast::Literal::Real(v)) if *v == 0.0 => None,
                                _ => Some(row),
                            }
                        })
                        .collect();
                    if !rows.iter().any(Option::is_some) {
                        continue;
                    }
                    // Completes the already-built second pass
                    // (`ddtemps12[j][k]`, shared across every `l`) with one
                    // more differentiate pass — never redoes the first two.
                    let dddtemps: Vec<PomExpr> = ddtemps12[j_idx][k_idx]
                        .iter()
                        .map(|pass2| crate::resolve::diff::d_dv_thrice_from_twice(pass2, e, f, &resolve_node))
                        .collect();
                    let dtemp1_j = dtemps1[j_idx].clone();
                    let dtemp2_k = dtemps2[k_idx].clone();
                    let dtemp3_l = dtemps3[l_idx].clone();
                    let ddtemp12_jk = ddtemps12[j_idx][k_idx].clone();
                    let ddtemp13_jl = ddtemps13[j_idx][l_idx].clone();
                    let ddtemp23_kl = ddtemps23[k_idx][l_idx].clone();
                    let fn_name = format!("{name}_{j_idx}_{k_idx}_{l_idx}");
                    let func_id = self.build_fn(&fn_name, &exprs, move |b, _slot, out_ptr| {
                        b.set_tape("__dtemp1", dtemp1_j);
                        b.set_tape("__dtemp2", dtemp2_k);
                        b.set_tape("__dtemp3", dtemp3_l);
                        b.set_tape("__ddtemp12", ddtemp12_jk);
                        b.set_tape("__ddtemp13", ddtemp13_jl);
                        b.set_tape("__ddtemp23", ddtemp23_kl);
                        b.set_tape("__dddtemp123", dddtemps);
                        b.force_tapes(&[
                            "__temp", "__dtemp1", "__dtemp2", "__dtemp3",
                            "__ddtemp12", "__ddtemp13", "__ddtemp23", "__dddtemp123",
                        ])?;
                        for (ci, row) in rows.iter().enumerate() {
                            if let Some(e) = row {
                                let value = b.emit_analog(e)?;
                                b.store_f64(value, out_ptr, ci);
                            }
                        }
                        Ok(())
                    })?;
                    triples.push(((a, b), (c, d), (e, f)));
                    func_ids.push(func_id);
                }
            }
        }
        if func_ids.is_empty() {
            return Ok(None);
        }
        Ok(Some((func_ids, triples)))
    }

    /// Disto2 shape: one compiled function per ordered branch pair `(j, k)`,
    /// each writing `out[ci] = ∂²(contrib_ci)/∂V(j)∂V(k)` for its own pair
    /// over the resistive contributions followed by the charge ones
    /// (DISTO-03). A separate function per pair — rather than one function
    /// unrolling every pair — keeps each Cranelift function's instruction
    /// count bounded; a many-branch device (e.g. a MOSFET with several
    /// controlling terminals) unrolled into a single function overwhelmed
    /// Cranelift's own compilation (`define_function`), not the symbolic
    /// differentiation, which stays fast.
    ///
    /// For each branch `k` the first-derivative tape `d(temps)/dV(k)` is
    /// built once; for each ordered pair `(j, k)` the cross tape
    /// `d²(temps)/dV(j)dV(k)` completes the pair's own first pass
    /// (`all_dtemps_inner[j]`, built once per branch) with a single more
    /// differentiate pass rather than redoing it — derivatives reference
    /// the shared value tape, never an inlined tree. Rows that fold to a
    /// literal zero (linear in `(j, k)`) are skipped, as are pairs with no
    /// nonzero row. Returns `None` when every contribution is linear — a
    /// fully linear device carries no `.disto` kernel at all.
    ///
    /// DISTO-04: the Volterra bookkeeping is over controlling *voltages*; a
    /// contribution reading a branch current `I(...)` couples to a current
    /// unknown and has no voltage-pair second derivative — fail loud,
    /// naming the device.
    #[allow(clippy::type_complexity)]
    fn compile_disto2(
        &mut self,
        name: &str,
        resistive: &[FlatContrib],
        charge: &[FlatContrib],
        temps: &[PomExpr],
    ) -> Result<Option<(Vec<FuncId>, Vec<((NodeId, NodeId), (NodeId, NodeId))>)>, CodegenError> {
        let contribs: Vec<&FlatContrib> = resistive.iter().chain(charge).collect();
        if contribs.is_empty() {
            return Ok(None);
        }
        self.guard_voltage_controlled("disto2", &contribs, temps)?;
        let branches = self.contrib_branches(&contribs, temps);
        let module = self.module;
        let resolve_node = |name: &str| -> Option<NodeId> {
            if piperine_lang::pom::is_ground(name) { return Some(NodeId::GROUND); }
            module.symbols.nodes().find(|(_, info)| info.name == name).map(|(id, _)| id)
        };

        // Each role's tape is built with its own marker name so its
        // entries stay self-consistent (`d_dv_named`): the `(c,d)` tape
        // references `__dtemp`, the `(a,b)` tape `__dtemp_inner`.
        let all_dtemps: Vec<Vec<PomExpr>> = branches
            .iter()
            .map(|&(a, b)| {
                temps
                    .iter()
                    .map(|t| crate::resolve::diff::d_dv(t, a, b, &resolve_node))
                    .collect()
            })
            .collect();
        let all_dtemps_inner: Vec<Vec<PomExpr>> = branches
            .iter()
            .map(|&(a, b)| {
                temps
                    .iter()
                    .map(|t| crate::resolve::diff::d_dv_named(t, a, b, &resolve_node, "__dtemp_inner"))
                    .collect()
            })
            .collect();

        let exprs: Vec<&PomExpr> = contribs.iter().map(|c| &c.expr).collect();
        let mut pairs: Vec<((NodeId, NodeId), (NodeId, NodeId))> = Vec::new();
        let mut func_ids: Vec<FuncId> = Vec::new();
        for (k_idx, &(c, d)) in branches.iter().enumerate() {
            for (j_idx, &(a, b)) in branches.iter().enumerate() {
                // Cheap check first (a handful of contribs): skip the pair
                // — and the expensive per-temp second-derivative tape below
                // — when every contribution's second derivative is zero.
                let rows: Vec<Option<PomExpr>> = contribs
                    .iter()
                    .map(|contrib| {
                        let e = crate::resolve::diff::d_dv_twice(&contrib.expr, a, b, c, d, &resolve_node);
                        match &e {
                            PomExpr::Literal(piperine_lang::parse::ast::Literal::Real(v)) if *v == 0.0 => None,
                            _ => Some(e),
                        }
                    })
                    .collect();
                if !rows.iter().any(Option::is_some) {
                    continue;
                }
                // Completes the already-built first pass (`all_dtemps_inner[j]`,
                // shared across every `k`) with one more differentiate
                // pass — never redoes it.
                let ddtemps: Vec<PomExpr> = all_dtemps_inner[j_idx]
                    .iter()
                    .map(|inner| {
                        crate::resolve::diff::d_dv_once_more_named(
                            inner, c, d, &resolve_node, "__dtemp_inner", "__dtemp", "__ddtemp",
                        )
                    })
                    .collect();
                let dtemp_k = all_dtemps[k_idx].clone();
                let dtemp_inner_j = all_dtemps_inner[j_idx].clone();
                let fn_name = format!("{name}_{j_idx}_{k_idx}");
                let func_id = self.build_fn(&fn_name, &exprs, move |b, _slot, out_ptr| {
                    b.set_deriv_tape(dtemp_k);
                    b.set_deriv_tape2(dtemp_inner_j);
                    b.set_ddtemp_tape(ddtemps);
                    b.force_tapes(&["__temp", "__dtemp", "__dtemp_inner", "__ddtemp"])?;
                    for (ci, row) in rows.iter().enumerate() {
                        if let Some(e) = row {
                            let value = b.emit_analog(e)?;
                            b.store_f64(value, out_ptr, ci);
                        }
                    }
                    Ok(())
                })?;
                pairs.push(((a, b), (c, d)));
                func_ids.push(func_id);
            }
        }
        if func_ids.is_empty() {
            return Ok(None);
        }
        Ok(Some((func_ids, pairs)))
    }

    /// Row shape: `out[i] = expr_i`.
    fn compile_rows(&mut self, name: &str, rows: &[PomExpr]) -> Result<FuncId, CodegenError> {
        let exprs: Vec<&PomExpr> = rows.iter().collect();
        self.build_fn(name, &exprs, |b, _slot, out_ptr| {
            for (i, row) in rows.iter().enumerate() {
                let value = b.emit_analog(row)?;
                b.store_f64(value, out_ptr, i);
            }
            Ok(())
        })
    }

    /// Force Jacobian shape: `out[i·n + j] = ∂E_i/∂V_j` per force row and
    /// terminal column.
    fn compile_force_jacobian(&mut self, name: &str, forces: &[FlatForce]) -> Result<FuncId, CodegenError> {
        let n = self.terminals.len();
        let exprs: Vec<&PomExpr> = forces.iter().map(|f| &f.expr).collect();
        let module = self.module;
        let resolve_node = |name: &str| -> Option<NodeId> {
            if piperine_lang::pom::is_ground(name) { return Some(NodeId::GROUND); }
            module.symbols.nodes().find(|(_, info)| info.name == name).map(|(id, _)| id)
        };
        let temps = self.flat.temps.clone();
        let mut seen = std::collections::HashSet::new();
        let mut branches: Vec<(NodeId, NodeId)> = Vec::new();
        let mut collect = |e: &PomExpr| {
            let mut pairs = Vec::new();
            crate::resolve::diff::collect_branches(e, &mut pairs, &resolve_node);
            for pair in pairs {
                if seen.insert(pair) {
                    branches.push(pair);
                }
            }
        };
        for f in forces { collect(&f.expr); }
        for t in &temps { collect(t); }
        self.build_fn(name, &exprs, move |b, slot, out_ptr| {
            for (a, bb) in branches {
                let dtemps: Vec<PomExpr> = temps
                    .iter()
                    .map(|t| crate::resolve::diff::d_dv(t, a, bb, &resolve_node))
                    .collect();
                b.set_deriv_tape(dtemps);
                for (i, force) in forces.iter().enumerate() {
                    let derivative = crate::resolve::diff::d_dv(&force.expr, a, bb, &resolve_node);
                    let g = b.emit_analog(&derivative)?;
                    if let Some(&col) = slot.get(&a) {
                        b.accumulate_f64(g, out_ptr, i * n + col);
                    }
                    if let Some(&col) = slot.get(&bb) {
                        let neg = b.builder.ins().fneg(g);
                        b.accumulate_f64(neg, out_ptr, i * n + col);
                    }
                }
            }
            Ok(())
        })
    }

    /// Build one function with the shared ABI: prepare parameter values and
    /// branch voltages, then let `body` emit the stores.
    fn build_fn(
        &mut self,
        name: &str,
        exprs: &[&PomExpr],
        body: impl FnOnce(&mut Builder, &HashMap<NodeId, usize>, Value) -> Result<(), CodegenError>,
    ) -> Result<FuncId, CodegenError> {
        let ptr_ty = self.jit.target_config().pointer_type();
        let mut sig = Signature::new(self.jit.isa().default_call_conv());
        for _ in 0..6 {
            sig.params.push(AbiParam::new(ptr_ty));
        }

        let func_id = self
            .jit
            .declare_function(name, Linkage::Export, &sig)
            .map_err(|e| CodegenError::Module(e.to_string()))?;

        let mut ctx = self.jit.make_context();
        ctx.func.signature = sig;
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut self.fb_ctx);

        let math: HashMap<&'static str, cranelift_codegen::ir::FuncRef> = self
            .math_ids
            .iter()
            .map(|(&name, &id)| (name, self.jit.declare_func_in_func(id, builder.func)))
            .collect();

        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let volts_ptr = builder.block_params(entry)[0];
        let params_ptr = builder.block_params(entry)[1];
        let state_ptr = builder.block_params(entry)[2];
        let vars_ptr = builder.block_params(entry)[3];
        let sim_ptr = builder.block_params(entry)[4];
        let out_ptr = builder.block_params(entry)[5];

        // Parameter values, indexed by ParamId.
        let params: Vec<Value> = (0..self.module.symbols.num_params())
            .map(|i| {
                builder
                    .ins()
                    .load(types::F64, MemFlags::trusted(), params_ptr, (i * 8) as i32)
            })
            .collect();

        // Branch voltages for every pair read by any expression.
        let module = self.module;
        let resolve_node = |name: &str| -> Option<NodeId> {
            if piperine_lang::pom::is_ground(name) { return Some(NodeId::GROUND); }
            module.symbols.nodes().find(|(_, info)| info.name == name).map(|(id, _)| id)
        };
        // Branches now live inside the temp tape (contributions hold only
        // `__temp` leaves), so scan both.
        let temps = self.flat.temps.clone();
        let mut pairs = Vec::new();
        for expr in exprs {
            crate::resolve::diff::collect_branches(expr, &mut pairs, &resolve_node);
        }
        for temp in &temps {
            crate::resolve::diff::collect_branches(temp, &mut pairs, &resolve_node);
        }
        let mut branch_voltages = HashMap::new();
        for (plus, minus) in pairs {
            let load = |builder: &mut FunctionBuilder, node: NodeId| match self.slot.get(&node) {
                Some(&i) => {
                    builder
                        .ins()
                        .load(types::F64, MemFlags::trusted(), volts_ptr, (i * 8) as i32)
                }
                None => builder.ins().f64const(0.0),
            };
            let vp = load(&mut builder, plus);
            let vm = load(&mut builder, minus);
            let v = builder.ins().fsub(vp, vm);
            branch_voltages.insert((plus, minus), v);
        }

        let resolver = Resolver::from_symbols(&self.module.symbols);
        let mut b = Builder::new_analog(
            &mut builder,
            self.module,
            &resolver,
            &math,
            branch_voltages,
            params,
            state_ptr,
            vars_ptr,
            sim_ptr,
            self.limits.clone(),
            self.limit_base,
        );
        b.set_value_tape(temps);
        body(&mut b, &self.slot, out_ptr)?;

        builder.ins().return_(&[]);
        builder.finalize();

        self.jit
            .define_function(func_id, &mut ctx)
            .map_err(|e| CodegenError::Module(e.to_string()))?;
        Ok(func_id)
    }
}
