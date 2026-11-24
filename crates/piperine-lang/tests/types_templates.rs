//! Scope: templates as values (generics phase D, GN-21 and GN-22; `docs/spec` §8.4): a template
//! `Name { params }` is a value of its module's type wherever a value goes (`E0319` for a missing
//! required param, `E0362` outside a const context, `E0112` as a carried type), a placement
//! through a template or a function result types as its module (`E0361` for a param block), and
//! `C: Fn(P)` holds for a module whose ports match.

use piperine_lang::pom::{Design, DesignDiagnostic, ExprId};
use piperine_lang::types::{Callee, ConstTerm, GenericValue, Ty, Types};

/// The natures and the two modules every source of this file shares.
const PRELUDE: &str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n\
module Cell(a: E) { cs: f64 = 1.0; }\n\
module Sizing(a: E) { size: f64; }\n";

/// A source checked from its top module.
struct TemplateSource {
    /// The whole text.
    text: String,
    /// The elaborated design.
    design: Design,
}

impl TemplateSource {
    /// `PRELUDE` followed by `body`, parsed and resolved without a diagnostic.
    fn new(body: &str) -> Self {
        let text = format!("{PRELUDE}{body}");
        let design = Design::builder().source("top.msl", text.clone()).build();
        assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{text}", design.diagnostics().iter().map(|d| d.diagnostic.code.to_string()).collect::<Vec<_>>());
        Self { text, design }
    }

    /// The checker's diagnostics for `Top`, as codes.
    fn codes(&self) -> Vec<String> {
        match Types::check(&self.design, "Top") {
            Ok(_) => Vec::new(),
            Err(found) => Self::codes_of(&found),
        }
    }

    /// The codes of `found`, in order.
    fn codes_of(found: &[DesignDiagnostic]) -> Vec<String> {
        found.iter().map(|d| d.diagnostic.code.to_string()).collect()
    }

    /// The first diagnostic message of `Top`'s check.
    fn message(&self) -> String {
        Types::check(&self.design, "Top").err().and_then(|found| found.first().map(|d| d.diagnostic.message.clone())).unwrap_or_default()
    }

    /// What the call whose text is `snippet` refers to.
    fn callee_of(&self, snippet: &str) -> Option<Callee> {
        let types = Types::check(&self.design, "Top").unwrap_or_else(|found| panic!("{:?}\n{}", Self::codes_of(&found), self.text));
        (0..self.design.expr_count()).map(|index| ExprId(index as u32)).find(|id| &self.text[self.design.expr_node(*id).span.range()] == snippet).and_then(|id| types.callee(id))
    }

    /// The type the checker gives the expression whose text is `snippet`.
    fn type_of(&self, snippet: &str) -> Option<Ty> {
        let types = Types::check(&self.design, "Top").unwrap_or_else(|found| panic!("{:?}\n{}", Self::codes_of(&found), self.text));
        (0..self.design.expr_count()).map(|index| ExprId(index as u32)).find(|id| &self.text[self.design.expr_node(*id).span.range()] == snippet).and_then(|id| types.ty(id).cloned())
    }
}

#[test]
fn a_template_is_a_value_of_its_modules_type() {
    // §8.4.1 (GN-21 AC 1): a template goes in a `const`, a function's result and a param value
    let source = TemplateSource::new(
        "const C1: Cell = Cell { cs: 30.0 };\nfn pick(big: bool) -> Cell { if big { Cell { cs: 2.0 } } else { Cell { cs: 1.0 } } }\n\
         module Top(x: E) { cell: Cell = C1; other: Cell = pick(true); }",
    );
    assert_eq!(source.codes(), Vec::<String>::new(), "{}", source.message());
    assert!(matches!(source.type_of("Cell { cs: 30.0 }"), Some(Ty::Module(_))));
}

