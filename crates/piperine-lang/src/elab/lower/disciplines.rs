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
//! **Both Annex F methods implemented (T26 then T60): Basic (§7.4.4.1,
//! the LRM's own stated default) and Detail (§7.4.4.2).** "The selection
//! of these discipline resolution modes shall be vendor-specific" (§7.4.4)
//! — Piperine exposes the choice as [`super::super::registry::
//! ElabContext::discipline_resolution_mode`] (seeded before elaboration,
//! `Basic` by default), not new grammar, since no VAMS Annex A production
//! picks it (C7). The two modes share every rule in
//! [`resolve_one_round`] below except one: **Detail skips the `connect
//! ... resolveto` branch entirely** for ordinary conflict resolution —
//! §7.4.4.2's own words, "the specified `resolveto` connect statements
//! are ignored in this mode unless coercion ... is used" — falling
//! straight through to the domain-based continuous-wins /
//! nature-compatibility / connectrules-bridge chain. Basic mode is
//! unaffected: its own worked example (§7.4.4.1) still resolves discrete
//! conflicts via `resolveto` exactly as before.
//!
//! **Scope note (documented, not silently narrowed):** the full LRM
//! algorithm resolves across the *entire* hierarchy. This pass resolves
//! within one module at a time — an unresolved port/net takes its
//! discipline from any instance port it's directly connected to whose
//! callee-side discipline is already known (declared, or itself already
//! resolved this round) — iterated to a fixed point. Cross-module
//! propagation through nested unresolved ports (a port that is itself
//! unresolved being connected to another unresolved port) is exactly what
//! the per-module fixed-point loop over the whole design (not just one
//! module) already gives it, since `design.modules` all resolve in the
//! same pass. `connect ... resolveto` (§7.4.4.3, T33/LRMC-C7-04/C10-20) is
//! implemented — [`super::connect::resolve_via_resolveto`] takes priority
//! over the domain-based (continuous-wins) conflict rule right below it,
//! exactly the LRM's own "shall be used unless the `resolveto` connect
//! statement overrides" ordering.

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
    fn run(&self, elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError> {
        // Iterate to a fixed point: resolving one net in one module can
        // supply the discipline a connected net in *another* module needs
        // next round (an instance's port, seen from the callee side, is
        // exactly the callee module's own port discipline).
        let names: Vec<String> = design.modules.keys().cloned().collect();
        loop {
            let mut progress = false;
            for name in &names {
                progress |= resolve_one_round(elab, name, design)?;
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
                    return Err(unresolved_err(elab, &p.name, name));
                }
            }
            for w in &m.wires {
                if w.ty.discipline_name() == UNRESOLVED {
                    return Err(unresolved_err(elab, &w.name, name));
                }
            }
        }
        Ok(())
    }
}

/// The Annex F method name for the error text — reflects whichever mode
/// ([`crate::elab::registry::DisciplineResolutionMode`]) is actually in
/// force for this elaboration run (T60), not hard-coded to Basic.
fn method_name(elab: &Elaborator) -> &'static str {
    match elab.ctx.discipline_resolution_mode {
        crate::elab::registry::DisciplineResolutionMode::Basic => "basic discipline resolution (LRM §7.4.4.1)",
        crate::elab::registry::DisciplineResolutionMode::Detail => "detail discipline resolution (LRM §7.4.4.2)",
    }
}

