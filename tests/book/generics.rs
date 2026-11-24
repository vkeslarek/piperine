//! Scope: chapter 4 of the Book, "Traits and generics" (feature `root-e2e`, RE-02 and RE-03): the file of
//! the chapter (`clip.msl`, closures passed to the methods of an `Option`) runs through `piperine::` and
//! gives the values the chapter states, the listings whose forms the compiler does not run yet (a trait's
//! methods, generic functions, a generic struct, const parameters, functions passed as values) are asserted
//! refused where a top reaches them, the forms the status box lists are asserted refused one by one, and
//! each mistake of the chapter's table gets the code the chapter names. Each test opens with the inventory
//! ids it holds, from `tests/book/inventory.tsv`.

#[allow(dead_code)] // the support modules are shared by every Book target; this one uses part of each
#[path = "support"]
mod support {
    pub mod book;
    pub mod chapter;
    pub mod faulty;
    pub mod inventory;
    pub mod model;
    pub mod printed;
    pub mod records;
    pub mod refusal;
    pub mod value_file;
}

use piperine::{Failure, Overrides};
use support::chapter::Chapter;
use support::faulty::Faulty;
use support::model::{Model, Ran};
use support::printed::Printed;
use support::records::Fields;
use support::refusal::Refusal;
use support::value_file::ValueFile;

/// A listing of the chapter whose forms the compiler does not run yet, reached from a top.
struct Reached;

impl Reached {
    /// The overrides of `piperine run <file> --top Top --analysis transient --stop 2e-6`.
    fn overrides() -> Overrides {
        Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(2e-6), ..Overrides::default() }
    }

    /// The text after a listing that makes a top run `statements` in a reaction at 1 us.
    fn top(statements: &str) -> String {
        format!("module Top() {{ }}\ndigital for Top {{ on at(1u) {{ {statements} }} }}\n")
    }

    /// The run of the block `id`, after `before`, on a top that runs `statements`.
    fn run(id: &str, before: &str, statements: &str) -> Result<Ran, Failure> {
        Chapter::read().wrapped("reached.msl", before, id, &Self::top(statements)).run(Self::overrides())
    }
}

#[test]
fn a_closure_is_passed_to_the_methods_of_an_option() {
    // generics#6 (run, `clip.msl`) with its listing generics#8; generics~3a14da6c "map is a method of Option
    // that takes a function: on Some(x) it calls the function with x and wraps the result in Some, and on None
    // it calls nothing" (2.1 doubled is 4.2, held to 3.3; the `None` gives 0.0 through `unwrap_or`)
    let record = ValueFile::only(Chapter::read().marked("clip.msl", "generics#6").run(ValueFile::transient()), "clip");
    for (name, printed) in [("amplified", "4.200000e0"), ("held", "3.300000e0"), ("shifted", "2.000000e0"), ("nothing", "0.000000e0")] {
        Printed::holds(printed, Fields::real(&record, name), name);
    }
    // the chapter's exercise: with a gain of 1.0 the reading is below the limit, and `min` leaves it
    let source = Chapter::read().text("generics#6").replace("let gain = 2.0;", "let gain = 1.0;");
    let ran = Model::written("clip.msl", &source).run(ValueFile::transient()).unwrap_or_else(|failure| panic!("{failure}"));
    let unity = ran.stream("clip")[0];
    Printed::holds("2.100000e0", Fields::real(unity, "amplified"), "amplified at a gain of 1.0");
    Printed::holds("2.100000e0", Fields::real(unity, "held"), "held at a gain of 1.0");
    // "It captures by copy": the closure reads the `gain` of the moment it is written
    let captured = ValueFile::written("", "let gain = 2.0; let seen: Option<f64> = Some(1.5); let amplified = seen.map(|v| v * gain); let gain = 10.0; emit!(\"r\", a = amplified.unwrap_or(0.0), g = gain);");
    Printed::holds("3.000000e0", Fields::real(&captured, "a"), "the closure took the first `gain`");
}