#[test]
fn a_template_leaving_a_required_param_is_e0319() {
    // §8.4.2 (GN-21 AC 2): every required param is bound in a template
    let source = TemplateSource::new("module Top(x: E) { cell: Sizing = Sizing { }; }");
    assert_eq!(source.codes(), ["E0319"], "{}", source.message());
    assert!(source.message().contains("size"), "{}", source.message());
}

#[test]
fn a_template_that_is_not_const_is_e0362() {
    // §8.4.6 (GN-21 AC 3): a template-valued expression is a const expression
    let source = TemplateSource::new("module Top(x: E) { let t = Cell { cs: v(x) }; }");
    assert_eq!(source.codes(), ["E0362"], "{}", source.message());
}

#[test]
fn a_template_is_never_carried_e0112() {
    // §8.4.1 (GN-21 AC 4): a template has no `Repr`, so it is never carried
    let source = TemplateSource::new("module Top(x: E) { signal s: Bit<Cell>; }");
    assert_eq!(source.codes(), ["E0112"], "{}", source.message());
}

#[test]
fn a_placement_through_a_template_or_a_function_types_as_its_module() {
    // §8.4.4 (GN-22 AC 1): `C1(x)`, `cell(x)` and `pick(true)(x)` place the module of the template, their connections checked
    // against its ports (`E0312` for the count, as for `Cell(x)`)
    let head = "const C1: Cell = Cell { cs: 30.0 };\nfn pick(big: bool) -> Cell { if big { Cell { cs: 2.0 } } else { Cell { cs: 1.0 } } }\n";
    let source = TemplateSource::new(&format!("{head}module Top(x: E) {{ cell: Cell = C1; let a = C1(x); let b = cell(x); let c = pick(true)(x); }}"));
    assert_eq!(source.codes(), Vec::<String>::new(), "{}", source.message());
    for call in ["C1(x)", "cell(x)", "pick(true)(x)"] {
        assert!(matches!(source.callee_of(call), Some(Callee::Module(_))), "{call} is a placement of Cell");
    }
    let wrong = TemplateSource::new(&format!("{head}module Top(x: E, y: E) {{ let a = C1(x, y); let c = pick(true)(x, y); }}"));
    assert_eq!(wrong.codes(), ["E0312", "E0312"], "{}", wrong.message());
}

#[test]
fn a_param_block_on_a_template_placement_is_e0361() {
    // §8.4.4 (GN-22 AC 2): the params belong to the template, so `C1(x) { cs: 2.0 }` and `pick(true)(x) { cs: 2.0 }` are `E0361`
    let head = "const C1: Cell = Cell { cs: 30.0 };\nfn pick(big: bool) -> Cell { Cell { cs: 2.0 } }\n";
    let source = TemplateSource::new(&format!("{head}module Top(x: E) {{ let a = C1(x) {{ cs: 2.0 }}; let c = pick(true)(x) {{ cs: 2.0 }}; }}"));
    assert_eq!(source.codes(), ["E0361", "E0361"], "{}", source.message());
}

/// Two modules with the ports of `Fn(E, E)`, one that differs, and a bank bound by `C: Fn(E, E)`.
const BANK: &str = "module Pair(a: E, b: E) { cs: f64 = 1.0; }\nmodule Twin(p: E, q: E) { cs: f64 = 2.0; }\n\
module Wrong(a: E, in b: Bit) { }\nmodule Short(a: E) { }\n\
module Bank<C: Fn(E, E)>(x: E, y: E) { cell: C; let first = cell(x, y); }\n";

#[test]
fn a_module_whose_ports_match_fits_fn_p_names_not_counting() {
    // §8.4.5 (GN-23 AC 1): `Pair(a, b)` and `Twin(p, q)` have the types and directions of `Fn(E, E)`, names not counting
    let source = TemplateSource::new(&format!("{BANK}module Top(x: E, y: E) {{ let one = Bank::<Pair>(x, y) {{ cell: Pair {{ cs: 3.0 }} }}; let two = Bank::<Twin>(x, y) {{ cell: Twin {{ }} }}; }}"));
    assert_eq!(source.codes(), Vec::<String>::new(), "{}", source.message());
}

