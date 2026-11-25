//! Scope: modules with a result in `piperine_lang::elab` for msl-digital (DIG-10, DIG-11): a
//! placement of one is hoisted into the body that places it (part III §8.3.12, Appendix C.1.1),
//! its carriers, drives, events and reactions named under the placement, its result read in
//! place with its value inputs substituted; `at`, `now()` and `analysis(Static)` as their IR
//! nodes. The modules are the buck's (`examples/buck/src`) and the stdlib's `transition`,
//! `above` and `init` as written.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

#[path = "support/buck.rs"]
#[allow(dead_code)]
mod buck;

use piperine_ir::{Body, CarrierRole, Env, EventSource, Expr, ExprId, Ir, Stmt, ValueType};
use support::Elab;

fn body<'a>(ir: &'a Ir, name: &str) -> Body<'a> {
    ir.bodies().find(|b| b.name() == name).unwrap_or_else(|| panic!("no body `{name}` in:\n{}", ir.print_structural()))
}

fn carrier_names(body: &Body) -> Vec<String> {
    body.carriers().iter().map(|c| c.name.clone()).collect()
}

fn index_of(body: &Body, name: &str) -> u32 {
    body.carriers().iter().position(|c| c.name == name).unwrap_or_else(|| panic!("no carrier `{name}`")) as u32
}

/// Every expression reachable from `root` through operands and the statements of blocks.
fn reach(ir: &Ir, root: ExprId) -> Vec<ExprId> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        out.push(e);
        match ir.expr(e) {
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => stack.push(*expr),
            Expr::Binary { lhs, rhs, .. } => stack.extend([*lhs, *rhs]),
            Expr::If { cond, then, otherwise } => stack.extend([*cond, *then, *otherwise]),
            Expr::Method { receiver, args, .. } => stack.extend(std::iter::once(*receiver).chain(args.iter().copied())),
            Expr::Call { args, .. } | Expr::Math { args, .. } => stack.extend(args.iter().copied()),
            Expr::Block { stmts, value } => {
                for s in stmts {
                    stack.extend(ir.stmt(*s).operands());
                }
                stack.extend(value.iter().copied());
            }
            _ => {}
        }
    }
    out
}

/// Every expression a reaction's statements reach.
fn reaction_exprs(ir: &Ir, body: &Body, k: usize) -> Vec<ExprId> {
    body.reactions()[k].stmts.iter().flat_map(|s| ir.stmt(*s).operands()).flat_map(|e| reach(ir, e)).collect()
}

/// Every expression the `analog` block reaches.
fn analog_exprs(ir: &Ir, body: &Body) -> Vec<ExprId> {
    body.analog().unwrap_or_default().iter().flat_map(|s| ir.stmt(*s).operands()).flat_map(|e| reach(ir, e)).collect()
}

/// `PowerSwitch` of examples/buck/src/power.msl, with its `analog` block and `body_diode`, placed
/// by a `Top` once per name of `names`.
fn power_switch(names: &[&str]) -> String {
    let lets: Vec<String> = names.iter().map(|n| format!("let {n} = PowerSwitch(d, s, g);")).collect();
    format!(
        "{}\n{}\nmodule Top() {{ signal d: Electrical; signal s: Electrical; signal g: Bit = false; {} }}",
        buck::item("power.msl", "fn body_diode"),
        buck::module_with_analog("power.msl", "PowerSwitch"),
        lets.join(" ")
    )
}

/// The stdlib `A2D` (std/convert.msl), placed by a `Top`.
const A2D: &str = "module Top() { signal a: Electrical; signal d: Bit; let x = A2D(a, d); }";

