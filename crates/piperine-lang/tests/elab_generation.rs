//! Scope: generation in a module body elaborated (generics GN-17, GN-20, part III §8.2.7, §8.2.23, §8.2.24): a `for` over
//! module items unrolls into one element of an array per iteration (placements named `name[k]`, nodes an array of nodes), an
//! `if` keeps the arm its condition selects, an empty range makes an empty array, and a reference into an arm that was not
//! elaborated or past a generated array is `E0342`. Each test states the rule it proves.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use piperine_ir::Ir;
use piperine_lang::elab::Elaborator;
use piperine_lang::pom::DesignDiagnostic;
use piperine_lang::types::UnfinishedForm;
use support::Elab;

/// Sources elaborated with generation in a body admitted: the form is behind the unfinished-forms gate until T77.
struct Admitted;

impl Admitted {
    /// The natures and the parts every source places.
    const HEAD: &'static str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\nmodule Resistor(p: E, n: E) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\n";

    /// `src` after the parts, elaborated from `Top`.
    fn run(src: &str) -> Result<Ir, Vec<DesignDiagnostic>> {
        let design = Elab::design(&format!("{}{src}", Self::HEAD));
        assert!(design.diagnostics().is_empty(), "the source must be clean: {:?}", Elab::codes(design.diagnostics()));
        Elaborator::new(&design).admitting(&[UnfinishedForm::Generation]).run("Top")
    }

    /// The IR of `src`, which elaborates and checks.
    fn ir(src: &str) -> Ir {
        let ir = Self::run(src).unwrap_or_else(|found| panic!("elaboration refused: {:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
        assert!(ir.check().is_empty(), "{:?}", ir.check());
        ir
    }

    /// The codes and messages of the elaboration of `src`, which fails.
    fn refused(src: &str) -> Vec<(String, String)> {
        Self::run(src).err().expect("the elaboration must be refused").iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect()
    }

    /// The names of the children of the body `Top`.
    fn children(ir: &Ir) -> Vec<String> {
        let body = ir.bodies().find(|b| b.name() == "Top" || b.name().starts_with("Top~")).expect("a body Top");
        body.children().iter().map(|c| c.name.clone()).collect()
    }

    /// The names of the nodes of the body `Top`, and its arrays of nodes as `(name, count)`.
    fn nodes(ir: &Ir) -> (Vec<String>, Vec<(String, u32)>) {
        let body = ir.bodies().find(|b| b.name() == "Top" || b.name().starts_with("Top~")).expect("a body Top");
        (body.nodes().iter().map(|n| n.name.clone()).collect(), body.node_arrays().iter().map(|a| (a.name.clone(), a.count)).collect())
    }
}

#[test]
fn generation_for_makes_arrays_and_if_keeps_the_taken_arm() {
    // §8.2.23: one element per iteration, indexed from 0, a nested `for` nesting the array
    let src = "module Top() {\n  signal a: E;\n  for k in 0..3 { let rk = Resistor(a, gnd) { r: 1k }; }\n  for i in 0..2 { for j in 0..2 { let cell = Resistor(a, gnd) { r: 2k }; } }\n}\n";
    let ir = Admitted::ir(src);
    assert_eq!(Admitted::children(&ir), ["rk[0]", "rk[1]", "rk[2]", "cell[0][0]", "cell[0][1]", "cell[1][0]", "cell[1][1]"]);
    // the loop variable is a constant of each iteration: a param block over it, and an index of an array of nodes
    let chain = "module Top() {\n  signal a: E;\n  signal tap: [E; 4];\n  for k in 0..3 { let rk = Resistor(tap[k], tap[k + 1]) { r: 1k * ((k + 1) as f64) }; }\n}\n";
    let ir = Admitted::ir(chain);
    assert_eq!(Admitted::children(&ir), ["rk[0]", "rk[1]", "rk[2]"]);
    let body = ir.bodies().find(|b| b.name() == "Top" || b.name().starts_with("Top~")).unwrap();
    let firsts: Vec<_> = body.children().iter().map(|c| c.nodes.clone()).collect();
    assert_eq!(firsts[0][1], firsts[1][0], "tap[1] joins section 0 and section 1");
    assert_ne!(firsts[0][0], firsts[2][1]);
    // a node declared in the loop is an array of nodes, and the name inside is the iteration's element
    let nodes = "module Top() {\n  signal a: E;\n  for k in 0..3 { signal mid: E; let r1 = Resistor(a, mid) { r: 1k }; let r2 = Resistor(mid, gnd) { r: 1k }; }\n}\n";
    let ir = Admitted::ir(nodes);
    let (names, arrays) = Admitted::nodes(&ir);
    assert!(["mid[0]", "mid[1]", "mid[2]"].iter().all(|name| names.iter().any(|n| n == name)), "{names:?}");
    assert_eq!(arrays, [("mid".to_string(), 3)]);
    // an `if` over a param elaborates its taken arm only
    let arms = "module Top() {\n  mode: usize = 2;\n  signal a: E;\n  if mode > 1 { let big = Resistor(a, gnd) { r: 1k }; } else { let small = Resistor(a, gnd) { r: 1k }; }\n}\n";
    assert_eq!(Admitted::children(&Admitted::ir(arms)), ["big"]);
    // the param a range reads is structural: the body is one variant per value
    let counted = "module Top() {\n  count: usize = 2;\n  signal a: E;\n  for k in 0..count { let rk = Resistor(a, gnd) { r: 1k }; }\n}\n";
    assert_eq!(Admitted::children(&Admitted::ir(counted)), ["rk[0]", "rk[1]"]);
}

#[test]
fn an_untaken_arm_or_an_index_outside_is_e0342() {
    // §8.2.24: a reference to an item of an arm that was not elaborated
    let arm = "module Top() {\n  mode: usize = 0;\n  signal a: E;\n  if mode > 1 { signal extra: E; }\n  let r = Resistor(a, extra) { r: 1k };\n}\n";
    let found = Admitted::refused(arm);
    assert!(found.iter().any(|(code, message)| code == "E0342" && message.contains("extra")), "{found:?}");
    // §8.2.23: an index outside a generated array
    let outside = "module Top() {\n  signal a: E;\n  for k in 0..3 { signal mid: E; let r1 = Resistor(a, mid) { r: 1k }; }\n  let r = Resistor(a, mid[3]) { r: 1k };\n}\n";
    let found = Admitted::refused(outside);
    assert!(found.iter().any(|(code, message)| code == "E0342" && message.contains("mid")), "{found:?}");
    let inside = "module Top() {\n  signal a: E;\n  for k in 0..3 { signal mid: E; let r1 = Resistor(a, mid) { r: 1k }; }\n  let r = Resistor(a, mid[2]) { r: 1k };\n}\n";
    assert!(Admitted::run(inside).is_ok(), "the last element exists");
}

#[test]
fn an_empty_generation_range_makes_an_empty_array() {
    // §8.2.23: a range of no iteration is an array of no element
    let src = "module Top() {\n  count: usize = 0;\n  signal a: E;\n  for k in 0..count { signal mid: E; let rk = Resistor(a, mid) { r: 1k }; }\n  let r = Resistor(a, gnd) { r: 1k };\n}\n";
    let ir = Admitted::ir(src);
    assert_eq!(Admitted::children(&ir), ["r"]);
    assert!(Admitted::nodes(&ir).1.is_empty(), "no node array");
}
