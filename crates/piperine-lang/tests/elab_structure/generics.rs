//! Scope: the elaborator's generic instances (generics GN-01, GN-02, GN-07, part I §3.8): the
//! elaborator reads the type table only through the substitution of the instance being lowered, a
//! generic function is instantiated once for each list of arguments, an instantiation that makes a
//! `where`, a length or a constant invalid is reported naming the item and its arguments, and the
//! POM keeps the generic item as written.

use std::path::{Path, PathBuf};

use crate::support;

use crate::elab_forms;

use elab_forms::IrReading;
use support::Elab;

/// The source files of `piperine_lang::elab`.
struct ElabSources;

impl ElabSources {
    /// Every `.rs` file below `src/elab`, with its path relative to the crate.
    fn files() -> Vec<(PathBuf, String)> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/elab");
        let mut found = Vec::new();
        Self::collect(&root, &mut found);
        found.sort();
        found.into_iter().map(|path| (path.strip_prefix(env!("CARGO_MANIFEST_DIR")).unwrap_or(&path).to_path_buf(), std::fs::read_to_string(&path).unwrap_or_default())).collect()
    }

    /// Appends the `.rs` files below `directory` to `found`.
    fn collect(directory: &Path, found: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(directory).expect("src/elab is readable").flatten() {
            let path = entry.path();
            if path.is_dir() {
                Self::collect(&path, found);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(path);
            }
        }
    }
}

#[test]
fn the_elaborator_reads_types_only_through_the_substitution() {
    // GN-01 AC 1, C47: no `Ty::Param` reaches the IR because the elaborator reads the type table of an expression, a
    // binding and a written type only through `typed`, `typed_local` and `typed_written`, which apply the substitution
    // of the instance being lowered; the three raw reads live in `ConstEval` (`elab/consts.rs`) and nowhere else
    let mut direct: Vec<String> = Vec::new();
    for (path, text) in ElabSources::files() {
        for (number, line) in text.lines().enumerate() {
            let reads = [".types.ty(", ".types.local(", ".types.written("].iter().any(|read| line.contains(read));
            if reads && !path.ends_with("consts.rs") {
                direct.push(format!("{}:{}: {}", path.display(), number + 1, line.trim()));
            }
        }
    }
    assert!(direct.is_empty(), "a direct read of the type table outside `ConstEval`:\n{}", direct.join("\n"));
    let consts = ElabSources::files().into_iter().find(|(path, _)| path.ends_with("elab/consts.rs")).map(|(_, text)| text).unwrap_or_default();
    for helper in ["fn typed(", "fn typed_local(", "fn typed_written("] {
        assert!(consts.contains(helper), "ConstEval has `{helper}`");
    }
}

/// A design whose `analog` block calls the generic functions of `items` with `statement`, and the diagnostics of elaborating it.
struct GenericCall;

impl GenericCall {
    /// The `(code, message)` of each diagnostic of elaborating `Top`, whose `analog` runs `statement`, after `items`.
    fn found(items: &str, statement: &str) -> Vec<(String, String)> {
        let src = format!("{items}\nmodule Top() {{ signal o: Electrical; }}\nanalog for Top {{ {statement} }}\n");
        match Elab::run(&src, "Top") {
            Ok(_) => Vec::new(),
            Err(found) => found.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect(),
        }
    }
}

#[test]
fn a_false_where_is_e0127_naming_the_instantiation() {
    // §3 rule 8, GN-01 AC 4: the `where` is checked for each instantiation; a false one is E0127 naming `clamp::<1>`
    let items = "fn clamp<const N: usize>(x: f64) -> f64 where N > 2 { x }";
    let found = GenericCall::found(items, "v(o) +> clamp::<1>(1.0);");
    assert_eq!(found.iter().map(|(code, _)| code.as_str()).collect::<Vec<_>>(), ["E0127"], "{found:?}");
    assert!(found[0].1.contains("the `where` of `clamp::<1>` is false"), "{}", found[0].1);
    // a true `where` passes the instantiation, which is then lowered
    assert_eq!(GenericCall::found(items, "v(o) +> clamp::<3>(1.0);"), Vec::<(String, String)>::new());
}