#[test]
fn a_placement_is_hoisted_under_its_name() {
    // DIG-10 AC 1, C90: `let gate = transition(..)` in `PowerSwitch` puts `transition`'s `in x`
    // and its five signals in `PowerSwitch`'s body as `gate.<item>`, ports first, as signals.
    let ir = Elab::ir(&power_switch(&["p"]), "Top");
    let ps = body(&ir, "PowerSwitch");
    let hoisted: Vec<String> = carrier_names(&ps).into_iter().filter(|n| n.starts_with("gate.")).collect();
    assert_eq!(hoisted, ["gate.x", "gate.xd", "gate.y0", "gate.y1", "gate.t0", "gate.dt"]);
    assert!(ps.carriers().iter().filter(|c| c.name.starts_with("gate.")).all(|c| c.role == CarrierRole::Signal && c.ty == ValueType::F64));
    let init = |n: &str| ps.carriers()[index_of(&ps, n) as usize].init;
    assert_eq!((init("gate.t0"), init("gate.dt"), init("gate.y0")), (Some(piperine_ir::Constant::Real(piperine_ir::Real::new(0.0))), Some(piperine_ir::Constant::Real(piperine_ir::Real::new(0.0))), Some(piperine_ir::Constant::Real(piperine_ir::Real::new(0.0)))), "`t0`, `dt` and `y0` start at 0.0 as `transition` writes");
    // A placement with no `let` of its own is `<module>$<k>`, `k` counting its placements in
    // source order; one inside a hoisted module nests under it: `A2D`'s two `above` heads and the
    // `init` each places.
    let ir = Elab::ir(A2D, "Top");
    let a2d = body(&ir, "A2D");
    assert_eq!(carrier_names(&a2d), ["d", "above$0.init$0.up", "above$1.init$0.up"]);
    // A module that places itself through results is `E0341`, at the placement.
    let src = "module r(x: f64) -> f64 { let z = q(x); x }\nmodule q(x: f64) -> f64 { let w = r(x); x }\nmodule M(a: Electrical) { let y = r(1.0); }\nanalog for M { i(a) +> y; }\nmodule Top() { signal n: Electrical; let m = M(n); }";
    let err = Elab::run(src, "Top").expect_err("r places q places r");
    assert_eq!(Elab::codes(&err), ["E0341"]);
    let span = err[0].diagnostic.span();
    assert_eq!(&src[span.start..span.end], "r(x)");
}

#[test]
fn value_inputs_are_substituted_at_each_read() {
    // DIG-10 AC 2, C91: a read of `gate` in `PowerSwitch`'s `analog` block is `transition`'s
    // result with `x` the hoisted carrier `gate.x`; `tr` and `tf` are the caller's `tsw`, read in
    // the caller's frame (§8.3.8): a read of `PowerSwitch`'s own param.
    let ir = Elab::ir(&power_switch(&["p"]), "Top");
    let ps = body(&ir, "PowerSwitch");
    let x = index_of(&ps, "gate.x");
    let analog = analog_exprs(&ir, &ps);
    assert!(analog.iter().any(|e| ir.expr(*e) == &Expr::Carrier(x)), "`x` reads `gate.x`");
    assert!(analog.iter().any(|e| ir.expr(*e) == &Expr::Env(Env::Time)) && analog.iter().any(|e| ir.expr(*e) == &Expr::Env(Env::Static)), "the result reads `now()` and `analysis(Static)`");
    let tsw = ps.params().iter().position(|p| p.name == "tsw").expect("tsw") as u32;
    // `dt <- if to > y { tr.unwrap_or(0.0) } else if to < y { tf.unwrap_or(0.0) } else { 0.0 }`.
    let dt = index_of(&ps, "gate.dt");
    let commits_of_dt: Vec<ExprId> = ps.reactions().iter().enumerate().flat_map(|(k, _)| reaction_exprs(&ir, &ps, k)).filter(|e| matches!(ir.expr(*e), Expr::Block { .. })).flat_map(|b| {
        let Expr::Block { stmts, .. } = ir.expr(b) else { return Vec::new() };
        stmts.iter().filter_map(|s| match ir.stmt(*s) {
            Stmt::Commit { carrier, value, .. } if *carrier == dt => Some(*value),
            _ => None,
        }).collect()
    }).collect();
    assert!(!commits_of_dt.is_empty(), "the edge reaction commits `dt`");
    let params_read: Vec<u32> = commits_of_dt.iter().flat_map(|v| reach(&ir, *v)).filter_map(|e| if let Expr::Param(p) = ir.expr(e) { Some(*p) } else { None }).collect();
    assert_eq!(params_read, [tsw, tsw], "`tr` and `tf` are the caller's `tsw`");
    // A value input read in the callee's reaction: `above`'s `e > 0.0` in `A2D` is the caller's
    // `v(a) - vth - hyst / 2.0`, its params `A2D`'s own; the head split per disjunct is one
    // reaction under `fired(..)` (FIX-13).
    let ir = Elab::ir(A2D, "Top");
    let text = ir.print_structural();
    assert!(text.contains("(fired(E1) && (((probe(a, gnd) - vth) - (hyst / 2.0)) > 0.0))) { d <- true }"), "{text}");
    assert!(text.contains("(fired(E3) && (((vth - (hyst / 2.0)) - probe(a, gnd)) > 0.0))) { d <- false }"), "{text}");
}

