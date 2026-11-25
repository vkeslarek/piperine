//! Scope: the standard library as shipped, read through the POM of the embedded design (STD-07 to
//! STD-10): which files exist and are embedded, what each namespace declares, that the prelude
//! makes the library visible by its single names, and that the library builds clean (no
//! diagnostic, a sound design, no member slot left unresolved). Every test builds its design from
//! text written in the test or from the embedded library; nothing here restates the library.

use std::collections::BTreeSet;
use std::path::PathBuf;

use piperine_lang::pom::{BUILTINS, BuiltinKind, Design, ExprKind, Reference, Reflect, Resolution};

#[path = "support/spec_tables.rs"]
mod spec_tables;

use spec_tables::{section, table_rows};

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use support::Elab;

fn embedded() -> Design {
    Design::builder().source("top.msl", "").build()
}

fn codes(design: &Design) -> Vec<String> {
    design.diagnostics().iter().map(|d| d.diagnostic.code.to_string()).collect()
}

fn std_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("std")
}

/// The text of the initializer of the constant `std::<path>`.
fn const_text(design: &Design, path: &str) -> String {
    let decl = design.std_crate().find(path).unwrap_or_else(|| panic!("std declares `{path}`"));
    decl.as_const().and_then(|c| c.value()).unwrap_or_else(|| panic!("`{path}` is a constant with a value")).text().to_string()
}

#[test]
fn every_std_file_is_embedded() {
    let design = embedded();
    let embedded: BTreeSet<String> = design
        .std_crate()
        .files()
        .iter()
        .map(|f| f.name().rsplit('/').next().unwrap().to_string())
        .collect();
    let mut on_disk: Vec<String> = std::fs::read_dir(std_dir())
        .expect("the std directory exists")
        .map(|e| e.expect("directory entry").file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".msl"))
        .collect();
    on_disk.sort();
    let missing: Vec<&String> = on_disk.iter().filter(|n| !embedded.contains(*n)).collect();
    assert!(missing.is_empty(), "std files that the embedded list does not name: {missing:?}");
    assert_eq!(design.std_crate().files()[0].name(), "std/lib.msl", "the root is first");
}

#[test]
fn constants_have_the_values_of_s1() {
    let design = embedded();
    let value = |name: &str| -> f64 { const_text(&design, &format!("constants::{name}")).parse().unwrap_or_else(|e| panic!("{name}: {e}")) };
    assert_eq!(value("Q"), 1.602176634e-19);
    assert_eq!(value("K_B"), 1.380649e-23);
    assert_eq!(value("EPS0"), 8.8541878128e-12);
    assert_eq!(value("PI"), std::f64::consts::PI);
    assert_eq!(value("E"), std::f64::consts::E);
    assert_eq!(const_text(&design, "constants::inf"), "1.0 / 0.0");
    for name in ["Q", "K_B", "EPS0", "PI", "E", "inf"] {
        assert!(design.std_crate().find(&format!("constants::{name}")).unwrap().is_pub(), "{name} is pub");
    }
}

#[test]
fn a_std_item_resolves_by_its_single_name() {
    let design = Design::builder().source("top.msl", "module M(a: Electrical, b: Bit) { }\nconst X: f64 = K_B;\n").build();
    assert_eq!(codes(&design), Vec::<String>::new(), "{:?}", design.diagnostics());
    for path in ["carriers::Electrical", "carriers::Bit", "constants::K_B"] {
        let item = design.std_crate().find(path).unwrap_or_else(|| panic!("std declares `{path}`"));
        assert!(item.krate().is_std());
        let single = path.rsplit("::").next().unwrap();
        let uses = item.references();
        assert!(uses.iter().any(|r| matches!(r, Reference::Path(p) if p.text() == single)), "`{path}` is referenced by its single name `{single}`: {} uses", uses.len());
    }
}

#[test]
fn no_export_has_a_core_name() {
    let design = embedded();
    let core: BTreeSet<&str> = BUILTINS
        .iter()
        .map(|row| row.name)
        .collect();
    let mut clashes = Vec::new();
    for namespace in design.std_crate().root().submodules() {
        if namespace.name() == Some("prelude") {
            continue;
        }
        for item in namespace.items().into_iter().filter(|i| i.is_pub()) {
            if let Some(name) = item.name() {
                if core.contains(name) {
                    clashes.push(format!("{}::{name}", namespace.name().unwrap_or("?")));
                }
            }
        }
    }
    assert!(clashes.is_empty(), "std exports named like a core name: {clashes:?}");
}

#[test]
fn embedded_std_has_no_diagnostics() {
    let design = embedded();
    assert_eq!(codes(&design), Vec::<String>::new(), "{:?}", design.diagnostics());
}

/// Member slots of `std` that the POM leaves `Unresolved` because the receiver is a carrier whose value has the
/// method (STD-06 AC 4: a known receiver with no linking rule stays `Unresolved`; the type checker finds the method
/// of the carried value). Part VII writes `base.unwrap_or(start)` over a `Bit<Option<f64>>` (§22.2.9) and `self.is_high()` over
/// a `Bit<Logic>` (§22.1.5).
const UNRESOLVED_METHODS_OF_A_CARRIED_VALUE: [&str; 4] = [
    "analog::idtmod: `base.unwrap_or(start)`",
    "analog::idtmod: `base.unwrap_or(start)`",
    "values::impl Bit<Logic>.rise: `self.is_high()`",
    "values::impl Bit<Logic>.fall: `self.is_low()`",
];

#[test]
fn std_has_no_unresolved_member() {
    let design = embedded();
    let mut unresolved = Vec::new();
    for decl in design.std_crate().decls() {
        for expr in decl.exprs() {
            if matches!(expr.kind(), ExprKind::Field { .. } | ExprKind::MethodCall { .. }) && expr.resolution() == Resolution::Unresolved {
                unresolved.push(format!("{}: `{}`", decl.qualified_name(), expr.text()));
            }
        }
    }
    let mut allowed: Vec<String> = UNRESOLVED_METHODS_OF_A_CARRIED_VALUE.iter().map(|m| m.to_string()).collect();
    allowed.sort();
    let mut listed = unresolved.clone();
    listed.sort();
    let unexpected: Vec<&String> = unresolved.iter().filter(|u| !UNRESOLVED_METHODS_OF_A_CARRIED_VALUE.contains(&u.as_str())).collect();
    assert!(unexpected.is_empty(), "member slots left Unresolved in std: {unexpected:?}");
    assert_eq!(listed, allowed, "every allowed slot is still unresolved: drop the stale entry");
}

#[test]
fn embedded_std_passes_check() {
    let design = embedded();
    assert_eq!(design.check(), Vec::<String>::new());
}