#[test]
fn instantiation_errors_name_the_item_and_its_arguments() {
    // GN-07 AC 3: two lengths that differ at an instantiation, and a const expression that fails, are reported at the
    // instantiation naming the item and its arguments
    let items = "fn grown<const N: usize>(a: [f64; N]) -> [f64; N + 1] { a }\nfn shrunk<const N: usize>(a: [f64; N]) -> f64 { let b = [0.0; N - 1]; 1.0 }";
    let lengths = GenericCall::found(items, "let r = grown([1.0, 2.0]);");
    assert!(lengths.iter().any(|(code, message)| code == "E0180" && message.contains("`grown::<2>`") && message.contains("an array of 3 elements is expected, found one of 2")), "{lengths:?}");
    let constant = GenericCall::found(items, "let e: [f64; 0] = []; let r = shrunk(e);");
    assert!(constant.iter().any(|(code, message)| code == "E0104" && message.contains("`shrunk::<0>`")), "{constant:?}");
}

/// The IR functions of a design: their names and the instances they record.
struct IrFunctions;

impl IrFunctions {
    /// The elaborated `Top` of `src`, which must pass `Ir::check`.
    fn of(items: &str, statements: &str) -> piperine_ir::Ir {
        Elab::ir(&format!("{items}\nmodule Top() {{ signal o: Electrical; }}\nanalog for Top {{ {statements} }}\n"), "Top")
    }

    /// The names of the functions, in id order.
    fn names(ir: &piperine_ir::Ir) -> Vec<String> {
        ir.functions().map(|function| function.name().to_string()).collect()
    }
}

#[test]
fn one_ir_function_per_distinct_instantiation_named_by_its_arguments() {
    // GN-02 AC 1, §3.8.10: a generic function is one IR function for each list of arguments, named from the function and
    // them, and each records the instance it is; a function as written records none
    let items = "fn pick<T>(a: T, b: T) -> T { a }\nfn plain(x: f64) -> f64 { x }";
    let ir = IrFunctions::of(items, "let x = pick(1.0, 2.0); let y = pick(true, false); let z = pick(3u8, 4u8); let w = plain(1.0); v(o) +> x + w;");
    let mut names = IrFunctions::names(&ir);
    names.sort();
    assert_eq!(names, ["pick::<bool>", "pick::<f64>", "pick::<u8>", "plain"]);
    for function in ir.functions() {
        match function.name() {
            "plain" => assert!(function.instance().is_none()),
            name => {
                let instance = function.instance().unwrap_or_else(|| panic!("`{name}` records its instance"));
                assert_eq!(instance.item, "pick");
                assert_eq!(instance.name(), name);
            }
        }
    }
}

#[test]
fn equal_arguments_make_one_instance() {
    // §3.8.10: two calls with equal arguments are one instance, whatever the values they pass
    let items = "fn pick<T>(a: T, b: T) -> T { a }";
    let ir = IrFunctions::of(items, "let x = pick(1.0, 2.0); let y = pick(3.0, 4.0); let z = pick::<f64>(5.0, 6.0); v(o) +> x + y + z;");
    assert_eq!(IrFunctions::names(&ir), ["pick::<f64>"]);
}

#[test]
fn an_endless_instantiation_chain_is_e0341() {
    // D7: `deep::<(f64,)>`, `deep::<((f64,),)>`, ... never ends: past 1 024 open instantiations it is
    // E0341 naming the function
    let items = "fn deep<T>(x: T) -> f64 { deep((x,)) }";
    let found = GenericCall::found(items, "v(o) +> deep(1.0);");
    assert!(found.iter().any(|(code, message)| code == "E0341" && message.contains("`deep`")), "{found:?}");
}

/// The declarations of a design named `name`, with whether each writes a `<..>` list.
struct Declarations;

impl Declarations {
    /// The `generics_span.is_some()` of each declaration of `design` called `name`, in declaration order.
    fn written_generic(design: &piperine_lang::pom::Design, name: &str) -> Vec<bool> {
        (0..design.decl_count())
            .map(|index| design.decl_data(piperine_lang::pom::DeclId(index as u32)))
            .filter(|data| data.name.as_ref().is_some_and(|written| written.text == name))
            .map(|data| data.generics_span.is_some())
            .collect()
    }
}