#[test]
fn an_omitted_option_input_reads_as_none() {
    // DIG-10 AC 3, C92: `transition(x)` leaves `td`, `tr`, `tf` out: `td.unwrap_or(0.0)` is the
    // literal `0.0`, the delay of `xd = x after td.unwrap_or(0.0)`.
    let src = "module Top(in g: Bit) { let y = transition(if g { 1.0 } else { 0.0 }); }";
    let ir = Elab::ir(src, "Top");
    let top = body(&ir, "Top");
    let xd = index_of(&top, "y.xd");
    let drive = top.drives().iter().find(|d| d.carrier == xd).expect("the drive of `y.xd`");
    let after = drive.after.expect("`after td.unwrap_or(0.0)`");
    assert_eq!(ir.expr(after), &Expr::Real(piperine_ir::Real::new(0.0)));
    // Given, an `f64` for `Option<f64>` is `Some` of it (§3.2.3): the delay is the value given.
    let src = "module Top(in g: Bit) { let y = transition(if g { 1.0 } else { 0.0 }, 2n); }";
    let ir = Elab::ir(src, "Top");
    let top = body(&ir, "Top");
    let xd = index_of(&top, "y.xd");
    let after = top.drives().iter().find(|d| d.carrier == xd).and_then(|d| d.after).expect("the delay");
    assert_eq!(ir.expr(after), &Expr::Real(piperine_ir::Real::new(2e-9)));
}

#[test]
fn two_placements_are_two_sets_of_carriers() {
    // DIG-10 AC 4, C93: `Load` (power.msl) places `transition` twice: `k_step.*` and `k_inject.*`.
    let src = format!("{}\nmodule Top() {{ signal p: Electrical; signal step: Bit = false; signal inject: Bit = false; let l = Load(p, step, inject); }}", buck::module_with_analog("power.msl", "Load"));
    let ir = Elab::ir(&src, "Top");
    let load = body(&ir, "Load");
    let names = carrier_names(&load);
    for set in ["k_step", "k_inject"] {
        let items: Vec<&str> = names.iter().filter_map(|n| n.strip_prefix(&format!("{set}."))).collect();
        assert_eq!(items, ["x", "xd", "y0", "y1", "t0", "dt"], "{set}");
    }
    assert_eq!(load.reactions().len(), 6, "three reactions per placement");
    assert_eq!(load.drives().len(), 4, "the drives of `x` and `xd` per placement");
}

#[test]
fn a_result_module_gets_no_body_and_no_instance() {
    // DIG-10 AC 5, C94.
    let ir = Elab::ir(&power_switch(&["p"]), "Top");
    assert!(ir.bodies().all(|b| b.name() != "transition"), "no body for `transition`");
    assert_eq!(ir.instances().map(|i| i.path().to_string()).collect::<Vec<_>>(), ["Top", "Top.p"], "no instance for `gate`");
    assert!(ps_children_are_empty(&ir));
    let ir = Elab::ir(A2D, "Top");
    assert!(ir.bodies().all(|b| b.name() != "above" && b.name() != "init"));
    assert_eq!(ir.instances().count(), 2);
}

