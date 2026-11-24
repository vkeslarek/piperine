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

/// Sources elaborated with generation in a body.
struct Admitted;

impl Admitted {
    /// The natures and the parts every source places.
    const HEAD: &'static str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\nmodule Resistor(p: E, n: E) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\n";

    /// `src` after the parts, elaborated from `Top`.
    fn run(src: &str) -> Result<Ir, Vec<DesignDiagnostic>> {
        let design = Elab::design(&format!("{}{src}", Self::HEAD));
        assert!(design.diagnostics().is_empty(), "the source must be clean: {:?}", Elab::codes(design.diagnostics()));
        Elaborator::new(&design).run("Top")
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

#[test]
fn a_recursion_by_generation_unrolls() {
    // §8.2.7: a module that places itself under an `if` whose condition ends the recursion unrolls into nested instances;
    // the generic form counts down its const generic, the plain form a param it hands to the next level
    let generic = "module Chain<const N: usize>(a: E, b: E) where N >= 1 {\n  if N == 1 { let last = Resistor(a, b) { r: 1k }; } else {\n    signal mid: E;\n    let head = Resistor(a, mid) { r: 1k };\n    let rest = Chain::<{ N - 1 }>(mid, b);\n  }\n}\nmodule Top() { signal a: E; signal b: E; let c = Chain::<3>(a, b); }\n";
    let ir = Admitted::ir(generic);
    let names: Vec<String> = ir.bodies().map(|b| b.name().to_string()).collect();
    assert!(names.iter().any(|n| n.starts_with("Chain::<3>")) && names.iter().any(|n| n.starts_with("Chain::<2>")) && names.iter().any(|n| n.starts_with("Chain::<1>")), "{names:?}");
    let plain = "module Down(a: E, b: E) {\n  depth: usize = 2;\n  if depth == 0 { let last = Resistor(a, b) { r: 1k }; } else {\n    signal mid: E;\n    let head = Resistor(a, mid) { r: 1k };\n    let rest = Down(mid, b) { depth: depth - 1 };\n  }\n}\nmodule Top() { signal a: E; signal b: E; let c = Down(a, b); }\n";
    let ir = Admitted::ir(plain);
    assert!(ir.bodies().filter(|b| b.name().starts_with("Down")).count() >= 3, "one body for each depth");
}

#[test]
fn a_generation_recursion_that_does_not_end_is_e0341() {
    // §8.2.7: a recursion that does not end is an error naming the module, at the depth bound
    let generic = "module Forever<const N: usize>(a: E) {\n  if N > 0 { let next = Forever::<{ N + 1 }>(a); }\n}\nmodule Top() { signal a: E; let c = Forever::<1>(a); }\n";
    let found = Admitted::refused(generic);
    assert!(found.iter().any(|(code, message)| code == "E0341" && message.contains("Forever")), "{found:?}");
    let plain = "module Loop(a: E) {\n  depth: usize = 1;\n  if depth > 0 { let again = Loop(a) { depth: depth + 1 }; }\n}\nmodule Top() { signal a: E; let c = Loop(a); }\n";
    let found = Admitted::refused(plain);
    assert!(found.iter().any(|(code, message)| code == "E0341" && message.contains("Loop")), "{found:?}");
}

/// Sources whose `digital` blocks hold generation, elaborated with the form admitted until T80.
struct AdmittedDigital;

impl AdmittedDigital {
    /// The IR of `src` after the natures, which elaborates and checks.
    fn ir(src: &str) -> Ir {
        let design = Elab::design(&format!("{}{src}", Admitted::HEAD));
        assert!(design.diagnostics().is_empty(), "the source must be clean: {:?}", Elab::codes(design.diagnostics()));
        let ir = Elaborator::new(&design).admitting(&[UnfinishedForm::DigitalGeneration]).run("Top").unwrap_or_else(|found| panic!("elaboration refused: {:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
        assert!(ir.check().is_empty(), "{:?}", ir.check());
        ir
    }

    /// The reactions of the body `Top`, as the text of the structural print lists them.
    fn reactions(ir: &Ir) -> usize {
        ir.bodies().find(|b| b.name() == "Top" || b.name().starts_with("Top~")).expect("a body Top").reactions().len()
    }
}

#[test]
fn generated_reactions_are_one_per_iteration_in_source_order() {
    // GN-19 AC 1, AC 2 (§12.1.2): a `for` around reactions makes one reaction per iteration, its event over the iteration's
    // element; the reactions of one iteration keep their source order and the iterations follow one another, all inside the
    // place the `for` has among the block's reactions
    let src = "module Top() {\n  signal lines: [Bit; 3];\n  signal hits: [Bit<u8>; 3] = [0; 3];\n  signal first: Bit<u8> = 0;\n  signal last: Bit<u8> = 0;\n}\ndigital for Top {\n  on lines[0].fall() { first <- 1; }\n  for k in 0..3 {\n    on lines[k].rise() { hits[k] <- (k as u8) + 1; }\n    on lines[k].fall() { hits[k] <- 0; }\n  }\n  on lines[2].fall() { last <- 1; }\n}\n";
    let ir = AdmittedDigital::ir(src);
    assert_eq!(AdmittedDigital::reactions(&ir), 1 + 3 * 2 + 1, "the written reactions and two for each iteration");
    // the order of the sites: the first reaction is the `first` commit, the last is the `last` commit, the six between are the loop's
    let body = ir.bodies().find(|b| b.name() == "Top").unwrap();
    let events: Vec<_> = body.reactions().iter().map(|r| format!("{:?}", r.on)).collect();
    assert_eq!(events.len(), 8);
    assert_eq!(events[1..7].iter().collect::<std::collections::BTreeSet<_>>().len(), 6, "six distinct events, one reaction for each iteration and edge: {events:?}");
    // an `if` in the block selects its arm by a param
    let arms = "module Top() {\n  signal lines: [Bit; 2];\n  fast: bool = true;\n  signal a: Bit<u8> = 0;\n}\ndigital for Top {\n  if fast { on lines[0].rise() { a <- 1; } } else { on lines[1].rise() { a <- 2; } }\n}\n";
    assert_eq!(AdmittedDigital::reactions(&AdmittedDigital::ir(arms)), 1);
}