#[test]
fn the_methods_of_a_trait_run_where_a_top_reaches_them() {
    // generics#1 (run): the trait `Receiver` and its two `impl`s; generics~872b633d "An item with a body, like reads_high,
    // is a default: a type gets it as written unless it gives its own". The constant of an `impl` is read, a method an
    // `impl` gives runs, and the default `reads_high`, which calls `switches_at` through `Self`, runs for `Family`
    let supply = |ttl: bool, vdd: f64| if ttl { 1.4 } else { 0.5 * vdd };
    let chapter = Chapter::read();
    let constant = chapter.wrapped("levels.msl", "", "generics#1", &Reached::top("emit!(\"r\", n = Family::LEVELS);")).run(Reached::overrides()).unwrap_or_else(|failure| panic!("{failure}"));
    ValueFile::int(constant.stream("r")[0], "n", 2);
    let own = Reached::run("generics#1", "", "emit!(\"r\", s = Family::Ttl.switches_at(5.0));").unwrap_or_else(|failure| panic!("{failure}"));
    Printed::holds("1.400000e0", Fields::real(own.stream("r")[0], "s"), "Family::Ttl.switches_at(5.0)");
    assert_eq!(supply(true, 5.0), 1.4);
    let high = Reached::run("generics#1", "", "emit!(\"r\", s = Family::Ttl.reads_high(2.0, 5.0));").unwrap_or_else(|failure| panic!("{failure}"));
    ValueFile::flag(high.stream("r")[0], "s", 2.0 > supply(true, 5.0));
    let low = Reached::run("generics#1", "", "emit!(\"r\", s = Family::Cmos.reads_high(2.0, 5.0));").unwrap_or_else(|failure| panic!("{failure}"));
    ValueFile::flag(low.stream("r")[0], "s", 2.0 > supply(false, 5.0));
}

#[test]
fn the_generic_listings_run_where_a_top_reaches_them() {
    // generics#2 (run in part): `pick` and `larger`, with generics#3, their calls, a fragment of it, run with the values
    // Rust gives; `overdrive`, which calls a method through its `Receiver` bound, runs by static dispatch (T36);
    // generics~a6fd5a57 "Every listing of this chapter runs": generics#4: the generic struct `Window` and its method, an instance `Window<f64>` (T38); generics#5: the const parameter of
    // `dot`, `mean_of` and `widened`; generics#9: `at_middle`, which takes any function, runs with `corrected`. The forms that still wait are asserted refused
    let chapter = Chapter::read();
    let receiver = chapter.text("generics#1");
    for call in ["pick(en, 3.3, 0.0)", "larger(3u8, 200)", "overdrive(Family::Cmos, 2.8, 3.3)", "larger::<u16>(3, 200)"] {
        assert!(chapter.text("generics#3").contains(call), "generics#3 shows {call}");
    }
    let picked = Reached::run("generics#2", &receiver, "emit!(\"r\", s = pick(true, 3.3, 0.0));").unwrap_or_else(|failure| panic!("{failure}"));
    Printed::holds("3.300000e0", Fields::real(picked.stream("r")[0], "s"), "pick(true, 3.3, 0.0)");
    let larger = Reached::run("generics#2", &receiver, "emit!(\"r\", s = larger(3u8, 200));").unwrap_or_else(|failure| panic!("{failure}"));
    ValueFile::int(larger.stream("r")[0], "s", i128::from(3u8.max(200)));
    let over = Reached::run("generics#2", &receiver, "emit!(\"r\", s = overdrive(Family::Cmos, 2.8, 3.3));").unwrap_or_else(|failure| panic!("{failure}"));
    Printed::holds("1.150000e0", Fields::real(over.stream("r")[0], "s"), "overdrive(Family::Cmos, 2.8, 3.3)");
    let window = Reached::run("generics#4", "", "emit!(\"r\", s = in_tolerance(3.3));").unwrap_or_else(|failure| panic!("{failure}"));
    ValueFile::flag(window.stream("r")[0], "s", (3.0..=3.6).contains(&3.3));
    // generics#5: a const parameter fixed by the length of an array argument runs its loops that many times
    let dot = Reached::run("generics#5", "", "emit!(\"r\", s = dot([1.0, 2.0], [3.0, 4.0]));").unwrap_or_else(|failure| panic!("{failure}"));
    Printed::holds("1.100000e1", Fields::real(dot.stream("r")[0], "s"), "dot([1.0, 2.0], [3.0, 4.0])");
    let mean = Reached::run("generics#5", "", "emit!(\"r\", s = mean_of([1.0, 2.0]));").unwrap_or_else(|failure| panic!("{failure}"));
    Printed::holds("1.500000e0", Fields::real(mean.stream("r")[0], "s"), "mean_of([1.0, 2.0])");
    let wide = Reached::run("generics#5", "", "let w = widened([1.0, 2.0]); emit!(\"r\", s = w[1] + 10.0 * w[2] + 100.0 * w.len() as f64);").unwrap_or_else(|failure| panic!("{failure}"));
    Printed::holds("3.020000e2", Fields::real(wide.stream("r")[0], "s"), "widened([1.0, 2.0])");
    // generics#9: `at_middle` takes a closure for its `Fn` bound (T37) and `corrected` a function by its type
    let ramp = Reached::run("generics#9", "", "emit!(\"r\", s = mid_ramp(1M));").unwrap_or_else(|failure| panic!("{failure}"));
    Printed::holds("5.000000e-1", Fields::real(ramp.stream("r")[0], "s"), "mid_ramp(1M)");
    let model = Model::written("reached.msl", &format!("{}\n{}", chapter.text("generics#9"), Reached::top("emit!(\"r\", s = clean(1.0));")));
    assert!(model.run(Reached::overrides()).is_ok(), "generics#9: `clean` calls a function value and runs");
}

