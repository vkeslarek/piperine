//! Scope: the checker's generation (generics GN-17, GN-19, part III §8.2.7, §8.2.23, §8.2.24): the range or condition of a
//! generation `for`/`if` is a const expression (`E0340`), a param inside generation is `E0322`, and an item declared in a
//! generation `for` has the array type of its element, one level per `for` around it, indexed by ordinal.

#[path = "support/pom.rs"]
mod support;
#[allow(dead_code)]
#[path = "support/generic_sources.rs"]
mod generic_sources;

use generic_sources::GenericSources;
use piperine_lang::pom::{DeclKind, Design};
use piperine_lang::types::Types;

/// Sources checked with generation in a module body.
struct GenerationSources;

impl GenerationSources {
    /// The codes and messages of the whole-model check of `src` that writes generation.
    fn pairs(src: &str) -> Vec<(String, String)> {
        GenericSources::standard_found(src).iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect()
    }

    /// The type the check gives the binding `name` of `src` after the natures, as the checker writes it.
    fn local(src: &str, name: &str) -> String {
        let design = Design::builder().source("top.msl", format!("{}{src}", GenericSources::HEAD)).build();
        let types = Types::check(&design, "Top").unwrap_or_else(|found| panic!("{:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
        let binding = GenericSources::binding(&design, name);
        types.local(binding).map_or_else(|| "?".to_string(), |ty| ty.name(&design))
    }

    /// The type the check gives the carrier or node array `name` of `Top` in `src`.
    fn carrier(src: &str, name: &str) -> String {
        let design = Design::builder().source("top.msl", format!("{}{src}", GenericSources::HEAD)).build();
        let types = Types::check(&design, "Top").unwrap_or_else(|found| panic!("{:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
        let decl = (0..design.decl_count()).map(|i| piperine_lang::pom::DeclId(i as u32)).find(|d| matches!(design.decl_data(*d).kind, DeclKind::Signal(_)) && design.decl_data(*d).name.as_ref().is_some_and(|n| n.text == name)).expect("a signal of that name");
        types.carrier(decl).map_or_else(|| "?".to_string(), |ty| ty.name(&design))
    }
}

/// The resistor every source places.
const PARTS: &str = "module Resistor(p: E, n: E) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\n";

#[test]
fn generated_items_are_typed_as_arrays_by_ordinal() {
    // §8.2.23: one array level per generation `for`, the element type inside; an `if` adds no level. A generated carrier is
    // judged like any (§3.3.4): `flag` has its initial value, so the source holds no other error
    let one = format!("{PARTS}module Top() {{\n  signal a: E;\n  signal b: E;\n  for k in 0..3 {{ let rk = Resistor(a, b) {{ r: 1k }}; }}\n}}\n");
    assert_eq!(GenerationSources::local(&one, "rk"), "[module `Resistor`; 3]");
    let nested = format!("{PARTS}module Top() {{\n  signal a: E;\n  signal b: E;\n  for i in 0..2 {{ for j in 0..3 {{ let rk = Resistor(a, b) {{ r: 1k }}; }} }}\n}}\n");
    assert_eq!(GenerationSources::local(&nested, "rk"), "[[module `Resistor`; 3]; 2]");
    let values = format!("{PARTS}module Top() {{\n  signal a: E;\n  for k in 0..4 {{ let half = k * 2; signal mid: E; signal flag: Bit = false; let rk = Resistor(a, mid) {{ r: 1k }}; }}\n}}\n");
    assert_eq!(GenerationSources::local(&values, "half"), "[i32; 4]");
    assert_eq!(GenerationSources::carrier(&values, "mid"), "[node of nature E; 4]");
    assert_eq!(GenerationSources::carrier(&values, "flag"), "[Bit<bool>; 4]");
    let arm = format!("{PARTS}module Top() {{\n  signal a: E;\n  for k in 0..2 {{ if k == 0 {{ let rk = Resistor(a, gnd) {{ r: 1k }}; }} else {{ let rl = Resistor(a, gnd) {{ r: 2k }}; }} }}\n}}\n");
    assert_eq!(GenerationSources::pairs(&arm), vec![], "an item of an `if` arm is typed whatever the arm");
    // the element is what the iteration reads: a node of the loop is read as one node, outside as the array
    let reads = format!("{PARTS}module Top() {{\n  signal a: E;\n  for k in 0..3 {{ signal mid: E; let rk = Resistor(a, mid) {{ r: 1k }}; let sk = Resistor(mid, gnd) {{ r: 1k }}; }}\n}}\n");
    assert_eq!(GenerationSources::pairs(&reads), vec![], "{:?}", GenerationSources::pairs(&reads));
}

#[test]
fn a_generation_range_that_is_not_const_is_e0340() {
    // §8.2.7: a signal, a port or a probe in the range or the condition
    let range = format!("{PARTS}module Top() {{\n  signal a: E;\n  signal count: E;\n  for k in 0..count {{ let rk = Resistor(a, gnd) {{ r: 1k }}; }}\n}}\n");
    let found = GenerationSources::pairs(&range);
    assert!(found.iter().any(|(code, message)| code == "E0340" && message.contains("`count`")), "{found:?}");
    let condition = format!("{PARTS}module Top() {{\n  signal a: E;\n  if v(a) > 0.5 {{ let rk = Resistor(a, gnd) {{ r: 1k }}; }}\n}}\n");
    assert!(GenerationSources::pairs(&condition).iter().any(|(code, _)| code == "E0340"), "{:?}", GenerationSources::pairs(&condition));
    let clean = format!("{PARTS}module Top() {{\n  size: usize = 3;\n  signal a: E;\n  for k in 0..size {{ let rk = Resistor(a, gnd) {{ r: 1k }}; }}\n  if size > 2 {{ let rg = Resistor(a, gnd) {{ r: 1k }}; }}\n}}\n");
    assert_eq!(GenerationSources::pairs(&clean), vec![], "a param is a const expression");
}

#[test]
fn a_param_inside_generation_is_e0322() {
    // §8.1.20: params are declared at the top level of the body
    let src = format!("{PARTS}module Top() {{\n  signal a: E;\n  for k in 0..2 {{ extra: f64 = 1.0; let rk = Resistor(a, gnd) {{ r: 1k }}; }}\n}}\n");
    let found = GenerationSources::pairs(&src);
    assert!(found.iter().any(|(code, message)| code == "E0322" && message.contains("extra")), "{found:?}");
}

#[test]
fn generation_in_a_digital_block_is_typed_per_iteration() {
    // GN-19 AC 1 (§12.1.14): one reaction per iteration, `k` a constant of the range's integer type, the event over the element of
    // an array of carriers; a range that reads a carrier is `E0340`
    let src = "module Top() {\n  signal lines: [Bit; 3] = [false; 3];\n  signal hits: [Bit<u8>; 3] = [0; 3];\n}\ndigital for Top {\n  for k in 0..3 { on lines[k].rise() { hits[k] <- (k as u8) + 1; } }\n}\n";
    assert_eq!(GenerationSources::pairs(src), vec![], "{:?}", GenerationSources::pairs(src));
    let nested = "module Top() {\n  signal lines: [Bit; 2] = [false; 2];\n  count: usize = 2;\n}\ndigital for Top {\n  for k in 0..count { if k == 0 { on lines[k].rise() { } } else { on lines[k].fall() { } } }\n}\n";
    assert_eq!(GenerationSources::pairs(nested), vec![], "{:?}", GenerationSources::pairs(nested));
    let runtime = "module Top() {\n  signal lines: [Bit; 2] = [false; 2];\n  signal count: Bit<u8> = 2;\n}\ndigital for Top {\n  for k in 0..count { on lines[k].rise() { } }\n}\n";
    assert!(GenerationSources::pairs(runtime).iter().any(|(code, message)| code == "E0340" && message.contains("`count`")), "{:?}", GenerationSources::pairs(runtime));
}