#[test]
fn a_replaced_std_without_an_item_is_e0161() {
    let design = Design::builder()
        .std_source("lib.msl", "pub mod prelude;\n")
        .std_source("prelude.msl", "")
        .source("top.msl", "module M(a: Electrical) { }\n")
        .build();
    assert_eq!(codes(&design), ["E0161"]);
    assert!(design.module("M").is_some(), "the build completes");
}

#[test]
fn a_std_file_sees_another_namespace_through_the_prelude() {
    let design = Design::builder()
        .std_source("std/lib.msl", "pub mod a;\npub mod b;\npub mod prelude;\n")
        .std_source("std/a.msl", "pub const A: u32 = 1;\n")
        .std_source("std/b.msl", "pub const B: u32 = A;\n")
        .std_source("std/prelude.msl", "pub use crate::a::*;\npub use crate::b::*;\n")
        .source("top.msl", "const C: u32 = B;\n")
        .build();
    assert_eq!(codes(&design), Vec::<String>::new(), "{:?}", design.diagnostics());
    assert!(design.std_crate().find("a::A").unwrap().references().len() >= 1, "b.msl reads A by its single name");
}

/// The declaration the method call `text` of the model `top.msl` resolves to.
fn called_decl<'d>(design: &'d Design, text: &str) -> piperine_lang::pom::Decl<'d> {
    let call = design
        .root_crate()
        .decls()
        .into_iter()
        .flat_map(|d| d.exprs())
        .find(|e| e.text() == text && matches!(e.kind(), ExprKind::MethodCall { .. }))
        .unwrap_or_else(|| panic!("the model calls `{text}`"));
    match call.resolution() {
        Resolution::Decl(id) => design.decl(id),
        other => panic!("`{text}` resolves to {other:?}, not to a declaration"),
    }
}

#[test]
fn bit_rise_resolves_into_std_values() {
    let design = Design::builder().source("top.msl", "module M(in clk: Bit) { let e = clk.rise(); }\n").build();
    assert_eq!(codes(&design), Vec::<String>::new(), "{:?}", design.diagnostics());
    let rise = called_decl(&design, "clk.rise()");
    assert_eq!(rise.name(), Some("rise"));
    assert!(rise.krate().is_std());
    assert_eq!(rise.file().name(), "values.msl");
    assert!(rise.declared_in().and_then(|d| d.as_impl()).is_some(), "rise is a function of an `impl`");
}

/// The expressions of a declaration, with the design they belong to.
fn calls_named<'d>(design: &'d Design, decl: piperine_lang::pom::Decl<'d>, name: &str) -> Vec<piperine_lang::pom::Expr<'d>> {
    decl.exprs()
        .into_iter()
        .filter(|e| matches!(e.kind(), ExprKind::Call { callee, .. } if design.expr(*callee).text().split("::<").next() == Some(name)))
        .collect()
}

#[test]
fn noise_table_is_noise_of_table_model() {
    let design = embedded();
    let table = design.std_crate().find("analog::noise_table").expect("std declares noise_table");
    let module = table.as_module().expect("noise_table is a module");
    // The module places `noise` (a core module) and its result is that call.
    let noise = calls_named(&design, table, "noise");
    assert_eq!(noise.len(), 1, "one placement of noise");
    assert_eq!(module.result(), Some(noise[0]), "the result is the noise call");
    let ExprKind::Call { callee, .. } = noise[0].kind() else { unreachable!() };
    assert!(matches!(design.expr(*callee).resolution(), Resolution::Builtin(id) if id.name() == "noise"));
    // Its first argument calls `table_model`, the intrinsic function of std.
    let model = calls_named(&design, table, "table_model");
    assert_eq!(model.len(), 1, "one call of table_model");
    let ExprKind::Call { callee, .. } = model[0].kind() else { unreachable!() };
    let Resolution::Decl(id) = design.expr(*callee).resolution() else { panic!("table_model resolves to a declaration") };
    assert!(design.decl(id).as_function().is_some_and(|f| f.is_intrinsic()));
}

#[test]
fn pure_bodies_name_no_primitive() {
    let design = embedded();
    let mut named = Vec::new();
    for name in ["values::same", "analog::deltalim", "analog::pnjlim", "analog::fetlim", "analog::limvds", "analog::db10", "analog::db20", "analog::normal"] {
        let item = design.std_crate().find(name).unwrap_or_else(|| panic!("std declares {name}"));
        assert!(item.as_function().is_some_and(|f| f.body().is_some()), "{name} has a body");
        for expr in item.exprs() {
            if let Resolution::Builtin(id) = expr.resolution() {
                let info = id.info();
                let primitive = matches!(info.kind, BuiltinKind::Fn | BuiltinKind::Module | BuiltinKind::Method | BuiltinKind::Event | BuiltinKind::Statement | BuiltinKind::Node);
                if primitive && info.name != "level" {
                    named.push(format!("{name}: {}", info.name));
                }
            }
        }
    }
    assert!(named.is_empty(), "bodies that name a core primitive: {named:?}");
}

#[test]
fn a_private_std_item_is_e0161() {
    let design = Design::builder()
        .source("top.msl", "fn f(s: std::analog::Slewing) {}\nfn g(s: Slewing) {}\n")
        .build();
    assert_eq!(codes(&design), ["E0161", "E0161"], "{:?}", design.diagnostics());
}

/// The modules of `std::analog` whose name starts with `prefix`, with their names.
fn analog_modules<'d>(design: &'d Design, prefix: &str) -> Vec<(String, piperine_lang::pom::Module<'d>)> {
    design
        .std_crate()
        .modules()
        .into_iter()
        .filter_map(|m| {
            let name = m.name()?;
            (name.starts_with(prefix) && m.file().name() == "analog.msl").then(|| (name.to_string(), m))
        })
        .collect()
}

#[test]
fn root_forms_place_their_coefficient_form() {
    let design = embedded();
    for (prefix, placed, forms) in [("zi_", "zi_nd", ["zi_np", "zi_zd", "zi_zp"]), ("laplace_", "laplace_nd", ["laplace_np", "laplace_zd", "laplace_zp"])] {
        let modules = analog_modules(&design, prefix);
        for form in forms {
            let (_, module) = modules.iter().find(|(n, _)| n == form).unwrap_or_else(|| panic!("std::analog declares {form}"));
            let calls = calls_named(&design, module.decl(), placed);
            assert_eq!(calls.len(), 1, "{form} places exactly one {placed}");
            let ExprKind::Call { callee, .. } = calls[0].kind() else { unreachable!() };
            let Resolution::Decl(id) = design.expr(*callee).resolution() else { panic!("{placed} resolves in {form}") };
            assert_eq!(design.decl(id).name(), Some(placed));
            assert!(design.decl(id).krate().is_std());
        }
    }
}