#[test]
fn a_module_whose_ports_differ_is_e0360_naming_the_first() {
    // §8.4.5 (GN-23 AC 2): `Wrong` differs at its second port, `Short` has too few: `E0360` naming the first port that differs
    let wrong = TemplateSource::new(&format!("{BANK}module Top(x: E, y: E) {{ let one = Bank::<Wrong>(x, y) {{ cell: Wrong {{ }} }}; }}"));
    assert_eq!(wrong.codes(), ["E0360"], "{}", wrong.message());
    assert!(wrong.message().contains("`b`") && wrong.message().contains("Wrong"), "names the module and its first differing port: {}", wrong.message());
    let short = TemplateSource::new(&format!("{BANK}module Top(x: E, y: E) {{ let one = Bank::<Short>(x, y) {{ cell: Short {{ }} }}; }}"));
    assert_eq!(short.codes(), ["E0360"], "{}", short.message());
}

#[test]
fn a_fn_bounded_parameter_is_inferred_from_its_template_param() {
    // §8.1 and §8.4.1 (GN-23): `cell: C` holds a template, and the placement that binds the param names `C`
    // (`Bank(x, y) { cell: Pair { cs: 3.0 } }` infers `C = Pair`); a placement that leaves it out is `E0181`
    let inferred = TemplateSource::new(&format!("{BANK}module Top(x: E, y: E) {{ let one = Bank(x, y) {{ cell: Pair {{ cs: 3.0 }} }}; }}"));
    assert_eq!(inferred.codes(), Vec::<String>::new(), "{}", inferred.message());
    let open = TemplateSource::new(&format!("{BANK}module Top(x: E, y: E) {{ let one = Bank(x, y); }}"));
    assert_eq!(open.codes(), ["E0181"], "{}", open.message());
    assert!(open.message().contains("`C`"), "{}", open.message());
}

#[test]
fn a_param_default_reads_earlier_params_and_a_template_params_trait() {
    // §8.1.16: a default is a const expression over earlier params; `fet.gain()` reads the template param `fet` through the trait
    // half of `N: Mosfet + Fn(E)` (§8.4.5), typed `f64` by the trait's signature
    let source = TemplateSource::new(
        "trait Mosfet { fn gain(self) -> f64; }\nimpl Mosfet for Cell { fn gain(self) -> f64 { self.cs * 2.0 } }\n\
         module Pair<N: Mosfet + Fn(E)>(a: E) { fet: N; level: f64 = fet.gain(); r: f64 = 2.0; g: f64 = level / r; let one = fet(a); }\n\
         module Top(x: E) { let pair = Pair::<_>(x) { fet: Cell { cs: 3.0 } }; }",
    );
    assert_eq!(source.codes(), Vec::<String>::new(), "{}", source.message());
    assert_eq!(source.type_of("fet.gain()"), Some(Ty::F64));
    assert_eq!(source.type_of("level / r"), Some(Ty::F64));
}

/// A module generic over a const and one generic over the nature of its ports.
const GENERIC_MODULES: &str = "module G<const N: usize>(a: E) where N >= 1 { k: f64 = 1.0; }\n\
    module S<K: Nature>(a: K, b: K) { r: f64 = 1.0; }\n";

