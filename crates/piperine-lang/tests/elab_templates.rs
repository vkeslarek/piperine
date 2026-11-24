//! Scope: templates as elaboration values (generics phase D, GN-21 to GN-23; `docs/spec` §8.4):
//! a template folds from a `const`, a param and the result of a function, two equal templates
//! are one value, a placement through any of them is the instance the written placement makes,
//! and a binning function places the module and params its arguments choose.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use piperine_ir::{Constant, Ir, Real};
use support::Elab;

/// One module with a single param `cs`, its behavior reading it.
const CELL: &str = "module Cell(a: Electrical) { cs: f64 = 1.0; }\nanalog for Cell { i(a, gnd) +> v(a, gnd) * cs; }\n";

/// The instances of an elaborated design, read by path.
struct Placed;

impl Placed {
    /// The value of the first param of the instance at `path`.
    fn first_param(ir: &Ir, path: &str) -> Constant {
        *ir.instance_at(path).unwrap_or_else(|| panic!("no instance {path}")).param(0).expect("the instance has a param")
    }

    /// The first param of the instance at `path` as a real.
    fn real(ir: &Ir, path: &str) -> Constant {
        Constant::Real(Real::new(match Self::first_param(ir, path) {
            Constant::Real(value) => value.get(),
            other => panic!("not a real: {other:?}"),
        }))
    }
}

#[test]
fn templates_fold_from_consts_functions_and_two_levels_of_params() {
    // §8.4.1 (GN-21 AC 1) and §8.4.6 (GN-22 AC 3): a template from a `const`, from a function's result, and through two
    // template-typed params, places what the innermost binding names
    let from_const = format!("{CELL}const C1: Cell = Cell {{ cs: 30.0 }};\nmodule Top() {{ cell: Cell = C1; signal x: Electrical; let c1 = cell(x); }}");
    assert_eq!(Placed::first_param(&Elab::ir(&from_const, "Top"), "Top.c1"), Constant::Real(Real::new(30.0)));
    let from_function = format!("{CELL}fn pick(big: bool) -> Cell {{ if big {{ Cell {{ cs: 2.0 }} }} else {{ Cell {{ cs: 1.0 }} }} }}\nmodule Top() {{ cell: Cell = pick(true); signal x: Electrical; let c1 = cell(x); }}");
    assert_eq!(Placed::first_param(&Elab::ir(&from_function, "Top"), "Top.c1"), Constant::Real(Real::new(2.0)));
    let two_levels = format!(
        "{CELL}const C1: Cell = Cell {{ cs: 30.0 }};\nmodule Inner() {{ cell: Cell; signal x: Electrical; let c1 = cell(x); }}\nmodule Mid() {{ cell: Cell; let i = Inner {{ cell: cell }}(); }}\nmodule Top() {{ let m = Mid {{ cell: C1 }}(); }}"
    );
    let ir = Elab::ir(&two_levels, "Top");
    assert_eq!(Placed::first_param(&ir, "Top.m.i.c1"), Constant::Real(Real::new(30.0)));
    // two equal templates are one value: one body variant of `Inner` for both bindings
    let equal = format!("{CELL}const C1: Cell = Cell {{ cs: 30.0 }};\nmodule Inner() {{ cell: Cell; signal x: Electrical; let c1 = cell(x); }}\nmodule Top() {{ let a = Inner {{ cell: C1 }}(); let b = Inner {{ cell: Cell {{ cs: 30.0 }} }}(); }}");
    let ir = Elab::ir(&equal, "Top");
    let bodies = |path: &str| ir.instance_at(path).map(|i| i.body().name().to_string());
    assert_eq!(bodies("Top.a"), bodies("Top.b"));
    assert_eq!(Placed::real(&ir, "Top.a.c1"), Placed::real(&ir, "Top.b.c1"));
}