fn ps_children_are_empty(ir: &Ir) -> bool {
    body(ir, "PowerSwitch").children().is_empty()
}

#[test]
fn an_expression_at_a_discrete_input_drives_the_hoisted_carrier() {
    // DIG-10 AC 6, C95: `transition(if g { 1.0 } else { 0.0 }, ..)` is a drive of `gate.x`
    // by that expression, undelayed.
    let ir = Elab::ir(&power_switch(&["p"]), "Top");
    let ps = body(&ir, "PowerSwitch");
    let (x, g) = (index_of(&ps, "gate.x"), index_of(&ps, "g"));
    let drives: Vec<_> = ps.drives().iter().filter(|d| d.carrier == x).collect();
    assert_eq!(drives.len(), 1);
    assert_eq!(drives[0].after, None);
    let Expr::If { cond, then, otherwise } = ir.expr(drives[0].value) else { panic!("the `if`: {:?}", ir.expr(drives[0].value)) };
    assert_eq!(ir.expr(*cond), &Expr::Carrier(g), "`g` is read directly");
    assert_eq!((ir.expr(*then), ir.expr(*otherwise)), (&Expr::Real(piperine_ir::Real::new(1.0)), &Expr::Real(piperine_ir::Real::new(0.0))));
    // `xd = x after td.unwrap_or(0.0)` is the callee's own definition, with `td` the caller's `0.0`.
    let xd = index_of(&ps, "gate.xd");
    let drive = ps.drives().iter().find(|d| d.carrier == xd).expect("the drive of `gate.xd`");
    assert_eq!(ir.expr(drive.value), &Expr::Carrier(x));
}

#[test]
fn the_stdlib_d2a_gives_transition_the_level_of_its_input() {
    // msl-digital T66, as part VII 22.4.1 writes `D2A` since explicit-unknowns T23 (replaces
    // `the_stdlib_d2a_reads_its_match_scrutinee_at_each_test`, which held `match d.known() { Some(true) => 1.0,
    // Some(false) => 0.0, None => 0.5 }`): `transition` gets `if d { 1.0 } else { 0.0 }` at its discrete input, a
    // drive of `ramp.x` (DIG-10 AC 6). A drive has no local, and `d` is read once, by the `if`.
    let ir = Elab::ir("module Top() { signal d: Bit = false; signal a: Electrical; signal vdd: Electrical; let x = D2A(d, a, vdd); }", "Top");
    let d2a = body(&ir, "D2A");
    let (x, d) = (index_of(&d2a, "ramp.x"), index_of(&d2a, "d"));
    let drive = d2a.drives().iter().find(|v| v.carrier == x).expect("the drive of `ramp.x`");
    let reached = reach(&ir, drive.value);
    assert!(!reached.iter().any(|e| matches!(ir.expr(*e), Expr::Local(_) | Expr::Block { .. })), "no local in a drive");
    assert_eq!(reached.iter().filter(|e| ir.expr(**e) == &Expr::Carrier(d)).count(), 1, "`d` is read once");
    let text = ir.print_structural();
    assert!(text.contains("drive ramp.x = if d { 1.0 } else { 0.0 }"), "{text}");
    // A binding inside `Some` would need a local: `P0001` by name.
    let src = "module M(in d: Bit, a: Electrical) { let r = transition(match Some(d) { Some(b) => if b { 1.0 } else { 0.0 }, None => 0.5 }, 0.0, 1n, 1n); }\n\
               analog for M { i(a) +> v(a) * r; }\n\
               module Top() { signal d: Bit = false; signal a: Electrical; let m = M(d, a); }";
    let diagnostics = Elab::run(src, "Top").expect_err("a binding in a drive is refused");
    assert_eq!(Elab::codes(&diagnostics), ["P0001"]);
    assert!(diagnostics[0].diagnostic.message.contains("a binding inside `Some` in a definition"), "{:?}", diagnostics[0].diagnostic.message);
}