#[test]
fn root_params_carry_a_where() {
    let design = embedded();
    let mut roots = 0;
    for prefix in ["zi_", "laplace_"] {
        for (name, module) in analog_modules(&design, prefix) {
            for param in module.params().into_iter().filter(|p| matches!(p.name(), Some("zeros" | "poles"))) {
                roots += 1;
                assert!(param.where_clause().is_some(), "{name}.{} has a where", param.name().unwrap());
            }
        }
    }
    assert_eq!(roots, 8, "the six root forms declare eight lists of roots");
}

#[test]
fn laplace_nd_has_state_signals_and_an_analog_block() {
    let design = embedded();
    let module = design.std_crate().find("analog::laplace_nd").and_then(|d| d.as_module()).expect("std declares laplace_nd");
    assert!(module.signals().iter().any(|s| s.ty().text().starts_with("[State;")), "a signal that is an array of State");
    assert_eq!(module.analog_blocks().len(), 1, "one analog block");
}

#[test]
fn laplace_nd_requires_a_nonzero_leading_denominator_coefficient() {
    // STD-02 AC 8 (F3): `den: [1.0, 0.0]` makes H(s) = s, improper and with fewer poles than the
    // D - 1 states promise, so the `where` of `den` tests the top coefficient `den[D - 1]`, not
    // "some coefficient". The POM evaluates no `where`: the guard is on the written clause.
    let design = embedded();
    let module = design.std_crate().find("analog::laplace_nd").and_then(|d| d.as_module()).expect("std declares laplace_nd");
    let den = module.params().into_iter().find(|p| p.name() == Some("den")).expect("laplace_nd has den");
    let clause = den.where_clause().expect("den has a where").text().to_string();
    assert_eq!(clause.replace(' ', ""), "den[D-1]!=0.0", "the where of den: {clause}");
}

/// A tiny evaluator of the f64 subset the limiter bodies use (`let`, `if`, arithmetic,
/// comparisons, `abs`, `ln`), over the parsed body: no other construct is accepted.
mod f64_eval {
    use piperine_lang::msl::ast::*;
    use std::collections::BTreeMap;

    #[derive(Clone, Copy)]
    pub enum V {
        F(f64),
        B(bool),
    }

    fn num(v: V) -> f64 {
        match v {
            V::F(x) => x,
            V::B(_) => panic!("a number was expected"),
        }
    }

    fn truth(v: V) -> bool {
        match v {
            V::B(b) => b,
            V::F(_) => panic!("a bool was expected"),
        }
    }

    pub fn block(b: &Block, env: &mut BTreeMap<String, V>) -> V {
        let mut last = V::F(f64::NAN);
        for stmt in &b.stmts {
            match &stmt.kind {
                StmtKind::Let { pat, init: Some(init), .. } => {
                    let PatKind::Ident { name, .. } = &pat.kind else { panic!("a plain `let` name") };
                    let v = expr(init, env);
                    env.insert(name.name.clone(), v);
                }
                StmtKind::Expr { expr: e, semi: false } => last = expr(e, env),
                other => panic!("unsupported statement {other:?}"),
            }
        }
        last
    }

    pub fn expr(e: &Expr, env: &mut BTreeMap<String, V>) -> V {
        match &e.kind {
            ExprKind::Lit(Lit { kind: LitKind::Float { value, .. }, .. }) => V::F(*value),
            ExprKind::Lit(Lit { kind: LitKind::Int(t), .. }) => V::F(t.parse().expect("an integer literal")),
            ExprKind::Path(p) if p.segments.len() == 1 => *env.get(&p.segments[0].ident.name).unwrap_or_else(|| panic!("unbound `{}`", p.segments[0].ident.name)),
            ExprKind::Paren(inner) => expr(inner, env),
            ExprKind::Block(b) => block(b, env),
            ExprKind::Unary { op: UnOp::Not, expr: inner } => V::B(!truth(expr(inner, env))),
            ExprKind::Unary { op: UnOp::Neg, expr: inner } => V::F(-num(expr(inner, env))),
            ExprKind::MethodCall { recv, method, args, .. } if args.is_empty() => {
                let x = num(expr(recv, env));
                V::F(match method.name.as_str() {
                    "abs" => x.abs(),
                    "ln" => x.ln(),
                    other => panic!("unsupported method {other}"),
                })
            }
            ExprKind::If { cond, then, else_ } => {
                if truth(expr(cond, env)) {
                    block(then, env)
                } else {
                    expr(else_.as_deref().expect("an `else`"), env)
                }
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let (l, r) = (expr(lhs, env), expr(rhs, env));
                match op {
                    BinOp::And => V::B(truth(l) && truth(r)),
                    BinOp::Or => V::B(truth(l) || truth(r)),
                    BinOp::Add => V::F(num(l) + num(r)),
                    BinOp::Sub => V::F(num(l) - num(r)),
                    BinOp::Mul => V::F(num(l) * num(r)),
                    BinOp::Div => V::F(num(l) / num(r)),
                    BinOp::Lt => V::B(num(l) < num(r)),
                    BinOp::Le => V::B(num(l) <= num(r)),
                    BinOp::Gt => V::B(num(l) > num(r)),
                    BinOp::Ge => V::B(num(l) >= num(r)),
                    other => panic!("unsupported operator {other:?}"),
                }
            }
            other => panic!("unsupported expression {other:?}"),
        }
    }
}

