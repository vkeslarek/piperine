//! Scope: the analog forms of the buck's power stage in `piperine_lang::elab` for msl-digital
//! (DIG-13 AC 1 to 3, the elaboration half of AC 6): the flow of a branch the module drives with
//! a potential read as a value, `ddt` under a potential contribution, and an `analog` block that
//! reads a hoisted result, a carrier through its flag reads, `now()` and `analysis(Static)`. The
//! cases are `Inductor`, `CurrentSense`, `PowerSwitch` and `Load` of `examples/buck/src/power.msl`,
//! their printed analog statements read against the `.msl`.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

#[path = "support/buck.rs"]
#[allow(dead_code)]
mod buck;

use piperine_ir::{Addend, Body, Env, Expr, ExprId, Ir, Operator, QuantityRole, Stmt};
use support::Elab;

fn body<'a>(ir: &'a Ir, name: &str) -> Body<'a> {
    ir.bodies().find(|b| b.name() == name).unwrap_or_else(|| panic!("no body `{name}` in:\n{}", ir.print_structural()))
}

/// The lines of the `analog` block of `body` in the structural print.
fn analog_lines(ir: &Ir, body: &str) -> Vec<String> {
    let text = ir.print_structural();
    let start = text.find(&format!("body {body}\n")).unwrap_or_else(|| panic!("no body {body}"));
    let rest = &text[start..];
    let at = rest.find("  analog\n").unwrap_or_else(|| panic!("no analog in {body}"));
    rest[at + "  analog\n".len()..].lines().take_while(|l| l.starts_with("    ")).map(|l| l.trim().to_string()).collect()
}

/// A design placing the buck modules `names` of power.msl, each with its `analog` block.
fn power(names: &[&str], top: &str) -> String {
    let modules: Vec<String> = names.iter().map(|n| buck::module_with_analog("power.msl", n)).collect();
    format!("{}\n{}\n{top}", buck::item("power.msl", "fn body_diode"), modules.join("\n"))
}

/// Every expression reachable from `root`.
fn reach(ir: &Ir, root: ExprId) -> Vec<ExprId> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        out.push(e);
        match ir.expr(e) {
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::IsKnown(expr) | Expr::High(expr) => stack.push(*expr),
            Expr::Binary { lhs, rhs, .. } => stack.extend([*lhs, *rhs]),
            Expr::KnownOr { value, default } => stack.extend([*value, *default]),
            Expr::If { cond, then, otherwise } => stack.extend([*cond, *then, *otherwise]),
            Expr::Method { receiver, args, .. } => stack.extend(std::iter::once(*receiver).chain(args.iter().copied())),
            Expr::Call { args, .. } | Expr::Math { args, .. } => stack.extend(args.iter().copied()),
            _ => {}
        }
    }
    out
}

#[test]
fn a_flow_probe_of_a_potential_branch_lowers() {
    // DIG-13 AC 1, C117: `Inductor`'s `v(p, n) +> ddt(l * i(p, n)) + dcr * i(p, n);` reads the flow
    // of its own potential branch `(p, n)` twice.
    let ir = Elab::ir(&power(&["Inductor"], "module Top() { signal a: Electrical; signal b: Electrical; let l = Inductor(a, b) { l: 1u }; }"), "Top");
    let inductor = body(&ir, "Inductor");
    assert_eq!(analog_lines(&ir, "Inductor"), ["b0.potential +> ddt (l * flow(b0)), direct (dcr * flow(b0))"]);
    let flows: Vec<ExprId> = inductor.analog().unwrap_or_default().iter().flat_map(|s| ir.stmt(*s).operands()).flat_map(|e| reach(&ir, e)).filter(|e| matches!(ir.expr(*e), Expr::Flow { .. })).collect();
    assert_eq!(flows.len(), 2);
    assert!(flows.iter().all(|e| ir.expr(*e) == &Expr::Flow { branch: 0 } && ir.expr_type(*e) == &piperine_ir::ValueType::F64));
    // A flow read written before the contribution to its branch reads the same branch.
    let src = "module L(p: Electrical, n: Electrical) { }\nanalog for L { let x = 2.0 * i(p, n); v(p, n) +> x; }\nmodule Top() { signal a: Electrical; let l = L(a, gnd); }";
    let ir = Elab::ir(src, "Top");
    assert_eq!(analog_lines(&ir, "L"), ["let x = (2.0 * flow(b0))", "b0.potential +> direct x"]);
    // `CurrentSense`: a flow contribution and two potential ones over node pairs, as written.
    let ir = Elab::ir(&power(&["CurrentSense"], "module Top() { signal a: Electrical; signal b: Electrical; signal c: Electrical; signal d: Electrical; let cs = CurrentSense(a, b, c, d); }"), "Top");
    assert_eq!(analog_lines(&ir, "CurrentSense"), ["b0.flow +> direct (probe(a, b) / rshunt)", "b1.potential +> direct (gain * probe(a, b))", "b2.potential +> direct ((-gain) * probe(a, b))"]);
}