#[test]
fn a_form_not_run_yet_is_stopped_before_the_run_with_p0001_naming_it() {
    // generics~e60a0a07 "A model that uses one is stopped before the run with the code P0001 and a
    // message that names the form, so nothing is ever computed by a rule other than the ones above": the form
    // of the chapter's status box, each on the smallest model (when one of them runs, this test fails and the
    // box loses it)
    // the const parameter `N` read as a value, the smallest model of the forms the box once listed, now runs
    let width = ValueFile::written("fn width<const N: usize>() -> usize { N }", "emit!(\"r\", y = width::<4>());");
    ValueFile::int(&width, "y", 4);
    // generics~c212674a: a generic module, which the box no longer lists since T71: the smallest one placed runs
    let model = piperine::Overrides { analysis: Some("op".to_string()), top: Some("Top".to_string()), ..piperine::Overrides::default() };
    Model::written("hold.msl", "module Hold<T: Repr + Default>(in d: Bit<T>, out q: Bit<T>) {}\nmodule Top() { signal d: Bit<Option<u8>> = None; signal q: Bit<Option<u8>>; let h = Hold::<Option<u8>>(d, q); }\n").run(model).unwrap_or_else(|failure| panic!("a generic module is refused: {:?}", Refusal::of(Err(failure), "g")));
    // generation in a module body runs since T77 (the smallest one: a placement in a `for`)
    Model::written("repeat.msl", "module Top() { signal a: Electrical; for k in 0..2 { let r = Short(a, gnd); } }\n").run(piperine::Overrides { analysis: Some("op".to_string()), top: Some("Top".to_string()), ..piperine::Overrides::default() }).unwrap_or_else(|failure| panic!("generation in a body is refused: {:?}", Refusal::of(Err(failure), "g")));
    // generation in a `digital` block is the form still stopped before the run, naming it
    let generation = Faulty::elaborating("generics~e60a0a07", "module Top() { signal lines: [Bit; 2] = [false; 2]; }\ndigital for Top { for k in 0..2 { on lines[k].rise() { } } }\n", "Top");
    assert!(generation.given.iter().any(|reported| reported.code == "P0001" && reported.message == "generation `for`/`if` in a `digital` block is not supported yet"), "{:?}", generation.given.iter().map(|reported| (&reported.code, &reported.message)).collect::<Vec<_>>());
}