#[test]
fn pnjlim_follows_ngspice_devpnjlim_branch_by_branch() {
    // F4: every branch of the ngspice `DEVpnjlim` as ngspice devsup.c has it, evaluated on
    // the body of `std::analog::pnjlim` as shipped.
    use std::collections::BTreeMap;
    let file = piperine_lang::msl::parser::parse(&std::fs::read_to_string(std_dir().join("analog.msl")).expect("analog.msl")).file;
    let body = file
        .items
        .iter()
        .find_map(|i| match &i.kind {
            piperine_lang::msl::ast::ItemKind::Fn(f) if f.name.name == "pnjlim" => f.body.clone(),
            _ => None,
        })
        .expect("analog.msl declares pnjlim with a body");
    let run = |new: f64, old: f64| -> f64 {
        let mut env = BTreeMap::from([("new".to_string(), f64_eval::V::F(new)), ("old".to_string(), f64_eval::V::F(old)), ("nvt".to_string(), f64_eval::V::F(0.025)), ("vcrit".to_string(), f64_eval::V::F(0.6))]);
        match f64_eval::block(&body, &mut env) {
            f64_eval::V::F(x) => x,
            f64_eval::V::B(_) => panic!("pnjlim returns a number"),
        }
    };
    let (vt, vcrit) = (0.025_f64, 0.6_f64);
    // 1. forward step above vcrit, old > 0, arg > 0: vold + vt * (2 + ln(arg - 2)).
    assert!((run(0.9, 0.7) - (0.7 + vt * (2.0 + (0.2 / vt - 2.0).ln()))).abs() < 1e-15);
    // 2. the same, arg <= 0 (old = 5.0, new = 0.7: arg = (0.7 - 5)/vt): vold - vt * (2 + ln(2 - arg)).
    assert!((run(0.7, 5.0) - (5.0 - vt * (2.0 + (2.0 - (0.7 - 5.0) / vt).ln()))).abs() < 1e-12);
    let _ = vcrit;
    // 3. forward step above vcrit, old <= 0: vt * ln(new / vt).
    assert!((run(0.9, -0.1) - vt * (0.9 / vt).ln()).abs() < 1e-15);
    assert!((run(0.9, 0.0) - vt * (0.9 / vt).ln()).abs() < 1e-15);
    // 4. new <= vcrit or a small step: unchanged.
    assert_eq!(run(0.5, 0.0), 0.5);
    assert_eq!(run(0.62, 0.6), 0.62);
    // 5. reverse bias, new < 0, old > 0: limited to -old - 1.
    assert_eq!(run(-3.0, 0.5), -1.5);
    assert_eq!(run(-1.0, 0.5), -1.0);
    // 6. reverse bias, new < 0, old <= 0: limited to 2 * old - 1.
    assert_eq!(run(-5.0, -1.0), -3.0);
    assert_eq!(run(-2.0, -1.0), -2.0);
    // 7. new == 0 is not reverse bias; the vnew > vcrit test fails: unchanged.
    assert_eq!(run(0.0, 1.0), 0.0);
}

/// The declaration the expression of the root crate with text `text` resolves to, if it does.
fn resolved_decl<'d>(design: &'d Design, text: &str) -> Option<piperine_lang::pom::Decl<'d>> {
    let expr = design.root_crate().decls().into_iter().flat_map(|d| d.exprs()).find(|e| e.text() == text)?;
    match expr.resolution() {
        Resolution::Decl(id) => Some(design.decl(id)),
        _ => None,
    }
}

#[test]
fn tgate_places_two_tran() {
    let design = embedded();
    let tran = design.std_crate().find("nets::Tran").expect("std declares Tran in nets");
    assert!(tran.as_module().is_some(), "Tran is a module");
    let tgate = design.std_crate().find("nets::Tgate").expect("std declares Tgate in nets");
    let calls = calls_named(&design, tgate, "Tran");
    assert_eq!(calls.len(), 2, "Tgate places two Tran");
    for call in calls {
        let ExprKind::Call { callee, .. } = call.kind() else { unreachable!() };
        let Resolution::Decl(id) = design.expr(*callee).resolution() else { panic!("Tran resolves to a declaration") };
        assert_eq!(design.decl(id).qualified_name(), tran.qualified_name());
    }
}

#[test]
fn a_std_path_names_the_same_declaration() {
    let design = Design::builder().source("top.msl", "fn f(a: std::nets::Strength, b: Strength) { }\n").build();
    assert_eq!(codes(&design), Vec::<String>::new(), "{:?}", design.diagnostics());
    let wire = design.std_crate().find("nets::Strength").expect("std declares Strength in nets");
    let uses: Vec<String> = wire.references().iter().filter_map(|r| if let Reference::Path(p) = r { Some(p.text().to_string()) } else { None }).collect();
    assert!(uses.iter().any(|t| t == "std::nets::Strength"), "the long path resolves to Strength: {uses:?}");
    assert!(uses.iter().any(|t| t == "Strength"), "the single name resolves to Strength: {uses:?}");
}

#[test]
fn transient_is_the_kind_and_tran_the_module() {
    let design = Design::builder()
        .source("top.msl", "nature Line { value: Driven }\nimpl Resolve<Driven> for Line { fn resolve<const N: usize>(d: [Driven; N]) -> Driven { Driven::strongest(d) } }\nmodule M(a: Line, b: Line, in en: Bit) { let k = analysis(Transient); let g = Tran(a, b, en); }\n")
        .build();
    assert_eq!(codes(&design), Vec::<String>::new(), "{:?}", design.diagnostics());
    let tran = resolved_decl(&design, "Tran").expect("Tran resolves to a declaration");
    assert!(tran.as_module().is_some() && tran.krate().is_std());
    assert_eq!(tran.file().name(), "nets.msl");
    let kind = design.root_crate().decls().into_iter().flat_map(|d| d.exprs()).find(|e| e.text() == "Transient").expect("the model writes Transient");
    assert!(matches!(kind.resolution(), Resolution::Builtin(id) if id.name() == "Transient"), "{:?}", kind.resolution());
}

/// The module of `std` that the placement `name(...)` of the root crate resolves to.
fn placed_module<'d>(design: &'d Design, name: &str) -> piperine_lang::pom::Decl<'d> {
    let decl = resolved_decl(design, name).unwrap_or_else(|| panic!("`{name}` resolves to a declaration"));
    assert!(decl.as_module().is_some(), "`{name}` is a module");
    assert!(decl.krate().is_std(), "`{name}` is a module of std");
    decl
}

#[test]
fn converters_resolve_into_std_convert() {
    let design = Design::builder()
        .source("top.msl", "module M(in d: Bit, a: Electrical, vdd: Electrical, q: Bit) { let x = D2A(d, a, vdd); let y = A2D(a, q); }\n")
        .build();
    assert_eq!(codes(&design), Vec::<String>::new(), "{:?}", design.diagnostics());
    for name in ["D2A", "A2D"] {
        assert_eq!(placed_module(&design, name).file().name(), "convert.msl", "{name}");
    }
}

#[test]
fn monitors_resolve_into_std_monitor() {
    let design = Design::builder()
        .source("top.msl", "module M(in clk: Bit, in d: Bit, in x: Bit, tpd: f64) { let s = Setup(clk, d) { ts: 1n }; let y = inertial(x, tpd); }\n")
        .build();
    assert_eq!(codes(&design), Vec::<String>::new(), "{:?}", design.diagnostics());
    for name in ["Setup", "inertial"] {
        assert_eq!(placed_module(&design, name).file().name(), "monitor.msl", "{name}");
    }
}

// ---- the library as a whole (STD-07 to STD-09, STD-17): layout, Index, intrinsics ----

/// The ten namespaces of the library, the prelude last.
const NAMESPACES: [&str; 10] = ["constants", "carriers", "values", "logic", "analog", "nets", "convert", "monitor", "io", "prelude"];

fn part_vii() -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spec/part_vii_stdlib.md")).expect("part VII exists")
}

