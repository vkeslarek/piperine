//! Scope: handles in the checker (generics GN-11, GN-12, part I §3.1.5): a function parameter of a nature type, and
//! `self` of an `impl` on a nature, is read, probed, passed on and has carrier methods called on it; a probe of
//! anything else in a function keeps `E0450`.

#[path = "support/pom.rs"]
mod support;
#[allow(dead_code)]
#[path = "support/generic_sources.rs"]
mod generic_sources;

use piperine_lang::pom::{DeclId, DeclKind, Design, ExprId};
use piperine_lang::types::{GenericValue, Types};

/// MSL sources over the standard library, checked from `Top`.
struct HandleSources;

impl HandleSources {
    /// The `(code, message)` pairs of the design and of the check of `Top` of `src`.
    fn found(src: &str) -> Vec<(String, String)> {
        let design = Design::builder().source("top.msl", src).build();
        let mut found: Vec<(String, String)> = design.diagnostics().iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect();
        if let Err(refused) = Types::check(&design, "Top") {
            found.extend(refused.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())));
        }
        found
    }
}

#[test]
fn a_handle_is_read_probed_passed_on_and_has_carrier_methods_in_the_checker() {
    // GN-11 AC 1 and 3, §3.1.5: a parameter of a nature type is probed (`v(a)`), passed to another function, and a
    // discrete one is read as its value and has carrier methods (`a.prev()`); `self` of an `impl` on a nature is a handle
    // (`temp(self)`)
    let probed = "fn level(a: Electrical) -> f64 { v(a) }\nfn through(a: Electrical) -> f64 { level(a) }\nmodule Top() { signal o: Electrical; }\nanalog for Top { v(o) +> through(o); }\n";
    assert_eq!(HandleSources::found(probed), []);
    let discrete = "fn held(a: Bit) -> bool { a }\nfn before(a: Bit) -> bool { a.prev() }\nfn both(a: Bit) -> bool { held(a) && before(a) }\nmodule Top() { signal c: Bit = false; signal d: Bit = false; }\ndigital for Top { on c.rise() { d <- both(c); } }\n";
    assert_eq!(HandleSources::found(discrete), []);
    let method = "impl Thermal { fn celsius(self) -> f64 { temp(self) - 273.15 } }\nmodule Top() { signal t: Thermal; signal o: Electrical; }\nanalog for Top { v(o) +> t.celsius(); }\n";
    assert_eq!(HandleSources::found(method), []);
    let design = Design::builder().source("top.msl", probed).build();
    let types = Types::check(&design, "Top").unwrap_or_else(|found| panic!("{found:?}"));
    let parameter = (0..design.decl_count()).map(|index| DeclId(index as u32)).find(|decl| matches!(design.decl_data(*decl).kind, DeclKind::FnParam(_)) && design.decl_data(*decl).name.as_ref().is_some_and(|name| name.text == "a")).expect("a parameter `a`");
    assert!(types.handle(parameter), "a nature-typed parameter is a handle");
}

#[test]
fn a_probe_of_a_handle_in_a_function_is_not_e0450() {
    // GN-12 AC 3, V3: the `E0450` of a probe in a function leaves for a probe whose operands are handles; a probe of
    // anything else keeps it, and so does a function that is not reached through a handle
    let handle = "fn level(a: Electrical) -> f64 { v(a) }\nmodule Top() { signal o: Electrical; }\nanalog for Top { v(o) +> level(o); }\n";
    assert!(HandleSources::found(handle).iter().all(|(code, _)| code != "E0450"));
    let other = "fn level(a: Electrical) -> f64 { let b = a; potential(gnd) }\nmodule Top() { signal o: Electrical; }\nanalog for Top { v(o) +> level(o); }\n";
    let found = HandleSources::found(other);
    assert!(found.iter().any(|(code, _)| code == "E0450"), "{found:?}");
    let number = "fn level(x: f64) -> f64 { potential(x) }\nmodule Top() { signal o: Electrical; }\nanalog for Top { v(o) +> level(1.0); }\n";
    let found = HandleSources::found(number);
    assert!(found.iter().any(|(code, _)| code == "E0450" || code == "E0180"), "{found:?}");
}

