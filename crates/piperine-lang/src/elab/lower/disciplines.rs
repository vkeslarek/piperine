//! `ResolveDisciplines` — LRM §7.4/Annex F: assigns a discipline to a
//! port/net declared with no explicit discipline, by propagating the
//! discipline of whatever it connects to.
//!
//! **Domain-aware conflict resolution (LRMC-C10-18/19, design.md P0.3):**
//! when two connected nets carry *different* disciplines, this pass now
//! reads each discipline's declared `domain` (`design.discipline(name)`)
//! before deciding — Annex F §7.4.4.1's own headline rule is that
//! **continuous wins over discrete**, so a domain mismatch is resolved,
//! not an error. A discipline with no declaration in this design (domain
//! unknown) can't take this shortcut, and two disciplines that share a
//! domain but are still genuinely different still hard-fail exactly as
//! before (T8's shared connectrules-eligibility check, not yet built,
//! will add a third option here). A net left unresolved after the
//! fixed-point loop consults an in-force `` `default_discipline ``
//! directive (Annex F step 4.b) before finally being declared unknown.
//!
//! **Annex F method chosen (T26 decision, per the task's own "decided
//! in-task against the LRM's worked examples" instruction): Basic
//! discipline resolution (§7.4.4.1).** The LRM names it as *the default*
//! of the two documented algorithms ("basic (the default) and detail",
//! §7.4.4); its own worked example (§7.4.4.1, Figure 7-2-adjacent text) is
//! the simpler of the two — propagate a neighbour's known discipline
//! across a direct connection — where "detail" additionally reasons about
//! per-net compatible-discipline *sets* before collapsing them. Given a
//! choice between the two correctly-specified methods, Basic is the one
//! this pass implements; Detail is out of scope (documented here, not
//! silently half-done).
//!
//! **Scope note (documented, not silently narrowed):** the full LRM
//! algorithm resolves across the *entire* hierarchy and vendor-specific
//! `connect ... resolveto` statements (§7.4.4.3, ch. 10). This pass
//! resolves within one module at a time — an unresolved port/net takes its
//! discipline from any instance port it's directly connected to whose
//! callee-side discipline is already known (declared, or itself already
//! resolved this round) — iterated to a fixed point. Cross-module
//! propagation through nested unresolved ports (a port that is itself
//! unresolved being connected to another unresolved port) is exactly what
//! the per-module fixed-point loop over the whole design (not just one
//! module) already gives it, since `design.modules` all resolve in the
//! same pass; `connect`/`connectrules`-driven resolution (ch. 7/10) is
//! future work (T52, `InsertConnectModules`).

use std::collections::HashMap;

use crate::pom::{Design, ElabError, ElabErrorKind, Module};

use super::passes::ElabPass;
use super::Elaborator;

/// The sentinel discipline name `lower_port_decl`/net lowering (`vams.rs`)
/// assigns when the source declares none — never a real discipline name
/// (VAMS identifiers can't be empty), so it can't collide with an
/// authored discipline.
pub(super) const UNRESOLVED: &str = "";

pub(super) struct ResolveDisciplines;