#[test]
fn the_pom_keeps_the_generic_function_as_written() {
    // GN-02 AC 2, MD-25: elaboration only reads the design: the POM holds `pick<T>` once, with its list, before and after
    // instantiating it at `f64` and at `u8`, and no declaration is named for an instance (`pick::<f64>`); the instances
    // are IR functions
    let src = "fn pick<T>(a: T, b: T) -> T { a }\nmodule Top() { signal o: Electrical; }\nanalog for Top { let x = pick(1.0, 2.0); let y = pick(3u8, 4u8); v(o) +> x; }\n";
    let design = Elab::design(src);
    assert!(design.diagnostics().is_empty(), "{:?}", Elab::codes(design.diagnostics()));
    let before = Declarations::written_generic(&design, "pick");
    let ir = piperine_lang::elab::Elaborator::new(&design).run("Top").unwrap_or_else(|found| panic!("{found:?}"));
    assert_eq!(before, [true], "one `pick`, written with its list");
    assert_eq!(Declarations::written_generic(&design, "pick"), before, "elaboration leaves the POM as it was");
    assert!(Declarations::written_generic(&design, "pick::<f64>").is_empty() && Declarations::written_generic(&design, "pick::<u8>").is_empty(), "no declaration for an instance");
    assert_eq!(IrFunctions::names(&ir).len(), 2, "the instances are IR functions");
}

#[test]
fn a_bound_that_writes_the_trait_arguments_picks_the_impl_of_that_list() {
    // GN-04 AC 5, T36: two `impl`s of `Convert<T>` for one type, `Convert<f64>` and `Convert<u8>`, are two IR functions,
    // and a call through `C: Convert<f64>` or `C: Convert<u8>` runs the one of its list (the bound's argument also gives
    // the result its type)
    let items = "trait Convert<T> { fn to(self) -> T; }\nstruct Cell;\nimpl Convert<f64> for Cell { fn to(self) -> f64 { 1.5 } }\nimpl Convert<u8> for Cell { fn to(self) -> u8 { 7 } }\nfn as_real<C: Convert<f64>>(c: C) -> f64 { c.to() }\nfn as_byte<C: Convert<u8>>(c: C) -> u8 { c.to() }";
    let ir = IrFunctions::of(items, "v(o) +> as_real(Cell) + as_byte(Cell) as f64;");
    assert_eq!(IrFunctions::names(&ir), ["as_real::<Cell>", "Cell.to::<f64>", "as_byte::<Cell>", "Cell.to::<u8>"]);
}

/// A call of an `extern "intrinsic"` function, written in each position a function body or a behavior has, and what
/// elaborating it reports.
struct ExternCalls;

impl ExternCalls {
    /// The `(code, message)` of each diagnostic of elaborating `src`'s `Top`; empty when it elaborates.
    fn found(src: &str) -> Vec<(String, String)> {
        match Elab::run(src, "Top") {
            Ok(_) => Vec::new(),
            Err(found) => found.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect(),
        }
    }
}

#[test]
fn a_pattern_call_lowers_in_any_position() {
    // replaces `a_generic_extern_call_is_never_dropped_in_any_position` (T45, GAPS L68): `matches_bits` is lowered, never
    // dropped and never refused, wherever the function holding it is called from (a reaction, `analog` or a `let`) and
    // wherever it stands in the function (tail, `if` arm, block tail, `match` arm, `let`): the elaborated IR holds its masks
    let tails = [
        "fn f(x: Logic<8>) -> bool { matches_bits(x, \"0000001x\") }",
        "fn f(x: Logic<8>) -> bool { if x.is_x() { matches_bits(x, \"0000001x\") } else { false } }",
        "fn f(x: Logic<8>) -> bool { { matches_bits(x, \"0000001x\") } }",
        "fn f(x: Logic<8>) -> bool { match x.is_x() { true => false, false => matches_bits(x, \"0000001x\") } }",
        "fn f(x: Logic<8>) -> bool { let y = matches_bits(x, \"0000001x\"); y }",
    ];
    let positions = [
        "module Top(in clk: Bit, out d: Bit = false) { }\ndigital for Top { on clk.rise() { d <- f(Logic::from(3u8)); } }",
        "module Top() { signal o: Electrical; }\nanalog for Top { let y = f(Logic::from(3u8)); v(o) +> if y { 1.0 } else { 0.0 }; }",
        "module Top() { signal o: Electrical; let b = f(Logic::from(3u8)); }\nanalog for Top { v(o) +> if b { 1.0 } else { 0.0 }; }",
    ];
    for tail in tails {
        for position in positions {
            let src = format!("{tail}\n{position}\n");
            let found = ExternCalls::found(&src);
            assert!(found.is_empty(), "{tail}\n{position}\n{found:?}");
        }
    }
}