#[test]
fn an_impl_is_compared_with_its_trait() {
    // generics~76d9eeda (the status box): "An impl is compared with the trait it provides: a missing or an extra item, a signature that
    // differs and a supertrait the type does not implement are E0187, found where the impl is written" (generics T27)
    let extra = ValueFile::refused("generics~e60a0a07", "trait Supply { const VDD: f64; }\nstruct Core;\nimpl Supply for Core { const VDD: f64 = 1.0; const EXTRA: f64 = 2.0; }", "emit!(\"r\", v = Core::VDD);");
    assert!(extra.first("E0187").message.contains("`impl Supply for Core` defines `EXTRA`, which the trait does not have"), "{:?}", extra.first("E0187").message);
    let missing = ValueFile::refused("generics~e60a0a07", "trait Supply { const VDD: f64; }\nstruct Core;\nimpl Supply for Core { }", "emit!(\"r\", v = 1.0);");
    assert!(missing.first("E0187").message.contains("`impl Supply for Core` leaves out `VDD`"), "{:?}", missing.first("E0187").message);
}

#[test]
fn e0900_a_bound_is_not_a_where_an_impl_type_or_a_qualified_path() {
    // generics~80317d1d (E0900), three rows of the chapter's table: a trait bound in a `where`, `impl Receiver`
    // as the type of a parameter, and `<Family as Receiver>::LEVELS`; the fix of each row is accepted
    let receiver = "trait Receiver { const LEVELS: u32; fn switches_at(self, vdd: f64) -> f64; }\n#[derive(PartialEq, Repr)] enum Family { Cmos, Ttl }\nimpl Receiver for Family { const LEVELS: u32 = 2; fn switches_at(self, vdd: f64) -> f64 { 0.5 * vdd } }";
    let bound = ValueFile::refused("generics~80317d1d", &format!("{receiver}\nfn over<R>(r: R) -> f64 where R: Receiver {{ 1.0 }}"), "emit!(\"r\", y = 1);");
    assert_eq!(bound.first("E0900").message, "trait-bound `where` clauses are not part of the language");
    let argument = ValueFile::refused("generics~80317d1d", &format!("{receiver}\nfn over(r: impl Receiver) -> f64 {{ 1.0 }}"), "emit!(\"r\", y = 1);");
    assert_eq!(argument.first("E0900").message, "`impl Trait` types are not part of the language");
    let qualified = ValueFile::refused("generics~80317d1d", receiver, "emit!(\"r\", y = <Family as Receiver>::LEVELS);");
    assert_eq!(qualified.first("E0900").message, "qualified paths are not part of the language");
    let through_the_type = ValueFile::written(receiver, "emit!(\"r\", y = Family::LEVELS);");
    ValueFile::int(&through_the_type, "y", 2);
}

#[test]
fn e0101_e0905_and_e0903_no_dyn_no_lifetime_and_no_type_alias() {
    // generics~399f441d (E0101): "fn over(r: &dyn Receiver) -> f64"; generics~8031801d (E0905): "fn first<'a>(xs:
    // &'a [f64]) -> f64"; generics~80317e1d (E0903): "type Volts = f64;"
    let dynamic = ValueFile::refused("generics~399f441d", "trait Receiver { const LEVELS: u32; }\nfn over(r: &dyn Receiver) -> f64 { 1.0 }", "emit!(\"r\", y = 1);");
    assert_eq!(dynamic.first("E0101").message, "`dyn` is not part of the language");
    let lifetime = ValueFile::refused("generics~8031801d", "fn first<'a>(xs: &'a [f64]) -> f64 { 1.0 }", "emit!(\"r\", y = 1);");
    assert_eq!(lifetime.first("E0905").message, "lifetime is not part of the language");
    let alias = ValueFile::refused("generics~80317e1d", "type Volts = f64;", "emit!(\"r\", y = 1);");
    assert_eq!(alias.first("E0903").message, "type aliases are not part of the language");
}