fn unresolved_err(elab: &Elaborator, net: &str, module: &str) -> ElabError {
    ElabError::from(ElabErrorKind::DisciplineResolutionFailed {
        net: net.to_string(),
        module: module.to_string(),
        method: method_name(elab).to_string(),
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
fn resolve_one_round(elab: &Elaborator, name: &str, design: &mut Design) -> Result<bool, ElabError> {
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
                    // §7.4.4.3 (LRMC-C7-04/C10-20, T33): an explicit
                    // `connect ... resolveto` naming this pair overrides
                    // every other rule below, coercion included — "if an
                    // interconnect is assigned a discipline, that
                    // discipline shall be used unless the `resolveto`
                    // connect statement overrides the discipline."
                    // `exclude` means the pair is forbidden outright.
                    //
                    // §7.4.4.2 (T60): Detail mode ignores `resolveto` for
                    // this ordinary conflict-resolution path — "the
                    // specified resolveto connect statements are ignored
                    // in this mode unless coercion ... is used" — so this
                    // branch is skipped outright in Detail, falling
                    // through to the domain-based rules below.
                    let resolveto_applies =
                        elab.ctx.discipline_resolution_mode != crate::elab::registry::DisciplineResolutionMode::Detail;
                    if let Some(target) =
                        resolveto_applies.then(|| super::connect::resolve_via_resolveto(elab, d, callee_disc)).flatten()
                    {
                        match target {
                            crate::ast::ConnectResolutionTarget::Discipline(forced) => {
                                // No-op guard: once the net already carries
                                // the forced discipline, a *different*
                                // connected port whose own discipline is
                                // fixed to something else would otherwise
                                // re-trigger this same conflict branch
                                // every round forever (the fixed-point loop
                                // never converges) — the forced value
                                // itself is the fixed point, not a
                                // candidate for further resolution.
                                if d != forced {
                                    resolved.insert(port_ref.net().to_string(), forced);
                                }
                            }
                            crate::ast::ConnectResolutionTarget::Exclude => {
                                return Err(ElabError::from(ElabErrorKind::DisciplineResolutionFailed {
                                    net: port_ref.net().to_string(),
                                    module: name.to_string(),
                                    method: "connect ... resolveto exclude (LRM §7.7.2)".to_string(),
                                    reason: format!(
                                        "disciplines `{d}` and `{callee_disc}` are explicitly excluded from resolving together"
                                    ),
                                    span: None,
                                }));
                            }
                        }
                        continue;
                    }
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
                        _ if super::connect::eligible_bridge(elab, d, callee_disc).is_some() => {
                            // A registered `connectrules` block covers
                            // this exact pair (T8, LRMC-C7-01) — defer to
                            // `InsertConnectModules` instead of vetoing
                            // the connection here. The net keeps its
                            // current discipline `d` unchanged; both sides
                            // still carry their real, different
                            // disciplines when `InsertConnectModules` runs
                            // next, which is exactly what connect-module
                            // selection needs to act on.
                        }
                        _ if disciplines_nature_compatible(design, d, callee_disc) => {
                            // §3.11.1 (LRMW2-24, T30): two same-domain
                            // disciplines with different *names* are still
                            // compatible when their bound natures are —
                            // the Base/Derived Nature and Units Value
                            // rules, not name equality. The LRM's own
                            // example: `electrical` (potential `Voltage`)
                            // connects to `highvolt` (potential
                            // `highvoltage`, `nature highvoltage :
                            // Voltage;`) via the Base Nature Rule. Neither
                            // side is rewritten — both keep their own,
                            // genuinely-compatible discipline.
                        }
                        _ => {
                            // Same domain (different names) with no
                            // eligible connectrules pair, or domain
                            // unknown for either side — still the same
                            // hard failure as before (spec Edge Case: two
                            // genuinely incompatible disciplines must
                            // still hard-fail).
                            return Err(ElabError::from(ElabErrorKind::DisciplineResolutionFailed {
                                net: port_ref.net().to_string(),
                                module: name.to_string(),
                                method: method_name(elab).to_string(),
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

/// §3.11.1 discipline compatibility, by nature derivation family — not
/// name equality (LRMW2-24, T30). Both `d1`/`d2` are already known to
/// share a domain (or have one/both unknown) by the time this is called
/// — the caller's domain-mismatch arms run first. Unknown discipline
/// names (no declaration in this design) can't be reasoned about and are
/// treated as incompatible, same as before this task (spec Edge Case: the
/// fix narrows the false-positive case, it never loosens genuine
/// incompatibility).
pub(super) fn disciplines_nature_compatible(design: &Design, d1: &str, d2: &str) -> bool {
    if d1 == d2 {
        return true; // Self Rule (Discipline).
    }
    let (Some(disc1), Some(disc2)) = (design.discipline(d1), design.discipline(d2)) else {
        return false;
    };
    // Domainless Discipline Rule: a discipline with no domain attribute
    // is compatible with all disciplines (no nature or domain conflict).
    if disc1.domain.is_none() || disc2.domain.is_none() {
        return true;
    }
    // Domain Incompatibility Rule: disciplines with different domain
    // attributes are incompatible — checked explicitly here because this
    // function is also called from `InsertConnectModules` (T30), whose
    // caller does *not* pre-establish same-domain the way
    // `ResolveDisciplines`'s own hard-fail fallback does.
    if disc1.domain != disc2.domain {
        return false;
    }
    // Natureless Discipline Rule: a discipline binding neither a
    // potential nor a flow nature is compatible with all other
    // disciplines of the same domain (domain equality just established
    // above).
    if (disc1.potential.is_none() && disc1.flow.is_none()) || (disc2.potential.is_none() && disc2.flow.is_none()) {
        return true;
    }
    // Potential Incompatibility Rule + Flow Incompatibility Rule: both
    // the bound potential natures and the bound flow natures must be
    // nature-compatible.
    natures_compatible(design, disc1.potential.as_deref(), disc2.potential.as_deref())
        && natures_compatible(design, disc1.flow.as_deref(), disc2.flow.as_deref())
}

/// §3.11.1 nature compatibility: Self Rule, Non-Existent Binding Rule,
/// Base Nature Rule (a derived nature is compatible with its base), the
/// Derived Nature Rule (two natures derived from the same base are
/// compatible with each other), and the Units Value Rule (two natures
/// with the same declared `units` are compatible regardless of lineage).
fn natures_compatible(design: &Design, a: Option<&str>, b: Option<&str>) -> bool {
    let (Some(a), Some(b)) = (a, b) else {
        return true; // Non-Existent Binding Rule.
    };
    if a == b {
        return true; // Self Rule (Nature).
    }
    let (Some(na), Some(nb)) = (design.nature(a), design.nature(b)) else {
        return false;
    };
    // Base Nature Rule + Derived Nature Rule: compatible if either one's
    // ancestor chain (inclusive of itself) contains the other's name —
    // covers both "derived compatible with its own base" and "two
    // natures derived from the same base" (the shared-base name appears
    // in both chains).
    let chain_a = nature_ancestor_chain(design, a);
    let chain_b = nature_ancestor_chain(design, b);
    if chain_a.iter().any(|n| chain_b.contains(n)) {
        return true;
    }
    // Units Value Rule: same declared `units`, regardless of lineage.
    matches!((&na.units, &nb.units), (Some(ua), Some(ub)) if ua == ub)
}

/// `name` and every ancestor reached by following `Nature::parent`,
/// nearest-first. Cycle-safe (a self-referential or looping `parent`
/// chain in the design would otherwise spin forever) via a visited set.
fn nature_ancestor_chain(design: &Design, name: &str) -> Vec<String> {
    let mut chain = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut current = Some(name.to_string());
    while let Some(n) = current {
        if !seen.insert(n.clone()) {
            break;
        }
        chain.push(n.clone());
        current = design.nature(&n).and_then(|nat| nat.parent.clone());
    }
    chain
}
