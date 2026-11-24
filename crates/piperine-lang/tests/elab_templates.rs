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