/// One backtick name of the first column of the Index.
#[derive(Debug)]
struct IndexName {
    name: String,
    /// Written inside parentheses (a member of the item before it) or as `Type::member`.
    member: bool,
    /// The namespace its entry's section maps to.
    namespace: &'static str,
}

impl IndexName {
    /// The namespace that holds the entry `entry` of the Index of Part VII (`§22.1.2`): `logic` for §22.1.1 and §22.1.2,
    /// the namespace of its section for every other.
    fn namespace_of_entry(entry: &str) -> &'static str {
        if entry.starts_with("§22.1.1") || entry.starts_with("§22.1.2") {
            return "logic";
        }
        let k: usize = entry.strip_prefix("§22.").and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next()).and_then(|n| n.parse().ok()).unwrap_or_else(|| panic!("entry `{entry}` names a §22 section"));
        ["values", "analog", "nets", "convert", "monitor", "io"][k - 1]
    }
}

/// Every backtick name of the first column of the §22 Index, with the namespace of its entry.
fn index_names() -> Vec<IndexName> {
    let part = part_vii();
    let mut names = Vec::new();
    for row in table_rows(&section(&part, "### Index")) {
        let namespace = IndexName::namespace_of_entry(&row[2]);
        let (mut depth, mut open, mut word) = (0, false, String::new());
        for c in row[0].chars() {
            match c {
                '`' if open => {
                    open = false;
                    let member = depth > 0 || word.contains("::");
                    let name = word.rsplit("::").next().unwrap().to_string();
                    names.push(IndexName { name, member, namespace });
                    word.clear();
                }
                '`' => open = true,
                '(' if !open => depth += 1,
                ')' if !open => depth -= 1,
                c if open => word.push(c),
                _ => {}
            }
        }
    }
    names
}

/// The table "Implemented by the engine" of §22, as the names of its first column.
fn engine_table() -> BTreeSet<String> {
    let part = part_vii();
    let s22 = section(&part, "## 22.");
    let at = s22.find("**Implemented by the engine.**").expect("§22 has the engine table");
    let table = &s22[at..];
    let table = &table[..table.find("\n>").expect("a note follows the engine table")];
    table_rows(table).into_iter().map(|row| row[0].trim_matches('`').to_string()).take_while(|n| !n.is_empty()).collect()
}

/// Pub items of `std` that the Index no longer lists: none since `resolved-nets` T27 removed `Drive`, `Wire` and `SumNet`.
const STD_BEHIND_THE_INDEX: [(&str, &str); 0] = [];

/// Index items of §22 that `std` does not declare yet: none since `resolved-nets` T27 gave `nets` `Driven`; a stale entry
/// fails `the_index_lists_stay_honest`.
const INDEX_AHEAD_OF_STD: [(&str, &str); 0] = [];

/// Members the Index lists under the items above: none since `Driven::strongest` is declared.
const INDEX_MEMBERS_AHEAD_OF_STD: [(&str, &str); 0] = [];

/// The names of the `pub` items of a namespace.
fn pub_names(design: &Design, namespace: &str) -> BTreeSet<String> {
    let ns = design.std_crate().root().submodules().into_iter().find(|n| n.name() == Some(namespace)).unwrap_or_else(|| panic!("std declares the namespace `{namespace}`"));
    ns.items().into_iter().filter(|i| i.is_pub()).filter_map(|i| i.name().map(str::to_string)).collect()
}

#[test]
fn std_root_declares_its_namespaces() {
    let design = embedded();
    let root = &design.std_crate().files()[0];
    let parsed = piperine_lang::msl::parser::parse(root.text());
    let mut declared = Vec::new();
    for item in &parsed.file.items {
        let piperine_lang::msl::ast::ItemKind::Mod(m) = &item.kind else { panic!("the root declares only namespaces, found {:?}", item.kind) };
        assert!(root.text()[item.span.0.start..item.span.0.end].contains("pub mod"), "`{}` is `pub`", m.name.name);
        declared.push(m.name.name.clone());
    }
    declared.sort();
    let mut expected: Vec<String> = NAMESPACES.iter().map(|n| n.to_string()).collect();
    expected.sort();
    assert_eq!(declared, expected);
}

#[test]
fn each_namespace_holds_its_section() {
    let design = embedded();
    let names = |ns: &str| pub_names(&design, ns);
    let set = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<BTreeSet<String>>();
    assert_eq!(names("constants"), set(&["Q", "K_B", "EPS0", "PI", "E", "inf", "AMBIENT_TEMPERATURE", "DEFAULT_MULTIPLICITY"]), "constants: §1 rule 3");
    assert_eq!(names("carriers"), set(&["Bit", "Electrical", "Thermal", "VSignal", "Short"]), "carriers: §4 and §11");
    for ns in ["values", "logic", "analog", "nets", "convert", "monitor", "io"] {
        let mut wanted: BTreeSet<String> = index_names().into_iter().filter(|n| !n.member && n.namespace == ns).map(|n| n.name).collect();
        for (space, name) in INDEX_AHEAD_OF_STD.iter().filter(|(space, _)| *space == ns) {
            assert!(wanted.remove(*name), "{space}::{name} is not an Index row any more: drop it from INDEX_AHEAD_OF_STD");
        }
        wanted.extend(STD_BEHIND_THE_INDEX.iter().filter(|(space, _)| *space == ns).map(|(_, name)| name.to_string()));
        assert_eq!(names(ns), wanted, "{ns}: the pub items are the Index rows of its section");
    }
}

#[test]
fn prelude_reexports_every_namespace() {
    let design = embedded();
    let prelude = design.std_crate().root().submodules().into_iter().find(|n| n.name() == Some("prelude")).expect("std declares the prelude");
    let mut globbed = BTreeSet::new();
    for item in prelude.items() {
        let u = item.as_use().unwrap_or_else(|| panic!("the prelude holds only `use`: found `{}`", item.text()));
        assert!(item.is_pub(), "`{}` is pub", item.text());
        let imports = u.imports();
        assert_eq!(imports.len(), 1, "{}", item.text());
        assert!(imports[0].glob && imports[0].alias.is_none(), "{}", item.text());
        let path = imports[0].path.text().to_string();
        globbed.insert(path.strip_prefix("crate::").unwrap_or_else(|| panic!("`{path}` starts with crate::")).to_string());
    }
    let expected: BTreeSet<String> = NAMESPACES.iter().filter(|n| **n != "prelude").map(|n| n.to_string()).collect();
    assert_eq!(globbed, expected);
    assert_eq!(prelude.items().len(), 9, "one re-export per namespace");
}