#[test]
fn at_now_and_static_lower_to_their_nodes() {
    // DIG-10 AC 7, C96: `on at(t0 + dt) && dt > 0.0` is an `At` event over `gate.t0 + gate.dt`;
    // `now()` and `analysis(Static)` are the time and static reads, wherever they stand.
    let ir = Elab::ir(&power_switch(&["p"]), "Top");
    let ps = body(&ir, "PowerSwitch");
    let (t0, dt) = (index_of(&ps, "gate.t0"), index_of(&ps, "gate.dt"));
    let at: Vec<_> = ps.events().iter().filter_map(|e| if let EventSource::At(t) = &e.source { Some((*t, e.guard)) } else { None }).collect();
    assert_eq!(at.len(), 1, "one timer");
    let Expr::Binary { lhs, rhs, .. } = ir.expr(at[0].0) else { panic!("t0 + dt") };
    assert_eq!((ir.expr(*lhs), ir.expr(*rhs)), (&Expr::Carrier(t0), &Expr::Carrier(dt)));
    assert!(at[0].1.is_some(), "`dt > 0.0` guards it");
    // The edge reaction commits `t0 <- now()`; the two others run under `if static()` and
    // `if !static()` (their guards read the analysis).
    let reads: Vec<&Expr> = (0..ps.reactions().len()).flat_map(|k| reaction_exprs(&ir, &ps, k)).map(|e| ir.expr(e)).collect();
    assert!(reads.contains(&&Expr::Env(Env::Time)) && reads.contains(&&Expr::Env(Env::Static)));
    let text = ir.print_structural();
    assert!(text.contains("if static() { gate.y0 <- gate.x; gate.y1 <- gate.x }"), "{text}");
    assert!(text.contains("if (!static()) {"), "{text}");
    assert!(text.contains("gate.t0 <- time()"), "{text}");
}

#[test]
fn a_result_module_is_hoisted_once_per_body() {
    // Edge case 4, C166: `PowerSwitch` placed twice is one body with one set of `gate.*`; each
    // instance binds its own nets.
    let ir = Elab::ir(&power_switch(&["hs", "ls"]), "Top");
    assert_eq!(ir.bodies().filter(|b| b.name() == "PowerSwitch").count(), 1);
    let ps = body(&ir, "PowerSwitch");
    assert_eq!(carrier_names(&ps).iter().filter(|n| n.starts_with("gate.")).count(), 6);
    for inst in ["Top.hs", "Top.ls"] {
        assert!(ir.net_at(&format!("{inst}.gate.x")).is_some(), "{inst}");
    }
}

#[test]
fn a_param_block_binds_the_hoisted_params() {
    // Edge case 5, C167: a param block on a placement of a module with a result binds the
    // hoisted items' params as constants; with none, the defaults.
    let src = "module scaled(x: f64) -> f64 { k: f64 = 2.0 where k > 0.0; k * x }\n\
               module M(a: Electrical) { let y = scaled(1.5) { k: 3.0 }; let z = scaled(1.5); }\nanalog for M { i(a) +> y * v(a) + z; }\n\
               module Top() { signal n: Electrical; let m = M(n); }";
    let ir = Elab::ir(src, "Top");
    let text = ir.print_structural();
    assert!(text.contains("b0.flow +> direct (((3.0 * 1.5) * probe(a, gnd)) + (2.0 * 1.5))"), "{text}");
    assert!(body(&ir, "M").params().is_empty(), "`k` is no param of the body that hoists it");
    let bad = src.replace("{ k: 3.0 }", "{ k: -1.0 }");
    let err = Elab::run(&bad, "Top").expect_err("`k > 0.0` is false");
    assert_eq!(Elab::codes(&err), ["E0317"]);
}