#[test]
fn a_logic_call_in_a_function_tail_lowers() {
    // replaces `a_logic_call_in_a_function_tail_is_refused_by_name` (T45, GAPS L68): `logic("00000011")` as a function
    // tail, called from a reaction, `analog` and a body `let`, elaborates
    let tail = "fn g() -> Logic<8> { logic(\"0000001x\") }";
    for position in [
        "module Top(in clk: Bit, out d: Bit<Logic<8>>) { }\ndigital for Top { on clk.rise() { d <- g(); } }",
        "module Top() { signal o: Electrical; }\nanalog for Top { v(o) +> if g().is_high() { 1.0 } else { 0.0 }; }",
        "module Top() { signal o: Electrical; let b = g(); }\nanalog for Top { v(o) +> if b.is_high() { 1.0 } else { 0.0 }; }",
    ] {
        let found = ExternCalls::found(&format!("{tail}\n{position}\n"));
        assert!(found.is_empty(), "{position}\n{found:?}");
    }
}

/// One value of each layout `encode` and `decode` lower through the level cast, with its level count.
struct LayoutCases;

impl LayoutCases {
    /// The `(name, value written in a call, level count)` of each layout of part I §2 rule 2 to 6 that is no `bool`,
    /// unsigned integer or hand-written type.
    const ALL: [(&'static str, &'static str, u32); 6] = [
        ("f64", "1.5", 64),
        ("i8", "-3i8", 8),
        ("struct", "Trio { a: 1u8, b: true, c: 2u8 }", 17),
        ("enum", "Mode::Y", 2),
        ("tuple", "(1u8, true)", 9),
        ("array", "[1u8, 2u8]", 16),
    ];

    /// A source whose reaction calls `function` on `value`, over the declarations every case uses.
    fn source(function: &str, value: &str) -> String {
        format!(
            "#[derive(Repr)]\nstruct Trio {{ a: u8, b: bool, c: u8 }}\n#[derive(Repr, PartialEq)]\nenum Mode {{ X, Y, Z }}\n\
             fn first<T: Repr>(x: T) -> bool {{ x.encode()[0] }}\n\
             fn back<T: Repr>(x: T) -> bool {{ match T::decode(x.encode()) {{ Some(_) => true, None => false }} }}\n\
             module Top(in clk: Bit, out d: Bit = false) {{ }}\n\
             digital for Top {{ on clk.rise() {{ d <- {function}({value}); }} }}\n"
        )
    }
}

#[test]
fn encode_and_decode_of_every_layout_lower_through_the_level_cast() {
    // GN-01 AC 1 residual (GAPS L70), §2 rules 2 to 6 and 9: `x.encode()` of `f64`, a signed integer, a derived struct,
    // enum, tuple and array is a `LevelCast` of `x` to `[bool; WIDTH]`, and `T::decode(levels)` into them a `LevelCast`
    // of the levels to `T`; the IR checks, and no `P0001` names either call
    for (name, value, levels) in LayoutCases::ALL {
        for function in ["first", "back"] {
            let ir = Elab::ir(&LayoutCases::source(function, value), "Top");
            let text = ir.to_string();
            assert!(text.contains("level_cast"), "{name} through `{function}` holds a level cast:\n{text}");
            assert!(text.contains(&format!("[bool; {levels}]")), "{name} through `{function}` names `[bool; {levels}]`:\n{text}");
        }
    }
}

#[test]
fn cast_between_widths_is_e0127_naming_both_types() {
    // §22.1.3 rule 2 as amended by T55: `cast` is `std/values.msl`'s generic function whose `where A::WIDTH == B::WIDTH` is
    // false for a cast between types of different widths, so the instantiation is `E0127` naming `cast::<Mode, u8>`
    let found = ExternCalls::found("#[derive(Repr)] enum Mode { Idle, Run, Halt }\nmodule Top() { }\nanalog for Top { let a = cast::<Mode, u8>(Mode::Run); }\n");
    assert!(found.iter().any(|(code, message)| code == "E0127" && message.contains("Mode") && message.contains("u8")), "{found:?}");
}

/// A generic `Pair<T>` whose `Repr` is written by hand (two copies of the levels of `T`), carried at two arguments.
struct HandWrittenPair;