#[test]
fn embedded_list_names_every_file_once() {
    let design = embedded();
    let names: Vec<String> = design.std_crate().files().iter().map(|f| f.name().rsplit('/').next().unwrap().to_string()).collect();
    assert_eq!(names[0], "lib.msl", "the root is first");
    let mut on_disk: Vec<String> = std::fs::read_dir(std_dir()).expect("the std directory exists").map(|e| e.expect("entry").file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".msl")).collect();
    on_disk.sort();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(sorted, on_disk, "every file of the directory once, and no other");
}

/// The declarations of `std` named `name` that are written in the file of `namespace`.
fn declared_in_file<'d>(design: &'d Design, namespace: &str, name: &str) -> Vec<piperine_lang::pom::Decl<'d>> {
    let file = format!("{namespace}.msl");
    design.std_crate().decls().into_iter().filter(|d| d.name() == Some(name) && d.file().name().ends_with(&file)).collect()
}

#[test]
fn every_index_item_is_declared() {
    let design = embedded();
    let missing: Vec<String> = index_names()
        .into_iter()
        .filter(|n| !INDEX_AHEAD_OF_STD.iter().chain(INDEX_MEMBERS_AHEAD_OF_STD.iter()).any(|(space, name)| *space == n.namespace && *name == n.name))
        .filter(|n| {
            if n.member { declared_in_file(&design, n.namespace, &n.name).is_empty() } else { design.std_crate().find(&format!("{}::{}", n.namespace, n.name)).is_none() }
        })
        .map(|n| format!("{}::{}", n.namespace, n.name))
        .collect();
    assert!(missing.is_empty(), "Index items that std does not declare in the namespace of their entry: {missing:?}");
}

#[test]
fn every_item_but_the_intrinsics_has_a_body() {
    let design = embedded();
    let engine = engine_table();
    let mut bodiless = Vec::new();
    for decl in design.std_crate().decls() {
        if let Some(f) = decl.as_function() {
            let name = decl.name().unwrap_or("?").to_string();
            if !engine.contains(&name) && f.body().is_none() {
                bodiless.push(format!("fn {}", decl.qualified_name()));
            }
        }
        if let Some(m) = decl.as_module() {
            if m.result_type().is_some() && m.result().is_none() {
                bodiless.push(format!("module {} has a result type but no result expression", decl.qualified_name()));
            }
        }
    }
    assert!(bodiless.is_empty(), "{bodiless:?}");
}

#[test]
fn intrinsics_are_exactly_the_engine_table() {
    let design = embedded();
    let intrinsics: BTreeSet<String> = design.std_crate().decls().into_iter().filter(|d| d.as_function().is_some_and(|f| f.is_intrinsic())).filter_map(|d| d.name().map(str::to_string)).collect();
    assert_eq!(intrinsics, engine_table());
    assert_eq!(intrinsics.len(), 5);
}

#[test]
fn every_intrinsic_reads_a_str() {
    let design = embedded();
    let mut without = Vec::new();
    for decl in design.std_crate().decls() {
        let Some(f) = decl.as_function().filter(|f| f.is_intrinsic()) else { continue };
        if !f.params().iter().any(|p| p.ty().is_some_and(|t| t.text().contains("str"))) {
            without.push(decl.qualified_name());
        }
    }
    assert!(without.is_empty(), "intrinsics with no `str` parameter: {without:?}");
}

#[test]
fn every_index_item_is_pub() {
    let design = embedded();
    let mut private = Vec::new();
    for n in index_names() {
        // A member of a trait is public by its trait; a member of an `impl` or a top-level item states `pub`.
        let decls = if n.member { declared_in_file(&design, n.namespace, &n.name) } else { design.std_crate().find(&format!("{}::{}", n.namespace, n.name)).into_iter().collect() };
        for d in decls {
            let in_trait = d.declared_in().is_some_and(|p| p.as_trait().is_some());
            if !in_trait && !d.is_pub() {
                private.push(format!("{}::{}", n.namespace, n.name));
            }
        }
    }
    assert!(private.is_empty(), "Index items declared without `pub`: {private:?}");
}

#[test]
fn std_declares_no_core_name() {
    let design = embedded();
    let core: BTreeSet<&str> = BUILTINS.iter().map(|row| row.name).collect();
    let mut clashes = Vec::new();
    for namespace in design.std_crate().root().submodules() {
        for item in namespace.items() {
            if let Some(name) = item.name().filter(|n| core.contains(n)) {
                clashes.push(format!("{}::{name}", namespace.name().unwrap_or("?")));
            }
        }
    }
    assert!(clashes.is_empty(), "std items named like a core name: {clashes:?}");
}

#[test]
fn prelude_exports_equal_the_index() {
    let design = embedded();
    let prelude = design.std_crate().root().submodules().into_iter().find(|n| n.name() == Some("prelude")).expect("std declares the prelude");
    let mut exported = BTreeSet::new();
    for item in prelude.items() {
        for import in item.as_use().expect("the prelude holds only `use`").imports() {
            assert!(import.glob, "{}", item.text());
            let ns = import.path.text().strip_prefix("crate::").expect("a crate path").to_string();
            exported.extend(pub_names(&design, &ns));
        }
    }
    let mut wanted: BTreeSet<String> = index_names().into_iter().filter(|n| !n.member).map(|n| n.name).collect();
    wanted.extend(["Bit", "Electrical", "Thermal", "VSignal", "Short", "Q", "K_B", "EPS0", "PI", "E", "inf", "AMBIENT_TEMPERATURE", "DEFAULT_MULTIPLICITY"].map(String::from));
    // spec-ahead and std-behind entries (explicit-unknowns, the lists above): by name across namespaces
    for (_, name) in INDEX_AHEAD_OF_STD {
        wanted.remove(name);
    }
    for (_, name) in STD_BEHIND_THE_INDEX {
        exported.remove(name);
    }
    let extra: Vec<&String> = exported.difference(&wanted).collect();
    let missing: Vec<&String> = wanted.difference(&exported).collect();
    assert!(extra.is_empty() && missing.is_empty(), "exported but not in the Index: {extra:?}; in the Index but not exported: {missing:?}");
}

#[test]
fn std_root_comment_names_the_namespaces() {
    let design = embedded();
    let root = design.std_crate().root();
    let comment: String = root.docs().iter().map(|d| d.text.clone()).collect::<Vec<_>>().join("\n");
    for ns in NAMESPACES {
        assert!(comment.contains(&format!("`{ns}`")), "the file comment of lib.msl names `{ns}`: {comment}");
    }
    for stale in ["future", "later", "not yet", "empty", "TODO"] {
        assert!(!comment.to_lowercase().contains(&stale.to_lowercase()), "the file comment says `{stale}`: {comment}");
    }
}