#[test]
fn a_template_placement_is_identical_to_the_written_placement() {
    // §8.4.9 (GN-22 AC 1): `C1(x)`, `cell(x)` and `pick(true)(x)` place the instance `Cell(x) { cs: 30.0 }` places
    let direct = Elab::ir(&format!("{CELL}module Top() {{ signal x: Electrical; let c1 = Cell(x) {{ cs: 30.0 }}; }}"), "Top");
    let through_const = format!("{CELL}const C1: Cell = Cell {{ cs: 30.0 }};\nmodule Top() {{ signal x: Electrical; let c1 = C1(x); }}");
    let through_function = format!("{CELL}fn thirty() -> Cell {{ Cell {{ cs: 30.0 }} }}\nmodule Top() {{ signal x: Electrical; let c1 = thirty()(x); }}");
    for source in [through_const, through_function] {
        let ir = Elab::ir(&source, "Top");
        assert_eq!(Placed::first_param(&ir, "Top.c1"), Placed::first_param(&direct, "Top.c1"), "{source}");
        assert_eq!(ir.instance_at("Top.c1").map(|i| i.body().name().to_string()), direct.instance_at("Top.c1").map(|i| i.body().name().to_string()), "{source}");
    }
}

#[test]
fn a_binning_function_places_the_module_it_chooses() {
    // §8.4.7 (GN-22 AC 3): the function reads the length param, so the param is structural and each length places its own
    // bin; the length that is not bound again takes its default's bin
    let source = format!(
        "{CELL}fn bin(len: f64) -> Cell {{ if len < 1.0 {{ Cell {{ cs: 0.3 }} }} else {{ Cell {{ cs: 0.6 }} }} }}\n\
         module Top() {{ len: f64 = 0.5; signal x: Electrical; let c1 = bin(len)(x); }}\n\
         module Parent() {{ let short = Top(); let long = Top {{ len: 2.0 }}(); }}"
    );
    let ir = Elab::ir(&source, "Parent");
    assert_eq!(Placed::first_param(&ir, "Parent.short.c1"), Constant::Real(Real::new(0.3)));
    assert_eq!(Placed::first_param(&ir, "Parent.long.c1"), Constant::Real(Real::new(0.6)));
}

/// A trait over module types, one module implementing it, and a probe that reads `vth()` of a template into a param default.
const MOSFET: &str = "trait Mosfet { fn vth(self) -> f64; }\n\
module Bsim4(d: Electrical) { vth0: f64 = 0.4; }\n\
impl Mosfet for Bsim4 { fn vth(self) -> f64 { self.vth0 + 0.1 } }\n\
impl Bsim4 { fn shifted(self, by: f64) -> Bsim4 { Bsim4 { vth0: self.vth0 + by } } }\n";

#[test]
fn a_module_impl_function_runs_on_a_template_at_elaboration() {
    // §3.1.4 (GN-23, the decision of 2026-11-22): `N1.vth()` in a param default runs the `impl` function with `self` the template,
    // its params readable; a method that returns another template places that one (binning)
    let probe = "module Probe() { vt: f64 = N1.vth(); signal p: Electrical; }\nanalog for Probe { v(p, gnd) +> vt; }\n";
    let source = format!("{MOSFET}const N1: Bsim4 = Bsim4 {{ vth0: 0.3 }};\n{probe}module Top() {{ let a = Probe(); }}");
    assert_eq!(Placed::first_param(&Elab::ir(&source, "Top"), "Top.a"), Constant::Real(Real::new(0.3 + 0.1)));
    let shifted = format!("{MOSFET}const N1: Bsim4 = Bsim4 {{ vth0: 0.3 }};\nconst N2: Bsim4 = N1.shifted(0.2);\nmodule Probe() {{ vt: f64 = N2.vth(); signal p: Electrical; }}\nanalog for Probe {{ v(p, gnd) +> vt; }}\nmodule Top() {{ let a = Probe(); }}");
    let ir = Elab::ir(&shifted, "Top");
    let param = |name: &str| ir.instance_at("Top.a").and_then(|i| i.body().params().iter().position(|p| p.name == name).and_then(|at| i.param(at).copied())).expect(name);
    assert_eq!(param("vt"), Constant::Real(Real::new(0.3 + 0.2 + 0.1)));
    // static dispatch through the bound: `N: Mosfet` picks the impl of the module the template names
    let bound = format!("{MOSFET}fn threshold<N: Mosfet>(n: N) -> f64 {{ n.vth() }}\nconst N1: Bsim4 = Bsim4 {{ vth0: 0.7 }};\nmodule Probe() {{ vt: f64 = threshold(N1); signal p: Electrical; }}\nanalog for Probe {{ v(p, gnd) +> vt; }}\nmodule Top() {{ let a = Probe(); }}");
    assert_eq!(Placed::first_param(&Elab::ir(&bound, "Top"), "Top.a"), Constant::Real(Real::new(0.7 + 0.1)));
}

