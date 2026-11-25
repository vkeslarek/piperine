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
    // §8.4.4 (GN-22 AC 1): `C1(x)`, `cell(x)`, `pick(true)(x)` and `t(x)` with `let t = Cell { .. }` place the module of the template, their connections checked
    // against its ports (`E0312` for the count, as for `Cell(x)`)
    let head = "const C1: Cell = Cell { cs: 30.0 };\nfn pick(big: bool) -> Cell { if big { Cell { cs: 2.0 } } else { Cell { cs: 1.0 } } }\n";
    let source = TemplateSource::new(&format!("{head}module Top(x: E) {{ cell: Cell = C1; let a = C1(x); let b = cell(x); let c = pick(true)(x); let t = Cell {{ cs: 2.0 }}; let d = t(x); let u = (C1); let e = u(x); }}"));
    assert_eq!(source.codes(), Vec::<String>::new(), "{}", source.message());
    // a body `let` bound to a template (finding 29) is a holder like the `const` and the param
    for call in ["C1(x)", "cell(x)", "pick(true)(x)", "t(x)", "u(x)"] {
        assert!(matches!(source.callee_of(call), Some(Callee::Module(_))), "{call} is a placement of Cell");
    }
    let wrong = TemplateSource::new(&format!("{head}module Top(x: E, y: E) {{ let a = C1(x, y); let c = pick(true)(x, y); let t = Cell {{ cs: 2.0 }}; let d = t(x, y); }}"));
    assert_eq!(wrong.codes(), ["E0312", "E0312", "E0312"], "{}", wrong.message());
    // the instance a placement names is no template: calling it reads it as a value, `E0333` (§8.2.14, finding 30)
    let instance = TemplateSource::new(&format!("{head}module Top(x: E) {{ let a = C1(x); let b = a(x); }}"));
    assert_eq!(instance.codes(), ["E0333"], "{}", instance.message());
    // a value that is neither a function nor a template in the callee's position is a type mismatch, `E0180` (§1.5, finding 32)
    let value = TemplateSource::new(&format!("{head}module Top(x: E) {{ let k = 2.0; let b = k(x); }}"));
    assert_eq!(value.codes(), ["E0180"], "{}", value.message());
    assert!(value.message().contains("its type `f64` is no function and no template"), "{}", value.message());
    // a choice of templates of one module that folds at elaboration holds a template too (§8.4.6, finding 31); one the run makes
    // is no const expression, `E0362`
    let chosen = TemplateSource::new(&format!("{head}module Top(x: E) {{ sel: bool = true; n: u8 = 0; let t = if sel {{ Cell {{ cs: 2.0 }} }} else {{ C1 }}; let d = t(x); let m = match n {{ 0 => C1, _ => pick(false) }}; let e = m(x); let b = {{ C1 }}; let f = b(x); }}"));
    assert_eq!(chosen.codes(), Vec::<String>::new(), "{}", chosen.message());
    for call in ["t(x)", "m(x)", "b(x)"] {
        assert!(matches!(chosen.callee_of(call), Some(Callee::Module(_))), "{call} is a placement of Cell");
    }
    let by_the_run = TemplateSource::new(&format!("{head}module Top(x: E) {{ let t = if v(x) > 0.5 {{ C1 }} else {{ Cell {{ cs: 2.0 }} }}; let d = t(x); }}"));
    assert_eq!(by_the_run.codes(), ["E0362"], "{}", by_the_run.message());
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