#[test]
fn the_fixture_prelude_is_gone() {
    // STD-16 AC 1: the file does not exist and no test file of the crate names it.
    let name = format!("{}/{}", "fixtures/pom", "prelude.msl");
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    assert!(!manifest.join("tests").join(&name).exists(), "tests/{name} is deleted");
    let mut naming = Vec::new();
    for entry in std::fs::read_dir(manifest.join("tests")).expect("tests directory") {
        let path = entry.expect("entry").path();
        if path.extension().is_some_and(|e| e == "rs") && std::fs::read_to_string(&path).expect("test source").contains(&name) {
            naming.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    assert!(naming.is_empty(), "test files that still name the fixture prelude: {naming:?}");
}

#[test]
fn no_pom_test_was_removed() {
    // STD-16 AC 2: the three targets keep at least the tests they had before the fixture left.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for (target, floor) in [("pom_classify", 39), ("pom_reflect", 31), ("pom_resolve", 48)] {
        let text = std::fs::read_to_string(manifest.join("tests").join(format!("{target}.rs"))).expect("target source");
        let count = text.lines().filter(|l| l.trim() == "#[test]").count();
        assert!(count >= floor, "{target} holds {count} tests, at least {floor} are required");
    }
}

#[test]
fn std_holds_no_removed_form() {
    // VS-08 AC 1 (§1.2.6, §1.4.6, §1.5.12): the files of `std` write no `&` borrow, no `*x` dereference, no `mut`,
    // no `Clone` or `Copy` and no lifetime; the drives of a resolved net are an array under a const parameter.
    for entry in std::fs::read_dir(std_dir()).expect("the std directory") {
        let path = entry.expect("an entry").path();
        let text = std::fs::read_to_string(&path).expect("a std file");
        for (number, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for removed in ["&str", "&[", "&'", "let mut ", "Clone", "Copy", "(&", ", &", "= &", " *x", "(*"] {
                assert!(!code.contains(removed), "{}:{} writes `{removed}`: {line}", path.display(), number + 1);
            }
        }
    }
    let nets = std::fs::read_to_string(std_dir().join("nets.msl")).expect("nets.msl");
    for item in ["pub fn strongest<const N: usize>(d: [Driven; N]) -> Driven", "pub fn sum<const N: usize>(d: [f64; N]) -> f64"] {
        assert!(nets.contains(item), "nets.msl holds `{item}`");
    }
}

#[test]
fn the_index_lists_stay_honest() {
    // A stale entry would hide a divergence the day `std` catches up: each spec-ahead item is still absent
    // from `std`, and each std-behind item is still declared there and no longer in the Index.
    let design = embedded();
    for (space, name) in INDEX_AHEAD_OF_STD {
        assert!(!pub_names(&design, space).contains(name), "{space}::{name} is declared now: drop it from INDEX_AHEAD_OF_STD");
    }
    for (space, name) in INDEX_MEMBERS_AHEAD_OF_STD {
        assert!(declared_in_file(&design, space, name).is_empty(), "{space}::{name} is declared now: drop it from INDEX_MEMBERS_AHEAD_OF_STD");
    }
    let listed: Vec<String> = index_names().into_iter().filter(|n| !n.member).map(|n| format!("{}::{}", n.namespace, n.name)).collect();
    for (space, name) in STD_BEHIND_THE_INDEX {
        assert!(pub_names(&design, space).contains(name), "{space}::{name} is gone from std: drop it from STD_BEHIND_THE_INDEX");
        assert!(!listed.contains(&format!("{space}::{name}")), "{space}::{name} is an Index row again: drop it from STD_BEHIND_THE_INDEX");
    }
}

/// The items of L58's generics group (GN-27 AC 1), each placed or called once in `GROUP_SOURCE`.
const GENERICS_GROUP: [(&str, &str); 20] = [
    ("Short", "carriers.msl"),
    ("since", "values.msl"),
    ("cast", "values.msl"),
    ("same", "values.msl"),
    ("idt", "analog.msl"),
    ("idtmod", "analog.msl"),
    ("zi_nd", "analog.msl"),
    ("zi_np", "analog.msl"),
    ("zi_zd", "analog.msl"),
    ("zi_zp", "analog.msl"),
    ("laplace_nd", "analog.msl"),
    ("laplace_np", "analog.msl"),
    ("laplace_zd", "analog.msl"),
    ("laplace_zp", "analog.msl"),
    ("noise_table", "analog.msl"),
    ("Stable", "monitor.msl"),
    ("Setup", "monitor.msl"),
    ("Hold", "monitor.msl"),
    ("inertial", "monitor.msl"),
    ("MinPulse", "monitor.msl"),
];

/// A module of the root crate that places or calls every item of [`GENERICS_GROUP`].
const GROUP_SOURCE: &str = "\
module Top(a: Electrical, b: Electrical, in clk: Bit, in en: Bit, in d: Bit, in v: Bit<u8>, x: f64, n: u8, tpd: f64, period: f64) {
    let s = Short(a, b);
    let age = since(d);
    let c = cast::<u8, i8>(n);
    let e = same(n, n);
    let i = idt(x, Some(0.0), None);
    let m = idtmod(x, Some(0.0), 1.0, Some(-0.5));
    let zn = zi_nd::<2, 1>(x, period) { num: [1.0, 0.5], den: [0.25] };
    let znp = zi_np::<2, 1>(x, period) { num: [1.0, 0.5], poles: [(0.5, 0.0)] };
    let zzd = zi_zd::<1, 1>(x, period) { zeros: [(0.5, 0.0)], den: [0.25] };
    let zzp = zi_zp::<1, 1>(x, period) { zeros: [(0.5, 0.0)], poles: [(0.5, 0.0)] };
    let ln = laplace_nd::<1, 2>(x) { num: [1.0], den: [1.0, 1.0] };
    let lnp = laplace_np::<1, 1>(x) { num: [1.0], poles: [(-1.0, 0.0)] };
    let lzd = laplace_zd::<1, 2>(x) { zeros: [(-1.0, 0.0)], den: [1.0, 1.0] };
    let lzp = laplace_zp::<1, 1>(x) { zeros: [(-1.0, 0.0)], poles: [(-2.0, 0.0)] };
    let nt = noise_table::<2>(None) { table: [(1.0, 1e-18), (1e3, 1e-20)] };
    let st = Stable(en, d);
    let su = Setup(clk, d) { ts: 1n };
    let ho = Hold(clk, d) { th: 1n };
    let ine = inertial(v, tpd);
    let mp = MinPulse(d) { w: 1n };
}
";

/// What the generics group's placement test reads from the root crate's call and placement expressions.
struct GroupItems;