/// A cell with a `pub` storage node `sn` held at `cs`, a bank that places it through the callable param `cell`, and the
/// bank's `pub let c0`.
const BANK: &str = "module Dram(a: Electrical) { cs: f64 = 1.0; pub signal sn: Electrical; hidden: f64 = 3.0; }\n\
analog for Dram { i(a, gnd) +> v(a, gnd); i(sn, gnd) +> v(sn, gnd) - cs; }\n\
module Bank<C: Fn(Electrical)>(a: Electrical) { cell: C; pub let c0 = cell(a); }\n";

#[test]
fn inst_name_through_a_template_param_reads_the_concrete_module() {
    // §8.4.8 (GN-23 AC 3): `bank.c0.sn` is a node of the `Dram` the bank's template names, so a probe of it reads the
    // cell's own storage node and the design elaborates
    let source = format!(
        "{BANK}module Top() {{ signal x: Electrical; let bank = Bank::<_>(x) {{ cell: Dram {{ cs: 2.0 }} }}; signal probe: Electrical; }}\n\
         analog for Top {{ i(probe, gnd) +> v(probe, gnd) - v(bank.c0.sn); }}"
    );
    let ir = Elab::ir(&source, "Top");
    assert!(ir.instance_at("Top.bank.c0").is_some(), "the cell the template names is placed");
    assert_eq!(Placed::first_param(&ir, "Top.bank.c0"), Constant::Real(Real::new(2.0)));
    // the probe reads the storage node of the cell the template places: the reached node of `Top` is that node
    let top = ir.instance_at("Top").expect("the top");
    let reached = top.body().nodes().iter().position(|node| node.name == "bank.c0.sn").expect("`Top` reaches `bank.c0.sn`");
    assert_eq!(top.node(reached).expect("a bound node").path(), "Top.bank.c0.sn");
}

#[test]
fn a_missing_item_through_a_template_param_is_e0343() {
    // §8.4.8: an item the concrete module lacks, or keeps private, is `E0343` at elaboration
    for item in ["missing", "hidden"] {
        let source = format!(
            "{BANK}module Top() {{ signal x: Electrical; let bank = Bank::<_>(x) {{ cell: Dram {{ cs: 2.0 }} }}; signal probe: Electrical; }}\n\
             analog for Top {{ i(probe, gnd) +> v(probe, gnd) - v(bank.c0.{item}); }}"
        );
        let diagnostics = Elab::run(&source, "Top").expect_err("the item is out of reach");
        assert!(Elab::codes(&diagnostics).contains(&"E0343".to_string()), "{item}: {diagnostics:?}");
    }
}

#[test]
fn a_node_of_a_child_of_a_child_is_reached_without_a_template() {
    // §8.2.8: `bank.c0.sn` names a `pub` node of the module a `pub let` of the bank places, a template or not
    let source = "module Dram(a: Electrical) { cs: f64 = 1.0; pub signal sn: Electrical; }\n\
        analog for Dram { i(a, gnd) +> v(a, gnd); i(sn, gnd) +> v(sn, gnd) - cs; }\n\
        module Bank(a: Electrical) { pub let c0 = Dram(a) { cs: 2.0 }; }\n\
        module Top() { signal x: Electrical; let bank = Bank(x); signal probe: Electrical; }\n\
        analog for Top { i(probe, gnd) +> v(probe, gnd) - v(bank.c0.sn); }";
    let ir = Elab::ir(source, "Top");
    let top = ir.instance_at("Top").expect("the top");
    let reached = top.body().nodes().iter().position(|node| node.name == "bank.c0.sn").expect("`Top` reaches `bank.c0.sn`");
    assert_eq!(top.node(reached).expect("a bound node").path(), "Top.bank.c0.sn");
}