#[test]
fn a_handle_returned_stored_or_written_is_e0133() {
    // GN-11 AC 2, D12: a handle is returned (a nature-typed result, `return a`), stored in a `let`, a struct field, a
    // tuple or array element or a closure capture, or written through, and each is `E0133` naming the handle; passing it
    // on is the use it is for
    let tail = "module Top() { signal o: Electrical; }\nanalog for Top { v(o) +> call(o); }\n";
    for (function, how) in [
        ("fn pick(a: Electrical) -> Electrical { a }", "returned"),
        ("fn pick(a: Electrical) -> f64 { return a; }", "returned"),
        ("fn pick(a: Electrical) -> f64 { let b = a; v(b) }", "stored in a `let`"),
        ("struct Hold { n: f64 }\nfn pick(a: Electrical) -> f64 { let h = (a, 1.0); 1.0 }", "stored in a tuple element"),
        ("fn pick(a: Electrical) -> f64 { let h = [a, a]; 1.0 }", "stored in an array element"),
        ("fn pick(a: Electrical) -> f64 { let f = |x: f64| x + v(a); f(1.0) }", "captured by a closure"),
        ("fn pick(a: Bit) -> bool { a = true; true }", "written through"),
        ("fn pick(a: Electrical) -> f64 { let c = { a }; 1.0 }", "stored in a `let`"),
        ("fn pick(a: Electrical) -> f64 { let c = if true { a } else { a }; 1.0 }", "stored in a `let`"),
        ("fn pick(a: Electrical) -> f64 { let c = match 1 { _ => a }; 1.0 }", "stored in a `let`"),
        ("fn pick(a: Electrical) -> f64 { let c = if false { 1.0 } else { let d = 2.0; { a } }; 1.0 }", "stored in a `let`"),
        ("fn pick(a: Electrical) -> f64 { let h = ({ a }, 1.0); 1.0 }", "stored in a tuple element"),
        ("fn pick(a: Electrical) -> f64 { return { a }; }", "returned"),
    ] {
        let found = HandleSources::found(&format!("{function}\nfn call(x: Electrical) -> f64 {{ pick(x) }}\n{tail}"));
        let e0133: Vec<&String> = found.iter().filter(|(code, _)| code == "E0133").map(|(_, message)| message).collect();
        assert!(!e0133.is_empty() && e0133.iter().any(|message| message.contains(how) || how == "returned"), "{function}: {found:?}");
    }
    // a discrete handle used as a value is its read (§3.1.6): every form of it is clean, and a node kept is not
    for function in [
        "fn pick(a: Bit) -> bool { let c = a; c }",
        "fn pick(a: Bit) -> bool { a }",
        "fn pick(a: Bit) -> bool { return a; }",
        "fn pick(a: Bit) -> bool { if true { a } else { !a } }",
        "fn pick(a: Bit) -> bool { a && true }",
        "fn pick(a: Bit) -> bool { let c = { a }; c }",
        "fn pick(a: Bit) -> bool { let c = match 1 { _ => a }; c }",
        "fn pick(a: Bit) -> bool { let h = (a, 1.0); h.0 }",
    ] {
        let found = HandleSources::found(&format!("{function}\nmodule Top(in clk: Bit, out d: Bit = false) {{ }}\ndigital for Top {{ on clk.rise() {{ d <- pick(clk); }} }}\n"));
        assert!(found.iter().all(|(code, _)| code != "E0133"), "{function}: {found:?}");
    }
    let passed = "fn level(a: Electrical) -> f64 { v(a) }\nfn call(a: Electrical) -> f64 { level(a) + level(a) }\n";
    assert_eq!(HandleSources::found(&format!("{passed}{tail}")), []);
}

