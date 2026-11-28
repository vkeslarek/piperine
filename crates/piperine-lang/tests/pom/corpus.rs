//! Scope: the POM over the specification's own code blocks (requirements
//! POM-06 AC 1 and POM-14 AC 1): every fence rebuilds to the syntax tree the
//! parser returned, and every fence builds without breaking an invariant.

use crate::spec_corpus;

use piperine_lang::msl::parser::SourceParser;
use piperine_lang::pom::Design;
use serde_json::{Value, json};
use spec_corpus::{Class, corpus};

/// The first line where two trees differ, spans included, with a little context.
fn first_difference(a: &impl std::fmt::Debug, b: &impl std::fmt::Debug) -> Option<String> {
    let (a, b) = (format!("{a:#?}"), format!("{b:#?}"));
    let (a, b): (Vec<_>, Vec<_>) = (a.lines().collect(), b.lines().collect());
    let at = (0..a.len().max(b.len())).find(|&i| a.get(i) != b.get(i))?;
    let around = |v: &[&str]| v[at.saturating_sub(6)..(at + 3).min(v.len())].join("\n");
    Some(format!("line {at}:\n--- rebuilt\n{}\n--- parsed\n{}", around(&a), around(&b)))
}

/// Rebuild the first file of `design` and demand the parsed tree, spans included.
fn assert_lossless(design: &Design, parsed: &piperine_lang::msl::ast::File, at: &str) {
    let rebuilt = design.files()[0].to_ast();
    if let Some(diff) = first_difference(&rebuilt, parsed) {
        panic!("{at}: the rebuilt tree differs from the parsed one at {diff}");
    }
}

#[test]
fn every_spec_block_round_trips_to_its_ast() {
    let mut compared = 0;
    let mut skipped = 0;
    for block in corpus() {
        let parsed = SourceParser::parse(&block.text);
        match block.class {
            Class::Valid => {}
            // A fragment is in the corpus only when the parser takes it as a file.
            Class::Fragment if parsed.diagnostics.is_empty() => {}
            Class::Fragment | Class::CompileFail => {
                skipped += 1;
                continue;
            }
        }
        let design = Design::builder().source("block.msl", block.text.as_str()).build();
        assert_lossless(&design, &parsed.file, &format!("{}:{}", block.file, block.line));
        compared += 1;
    }
    println!("compared {compared} fences ({skipped} not file-parsable or compile_fail)");
    assert!(compared > 0, "no fence was compared: the corpus is empty");
}

#[test]
fn a_loaded_file_rebuilds_to_its_own_tree() {
    let design = Design::builder()
        .source("top.msl", "/// outer\nmod b;\nconst X: u32 = 1;")
        .source("b.msl", "//! inner\nfn f() { }\nconst Y: u32 = 2;")
        .build();
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let files: Vec<_> = design.files().into_iter().filter(|f| !f.krate().is_std()).collect();
    assert_eq!(files.len(), 2);
    for file in files {
        let parsed = SourceParser::parse(file.text()).file;
        let rebuilt = file.to_ast();
        if let Some(diff) = first_difference(&rebuilt, &parsed) {
            panic!("{}: differs at {diff}", file.name());
        }
    }
}