#[test]
fn gnd_connects_through_a_callable_bound_as_at_a_placement() {
    // §8.1.10, §8.4.5: a placement through `cell: C`, `C: Fn(E, E)`, takes what a placement of the module takes at each node
    // input: `gnd`, a node of any nature, as well as a node of `E`; a node of another nature is `E0251`, as at a placement
    let bank = |connections: &str| {
        TemplateSource::new(&format!(
            "module R(a: E, b: E) {{ r: f64 = 1.0; }}\n\
             nature F {{ potential: T {{ access: te, units: \"K\", abstol: 1u }}, flow: P {{ access: pw, units: \"W\", abstol: 1p }} }}\n\
             module H<C: Fn(E, E)>(a: E) {{ cell: C; signal f: F; let c = cell({connections}); }}\n\
             module Top(x: E) {{ let h = H::<_>(x) {{ cell: R {{ r: 2.0 }} }}; }}"
        ))
    };
    for connections in ["a, gnd", "gnd, a", "a, a"] {
        let source = bank(connections);
        assert_eq!(source.codes(), Vec::<String>::new(), "{connections}: {}", source.message());
    }
    assert_eq!(bank("a, f").codes(), ["E0251"]);
}

#[test]
fn a_template_leaves_an_option_param_out_as_a_placement_does() {
    // §8.1.17, §8.4.2: an `Option<T>` param with no default is not required, so a template that leaves it out is clean in a
    // `const`, a param value and in place, as the written placement is; a required one it leaves out is still `E0319`
    let module = "module D(a: E) { r: f64 = 1.0; o: Option<f64>; }\n";
    for tail in [
        "const T: D = D { r: 2.0 };\nmodule Top(x: E) { let d = T(x); }",
        "module Top(x: E) { cell: D = D { r: 2.0 }; }",
        "module Top(x: E) { let d = D { r: 2.0 }(x); }",
        "module Top(x: E) { let d = D(x) { r: 2.0 }; }",
    ] {
        let source = TemplateSource::new(&format!("{module}{tail}"));
        assert_eq!(source.codes(), Vec::<String>::new(), "{tail}: {}", source.message());
    }
    let required = TemplateSource::new("module D(a: E) { r: f64; o: Option<f64>; }\nconst T: D = D { o: 1.0 };\nmodule Top(x: E) { let d = T(x); }");
    assert_eq!(required.codes(), ["E0319"], "{}", required.message());
    assert!(required.message().contains("the required param `r`"), "{}", required.message());
}

#[test]
fn a_cast_of_a_template_item_is_e0181() {
    // §1.4.5, §8.4.8: `as` gives its result a type, never its operand, so `c0.cs as f32` through a template has no type to give
    // the read: `E0181` at the read, never `E0182` from a node; a typed `let` first casts as usual
    let bank = |items: &str| {
        TemplateSource::new(&format!(
            "module Bank<C: Fn(E)>(a: E) {{ cell: C; let c0 = cell(a); signal p: E; }}\nanalog<C> for Bank<C> {{ {items} }}\n\
             module Top(x: E) {{ let bank = Bank::<_>(x) {{ cell: Cell {{ cs: 2.0 }} }}; }}"
        ))
    };
    let cast = bank("i(p, gnd) +> v(p, gnd) * (c0.cs as f64);");
    assert_eq!(cast.codes(), ["E0181"], "{}", cast.message());
    let typed = bank("let cs: f64 = c0.cs; i(p, gnd) +> v(p, gnd) * (cs as f64);");
    assert_eq!(typed.codes(), Vec::<String>::new(), "{}", typed.message());
}

#[test]
fn an_unread_let_of_a_template_item_is_e0181() {
    // §8.4.8: `let x = c0.cs;` that nothing reads gets no type from a use, so it is `E0181` at the read; a read of it that wants a
    // value types it
    let bank = |items: &str| {
        TemplateSource::new(&format!(
            "module Bank<C: Fn(E)>(a: E) {{ cell: C; let c0 = cell(a); signal p: E; }}\nanalog<C> for Bank<C> {{ {items} }}\n\
             module Top(x: E) {{ let bank = Bank::<_>(x) {{ cell: Cell {{ cs: 2.0 }} }}; }}"
        ))
    };
    let unread = bank("let x = c0.cs; i(p, gnd) +> 1.0;");
    assert_eq!(unread.codes(), ["E0181"], "{}", unread.message());
    let read = bank("let x = c0.cs; i(p, gnd) +> x;");
    assert_eq!(read.codes(), Vec::<String>::new(), "{}", read.message());
}