#[test]
fn a_carrier_method_call_follows_the_lookup_order() {
    // GN-10 / §3.1.7: a method called on a carrier is the carrier's own, then the `impl`s of its nature (inherent and
    // trait, a project nature or a prelude one, `impl Bit` being `impl Bit<bool>`), then the methods of the value read;
    // `self` of such an `impl` is a handle
    let clean = |src: &str| assert_eq!(HandleSources::found(src), [], "{src}");
    // a trait impl and an inherent impl on a prelude nature
    clean("trait Describe { fn tag(self) -> f64; }\nimpl Describe for Thermal { fn tag(self) -> f64 { temp(self) } }\nmodule Top() { signal t: Thermal; signal o: Electrical; }\nanalog for Top { v(o) +> t.tag(); }\n");
    clean("impl Electrical { fn volts(self) -> f64 { v(self) } }\nmodule Top() { signal a: Electrical; signal o: Electrical; }\nanalog for Top { v(o) +> a.volts(); }\n");
    // `impl Bit` is the carriers of `bool`, `impl Bit<f64>` the carriers of `f64`
    clean("impl Bit { fn flipped(self) -> bool { !self } }\nmodule Top() { signal c: Bit = false; signal d: Bit = false; }\ndigital for Top { on c.rise() { d <- c.flipped(); } }\n");
    clean("impl Bit<f64> { fn doubled(self) -> f64 { self * 2.0 } }\nmodule Top() { signal s: Bit<f64> = 1.0; signal d: Bit<f64> = 1.0; }\ndigital for Top { on s.changed() { d <- s.doubled(); } }\n");
    let wrong_value = "impl Bit { fn flipped(self) -> bool { !self } }\nmodule Top() { signal s: Bit<f64> = 1.0; signal d: Bit = false; }\ndigital for Top { on s.changed() { d <- s.flipped(); } }\n";
    assert!(!HandleSources::found(wrong_value).is_empty(), "`impl Bit` is not written for a carrier of `f64`");
    // the impl of the nature comes before the value's own method: the `abs` of `Bit<f64>` below gives a `bool`, which `f64::abs` never does
    clean("impl Bit<f64> { fn abs(self) -> bool { true } }\nmodule Top() { signal s: Bit<f64> = 1.0; signal d: Bit = false; }\ndigital for Top { on s.changed() { d <- s.abs(); } }\n");
    // with no impl, the value's method answers
    clean("module Top() { signal s: Bit<f64> = 1.0; signal d: Bit<f64> = 1.0; }\ndigital for Top { on s.changed() { d <- s.abs(); } }\n");
    // the carrier's own method is not shadowed: a project `prev` on a nature is `E0131`
    let own = "impl Bit { fn prev(self) -> bool { true } }\nmodule Top() { signal c: Bit = false; }\n";
    assert!(generic_sources::GenericSources::standard_codes(own).iter().any(|code| code == "E0131"), "{:?}", generic_sources::GenericSources::standard_codes(own));
}

#[test]
fn solve_reading_functions_are_accepted_where_the_table_allows() {
    // GN-12 AC 1, D13: a function that probes a handle or reads the environment of the solve is solve-reading, and its
    // calls are accepted in `analog`, a reaction body, a body `let` and a value input
    let level = "fn level(a: Electrical) -> f64 { v(a) }\nfn clock() -> f64 { now() }\nfn through(a: Electrical) -> f64 { level(a) + clock() }\n";
    let in_analog = format!("{level}module Top() {{ signal o: Electrical; }}\nanalog for Top {{ v(o) +> through(o); }}\n");
    assert_eq!(HandleSources::found(&in_analog), []);
    let in_let = format!("{level}module Top() {{ signal o: Electrical; let t = clock(); }}\nanalog for Top {{ v(o) +> t; }}\n");
    assert_eq!(HandleSources::found(&in_let), []);
    let in_reaction = format!("{level}module Top() {{ signal c: Bit = false; signal d: Bit<f64> = 0.0; }}\ndigital for Top {{ on c.rise() {{ d <- clock(); }} }}\n");
    assert_eq!(HandleSources::found(&in_reaction), []);
    let in_input = format!("{level}module Leaf(x: f64) {{ }}\nmodule Top() {{ let l = Leaf(clock()); }}\n");
    assert_eq!(HandleSources::found(&in_input), []);
}

#[test]
fn a_solve_reading_call_in_a_const_context_is_e0122() {
    // GN-12 AC 2, D13: a const and a param default refuse a solve-reading call with `E0122` naming the function (and, after the merge with params-and-io, its class; the
    // position is the label); a definition that reads the solve keeps `E0336` (§8.2.5)
    let level = "fn level(a: Electrical) -> f64 { v(a) }\nfn clock() -> f64 { now() }\nfn viaclock() -> f64 { clock() }\n";
    for (item, _position) in [
        ("const T0: f64 = clock();\nmodule Top() { let x = T0; }\n", "a const context"),
        ("const T0: f64 = viaclock();\nmodule Top() { let x = T0; }\n", "a const context"),
        ("module Top() { p: f64 = clock(); }\n", "a const context"),
    ] {
        let found = HandleSources::found(&format!("{level}{item}"));
        assert!(found.iter().any(|(code, message)| code == "E0122" && message.contains("solve-reading") && (message.contains("`clock`") || message.contains("`viaclock`"))), "{item}: {found:?}");
    }
    // a function that reads nothing of the solve is pure, and a const may call it
    let pure = "fn double(x: f64) -> f64 { x * 2.0 }\nconst T0: f64 = double(1.0);\nmodule Top() { let x = T0; }\n";
    assert_eq!(HandleSources::found(pure), []);
}

