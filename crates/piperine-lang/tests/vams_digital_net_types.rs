//! LRM-COVERS: 3.7, 7.3.1
//!
//! LRMW2-45 (`lrm-conformance-wave2`, T52): `reg`/`wire`/`tri`/`wreal` are
//! IEEE 1364-2005 net-type keywords VAMS carries forward (§3.7), not
//! discipline names — `elab/lower/vams.rs::declarator_net_type` used to
//! build `NetType::Discipline(name)` for all four regardless. Since
//! `resolve/pom/structure.rs::domain_of` treats an *unresolvable*
//! discipline name as the conservative default (`Domain::Analog`, no
//! error — an unknown discipline is never validated at elaboration, only
//! silently defaulted at codegen time), the practical pre-T52 bug was a
//! **silent misclassification** specific to `wire`/`tri`/`reg`: a
//! `wire in;` read from a continuous context (§7.3.1 Table 7-1's own
//! `onebit_dac` worked example, `if (in == 0) ...`) landed in the
//! *analog* domain by the same accident, so it never routed through the
//! digital-to-analog shadow-variable bridge that comparison needs — worse
//! than a loud error. `wreal` happened to land in the *correct* domain
//! (`Analog` — §3.7 gives it real-valued, non-4-state semantics, which
//! the analog domain's own plain-`f64` representation already matches) by
//! the same accidental fallback, but its classification was never
//! *intentional*, only a side effect of "unresolvable discipline defaults
//! to analog" — fragile, and indistinguishable from a genuine typo'd
//! discipline name. This task makes every one of the four keywords'
//! domain an explicit, named classification (`NetType::Digital1364`) that
//! no longer depends on that fallback at all. This file covers the
//! language/POM layer (correct `NetType` classification, all four
//! keywords); the *simulated* proofs are root `tests/onebit_dac_e2e.rs`
//! (`wire`/`reg`, Table 7-1) and the pre-existing root `tests/
//! vams_continuous_assign_e2e.rs::wreal_target_is_driven_by_assign_end_
//! to_end` (`wreal`, unaffected by this task).

use piperine_lang::pom::{DigitalNetKind, NetType};
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

/// §3.7's own `drv`/`top` worked example (`wreal in;` re-declaring a port,
/// `wreal wrstim;` as a plain wire): both `wreal` sites resolve to
/// `NetType::Digital1364(Wreal)` — before this fix, both would have
/// carried `NetType::Discipline("wreal")` and failed elaboration outright
/// with the "unknown discipline" error this test's second half proves is
/// gone.
#[test]
fn wreal_parses_as_a_net_type_not_a_discipline() {
    let d = elab(
        r#"
        module drv(in, out);
            input in;
            output out;
            wreal in;
            electrical out;
            analog V(out) <+ in;
        endmodule
        module top();
            real stim;
            electrical load;
            wreal wrstim;
            assign wrstim = stim;
            drv f1(wrstim, load);
        endmodule
        "#,
    );
    let drv = d.module("drv").expect("drv elaborated");
    let in_port = drv.ports().iter().find(|p| p.name() == "in").expect("port `in`");
    assert_eq!(in_port.net_type(), &NetType::Digital1364(DigitalNetKind::Wreal));

    let top = d.module("top").expect("top elaborated");
    let wrstim = top.wires().iter().find(|w| w.name() == "wrstim").expect("wire `wrstim`");
    assert_eq!(wrstim.net_type(), &NetType::Digital1364(DigitalNetKind::Wreal));
}

/// §3.7: `wire`/`tri`/`reg` classify the same way as `wreal` — all four
/// are §3.7's own net-type family, not disciplines.
#[test]
fn wire_tri_reg_all_parse_as_digital1364() {
    let d = elab(
        r#"
        module top ();
            wire w;
            tri t;
            reg r;
        endmodule
        "#,
    );
    let top = d.module("top").expect("top elaborated");
    let ty_of = |name: &str| top.wires().iter().find(|w| w.name() == name).map(|w| w.net_type().clone());
    assert_eq!(ty_of("w"), Some(NetType::Digital1364(DigitalNetKind::Wire)));
    assert_eq!(ty_of("t"), Some(NetType::Digital1364(DigitalNetKind::Tri)));
    assert_eq!(ty_of("r"), Some(NetType::Digital1364(DigitalNetKind::Reg)));
}

/// Discrimination: a genuinely unknown, undeclared discipline name (never
/// one of the four §3.7 keywords) must still classify as an ordinary
/// `Discipline`, not silently swallowed into `Digital1364` —
/// `DigitalNetKind::from_keyword` matches only its exact four spellings.
#[test]
fn a_genuinely_unknown_discipline_name_stays_an_ordinary_discipline() {
    let d = elab("module top (); bogus_disc a; endmodule");
    let top = d.module("top").expect("top elaborated");
    let a = top.wires().iter().find(|w| w.name() == "a").expect("wire `a`");
    assert_eq!(
        a.net_type(),
        &NetType::Discipline("bogus_disc".to_string()),
        "an unrecognized leading identifier must still classify as Discipline, never Digital1364"
    );
}
