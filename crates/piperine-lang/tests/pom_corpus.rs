//! Scope: the POM over the specification's own code blocks (requirements
//! POM-06 AC 1 and POM-14 AC 1): every fence rebuilds to the syntax tree the
//! parser returned, and every fence builds without breaking an invariant.

#[path = "support/spec_corpus.rs"]
mod spec_corpus;

use piperine_lang::msl::parser::parse;
use piperine_lang::pom::Design;
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
        let parsed = parse(&block.text);
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
    let files = design.files();
    assert_eq!(files.len(), 2);
    for file in files {
        let parsed = parse(file.text()).file;
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
nature Level { value: Bit, resolve: pick }
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
    let parsed = parse(src);
    assert!(!parsed.diagnostics.is_empty());
    let design = Design::builder().source("top.msl", src).build();
    assert_lossless(&design, &parsed.file, "the hand-written source");
}