#[test]
fn the_type_of_a_generic_template_carries_its_arguments() {
    // §8.4.3: a template of a generic module is of the type `Name<G>`, so `G::<3> { .. }` where a `G<2>` is wanted is `E0180`
    // naming both, and an argument the template leaves out is the one the wanted type names
    let wrong = TemplateSource::new(&format!("{GENERIC_MODULES}const T: G<2> = G::<3> {{ k: 3.0 }};\nmodule Top(x: E) {{ let c = T(x); }}"));
    assert_eq!(wrong.codes(), ["E0180"]);
    assert!(wrong.message().contains("expected `module `G<2>``, found `module `G<3>``"), "{}", wrong.message());
    let inferred = TemplateSource::new(&format!("{GENERIC_MODULES}const T: G<2> = G {{ k: 3.0 }};\nmodule Top(x: E) {{ let c = T(x); }}"));
    assert_eq!(inferred.codes(), Vec::<String>::new(), "{}", inferred.message());
    let Some(Ty::Module(template)) = inferred.type_of("G { k: 3.0 }") else { panic!("a template is of a module type") };
    assert_eq!(template.arguments, [GenericValue::Const(ConstTerm::Value(2))], "the template's type names the argument 2");
}

#[test]
fn a_placement_through_a_template_of_a_nature_generic_module_checks_its_ports() {
    // §8.4.3, §8.4.4: the ports of `S<K>` are read under the arguments the template's type names: `S<E>` takes nodes of `E`,
    // from a template in place, a `const`, a param of type `S<E>` and a param bounded by `Fn(E, E)`; `S<F>` at a node of `E` is
    // `E0251`, a node of another nature
    for tail in [
        "module Top(x: E, y: E) { let c = S::<E> { r: 2.0 }(x, y); }",
        "const T: S<E> = S::<E> { r: 2.0 };\nmodule Top(x: E, y: E) { let c = T(x, y); }",
        "module H(a: E, b: E) { cell: S<E>; let c = cell(a, b); }\nmodule Top(x: E, y: E) { let h = H(x, y) { cell: S::<E> { r: 2.0 } }; }",
        "module H<C: Fn(E, E)>(a: E, b: E) { cell: C; let c = cell(a, b); }\nmodule Top(x: E, y: E) { let h = H::<_>(x, y) { cell: S::<E> { r: 2.0 } }; }",
    ] {
        let source = TemplateSource::new(&format!("{GENERIC_MODULES}{tail}"));
        assert_eq!(source.codes(), Vec::<String>::new(), "{tail}: {}", source.message());
    }
    let other = "nature F { potential: T { access: te, units: \"K\", abstol: 1u }, flow: P { access: pw, units: \"W\", abstol: 1p } }\n";
    let wrong = TemplateSource::new(&format!("{GENERIC_MODULES}{other}const T: S<F> = S::<F> {{ r: 2.0 }};\nmodule Top(x: E, y: E) {{ let c = T(x, y); }}"));
    assert_eq!(wrong.codes()[0], "E0251", "{}", wrong.message());
}

#[test]
fn a_non_node_item_through_a_template_takes_the_type_its_context_wants() {
    // §8.4.8: `c0.cs` names an item of a module only elaboration knows; the checker gives it the type its context wants (here the
    // `f64` of a contribution) and records it for the elaborator; a context that wants none, a method's receiver, is `E0181`
    let bank = |items: &str| {
        TemplateSource::new(&format!(
            "module Bank<C: Fn(E)>(a: E) {{ cell: C; let c0 = cell(a); signal p: E; }}\nanalog<C> for Bank<C> {{ {items} }}\n\
             module Top(x: E) {{ let bank = Bank::<_>(x) {{ cell: Cell {{ cs: 2.0 }} }}; }}"
        ))
    };
    let typed = bank("i(p, gnd) +> c0.cs * 2.0;");
    let types = Types::check(&typed.design, "Top").unwrap_or_else(|found| panic!("{:?}", TemplateSource::codes_of(&found)));
    let read = (0..typed.design.expr_count()).map(|index| ExprId(index as u32)).find(|id| &typed.text[typed.design.expr_node(*id).span.range()] == "c0.cs").expect("the read");
    assert_eq!(types.template_item(read), Some(&Ty::F64), "the read takes the `f64` its context wants");
    let untyped = bank("i(p, gnd) +> c0.cs.abs();");
    assert_eq!(untyped.codes(), ["E0181"], "{}", untyped.message());
}
