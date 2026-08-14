//! `InsertConnectModules` — LRM chapter 7: automatic connect-module
//! selection at a discipline-domain crossing (T52, VAMS-20).
//!
//! **Selection algorithm (LRM ch. 7 / §7.4.4.3), implemented and
//! independently tested here:** a discipline pair `(A, B)` is eligible
//! for automatic bridging only if some registered `connectrules` block
//! declares a `connect` rule naming both `A` and `B` (`elab.syms.
//! connectrules`, registered by `register.rs` starting this task). Given
//! eligibility, the unique `connectmodule`-kind declaration whose two
//! ports' disciplines are exactly `{A, B}` (either order) is selected;
//! zero or more-than-one candidate fails loud (E4005
//! `ConnectModuleAmbiguous`), naming every candidate found in the `issue`
//! text (E-STD 3).
//!
//! **SPEC_DEVIATION (documented, carried forward — found while
//! implementing, not assumed away):** `ResolveDisciplines` (T26,
//! `disciplines.rs::resolve_one_round`) already hard-fails
//! (`DisciplineResolutionFailed`, E4003) the instant it sees an instance
//! port connected to a net whose *already-resolved* discipline differs
//! from the port's declared discipline — see that function's own
//! `Some(d) if d != callee_disc => Err(...)` arm
//! (`disciplines.rs:122-131`). Since `InsertConnectModules` runs *after*
//! `ResolveDisciplines` in `PASSES` (per this task's own stated position,
//! "after `ResolveDisciplines`, before `FlattenHierarchy`"), a genuine
//! cross-discipline connection in a real `.vams` source **never survives
//! to reach this pass** — elaboration already aborted one pass earlier.
//! Closing this gap for real needs `ResolveDisciplines` itself to treat a
//! connectrules-eligible mismatch as "defer to `InsertConnectModules`"
//! rather than an unconditional conflict — a coupled change to
//! `disciplines.rs`, outside this task's stated `Where`
//! (`elab/lower/connect.rs`, `passes.rs`, the test file). Carried forward
//! as documented debt, exactly as `disciplines.rs`'s own module doc
//! already flagged: "`connect`/`connectrules`-driven resolution (ch.
//! 7/10) is future work (T52, `InsertConnectModules`)" — T52, on
//! inspection, still needs that follow-up to close the loop. What this
//! pass delivers today: the selection algorithm (independently proven
//! correct by unit tests that construct a `Design`/`Elaborator` directly,
//! bypassing `ResolveDisciplines` entirely — the same technique
//! `flatten.rs`'s own tests use for derived-artifact assertions), wired
//! into `PASSES` at the stated position, and provably a no-op (zero
//! regressions) over every design in today's corpus, since none of them
//! can reach this pass with a live mismatch for it to act on.

use piperine_vams::ast::ModuleKind;

use crate::pom::{Design, ElabError, ElabErrorKind, Instance};

use super::passes::ElabPass;
use super::Elaborator;

pub(super) struct InsertConnectModules;