#[test]
fn a_cell_of_a_generated_bank_is_read_through_its_indexes() {
    // §8.2.23, §8.4.8: `bank.cells[r][c].sn` names the storage node of the cell the iteration `(r, c)` places
    let source = "module Dram(a: Electrical) { cs: f64 = 1.0; pub signal sn: Electrical; }\n\
        analog for Dram { i(a, gnd) +> v(a, gnd); i(sn, gnd) +> v(sn, gnd) - cs; }\n\
        module Bank<C: Fn(Electrical), const ROWS: usize, const COLS: usize>(a: Electrical) { cell: C; for r in 0..ROWS { for c in 0..COLS { pub let cells = cell(a); } } }\n\
        module Top() { signal x: Electrical; let bank = Bank::<_, 2, 3>(x) { cell: Dram { cs: 2.0 } }; signal probe: Electrical; }\n\
        analog for Top { i(probe, gnd) +> v(probe, gnd) - v(bank.cells[1][2].sn); }";
    let ir = Elab::ir(source, "Top");
    let top = ir.instance_at("Top").expect("the top");
    let reached = top.body().nodes().iter().position(|node| node.name == "bank.cells[1][2].sn").expect("`Top` reaches the cell");
    assert_eq!(top.node(reached).expect("a bound node").path(), "Top.bank.cells[1][2].sn");
}

/// A cell with a `pub` storage node and `pub` param `cs`, and a bank that places it through the callable param `cell` and reads
/// the cell in its own analog block (`items` is the read).
fn own_bank(visibility: &str, items: &str) -> String {
    format!(
        "module Dram(a: Electrical) {{ pub cs: f64 = 1.0; pub signal sn: Electrical; }}\n\
         analog for Dram {{ i(a, gnd) +> v(a, gnd); i(sn, gnd) +> v(sn, gnd) - cs; }}\n\
         module Bank<C: Fn(Electrical)>(a: Electrical) {{ cell: C; {visibility}let c0 = cell(a); }}\n\
         analog<C> for Bank<C> {{ {items} }}\n\
         module Top() {{ signal x: Electrical; {visibility}let bank = Bank::<_>(x) {{ cell: Dram {{ cs: 2.0 }} }}; }}"
    )
}

#[test]
fn inst_name_in_the_module_that_places_the_template_reads_the_concrete_module() {
    // §8.4.8: `v(c0.sn)` in the bank's own analog block names the storage node of the `Dram` the template places
    let ir = Elab::ir(&own_bank("", "i(a, gnd) +> v(c0.sn);"), "Top");
    let bank = ir.instance_at("Top.bank").expect("the bank");
    let reached = bank.body().nodes().iter().position(|node| node.name == "c0.sn").expect("the bank reaches `c0.sn`");
    assert_eq!(bank.node(reached).expect("a bound node").path(), "Top.bank.c0.sn");
}

#[test]
fn a_non_node_item_through_a_template_is_refused_by_name_never_as_a_type_error() {
    // §8.4.8: `c0.cs` is a param of the concrete `Dram`, which only elaboration knows (GAPS §9, `non-node item through a
    // template`): the refusal is `P0001` naming the construct, in the placing module and from its parent, never `E0180`/`E0417`
    let from_parent = format!(
        "{}\nmodule Outer() {{ signal y: Electrical; let top = Top(); signal probe: Electrical; }}\nanalog for Outer {{ i(probe, gnd) +> top.bank.c0.cs; }}",
        own_bank("pub ", "")
    );
    for source in [own_bank("", "i(a, gnd) +> c0.cs;"), own_bank("", "i(a, gnd) +> c0.cs * 2.0;"), from_parent] {
        let entry = if source.contains("module Outer") { "Outer" } else { "Top" };
        let diagnostics = Elab::run(&source, entry).expect_err("a non-node item through a template is not typed");
        assert_eq!(Elab::codes(&diagnostics), ["P0001"], "{source}");
        assert!(diagnostics[0].diagnostic.message.contains("a non-node item of a child read through a template param"), "{diagnostics:?}");
    }
}