#[test]
fn a_nature_generic_handle_function_type_checks_and_records_its_nature() {
    // GN-11 AC 4, GN-10 AC 2: `fn f<N: Nature>(a: N)` types `a` as a handle of `N`: it is probed (`potential(a)`,
    // `temp(a)`), a bound such as `W: Nature + Drive` is satisfied by the nature's trait `impl`, and the call's nature is
    // inferred from the carrier passed and recorded in its instantiation
    let probe = |nature: &str, probe: &str| format!("fn level<N: Nature>(a: N) -> f64 {{ {probe} }}\nmodule Top() {{ signal t: {nature}; signal o: Electrical; }}\nanalog for Top {{ v(o) +> level(t); }}\n");
    let electrical = probe("Electrical", "potential(a)");
    let thermal = probe("Thermal", "temp(a)");
    for (src, nature) in [(electrical, "Electrical"), (thermal, "Thermal")] {
        let design = Design::builder().source("top.msl", src.clone()).build();
        assert_eq!(HandleSources::found(&src), [], "{src}");
        let types = Types::check(&design, "Top").unwrap_or_else(|found| panic!("{found:?}"));
        let call = (0..design.expr_count()).map(|index| ExprId(index as u32)).find(|expr| types.instantiation(*expr).is_some()).expect("a generic call");
        let instantiation = types.instantiation(call).expect("its instantiation");
        let GenericValue::Type(argument) = &instantiation.arguments[0] else { panic!("a type argument: {instantiation:?}") };
        assert_eq!(argument.name(&design), format!("node of nature {nature}"), "{src}");
    }
    let capability = "trait Drive { fn push(self) -> f64; }\nimpl Drive for Electrical { fn push(self) -> f64 { v(self) } }\nfn go<W: Nature + Drive>(a: W) -> f64 { a.push() }\nmodule Top() { signal o: Electrical; }\nanalog for Top { v(o) +> go(o); }\n";
    assert_eq!(HandleSources::found(capability), []);
    let refused = "trait Drive { fn push(self) -> f64; }\nimpl Drive for Electrical { fn push(self) -> f64 { v(self) } }\nfn go<W: Nature + Drive>(a: W) -> f64 { 0.0 }\nmodule Top() { signal t: Thermal; signal o: Electrical; }\nanalog for Top { v(o) +> go(t); }\n";
    assert!(HandleSources::found(refused).iter().any(|(code, _)| code == "E0187"), "{:?}", HandleSources::found(refused));
    // a discrete carrier is a nature too: the call's nature is the carrier, not the value it reads
    let discrete = "fn tick<N: Nature>(a: N) -> f64 { 1.0 }\nmodule Top() { signal c: Bit = false; signal o: Electrical; }\nanalog for Top { v(o) +> tick(c); }\n";
    assert_eq!(HandleSources::found(discrete), []);
}

#[test]
fn a_carrier_generic_over_its_value_type_is_a_handle() {
    // GN-11: `fn earlier<T: Repr>(a: Bit<T>) -> T` is a handle of a carrier whose value type is the parameter; `a.prev()` is a `T`
    let src = "fn earlier<T: Repr>(a: Bit<T>) -> T { a.prev() }\nmodule Top() { signal c: Bit<u8> = 0; signal d: Bit<u8> = 1; }\ndigital for Top { on c.changed() { d <- earlier(c); } }\n";
    assert_eq!(HandleSources::found(src), []);
}

#[test]
fn a_let_bound_to_a_node_keeps_its_nature_for_a_probe() {
    // §6 rule 4, §8.2.8, §8.4.8: `let y = x; v(y)` probes `x` by the access of its nature, as `v(x)` does, in a plain module, through a
    // child placed by name and through a template; the flow access of the nature names its flow (`i(y, gnd)` as a target)
    let child = "module Dram(a: Electrical) { pub signal sn: Electrical; }\nanalog for Dram { i(a, gnd) +> v(a, gnd); }\n";
    for src in [
        "module Top() { signal x: Electrical; signal p: Electrical; }\nanalog for Top { let y = x; i(p, gnd) +> v(y) - 1.0; i(y, gnd) +> v(y); }".to_string(),
        format!("{child}module Top() {{ signal x: Electrical; let c0 = Dram(x); signal p: Electrical; }}\nanalog for Top {{ let y = c0.sn; i(p, gnd) +> v(y); }}"),
        format!("{child}module Bank<C: Fn(Electrical)>(a: Electrical) {{ cell: C; let c0 = cell(a); signal p: Electrical; }}\nanalog<C> for Bank<C> {{ let y = c0.sn; i(p, gnd) +> v(y); }}\nmodule Top() {{ signal x: Electrical; let bank = Bank::<_>(x) {{ cell: Dram {{ }} }}; }}"),
    ] {
        assert_eq!(HandleSources::found(&src), Vec::<(String, String)>::new(), "{src}");
    }
}