#[test]
fn emit_lowers_to_a_site_with_its_texts() {
    // DIG-11 AC 1, C99: the stdlib's `Never` (std/monitor.msl), `on clk.rise() { require("never",
    // !bad); }`, is one reaction with one site of the stream `checks`: the texts `kind = require`
    // and `name = never` stay on the site, `pass` is the one value field.
    let ir = Elab::ir("module Top(in clk: Bit, in bad: Bit) { let n = Never(clk, bad); }", "Top");
    let never = body(&ir, "Never");
    assert_eq!(never.reactions().len(), 1);
    assert_eq!(never.streams().len(), 1);
    assert_eq!(never.streams()[0].name, "checks");
    assert_eq!(never.streams()[0].fields.iter().map(|f| (f.name.as_str(), f.ty.clone())).collect::<Vec<_>>(), [("pass", ValueType::Bool)]);
    let statements = never.reaction_statements(0);
    assert_eq!(statements.len(), 1, "one record per run, no commit");
    let site = never.record_site(0, 0).expect("the site of statement 0");
    assert_eq!(site.stream, "checks");
    assert_eq!(site.texts, [("kind", "require"), ("name", "never")]);
    // `emit!` written in a reaction: its fields in source order, texts and values interleaved.
    let src = "module Top(in clk: Bit, in bad: Bit) { }\ndigital for Top { on clk.rise() { emit!(\"checks\", kind = \"require\", name = \"never\", pass = !bad); } }";
    let ir = Elab::ir(src, "Top");
    let top = body(&ir, "Top");
    let Stmt::Emit { stream, fields } = ir.stmt(top.reactions()[0].stmts[0]) else { panic!("an emit") };
    assert_eq!(*stream, 0);
    use piperine_ir::EmitField;
    assert!(matches!(&fields[..], [EmitField::Text { name: k, text: kt }, EmitField::Text { name: n, text: nt }, EmitField::Value { field: 0, .. }] if (k.as_str(), kt.as_str(), n.as_str(), nt.as_str()) == ("kind", "require", "name", "never")), "{fields:?}");
}

#[test]
fn a_function_that_emits_is_inlined() {
    // DIG-11 AC 2, C102: a reaction calling `require` holds the `emit` itself; no IR function is
    // made for `require` (an IR function is a pure value).
    let src = "module Top(in clk: Bit, in bad: Bit) { }\ndigital for Top { on clk.rise() { require(\"never\", !bad); } }";
    let ir = Elab::ir(src, "Top");
    assert!(ir.functions().all(|f| f.name() != "require"), "no function `require`");
    let top = body(&ir, "Top");
    let emits: Vec<_> = top.reaction_statements(0).into_iter().filter(|s| matches!(ir.stmt(*s), Stmt::Emit { .. })).collect();
    assert_eq!(emits.len(), 1);
    assert_eq!(top.record_site(0, 0).map(|s| s.texts), Some(vec![("kind", "require"), ("name", "never")]));
    // `log` from a reaction of the buck's `Supervisor` shape: the stream `log` with the value
    // field `severity` (`Severity::Warning`, variant 1 of the IR enum `Severity`) and the text `msg`.
    let src = "module Top(in clk: Bit) { }\ndigital for Top { on clk.rise() { log(Severity::Warning, \"x\"); } }";
    let ir = Elab::ir(src, "Top");
    assert!(ir.functions().all(|f| f.name() != "log"));
    let top = body(&ir, "Top");
    assert_eq!(top.streams()[0].name, "log");
    assert_eq!(top.streams()[0].fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["severity"]);
    assert_eq!(top.record_site(0, 0).map(|s| s.texts), Some(vec![("msg", "x")]));
    // The argument is held by a local of the reaction, the emit reads it.
    let stmts: Vec<&Stmt> = top.reactions()[0].stmts.iter().map(|s| ir.stmt(*s)).collect();
    let [Stmt::Let { local, init }, Stmt::Emit { fields, .. }] = stmts[..] else { panic!("the argument, then the emit: {stmts:?}") };
    assert!(matches!(ir.expr(*init), Expr::VariantOf { variant: 1, fields, .. } if fields.is_empty()), "`Severity::Warning`: {:?}", ir.expr(*init));
    let piperine_ir::EmitField::Value { value, .. } = &fields[0] else { panic!("severity first") };
    assert_eq!(ir.expr(*value), &Expr::Local(*local));
}