impl GroupItems {
    /// The declaration the callee `name` (with or without a turbofish) of the root crate resolves to.
    fn resolved<'d>(design: &'d Design, name: &str) -> Option<piperine_lang::pom::Decl<'d>> {
        let expr = design.root_crate().decls().into_iter().flat_map(|d| d.exprs()).find(|e| e.text().split("::<").next() == Some(name) && matches!(e.resolution(), Resolution::Decl(_)))?;
        match expr.resolution() {
            Resolution::Decl(id) => Some(design.decl(id)),
            _ => None,
        }
    }
}

#[test]
fn each_generics_group_item_resolves_to_its_std_declaration() {
    // GN-27 AC 1: no item of L58's generics group stands for a checker intrinsic: each one written in a
    // project resolves to the declaration written in its `std` file, and the project builds clean
    let design = Design::builder().source("top.msl", GROUP_SOURCE).build();
    assert_eq!(codes(&design), Vec::<String>::new(), "{:?}", design.diagnostics());
    for (name, file) in GENERICS_GROUP {
        let decl = GroupItems::resolved(&design, name).unwrap_or_else(|| panic!("`{name}` resolves to a declaration"));
        assert!(decl.krate().is_std(), "`{name}` is declared in std");
        assert_eq!(decl.file().name().rsplit('/').next(), Some(file), "`{name}` is declared in {file}");
        assert!(decl.as_module().is_some() || decl.as_function().is_some_and(|f| !f.is_intrinsic()), "`{name}` has a source body");
    }
}

/// The items of L58's generics group, each placed or called by a source of its own under `Top` (GN-27): the placement is the
/// source a user writes for the item, over the real standard library.
struct GenericsGroup;

impl GenericsGroup {
    /// The head every source shares: a mode enum with a `Default` for `inertial`.
    const HEAD: &'static str = "#[derive(Repr, PartialEq)]\nenum Mode { Idle, Run }\nimpl Default for Mode { fn default() -> Mode { Mode::Idle } }\n";

    /// `(item, source)`: the source places the item from `Top`.
    fn sources() -> Vec<(&'static str, String)> {
        let top = |items: &str| format!("{}module Top() {{ signal a: Electrical; signal b: Electrical; signal clk: Bit = false; signal d: Bit<u8> = 0; signal m: Bit<Mode> = Mode::Idle; {items} }}\n", Self::HEAD);
        vec![
            ("Short", top("let s = Short(a, b);")),
            ("since", top("let w = since(d);")),
            ("idt", top("let i = idt(1.0, Some(0.0));")),
            ("idtmod", top("let i = idtmod(1.0, Some(0.0), 1.0, Some(0.0));")),
            ("zi_nd", top("let z = zi_nd::<1, 1>(1.0, 1u) { num: [0.5], den: [-0.5] };")),
            ("zi_np", top("let z = zi_np::<1, 1>(1.0, 1u) { num: [0.5], poles: [(0.5, 0.0)] };")),
            ("zi_zd", top("let z = zi_zd::<1, 1>(1.0, 1u) { zeros: [(0.5, 0.0)], den: [-0.5] };")),
            ("zi_zp", top("let z = zi_zp::<1, 1>(1.0, 1u) { zeros: [(0.5, 0.0)], poles: [(0.5, 0.0)] };")),
            ("laplace_nd", top("let l = laplace_nd::<1, 2>(1.0) { num: [1.0], den: [1.0, 1u] };")),
            ("laplace_np", top("let l = laplace_np::<1, 1>(1.0) { num: [1.0], poles: [(-1M, 0.0)] };")),
            ("laplace_zd", top("let l = laplace_zd::<0, 2>(1.0) { zeros: [], den: [1.0, 1u] };")),
            ("laplace_zp", top("let l = laplace_zp::<0, 1>(1.0) { zeros: [], poles: [(-1M, 0.0)] };")),
            ("noise_table", format!("{}module Top() {{ signal a: Electrical; }}\nanalog for Top {{ i(a, gnd) +> v(a, gnd) / 1k + noise_table::<2>(\"t\") {{ table: [(1k, 1e-18), (10k, 4e-18)] }}; }}\n", Self::HEAD)),
            ("Stable", top("let s = Stable(clk, d);")),
            ("Setup", top("signal viol: Bit; let s = Setup(clk, d, viol) { ts: 1n };")),
            ("Hold", top("let h = Hold(clk, d) { th: 1n };")),
            ("MinPulse", top("let p = MinPulse(clk) { w: 1n };")),
            ("inertial", top("let i = inertial(m, 1n);")),
            ("cast", format!("{}\ndigital for Top {{ on at(1n) {{ let c = cast::<u8, u8>(d); }} }}\n", top(""))),
            ("same", format!("{}\ndigital for Top {{ on at(1n) {{ let e = same(d, d); }} }}\n", top(""))),
        ]
    }
}

#[test]
fn no_generics_group_item_is_refused_because_generic() {
    // GN-27 AC 1: every item of the group places or is called, and nothing it is refused for names "generic": a form that
    // is still refused would name its own form (`a_blocked_item_is_refused_naming_its_form`)
    for (item, source) in GenericsGroup::sources() {
        let design = Elab::design(&source);
        assert!(design.diagnostics().is_empty(), "{item}: the source must be clean: {:?}", design.diagnostics().iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect::<Vec<_>>());
        if let Err(found) = piperine_lang::elab::Elaborator::new(&design).run("Top") {
            let messages: Vec<String> = found.iter().map(|d| d.diagnostic.message.clone()).collect();
            assert!(messages.iter().all(|message| !message.to_lowercase().contains("generic")), "{item} is refused because it is generic: {messages:?}");
            panic!("{item} does not elaborate: {messages:?}");
        }
    }
}

#[test]
fn a_blocked_item_is_refused_naming_its_form() {
    // GN-27 AC 2 (DE1, L33): a generic module that places an `extern` module is refused naming the `extern` module, never
    // "generic": the refusal names the form that blocks the item
    let source = "#[link(\"x.osdi\", model = \"x\")]\nextern \"osdi\" module Foreign(a: Electrical) { }\nmodule Wrap<N: Nature>(a: N) { let f = Foreign(a); }\nmodule Top() { signal a: Electrical; let w = Wrap::<Electrical>(a); }\n";
    let design = Elab::design(source);
    let refused = match piperine_lang::elab::Elaborator::new(&design).run("Top") {
        Ok(_) => panic!("an extern module is not placed yet (L33)"),
        Err(found) => found.iter().map(|d| d.diagnostic.message.clone()).collect::<Vec<_>>(),
    };
    let text = refused.join(" | ");
    assert!(text.to_lowercase().contains("extern"), "the refusal names the form: {text}");
    assert!(!text.to_lowercase().contains("generic"), "never `generic`: {text}");
}