impl ElabPass for ResolveDisciplines {
    fn run(&self, _elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError> {
        // Iterate to a fixed point: resolving one net in one module can
        // supply the discipline a connected net in *another* module needs
        // next round (an instance's port, seen from the callee side, is
        // exactly the callee module's own port discipline).
        let names: Vec<String> = design.modules.keys().cloned().collect();
        loop {
            let mut progress = false;
            for name in &names {
                progress |= resolve_one_round(name, design)?;
            }
            if !progress {
                break;
            }
        }

        // Annex F §7.4.4.1 step 4.b: a net that never received a
        // discipline from any connection consults an in-force
        // `` `default_discipline `` directive before being declared
        // unknown (LRMC-C10-19) — applied once, after the fixed point, so
        // it never pre-empts a real connection's discipline.
        if let Some(default) = design.directives().default_discipline.clone() {
            for name in &names {
                let m = design.modules.get_mut(name).unwrap();
                for p in &mut m.ports {
                    if p.ty.discipline_name() == UNRESOLVED {
                        p.ty = crate::pom::NetType::Discipline(default.clone());
                    }
                }
                for w in &mut m.wires {
                    if w.ty.discipline_name() == UNRESOLVED {
                        w.ty = crate::pom::NetType::Discipline(default.clone());
                    }
                }
            }
        }

        // Anything still unresolved (no connection supplied a discipline,
        // and no `default_discipline` was in force) has no way to get
        // one — fail loud naming the method and the reason (E4003).
        for name in &names {
            let m = &design.modules[name];
            for p in &m.ports {
                if p.ty.discipline_name() == UNRESOLVED {
                    return Err(unresolved_err(&p.name, name));
                }
            }
            for w in &m.wires {
                if w.ty.discipline_name() == UNRESOLVED {
                    return Err(unresolved_err(&w.name, name));
                }
            }
        }
        Ok(())
    }
}

fn unresolved_err(net: &str, module: &str) -> ElabError {
    ElabError::from(ElabErrorKind::DisciplineResolutionFailed {
        net: net.to_string(),
        module: module.to_string(),
        method: "basic discipline resolution (LRM §7.4.4.1)".to_string(),
        reason: "no neighbouring net carries a discipline".to_string(),
        span: None,
    })
}

/// One propagation round over `module`: for every instance, compare each
/// connected net against the callee module's own port discipline at that
/// position — if the local net is unresolved and the callee side is
/// known, adopt it; if the local net is already resolved to something
/// else, that's a conflict (both directions are checked, since either
/// side could be the unresolved one). Returns whether anything changed.
fn resolve_one_round(name: &str, design: &mut Design) -> Result<bool, ElabError> {
    let module = design.modules.get(name).cloned().ok_or_else(|| {
        ElabError::from(ElabErrorKind::Other(format!("ResolveDisciplines: unknown module `{name}`")))
    })?;

    let mut resolved: HashMap<String, String> = HashMap::new();
    for inst in &module.instances {
        let Some(callee) = design.modules.get(&inst.module) else { continue };
        for (i, port_ref) in inst.ports.iter().enumerate() {
            let Some(callee_port) = callee.ports.get(i) else { continue };
            let callee_disc = callee_port.ty.discipline_name();
            if callee_disc == UNRESOLVED {
                continue;
            }
            let local_disc = local_net_discipline(&module, port_ref.net());
            match local_disc.as_deref() {
                Some(d) if d == UNRESOLVED => {
                    resolved.insert(port_ref.net().to_string(), callee_disc.to_string());
                }
                Some(d) if d != callee_disc => {
                    // Annex F §7.4.4.1's headline rule: when the two
                    // discipline *domains* are known and differ,
                    // continuous wins over discrete — this is Basic
                    // resolution's own conflict-avoidance rule, not a
                    // genuine incompatibility (LRMC-C10-18). Domain is
                    // read from each discipline's own declaration
                    // (`design.discipline(name)`); a discipline with no
                    // declaration in this design (domain unknown) can't
                    // participate in this shortcut and falls through to
                    // the existing hard failure below, same as a
                    // genuinely same-domain-incompatible pair (spec Edge
                    // Case: the fix narrows the false-positive case, it
                    // does not loosen real incompatibility detection).
                    let d_domain = design.discipline(d).and_then(|dd| dd.domain);
                    let callee_domain = design.discipline(callee_disc).and_then(|dd| dd.domain);
                    use crate::ast::Domain;
                    match (d_domain, callee_domain) {
                        (Some(Domain::Continuous), Some(Domain::Discrete)) => {
                            // `d` (the local net's current discipline) is
                            // already continuous — nothing to change.
                        }
                        (Some(Domain::Discrete), Some(Domain::Continuous)) => {
                            resolved.insert(port_ref.net().to_string(), callee_disc.to_string());
                        }
                        _ => {
                            // Same domain (different names) — T8's shared
                            // connectrules-eligibility check isn't built
                            // yet (design.md P0.3 depends-on note); until
                            // then this is the same hard failure as
                            // before.
                            return Err(ElabError::from(ElabErrorKind::DisciplineResolutionFailed {
                                net: port_ref.net().to_string(),
                                module: name.to_string(),
                                method: "basic discipline resolution (LRM §7.4.4.1)".to_string(),
                                reason: format!(
                                    "incompatible disciplines on drivers and receivers — `{d}` vs `{callee_disc}` (from instance `{}`)",
                                    inst.name()
                                ),
                                span: None,
                            }));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    if resolved.is_empty() {
        return Ok(false);
    }
    let m = design.modules.get_mut(name).unwrap();
    for p in &mut m.ports {
        if let Some(d) = resolved.get(&p.name) {
            p.ty = crate::pom::NetType::Discipline(d.clone());
        }
    }
    for w in &mut m.wires {
        if let Some(d) = resolved.get(&w.name) {
            w.ty = crate::pom::NetType::Discipline(d.clone());
        }
    }
    Ok(true)
}

/// The current discipline of a port or wire named `net` in `module`,
/// `None` if `net` names neither (e.g. it's a port of a *different*
/// module reached through the connection — nothing to update locally).
/// Owned (not borrowed): the caller holds a separate clone of `module`
/// from the one about to be mutated in `design.modules`, so there is no
/// live borrow to extend — a plain `String` is simplest.
fn local_net_discipline(module: &Module, net: &str) -> Option<String> {
    module
        .ports
        .iter()
        .find(|p| p.name == net)
        .map(|p| p.ty.discipline_name().to_string())
        .or_else(|| module.wires.iter().find(|w| w.name == net).map(|w| w.ty.discipline_name().to_string()))
}