impl HandWrittenPair {
    /// The source: `Pair<u8>` and `Pair<bool>` each carried, so each instance is reached.
    const SOURCE: &'static str = "#[derive(PartialEq)] struct Pair<T> { a: T, b: T }\n\
        impl<T: Repr> Repr for Pair<T> {\n\
            const WIDTH: u32 = 2 * T::WIDTH;\n\
            fn encode(self) -> [bool; 2 * T::WIDTH] { let first = self.a.encode(); let second = self.b.encode(); [false; 2 * T::WIDTH] }\n\
            fn decode(levels: [bool; 2 * T::WIDTH]) -> Option<Pair<T>> { None }\n\
        }\n\
        module Top() { signal s: Bit<Pair<u8>> = Pair { a: 1u8, b: 2u8 }; signal t: Bit<Pair<bool>> = Pair { a: true, b: false }; }\n";
}

#[test]
fn each_instance_of_a_generic_hand_written_repr_has_its_own_codec() {
    // GN-32 AC 1 (§2 rule 2, §3.8.10): `Pair<u8>` and `Pair<bool>` are two arena entries, each `ReprForm::HandWritten` with the
    // folded `WIDTH` (`2 * T::WIDTH`: 16 and 2) and its own `encode` and `decode` named for the instance
    let ir = Elab::ir(HandWrittenPair::SOURCE, "Top");
    let mut found: Vec<(String, u32)> = Vec::new();
    for ty in ir.types() {
        if let piperine_ir::ReprForm::HandWritten { width, encode, decode } = &ty.repr {
            let instance = ty.instance.as_ref().expect("a hand-written instance is an instance").arguments.join(", ");
            assert!(ir.function(*encode).name().contains("encode") && ir.function(*encode).name().contains(&instance), "{}", ir.function(*encode).name());
            assert!(ir.function(*decode).name().contains("decode") && ir.function(*decode).name().contains(&instance), "{}", ir.function(*decode).name());
            found.push((instance, *width));
        }
    }
    found.sort();
    assert_eq!(found, [("bool".to_string(), 2), ("u8".to_string(), 16)]);
}

#[test]
fn a_const_fn_and_a_generic_repeat_fold_in_a_const_context() {
    // GN-14 AC 2 (§3.8, §1.8): a `const fn` called as a param value folds with arrays and loops, and an array repeat whose
    // length reads a const generic folds in a generic `const fn` called with explicit arguments
    let src = "\
        const fn poly_q(coefficients: [f64; 3]) -> f64 { let sum = 0.0; for k in 0..3 { sum += coefficients[k] * (k as f64 + 1.0); } sum }\n\
        const fn zeros<const N: usize>() -> [f64; N + 1] { [0.0; N + 1] }\n\
        const fn tail_sum<const N: usize>(xs: [f64; N]) -> f64 { let wide = [1.0; N + 1]; let sum = 0.0; for k in 0..N + 1 { sum += wide[k]; } sum + xs[N - 1] }\n\
        module Top() {\n\
            q: f64 = poly_q([1.0, 2.0, 3.0]);\n\
            z: f64 = zeros::<3>()[3] + 7.0;\n\
            t: f64 = tail_sum([4.0, 5.0]);\n\
        }\n";
    let ir = Elab::ir(src, "Top");
    let real = |name: &str| match IrReading::default_of(&ir, name) {
        piperine_ir::Constant::Real(value) => value.get(),
        other => panic!("{name} is {other:?}"),
    };
    assert_eq!(real("q"), 1.0 + 4.0 + 9.0, "a `const fn` over an array and a loop");
    assert_eq!(real("z"), 7.0, "an array repeat of a generic length (3 + 1 = 4 elements)");
    assert_eq!(real("t"), 3.0 + 5.0, "a repeat of `N + 1` read in a loop");
}

#[test]
fn a_struct_that_holds_a_generic_instance_has_the_instances_entry() {
    // resolved-nets T27 (the library's `Driven` holds a `Logic<1>`): a struct whose field is the instance `G<1>` of a generic
    // struct gets an arena entry for that instance too, so its constant is a struct of a struct and the IR checks
    let src = "\
        #[derive(Repr)]\n\
        struct G<const N: usize> { x: UInt<N> }\n\
        #[derive(Repr)]\n\
        struct H { g: G<1> }\n\
        module Top() { signal h: Bit<H> = H { g: G { x: 1 } }; }\n";
    let ir = Elab::ir(src, "Top");
    assert!(ir.check().is_empty(), "the IR with a nested generic instance checks");
}