#[test]
fn every_item_kind_round_trips_when_written_by_hand() {
    let src = "\
//! file docs
use a::{b, c as d, e::*};
use f::g;
/// a struct
#[derive(Repr)]
pub struct S<T: Bound = u8, const N: u32 = 4> { /// field
    pub a: T, in b: flip Bundle, out c: Bit }
struct Tup(u32, pub f64);
struct Unit;
enum E { A, B(u32), C { x: f64 }, #[d] D = 4 }
trait Tr<T>: Super + Other { const K: u32; fn f(self) -> u32; fn g(x: T) -> T { x } }
impl<T> Tr<T> for S<T> { const K: u32 = 1; pub fn f(self) -> u32 { 1 } }
impl S { }
mod inline { fn f() { } mod deeper { } }
nature Voltage { potential: Voltage { access: v, units: \"V\" }, flow: Current { access: i } }
nature Level { value: Bit }
extern \"osdi\" module Dev<N: Nature>(in p: Electrical, out q: Electrical = d) -> u32 where true { }
module M<const W: u32>(a: Electrical, b: Electrical) {
    /// width
    pub w: u32 = 4 where w > 0;
    signal s: Bit<W> = 0;
    let (x, y) = (1, 2);
    s = a after 1;
    for i in 0..W { signal t: Bit; if i == 0 { s = 1; } else if i == 1 { s = 2; } else { s = 3; } }
    if W > 1 { signal u: Bit; } else { signal v: Bit; }
    42
}
analog for M { let k = 1; a +> b.v * 2.0; signal z: f64; }
analog<W> for M<W> { }
digital for M { on rising(a) { s <- 1 after 2; } for i in 0..4 { on at(i) { } } if true { on x { } } else if false { on y { } } else { on z { } } }
pub(crate_not_a_thing) fn broken() {}
struct ;
";
    // The last two items are malformed (and the module result is reported by a
    // context check): the tree has Error items and the rebuild keeps them in place.
    let parsed = SourceParser::parse(src);
    assert!(!parsed.diagnostics.is_empty());
    let design = Design::builder().source("top.msl", src).build();
    assert_lossless(&design, &parsed.file, "the hand-written source");
}

#[test]
fn every_spec_block_builds_with_consistent_invariants() {
    let mut built = 0;
    let mut failures = Vec::new();
    for block in corpus() {
        let at = format!("{}:{}", block.file, block.line);
        let result = std::panic::catch_unwind(|| {
            let design = Design::builder().source("block.msl", block.text.as_str()).build();
            design.check()
        });
        match result {
            Ok(problems) if problems.is_empty() => built += 1,
            Ok(problems) => failures.push(format!("{at}: {}", problems.join("; "))),
            Err(_) => failures.push(format!("{at}: the build panicked")),
        }
    }
    println!("built {built} fences, {} failed", failures.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(built > 0, "no fence was built: the corpus is empty");
}

/// A well-formed design with a path, an expression, a statement item and a
/// resolution slot, as JSON, to corrupt one link at a time.
fn design_json() -> Value {
    let src = "const X: u32 = { fn g() { } a };";
    let design = Design::builder().source("top.msl", src).build();
    assert!(design.check().is_empty(), "{:?}", design.check());
    serde_json::to_value(&design).expect("a design serializes")
}

/// Corrupt the design with `edit` and return what `check()` reports.
fn corrupted(edit: impl FnOnce(&mut Value)) -> Vec<String> {
    let mut json = design_json();
    edit(&mut json);
    let design: Design = serde_json::from_value(json).expect("a corrupted design still deserializes");
    design.check()
}

fn reports(problems: &[String], word: &str) -> bool {
    problems.iter().any(|p| p.contains(word))
}

#[test]
fn check_reports_every_kind_of_broken_link() {
    // An id past the end of its arena.
    let p = corrupted(|j| j["exprs"][0]["kind"] = json!({ "Path": 9999 }));
    assert!(reports(&p, "path 9999"), "{p:?}");
    let p = corrupted(|j| j["stmts"][0]["kind"] = json!({ "Item": 9999 }));
    assert!(reports(&p, "decl 9999"), "{p:?}");
    // An owner that does not exist.
    let p = corrupted(|j| j["exprs"][0]["owner"] = json!(9999));
    assert!(reports(&p, "owner"), "{p:?}");
    let p = corrupted(|j| j["types"][0]["owner"] = json!(9999));
    assert!(reports(&p, "owner"), "{p:?}");
    // A resolved link that points at no declaration.
    let p = corrupted(|j| j["paths"][0]["segments"][0]["res"] = json!({ "Decl": 9999 }));
    assert!(reports(&p, "resolution"), "{p:?}");
    // A declaration listed under two parents, or twice under one.
    let p = corrupted(|j| {
        let child = j["decls"][0]["children"][0].clone();
        j["decls"][0]["children"].as_array_mut().unwrap().push(child);
    });
    assert!(reports(&p, "twice"), "{p:?}");
    // Parent and child disagree.
    let p = corrupted(|j| j["decls"][1]["parent"] = json!(2));
    assert!(!p.is_empty());
    // Two declarations that are each other's parent: no root reaches them.
    let p = corrupted(|j| {
        j["decls"][0]["children"] = json!([]);
        j["decls"][1]["parent"] = json!(2);
        j["decls"][1]["children"] = json!([2]);
        j["decls"][2]["parent"] = json!(1);
        j["decls"][2]["children"] = json!([1]);
    });
    assert!(reports(&p, "not reachable from any root"), "{p:?}");
    // A child expression past the end of the arena.
    let p = corrupted(|j| j["exprs"][0]["kind"] = json!({ "Unary": { "op": "Neg", "expr": 9999 } }));
    assert!(reports(&p, "expression 9999"), "{p:?}");
}

#[test]
fn docs_at_every_position_round_trip() {
    // POL-12 AC 5: a design built from docs and attributes at every new
    // position converts back to the parsed tree, spans included.
    let src = "\
fn f() {
    /// a let
    #[x]
    let y = 1;
    /// a call
    g();
    /// a block
    if y { }
}
nature N {
    /// pot
    potential: V { access: v },
    #[f]
    flow: I { access: i },
}
nature L {
    /// value
    value: bool,
}
module M() {
    /// def
    #[d = 1]
    s = x after 1;
    /// gen
    for i in 0..2 { /// inner
        s = i; }
    /// gen if
    if a { } else { }
}
analog for M {
    /// contribution
    #[c]
    i(a) +> 1.0;
}
digital for M {
    /// reaction
    on e {
        /// commit
        q <- 1;
    }
    /// dgen
    for i in 0..2 { /// inner reaction
        on x { } }
}
";
    let parsed = SourceParser::parse(src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let design = Design::builder().source("top.msl", src).build();
    assert_lossless(&design, &parsed.file, "docs at every position");
}
