//! Scope: the elaborator's generic module instances (generics GN-13, GN-15, part III §8): one body per distinct list of
//! generic arguments, a false module `where` reported for the instance, a chain of growing arguments bounded, and the POM
//! left as written.

use piperine_ir::{CarrierRole, Ir};
use piperine_lang::elab::Elaborator;
use piperine_lang::pom::{Design, DesignDiagnostic};

/// A design over the real standard library, elaborated, generic modules among them.
struct ModuleElab;

impl ModuleElab {
    /// The elaboration of `top` of `src`.
    fn run(src: &str, top: &str) -> Result<Ir, Vec<DesignDiagnostic>> {
        let design = Design::builder().source("top.msl", src).build();
        assert!(design.diagnostics().is_empty(), "the source must be clean: {:?}", design.diagnostics());
        Elaborator::new(&design).run(top)
    }

    /// The `(code, message)` of each diagnostic of elaborating `top`.
    fn refused(src: &str, top: &str) -> Vec<(String, String)> {
        match Self::run(src, top) {
            Ok(_) => Vec::new(),
            Err(found) => found.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect(),
        }
    }
}

#[test]
fn one_body_per_distinct_module_instance_and_the_pom_as_written() {
    // GN-15 AC 1, GN-13 AC 4: a placement is lowered at its arguments, equal arguments share one body, and the POM keeps
    // the generic module and each placement as written
    let src = "module Tap<const N: usize>(a: Electrical, b: Electrical) where N >= 1 { k: f64 = 1.0; }\n\
               module Top() { signal x: Electrical; signal y: Electrical;\n\
               let p = Tap::<2>(x, y); let q = Tap::<2>(y, x); let r = Tap::<3>(x, y); }\n";
    let ir = ModuleElab::run(src, "Top").unwrap_or_else(|found| panic!("{found:?}"));
    assert!(ir.check().is_empty(), "{:?}", ir.check());
    let mut names: Vec<String> = ir.bodies().map(|body| body.name().to_string()).collect();
    names.sort();
    assert_eq!(names, ["Tap::<2>", "Tap::<3>", "Top"], "one body per distinct list of arguments");
    let design = Design::builder().source("top.msl", src).build();
    let tap = design.module("Tap").expect("the generic module stays in the POM");
    assert_eq!(tap.generics().len(), 1, "its generic parameter is as written");
    let top = design.module("Top").expect("Top");
    assert_eq!(top.instances().len(), 3, "each placement is walkable as written");
    assert_eq!(design.modules().len(), 2, "no instance body is a module of the POM");
}

#[test]
fn a_false_module_where_is_e0304_naming_the_instance() {
    // GN-13 AC 4: the `where` of a module is discharged for each instance; a false one is E0304 naming `Tap::<0>`
    let src = "module Tap<const N: usize>(a: Electrical, b: Electrical) where N >= 1 { }\n\
               module Top() { signal x: Electrical; signal y: Electrical; let p = Tap::<0>(x, y); }\n";
    let found = ModuleElab::refused(src, "Top");
    assert_eq!(found.iter().map(|(code, _)| code.as_str()).collect::<Vec<_>>(), ["E0304"], "{found:?}");
    assert!(found[0].1.contains("`Tap::<0>`"), "{}", found[0].1);
}

#[test]
fn a_chain_of_growing_module_arguments_is_e0341() {
    // D7: a module that places itself at a growing argument never ends; the chain is cut at depth 1 024 naming the module
    let src = "module Grow<const N: usize>(a: Electrical) { let next = Grow::<{ N + 1 }>(a); }\n\
               module Top() { signal x: Electrical; let g = Grow::<1>(x); }\n";
    let found = ModuleElab::refused(src, "Top");
    assert!(found.iter().any(|(code, message)| code == "E0341" && message.contains("Grow")), "{found:?}");
}