#[test]
fn a_potential_contribution_keeps_its_ddt() {
    // DIG-13 AC 2, C119: a potential contribution with a `ddt` addend and a direct one.
    let ir = Elab::ir(&power(&["Inductor"], "module Top() { signal a: Electrical; let l = Inductor(a, gnd) { l: 1u, dcr: 2m }; }"), "Top");
    let inductor = body(&ir, "Inductor");
    let stmts = inductor.analog().expect("an analog block");
    let Stmt::Contribute { branch, quantity, addends } = ir.stmt(stmts[0]) else { panic!("a contribution") };
    assert_eq!((*branch, *quantity), (0, QuantityRole::Potential));
    let ops: Vec<(Operator, bool)> = addends.iter().map(|Addend { operator, negated, .. }| (*operator, *negated)).collect();
    assert_eq!(ops, [(Operator::Ddt, false), (Operator::Direct, false)]);
    // The case `v(p, n) +> ddt(v(p))` that `elab_unlowered` refused under M1 is a potential
    // contribution with one `Ddt` addend.
    let src = "module M(p: Electrical, n: Electrical) { }\nanalog for M { v(p, n) +> ddt(v(p)); }\nmodule Top() { signal s: Electrical; let m = M { }(s, gnd); }";
    let ir = Elab::ir(src, "Top");
    assert_eq!(analog_lines(&ir, "M"), ["b0.potential +> ddt probe(p, gnd)"]);
}

#[test]
fn a_flow_probe_of_a_branch_without_a_potential_contribution_is_p0001() {
    // DIG-13 AC 3, C121: `i(a, b)` of a branch that has only a flow contribution.
    let src = "module M(p: Electrical, n: Electrical) { }\nanalog for M { i(p, n) +> v(p, n); let x = i(p, n); }\nmodule Top() { signal s: Electrical; let m = M(s, gnd); }";
    let err = Elab::run(src, "Top").expect_err("a flow read of a flow branch");
    assert_eq!(Elab::codes(&err), ["P0001"]);
    assert!(err[0].diagnostic.message.contains("flow probe"), "{}", err[0].diagnostic.message);
    // And of a pair of nodes with no contribution at all.
    let src = "module M(p: Electrical, n: Electrical) { }\nanalog for M { v(p, n) +> 1.0; let x = i(p, gnd); }\nmodule Top() { signal s: Electrical; let m = M(s, gnd); }";
    let err = Elab::run(src, "Top").expect_err("a flow read of no branch");
    assert_eq!(Elab::codes(&err), ["P0001"]);
}

#[test]
fn analog_reads_a_hoisted_result() {
    // DIG-13 AC 6 (elaboration half), C126: `PowerSwitch`'s `analog` block reads `gate`, the
    // hoisted result of `transition` (the carriers `gate.*` through `known_or`, `now()` and
    // `analysis(Static)`), and contributes `gch * v(d, s) - body_diode(v(s, d), vf, rd)` with
    // `ddt(coss * v(d, s))` (power.msl).
    let ir = Elab::ir(&power(&["PowerSwitch"], "module Top() { signal d: Electrical; signal s: Electrical; signal g: Bit; let p = PowerSwitch(d, s, g); }"), "Top");
    let ps = body(&ir, "PowerSwitch");
    let lines = analog_lines(&ir, "PowerSwitch");
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("let gch = ((if static() { gate.x.known_or(0.0) } else {"), "{}", lines[0]);
    assert_eq!(lines[1], "b0.flow +> direct ((gch * probe(d, s)) - body_diode(probe(s, d), vf, rd)), ddt (coss * probe(d, s))");
    let reads: Vec<Expr> = ps.analog().unwrap_or_default().iter().flat_map(|s| ir.stmt(*s).operands()).flat_map(|e| reach(&ir, e)).map(|e| ir.expr(e).clone()).collect();
    assert!(reads.contains(&Expr::Env(Env::Time)) && reads.contains(&Expr::Env(Env::Static)));
    // A carrier is read in `analog` only as the receiver of a flag read (§3.3.5): every carrier
    // read of the block is under `known_or`.
    let carriers = reads.iter().filter(|e| matches!(e, Expr::Carrier(_))).count();
    let guarded = reads.iter().filter(|e| matches!(e, Expr::KnownOr { value, .. } if matches!(ir.expr(*value), Expr::Carrier(_)))).count();
    assert!(carriers > 0 && carriers == guarded, "{carriers} reads, {guarded} under `known_or`");
    // `Load` reads two hoisted results in one contribution.
    let ir = Elab::ir(&power(&["Load"], "module Top() { signal p: Electrical; signal a: Bit; signal b: Bit; let l = Load(p, a, b); }"), "Top");
    let lines = analog_lines(&ir, "Load");
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("b0.flow +> direct ((probe(p, gnd) * ((1.0 / r_base) + (if static() { k_step.x.known_or(0.0) }"), "{}", lines[0]);
    assert!(lines[0].contains("k_inject.x.known_or(0.0)") && lines[0].ends_with("* i_inject))"), "{}", lines[0]);
    // A carrier read, `now()` and `analysis(Static)` written in `analog` itself.
    let src = "module M(p: Electrical, in g: Bit) { }\nanalog for M { i(p) +> if g && !analysis(Static) { now() } else { 0.0 }; }\nmodule Top() { signal a: Electrical; signal g: Bit; let m = M(a, g); }";
    let ir = Elab::ir(src, "Top");
    assert_eq!(analog_lines(&ir, "M"), ["b0.flow +> direct if (g.known_or(false) && (!static())) { time() } else { 0.0 }"]);
}