#[test]
fn a_template_param_value_may_read_the_placing_modules_params() {
    // §8.4.6 (finding 36): a template value handed to a param of module type is a const expression over params, so it may read
    // the placing module's params; one that reads a port is `E0362`
    let head = "module Plain(a: E) { cell: Cell; let c0 = cell(a); }\nfn sized(l: f64) -> Cell { Cell { cs: l } }\n\
                fn pick(big: bool) -> Cell { if big { Cell { cs: 2.0 } } else { Cell { cs: 1.0 } } }\n";
    for (form, codes) in [
        ("let b = Plain(x) { cell: Cell { cs: l } };", &[][..]),
        ("let b = Plain(x) { cell: sized(l) };", &[]),
        ("let b = Plain(x) { cell: pick(sel) };", &[]),
        ("let b = Plain(x) { cell: if sel { Cell { cs: l } } else { Cell } };", &[]),
        ("let b = Plain(x) { cell: Cell { cs: v(x) } };", &["E0362"]),
    ] {
        let source = TemplateSource::new(&format!("{head}module Top(x: E) {{ l: f64 = 2.0; sel: bool = true; {form} }}"));
        assert_eq!(source.codes(), codes, "{form}: {}", source.message());
    }
}

#[test]
fn a_choice_of_two_modules_is_one_e0180_and_its_call_adds_none() {
    // finding 37: the arms of a template choice name two modules (`E0180`, §1.5.4); a call through the `let` that holds it adds
    // no second diagnostic, and a template's type is never "no function and no template"
    for form in [
        "let t = if sel { Cell {} } else { Sizing { size: 1.0 } }; let b = t(x);",
        "let t = if sel { Cell {} } else { Sizing { size: 1.0 } };",
        "let b = t(x); let t = if sel { Cell {} } else { Sizing { size: 1.0 } };",
    ] {
        let source = TemplateSource::new(&format!("module Top(x: E) {{ sel: bool = true; {form} }}"));
        assert_eq!(source.codes(), ["E0180"], "{form}: {}", source.message());
        assert!(!source.message().contains("no function and no template"), "{form}: {}", source.message());
    }
}

#[test]
fn a_bare_module_name_used_as_a_value_is_its_template_with_every_default() {
    // §8.4 amendment (finding 35): a module's bare name used as a value is the template `Name {}`, every param at its
    // default, wherever a template goes (a `let`, a head, a param value, a function result); a required param it leaves
    // unbound is `E0319`, alone
    let head = "fn cell() -> Cell { Cell }\nfn sizing() -> Sizing { Sizing }\nmodule Bank(a: E) { cell: Cell = Cell; }\n";
    let forms: [(&str, &[&str]); 10] = [
        ("let t = Cell; let b = t(x);", &[]),
        ("let b = (Cell)(x);", &[]),
        ("let b = cell()(x);", &[]),
        ("let b = Bank(x) { cell: Cell };", &[]),
        ("let b = Bank(x);", &[]),
        ("let t = Sizing; let b = t(x);", &["E0319"]),
        ("let b = (Sizing)(x);", &["E0319"]),
        ("let t = Sizing;", &["E0319"]),
        ("let b = sizing()(x);", &["E0319"]),
        ("let b = (if true { Cell } else { Cell { cs: 2.0 } })(x);", &[]),
    ];
    for (form, codes) in forms {
        let source = TemplateSource::new(&format!("{head}module Top(x: E) {{ {form} }}"));
        assert_eq!(source.codes(), codes, "{form}: {}", source.message());
    }
}