#[test]
fn a_text_field_that_is_not_a_literal_is_p0001() {
    // DIG-11 AC 3, C103: `log(sev, msg)` with `msg` a function parameter that is no literal at
    // the call: the text cannot stay on the site.
    let src = "fn note(ok: bool) { log(Severity::Info, if ok { \"up\" } else { \"down\" }); }\nmodule Top(in clk: Bit, in c: Bit) { }\ndigital for Top { on clk.rise() { note(c); } }";
    let err = Elab::run(src, "Top").expect_err("a text that is no text");
    assert_eq!(Elab::codes(&err), ["P0001"]);
    assert!(err[0].diagnostic.message.contains("not a text after inlining"), "{}", err[0].diagnostic.message);
    let span = err[0].diagnostic.span();
    assert_eq!(&src[span.start..span.end], "if ok { \"up\" } else { \"down\" }");
}

#[test]
fn a_value_input_passed_by_name_reads_its_argument() {
    // GN-13, GN-28 (§8.1.9, §8.3.7): `scaled(x, gain: 3.0)` binds the trailing `Option` input `gain` by name, so the
    // result reads it where `scaled(x, 3.0)` does, and an input left out reads `None`
    let items = "module scaled(x: f64, gain: Option<f64>) -> f64 { x * gain.unwrap_or(1.0) }\n";
    let positional = format!("{items}module Top(o: Electrical) {{ }}\nanalog for Top {{ v(o) +> scaled(2.0, 3.0); }}\n");
    let named = format!("{items}module Top(o: Electrical) {{ }}\nanalog for Top {{ v(o) +> scaled(2.0, gain: 3.0); }}\n");
    assert_eq!(Elab::ir(&named, "Top").print_structural(), Elab::ir(&positional, "Top").print_structural(), "the same body");
}

/// A module with a result whose params are an array and a tuple of reals, placed by `Top` (part III §8.3.12: a placement binds
/// each param to a constant, a composite one included).
const COMPOSITE_PARAMS: &str = "module Poly(x: f64) -> f64 { c: [f64; 2] = [1.0, 2.0]; t: (f64, f64) = (3.0, 4.0); x * c[1] + t.0 }\nmodule Top() { signal y: Bit<f64> = 0.0; let a = Poly(2.0) { c: [5.0, 7.0] }; let b = Poly(2.0) { }; }\ndigital for Top { on every(1n, 1n) { y <- a + b; } }";

#[test]
fn a_composite_param_of_a_hoisted_placement_is_bound_to_its_constant() {
    // §8.3.12: the placement `a` gives the array `c` its own value, `b` leaves the default; each read of the result folds `c[1]`
    // and `t.0` from its frame's constant: no refusal, and the two placements hold the two arrays' elements
    let ir = Elab::ir(COMPOSITE_PARAMS, "Top");
    let printed = ir.print_structural();
    for wanted in ["([5.0, 7.0] : [f64; 2])[1i32]", "([1.0, 2.0] : [f64; 2])[1i32]"] {
        assert!(printed.contains(wanted), "the read `c[1]` goes through the placement's own array {wanted}:\n{printed}");
    }
    assert!(printed.contains("((3.0, 4.0) : (f64, f64)).0"), "the tuple default `t.0` is read through its constant:\n{printed}");
}