#[test]
fn a_param_read_by_a_generic_argument_or_a_length_is_structural() {
    // GN-20 AC 1 (§8.1.2): `count` is read by the const generic argument of `Taps::<{ count }>` and `n` by the length of an
    // array: each fixes structure, so each param is `structural` in the body that holds it and the body is lowered per value
    let src = "module Taps<const N: usize>(a: Electrical) { signal rails: [Electrical; N]; }\n\
               module Top() { count: usize = 2; n: usize = 3; signal x: Electrical; signal sized: [Electrical; n]; let t = Taps::<{ count }>(x); }\n";
    let ir = ModuleElab::run(src, "Top").unwrap_or_else(|found| panic!("{found:?}"));
    assert!(ir.check().is_empty(), "{:?}", ir.check());
    let top = ir.bodies().find(|body| body.name().starts_with("Top")).expect("a body of Top");
    let structural: Vec<&str> = top.params().iter().filter(|param| param.structural).map(|param| param.name.as_str()).collect();
    assert_eq!(structural, ["count", "n"], "the params the generic argument and the length read");
    assert!(ir.bodies().any(|body| body.name() == "Taps::<2>"), "the instance takes the value of `count`");
}

#[test]
fn a_resolved_nature_parameter_lowers_to_a_resolved_carrier_of_the_instance_nature() {
    // §22.3: `Tribuf<W>` placed on `Line` is the body `Tribuf::<Line>`, whose `w` is a resolved carrier of `Line` driven
    // by the instance, and the placement on a nature without the `impl` is refused at the check, not here
    let src = "nature Line { value: Driven }\n\
               impl Resolve<Driven> for Line { fn resolve<const N: usize>(drives: [Driven; N]) -> Driven { Driven::strongest(drives) } }\n\
               module Top() { signal n: Line; let a = Tribuf(n, true, true); let b = Pullup(n); }\n";
    let ir = ModuleElab::run(src, "Top").unwrap_or_else(|found| panic!("{found:?}"));
    assert!(ir.check().is_empty(), "{:?}", ir.check());
    let names: Vec<String> = ir.bodies().map(|body| body.name().to_string()).collect();
    assert!(names.iter().any(|name| name == "Tribuf::<Line>") && names.iter().any(|name| name == "Pullup::<Line>"), "{names:?}");
    let tribuf = ir.bodies().find(|body| body.name() == "Tribuf::<Line>").expect("the instance body");
    let w = tribuf.carriers().iter().find(|carrier| carrier.name == "w").expect("the port `w`");
    assert!(matches!(w.role, CarrierRole::Resolved { port: true, .. }), "`w` is a resolved port of the instance: {:?}", w.role);
    let net = ir.nets().find(|net| net.path() == "Top.n").expect("the net of `n`");
    let resolution = net.resolution().expect("`n` is a resolved net of `Line`");
    assert_eq!(resolution.drives.len(), 2, "the tribuf and the pull-up each hold one drive of it");
}

#[test]
fn a_generic_module_passing_its_resolved_nature_on_lowers_its_inner_instances_at_the_nature() {
    // §22.3.8: `Tgate<W>` places `Tran(a, y, s)` on its own `W`; the instance `Tgate::<Line>` places `Tran::<Line>`, not a
    // body of the parameter
    let src = "nature Line { value: Driven }\n\
               impl Resolve<Driven> for Line { fn resolve<const N: usize>(drives: [Driven; N]) -> Driven { Driven::strongest(drives) } }\n\
               module Top() { signal a: Line; signal y: Line; let g = Tgate(a, y, true, false); }\n";
    let ir = ModuleElab::run(src, "Top").unwrap_or_else(|found| panic!("{found:?}"));
    assert!(ir.check().is_empty(), "{:?}", ir.check());
    let names: Vec<String> = ir.bodies().map(|body| body.name().to_string()).collect();
    assert!(names.iter().any(|name| name == "Tran::<Line>") && names.iter().all(|name| !name.contains("::<W>")), "{names:?}");
}