#[test]
fn every_callee_form_the_grammar_admits_is_called_by_its_value() {
    // Part III `callee ::= path generic_args? | call | '(' expr ')'` (finding 34, the class "every callee form"): each alternative is
    // a placement when its value is a template (§8.4.4, §8.4.6), a call when it is a function, and `E0180` when it is neither;
    // no form reaches a generic refusal
    let head = "const C1: Cell = Cell { cs: 30.0 };\nfn pick(big: bool) -> Cell { if big { Cell { cs: 2.0 } } else { Cell { cs: 1.0 } } }\n\
                fn twice(y: f64) -> f64 { y * 2.0 }\nfn doubler() -> fn(f64) -> f64 { twice }\n";
    let forms: [(&str, &[&str]); 14] = [
        ("let b = Cell(x);", &[]),
        ("let b = twice(1.0);", &[]),
        ("let b = pick(true)(x);", &[]),
        ("let b = doubler()(1.0);", &[]),
        ("let b = (Cell { cs: 2.0 })(x);", &[]),
        ("let b = (C1)(x);", &[]),
        ("let b = (if true { Cell { cs: 2.0 } } else { C1 })(x);", &[]),
        ("let b = (match n { 0 => C1, _ => Cell { cs: 2.0 } })(x);", &[]),
        ("let b = ({ C1 })(x);", &[]),
        ("let b = (|y: f64| y * 2.0)(1.0);", &[]),
        ("let b = (twice)(1.0);", &[]),
        ("let b = (if true { twice } else { twice })(1.0);", &[]),
        ("let b = (2.0)(x);", &["E0180"]),
        ("let b = (if v(x) > 0.5 { C1 } else { Cell { cs: 2.0 } })(x);", &["E0362"]),
    ];
    for (form, codes) in forms {
        let source = TemplateSource::new(&format!("{head}module Top(x: E) {{ n: u8 = 0; {form} }}"));
        assert_eq!(source.codes(), codes, "{form}: {}", source.message());
    }
}

#[test]
fn an_array_of_templates_of_one_module_checks_and_places_its_element() {
    // §8.4.1, §8.4.4 (GAPS L88): `[Cell { }, Cell { cs: 2.0 }]` is an array of `Cell` values, `fs[1](x)` places the module of its
    // element, a `[Cell; 2]` `const` holds one, and an array of two modules is a mismatch (`E0180`)
    let held = TemplateSource::new("module Top() { signal x: E; let fs = [Cell { }, Cell { cs: 2.0 }]; let c = fs[1](x); }\n");
    assert!(held.codes().is_empty(), "{:?}", held.codes());
    let declared = TemplateSource::new("const PAIR: [Cell; 2] = [Cell { }, Cell { cs: 2.0 }];\nmodule Top() { signal x: E; let c = PAIR[0](x); }\n");
    assert!(declared.codes().is_empty(), "{:?}", declared.codes());
    let mixed = TemplateSource::new("module Top() { signal x: E; let fs = [Cell { }, Sizing { size: 1.0 }]; }\n");
    assert_eq!(mixed.codes(), ["E0180"], "two modules in one array");
}

#[test]
fn a_param_declared_as_an_array_of_templates_checks_its_default_and_its_placement() {
    // §8.1, §8.4.1, §8.4.4 (GAPS L89): `pair: [Cell; 2] = [..]` is a param whose elements are templates of `Cell`; `pair[k](x)` places
    // the module of an element; a default of the wrong module is `E0180`, a default of another length a mismatch
    let ok = TemplateSource::new(
        "module Top() { pair: [Cell; 2] = [Cell { }, Cell { cs: 2.0 }]; signal x: E; let c = pair[1](x); for k in 0..2 { let d = pair[k](x); } }\n",
    );
    assert!(ok.codes().is_empty(), "{:?}: {}", ok.codes(), ok.message());
    let wrong = TemplateSource::new(
        "module Top() { pair: [Cell; 2] = [Cell { }, Sizing { size: 1.0 }]; }\n",
    );
    assert_eq!(wrong.codes(), ["E0180"], "an element of another module");
    let short = TemplateSource::new("module Top() { pair: [Cell; 2] = [Cell { }]; }\n");
    assert!(
        !short.codes().is_empty(),
        "a default of one element for two"
    );
}