#[test]
fn an_out_of_range_generated_cell_names_its_element_and_the_length_of_its_own_loop() {
    // §8.2.23, §8.4.8: on a 2x3 bank `[2][0]` is past the 2 rows and `[0][3]` past the 3 columns of row 0, whatever the flat count (6)
    let bank = "module Dram(a: Electrical) { cs: f64 = 1.0; pub signal sn: Electrical; }\n\
        analog for Dram { i(a, gnd) +> v(a, gnd); i(sn, gnd) +> v(sn, gnd) - cs; }\n\
        module Bank<C: Fn(Electrical), const ROWS: usize, const COLS: usize>(a: Electrical) { cell: C; for r in 0..ROWS { for c in 0..COLS { pub let cells = cell(a); } } }\n\
        module Top() { signal x: Electrical; let bank = Bank::<_, 2, 3>(x) { cell: Dram { cs: 2.0 } }; signal probe: Electrical; }\n";
    for (read, element, length) in [("bank.cells[2][0].sn", 2, 2), ("bank.cells[0][3].sn", 3, 3)] {
        let source = format!("{bank}analog for Top {{ i(probe, gnd) +> v(probe, gnd) - v({read}); }}");
        let diagnostics = Elab::run(&source, "Top").expect_err("the cell is past the generation");
        assert_eq!(Elab::codes(&diagnostics), ["E0342"], "{read}");
        let expected = format!("the element {element} of the generated array `cells` does not exist: it has {length}");
        assert!(diagnostics[0].diagnostic.message.contains(&expected), "{read}: {diagnostics:?}");
    }
}

/// A swappable pair: `Pair<N: Mosfet + Fn(..)>` reads `fet.gain()` where `reads` puts it.
fn mosfet_pair(params: &str, analog: &str) -> String {
    format!(
        "trait Mosfet {{ fn gain(self) -> f64; }}\n\
         module Square(g: Electrical, d: Electrical) {{ k: f64 = 1.0; }}\n\
         analog for Square {{ potential(d) +> k * v(g) * v(g); }}\n\
         impl Mosfet for Square {{ fn gain(self) -> f64 {{ self.k * 2.0 }} }}\n\
         module Linear(g: Electrical, d: Electrical) {{ k: f64 = 1.0; }}\n\
         analog for Linear {{ potential(d) +> k * v(g); }}\n\
         impl Mosfet for Linear {{ fn gain(self) -> f64 {{ self.k * 0.5 }} }}\n\
         module Pair<N: Mosfet + Fn(Electrical, Electrical)>(g: Electrical, d: Electrical) {{ fet: N; {params} signal s: Electrical; let left = fet(g, d); }}\n\
         analog<N> for Pair<N> {{ {analog} }}\n\
         module Top() {{ signal g: Electrical; signal d: Electrical; let a = Pair::<_>(g, d) {{ fet: Square {{ k: 3.0 }} }}; let b = Pair::<_>(g, d) {{ fet: Linear {{ k: 3.0 }} }}; }}"
    )
}

#[test]
fn the_trait_of_a_combined_bound_dispatches_to_the_template_modules_impl() {
    // §8.4.5, §3.1.4: `fet.gain()` through `N: Mosfet + Fn(..)` runs the `impl Mosfet` of the module the template names, in a body
    // `let` and straight in the `analog` block; each placement keeps its own model's answer (a param default that reads the
    // param `fet` is the general §8.1.16 gap, `GAPS.md` §9)
    for analog in ["let gain = fet.gain(); potential(s) +> gain;", "potential(s) +> fet.gain();"] {
        let printed = Elab::ir(&mosfet_pair("", analog), "Top").print_structural();
        let body_of = |model: &str| printed.split("\nbody ").find(|body| body.starts_with(&format!("\"Pair::<module `{model}`>"))).unwrap_or_else(|| panic!("no body for {model}: {printed}")).to_string();
        let (square, linear) = (body_of("Square"), body_of("Linear"));
        assert!(square.contains("direct 6.0") || square.contains("let gain = 6.0"), "{analog}: {square}");
        assert!(linear.contains("direct 1.5") || linear.contains("let gain = 1.5"), "{analog}: {linear}");
        assert!(!square.contains("1.5") && !linear.contains("6.0"), "{analog}: a body holds only its own model's gain");
    }
}