impl ElabPass for InsertConnectModules {
    fn run(&self, elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError> {
        let names: Vec<String> = design.modules.keys().cloned().collect();
        for name in &names {
            let module = design.modules[name].clone();
            let mut inserted = Vec::new();
            for inst in &module.instances {
                let Some(callee) = design.modules.get(&inst.module) else { continue };
                for (i, port_ref) in inst.ports.iter().enumerate() {
                    let Some(callee_port) = callee.ports.get(i) else { continue };
                    let callee_disc = callee_port.ty.discipline_name();
                    let Some(local_disc) = local_net_discipline(&module, port_ref.net()) else { continue };
                    // Same discipline (the overwhelming common case) or
                    // either side genuinely undeclared (Annex F/T26's own
                    // job, not ours) — nothing to bridge.
                    if local_disc == callee_disc || local_disc.is_empty() || callee_disc.is_empty() {
                        continue;
                    }
                    let target = select_connect_module(elab, design, &local_disc, callee_disc)?;
                    inserted.push(Instance {
                        span: None,
                        label_span: None,
                        type_span: None,
                        attributes: vec![],
                        doc: None,
                        label: Some(format!("__connect_{}_{}", inst.name(), port_ref.net())),
                        module: target,
                        ports: vec![port_ref.clone(), port_ref.clone()],
                        params: vec![],
                    });
                }
            }
            if !inserted.is_empty() {
                // MD-25: appended to the *derived* instance list, never to
                // `design.modules` — the same field `UnrollGenerate` (T25)
                // writes, folded into `Design::flat_modules` only by
                // `FlattenHierarchy`, never visible from `Design::modules`.
                elab.generated_instances.entry(name.clone()).or_default().extend(inserted);
            }
        }
        Ok(())
    }
}

/// The current discipline of a port or wire named `net` in `module`
/// (mirrors `disciplines.rs::local_net_discipline` — duplicated rather
/// than shared because that function is `disciplines.rs`-private and this
/// pass's scope note above already explains the two passes are
/// deliberately not coupled further).
fn local_net_discipline(module: &crate::pom::Module, net: &str) -> Option<String> {
    module
        .ports
        .iter()
        .find(|p| p.name == net)
        .map(|p| p.ty.discipline_name().to_string())
        .or_else(|| module.wires.iter().find(|w| w.name == net).map(|w| w.ty.discipline_name().to_string()))
}

/// LRM ch. 7 connect-module selection for the discipline pair
/// `(from_discipline, to_discipline)`. Eligibility requires a registered
/// `connectrules` block naming both disciplines in one `connect` rule;
/// given eligibility, the unique declared `connectmodule` whose two ports'
/// disciplines match the pair (either order) is returned. Fails loud
/// (E4005) on zero or multiple candidates, naming every candidate found.
pub(super) fn select_connect_module(
    elab: &Elaborator,
    design: &Design,
    from_discipline: &str,
    to_discipline: &str,
) -> Result<String, ElabError> {
    let eligible = elab.syms.connectrules.values().flat_map(|cr| &cr.rules).any(|rule| {
        rule.disciplines.iter().any(|d| d == from_discipline)
            && rule.disciplines.iter().any(|d| d == to_discipline)
    });
    if !eligible {
        return Err(ambiguous_err(
            from_discipline,
            to_discipline,
            "matched no `connectrules` rule naming both disciplines".to_string(),
        ));
    }

    let mut candidates: Vec<String> = Vec::new();
    for (mod_name, decl) in &elab.syms.module_decls {
        if decl.kind != ModuleKind::Connectmodule {
            continue;
        }
        let Some(target) = design.modules.get(mod_name) else { continue };
        if target.ports.len() != 2 {
            continue;
        }
        let a = target.ports[0].ty.discipline_name();
        let b = target.ports[1].ty.discipline_name();
        let matches_pair = (a == from_discipline && b == to_discipline) || (a == to_discipline && b == from_discipline);
        if matches_pair {
            candidates.push(mod_name.clone());
        }
    }
    candidates.sort();

    match candidates.len() {
        1 => Ok(candidates.into_iter().next().unwrap()),
        0 => Err(ambiguous_err(
            from_discipline,
            to_discipline,
            "matched no declared `connectmodule` with two ports of these disciplines".to_string(),
        )),
        n => Err(ambiguous_err(
            from_discipline,
            to_discipline,
            format!("is ambiguous — {n} candidates matched: {}", candidates.join(", ")),
        )),
    }
}

fn ambiguous_err(from_discipline: &str, to_discipline: &str, issue: String) -> ElabError {
    ElabError::from(ElabErrorKind::ConnectModuleAmbiguous {
        from_discipline: from_discipline.to_string(),
        to_discipline: to_discipline.to_string(),
        issue,
        from_span: None,
        to_span: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use piperine_vams::ast::{ConnectRule, ConnectrulesDecl, ModuleDecl, ModuleKind, Span};
    use crate::pom::{Direction, Module, NetRef, NetType, Port};

    fn span() -> Span {
        Span::new(0, 0)
    }

    fn port(name: &str, discipline: &str) -> Port {
        Port {
            span: None,
            attributes: vec![],
            doc: None,
            direction: Direction::Inout,
            name: name.to_string(),
            ty: NetType::Discipline(discipline.to_string()),
        }
    }

    fn empty_module_decl(name: &str, kind: ModuleKind) -> ModuleDecl {
        ModuleDecl { span: span(), attrs: vec![], kind, name: name.to_string(), param_ports: vec![], ports: None, items: vec![] }
    }

    fn plain_module(name: &str, ports: Vec<Port>) -> Module {
        Module::new(name.to_string(), ports, vec![], vec![], vec![], vec![], vec![])
    }

    fn connectrules_naming(disciplines: &[&str]) -> HashMap<String, ConnectrulesDecl> {
        let mut m = HashMap::new();
        m.insert(
            "rules".to_string(),
            ConnectrulesDecl {
                span: span(),
                name: "rules".to_string(),
                rules: vec![ConnectRule {
                    span: span(),
                    disciplines: disciplines.iter().map(|s| s.to_string()).collect(),
                    body: vec![],
                }],
            },
        );
        m
    }

    fn elab_with(module_decls: Vec<(&str, ModuleKind)>, connectrules: HashMap<String, ConnectrulesDecl>) -> Elaborator {
        let mut elab = Elaborator::new();
        for (name, kind) in module_decls {
            elab.syms.module_decls.insert(name.to_string(), empty_module_decl(name, kind));
        }
        elab.syms.connectrules = connectrules;
        elab
    }

    #[test]
    fn unique_candidate_selects_that_connectmodule() {
        let elab = elab_with(
            vec![("bridge", ModuleKind::Connectmodule)],
            connectrules_naming(&["logic", "electrical"]),
        );
        let mut design = Design::new();
        design.modules_map_mut().insert(
            "bridge".to_string(),
            plain_module("bridge", vec![port("a", "logic"), port("b", "electrical")]),
        );
        let selected = select_connect_module(&elab, &design, "logic", "electrical").unwrap();
        assert_eq!(selected, "bridge");
        // Order-independence: the reverse pair must select the same module.
        let selected_rev = select_connect_module(&elab, &design, "electrical", "logic").unwrap();
        assert_eq!(selected_rev, "bridge");
    }

    #[test]
    fn multiple_candidates_fail_loud_listing_every_candidate() {
        let elab = elab_with(
            vec![("bridge_a", ModuleKind::Connectmodule), ("bridge_b", ModuleKind::Connectmodule)],
            connectrules_naming(&["logic", "electrical"]),
        );
        let mut design = Design::new();
        design.modules_map_mut().insert(
            "bridge_a".to_string(),
            plain_module("bridge_a", vec![port("a", "logic"), port("b", "electrical")]),
        );
        design.modules_map_mut().insert(
            "bridge_b".to_string(),
            plain_module("bridge_b", vec![port("a", "electrical"), port("b", "logic")]),
        );
        let err = select_connect_module(&elab, &design, "logic", "electrical").unwrap_err();
        let ElabErrorKind::ConnectModuleAmbiguous { issue, .. } = err.kind else {
            panic!("expected ConnectModuleAmbiguous, got {err:?}");
        };
        assert!(issue.contains("bridge_a"), "{issue}");
        assert!(issue.contains("bridge_b"), "{issue}");
        assert!(issue.contains('2'), "{issue}");
    }

    #[test]
    fn no_matching_connectmodule_fails_loud() {
        let elab = elab_with(vec![], connectrules_naming(&["logic", "electrical"]));
        let design = Design::new();
        let err = select_connect_module(&elab, &design, "logic", "electrical").unwrap_err();
        assert!(matches!(err.kind, ElabErrorKind::ConnectModuleAmbiguous { .. }));
    }

    #[test]
    fn pair_with_no_connectrules_entry_is_not_eligible() {
        let elab = elab_with(vec![("bridge", ModuleKind::Connectmodule)], HashMap::new());
        let mut design = Design::new();
        design.modules_map_mut().insert(
            "bridge".to_string(),
            plain_module("bridge", vec![port("a", "logic"), port("b", "electrical")]),
        );
        let err = select_connect_module(&elab, &design, "logic", "electrical").unwrap_err();
        let ElabErrorKind::ConnectModuleAmbiguous { issue, .. } = err.kind else {
            panic!("expected ConnectModuleAmbiguous, got {err:?}");
        };
        assert!(issue.contains("no `connectrules` rule"), "{issue}");
    }

    #[test]
    fn ordinary_module_kind_is_never_a_candidate() {
        // A plain `module` (not `connectmodule`) with matching port
        // disciplines must never be selected, even if it would otherwise
        // match — LRM ch. 7 requires the `connectmodule` keyword.
        let elab = elab_with(vec![("not_a_bridge", ModuleKind::Module)], connectrules_naming(&["logic", "electrical"]));
        let mut design = Design::new();
        design.modules_map_mut().insert(
            "not_a_bridge".to_string(),
            plain_module("not_a_bridge", vec![port("a", "logic"), port("b", "electrical")]),
        );
        let err = select_connect_module(&elab, &design, "logic", "electrical").unwrap_err();
        assert!(matches!(err.kind, ElabErrorKind::ConnectModuleAmbiguous { .. }));
    }

    /// MD-25 + the pass's own scan: a genuine mismatch (constructed
    /// directly, bypassing `ResolveDisciplines` — see this module's own
    /// scope-note doc comment for why that pass would otherwise veto the
    /// scenario first) produces a bridge instance that lands only in
    /// `Elaborator::generated_instances`, never in `Design::modules`.
    #[test]
    fn matched_bridge_lands_only_in_generated_instances_not_design_modules() {
        let mut elab = elab_with(
            vec![("bridge", ModuleKind::Connectmodule), ("leaf", ModuleKind::Module)],
            connectrules_naming(&["logic", "electrical"]),
        );
        let mut design = Design::new();
        design.modules_map_mut().insert(
            "bridge".to_string(),
            plain_module("bridge", vec![port("a", "logic"), port("b", "electrical")]),
        );
        design.modules_map_mut().insert("leaf".to_string(), plain_module("leaf", vec![port("p", "logic")]));
        let mut top = plain_module("top", vec![]);
        top.wires.push(crate::pom::Wire {
            span: None,
            attributes: vec![],
            doc: None,
            name: "n".to_string(),
            ty: NetType::Discipline("electrical".to_string()),
        });
        top.instances.push(Instance {
            span: None,
            label_span: None,
            type_span: None,
            attributes: vec![],
            doc: None,
            label: Some("u1".to_string()),
            module: "leaf".to_string(),
            ports: vec![NetRef::simple("n")],
            params: vec![],
        });
        design.modules_map_mut().insert("top".to_string(), top);

        let design_before_len: usize = design.modules().map(|m| m.instances.len()).sum();

        InsertConnectModules.run(&mut elab, &mut design).unwrap();

        // `Design::modules` is completely unchanged — still exactly the
        // one authored instance (`u1`), never the connect bridge.
        let design_after_len: usize = design.modules().map(|m| m.instances.len()).sum();
        assert_eq!(design_before_len, design_after_len, "Design::modules must be untouched (MD-25)");
        let top_after = design.modules.get("top").unwrap();
        assert_eq!(top_after.instances.len(), 1);
        assert_eq!(top_after.instances[0].name(), "u1");

        // The bridge instance lands in the derived-only bookkeeping.
        let generated = elab.generated_instances.get("top").expect("a bridge was generated for `top`");
        assert_eq!(generated.len(), 1);
        assert_eq!(generated[0].module, "bridge");
    }

    #[test]
    fn matching_disciplines_produce_no_bridge() {
        let mut elab = elab_with(
            vec![("leaf", ModuleKind::Module)],
            connectrules_naming(&["electrical", "electrical"]),
        );
        let mut design = Design::new();
        design.modules_map_mut().insert("leaf".to_string(), plain_module("leaf", vec![port("p", "electrical")]));
        let mut top = plain_module("top", vec![]);
        top.wires.push(crate::pom::Wire {
            span: None,
            attributes: vec![],
            doc: None,
            name: "n".to_string(),
            ty: NetType::Discipline("electrical".to_string()),
        });
        top.instances.push(Instance {
            span: None,
            label_span: None,
            type_span: None,
            attributes: vec![],
            doc: None,
            label: Some("u1".to_string()),
            module: "leaf".to_string(),
            ports: vec![NetRef::simple("n")],
            params: vec![],
        });
        design.modules_map_mut().insert("top".to_string(), top);

        InsertConnectModules.run(&mut elab, &mut design).unwrap();
        assert!(elab.generated_instances.get("top").is_none(), "no discipline crossing => no bridge inserted");
    }
}