#[test]
fn an_option_result_of_a_hoisted_module_is_read_as_the_pair_of_its_halves() {
    // §3.2.10 with §8.3.12 (GAPS L89): a module whose result is an `Option<u8>` read in a reaction (`.is_some()`, `.unwrap_or`)
    // lowers to the two halves of its result expression, read in the callee's frame
    let src = "module Hold(in x: Bit<Option<u8>>) -> Option<u8> { x }\nmodule Top() {\n  signal x: Bit<Option<u8>> = None;\n  signal some: Bit = false;\n  signal n: Bit<u8> = 0;\n  let h = Hold(x);\n}\ndigital for Top { on at(1n) { some <- h.is_some(); n <- h.unwrap_or(7); } }\n";
    let ir = Elab::ir(&src, "Top");
    let top = body(&ir, "Top");
    let printed = ir.print_structural();
    assert!(!reaction_exprs(&ir, &top, 0).is_empty() && printed.contains("some <- h.x.is") && printed.contains("n <- if h.x.is"), "the reaction reads the two halves of the result:\n{printed}");
}

/// One `Option`-carrying param of a module with a result, placed by `Top` at its default (part III §3.2.10 with §8.3.12).
fn option_param_module(declaration: &str, read: &str, given: &str) -> String {
    format!("module Inner(x: f64) -> f64 {{ {declaration} x * {read} }}\nmodule Top() {{ signal y: Bit<f64>; let b = Inner(1.0){given}; y = b; }}")
}

#[test]
fn an_option_param_of_a_hoisted_placement_is_bound_to_its_constant() {
    // §3.2.10, §8.3.12: an `Option` param, an array of them, one inside a tuple or a struct, by default and given, binds to its pair
    // `(is, value)`: `Some(2.0)` is `option(true, 2.0)` and `None` is `option(false, 0.0)`; no refusal, no abort
    let cases = [
        ("c: Option<f64> = Some(2.0);", "c.unwrap_or(5.0)", "", "option(true, 2.0)"),
        ("c: Option<f64> = Some(2.0);", "c.unwrap_or(5.0)", " { c: Some(3.0) }", "option(true, 3.0)"),
        ("c: Option<f64> = Some(2.0);", "c.unwrap_or(5.0)", " { c: None }", "option(false, 0.0)"),
        ("c: [Option<f64>; 2] = [Some(1.0), None];", "c[0].unwrap_or(5.0)", "", "[(option(true, 1.0) : Option<f64>), (option(false, 0.0) : Option<f64>)]"),
        ("c: [Option<f64>; 2] = [Some(1.0), None];", "c[0].unwrap_or(5.0)", " { c: [None, Some(2.0)] }", "[(option(false, 0.0) : Option<f64>), (option(true, 2.0) : Option<f64>)]"),
        ("c: (f64, Option<f64>) = (1.0, Some(2.0));", "c.1.unwrap_or(5.0)", "", "(1.0, (option(true, 2.0) : Option<f64>))"),
    ];
    for (declaration, read, given, wanted) in cases {
        let printed = Elab::ir(&option_param_module(declaration, read, given), "Top").print_structural();
        assert!(printed.contains(wanted), "{declaration} {given}: the read goes through {wanted}:\n{printed}");
    }
    let record = "struct P { a: Option<f64>, b: f64 }\nmodule Inner(x: f64) -> f64 { p: P = P { a: Some(1.0), b: 2.0 }; x * p.a.unwrap_or(3.0) + p.b }\nmodule Top() { signal y: Bit<f64>; let b = Inner(1.0); y = b; }";
    assert!(Elab::ir(record, "Top").print_structural().contains("option(true, 1.0)"));
    let whole = "fn sum(a: [Option<f64>; 2]) -> f64 { a[0].unwrap_or(1.0) + a[1].unwrap_or(2.0) }\nmodule Inner(x: f64) -> f64 { c: [Option<f64>; 2] = [Some(1.0), None]; x * sum(c) }\nmodule Top() { signal y: Bit<f64>; let b = Inner(1.0); y = b; }";
    assert!(Elab::ir(whole, "Top").print_structural().contains("sum(([(option(true, 1.0)"), "the array is passed whole to the fn");
}
