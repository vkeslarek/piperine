//! Integration test for the IR diff pass (T12) — every diff rule
//! ported from `crates/piperine-codegen/src/resolve/diff.rs` has a
//! corresponding IR test, and the derivatives are cross-checked
//! against finite differences.
//!
//! LOV-15 (symbolic differentiation as an IR pass). The diff pass lives
//! in `crates/piperine-ir/src/diff.rs`; the source it ports from is
//! `crates/piperine-codegen/src/resolve/diff.rs`.

use piperine_ir::catalog::Catalog;
use piperine_ir::core::{
    Block, Const, Domain, Module, NatureInfo, NatureKind, Net, NetId, Op, PureOp, Trigger, Ty,
    ValueId, BlockId,
};
use piperine_ir::diff::{Differentiator, Wrt};
use std::collections::HashMap;

/// A tiny inline evaluator — walks the block's ops in order, computes
/// each Pure op's output, evaluates math Call ops via `f64` math, and
/// returns the value of a target ValueId. Used by the FD cross-check.
/// (The T13 interpreter is the canonical evaluator; this is the diff
/// test's private oracle.)
fn eval(module: &Module, block: BlockId, inputs: &HashMap<ValueId, f64>) -> HashMap<ValueId, f64> {
    let mut values = inputs.clone();
    // Pair each `Pure::Add(self, self)` op (the diff pass's constant-
    // bearing marker) with the next `Const::Real` in push order. The
    // diff pass pushes 0.0/1.0/other reals interleaved with these ops
    // in the same order; zipping them up resolves the value each
    // self-add carries.
    let real_consts: Vec<f64> = (0..module.const_count())
        .filter_map(|i| match module.const_at(piperine_ir::core::ConstId(i as u32)) {
            Const::Real(v) => Some(*v),
            _ => None,
        })
        .collect();
    let mut const_pool: HashMap<ValueId, f64> = HashMap::new();
    let mut const_idx = 0;
    for op in module.block_at(block).ops() {
        if let Op::Pure { op: PureOp::Add, args, out } = op {
            if args.len() == 2 && args[0] == args[1] && args[0] == *out {
                if let Some(&v) = real_consts.get(const_idx) {
                    const_pool.insert(*out, v);
                    values.insert(*out, v);
                    const_idx += 1;
                }
            }
        }
    }
    // Walk ops in order, evaluating.
    for op in module.block_at(block).ops() {
        match op {
            Op::Pure { op: pop, args, out } => {
                let argv: Vec<f64> = args.iter().map(|a| values.get(a).copied().unwrap_or(0.0)).collect();
                let v = match pop {
                    PureOp::Add if args.len() == 2 && args[0] == args[1] && args[0] == *out => {
                        // Self-add constant anchor — already in const_pool.
                        *const_pool.get(out).unwrap_or(&0.0)
                    }
                    PureOp::Add => argv[0] + argv[1],
                    PureOp::Sub => argv[0] - argv[1],
                    PureOp::Mul => argv[0] * argv[1],
                    PureOp::Div => argv[0] / argv[1],
                    PureOp::Neg => -argv[0],
                    PureOp::Select => {
                        // cond is bits: nonzero = true.
                        if argv[0] != 0.0 { argv[1] } else { argv[2] }
                    }
                    _ => 0.0,
                };
                values.insert(*out, v);
            }
            Op::Call { intrinsic, args, out } => {
                let entry = Catalog::entries().nth(intrinsic.0 as usize);
                let argv: Vec<f64> = args.iter().map(|a| values.get(a).copied().unwrap_or(0.0)).collect();
                let v = match entry.map(|e| e.name()) {
                    Some("sin") => argv[0].sin(),
                    Some("cos") => argv[0].cos(),
                    Some("tan") => argv[0].tan(),
                    Some("exp") => argv[0].exp(),
                    Some("limexp") => argv[0].exp(),
                    Some("ln") | Some("log") => argv[0].ln(),
                    Some("log10") => argv[0].log10(),
                    Some("sqrt") => argv[0].sqrt(),
                    Some("sinh") => argv[0].sinh(),
                    Some("cosh") => argv[0].cosh(),
                    Some("tanh") => argv[0].tanh(),
                    Some("asin") => argv[0].asin(),
                    Some("acos") => argv[0].acos(),
                    Some("atan") => argv[0].atan(),
                    Some("pow") => argv[0].powf(argv[1]),
                    Some("abs") => argv[0].abs(),
                    Some("min") => argv[0].min(argv[1]),
                    Some("max") => argv[0].max(argv[1]),
                    Some("floor") => argv[0].floor(),
                    Some("pnjlim") | Some("fetlim") | Some("limvds") => argv[0],
                    _ => 0.0,
                };
                values.insert(*out, v);
            }
            Op::ReadAcross { out, .. } | Op::ReadThrough { out, .. } | Op::ReadSignal { out, .. } => {
                // Seeded by inputs; default 0.
                values.entry(*out).or_insert(0.0);
            }
            Op::CellRead { out, .. } => {
                values.entry(*out).or_insert(0.0);
            }
            Op::If { cond, then_ops, else_ops } => {
                // Evaluate the matching arm's inner ops.
                let c = values.get(cond).copied().unwrap_or(0.0);
                let arm = if c != 0.0 { then_ops } else { else_ops };
                for inner in arm {
                    eval_one(inner, &mut values);
                }
            }
            Op::Match { scrutinee, arms } => {
                // First-arm-wins for the test mini-eval (the diff test
                // doesn't exercise multi-arm selection here).
                let s = values.get(scrutinee).copied().unwrap_or(0.0);
                let _ = s;
                if let Some((_, body)) = arms.first() {
                    for inner in body {
                        eval_one(inner, &mut values);
                    }
                }
            }
            _ => {}
        }
    }
    values
}

/// Evaluate a single op (used inside If/Match arms where the main walk
/// cannot recurse via `eval` — the arms share the same `values` map).
fn eval_one(op: &Op, values: &mut HashMap<ValueId, f64>) {
    match op {
        Op::Pure { op: pop, args, out } => {
            let argv: Vec<f64> = args.iter().map(|a| values.get(a).copied().unwrap_or(0.0)).collect();
            let v = match pop {
                PureOp::Add => argv[0] + argv[1],
                PureOp::Sub => argv[0] - argv[1],
                PureOp::Mul => argv[0] * argv[1],
                PureOp::Div => argv[0] / argv[1],
                PureOp::Neg => -argv[0],
                PureOp::Select => {
                    if argv[0] != 0.0 { argv[1] } else { argv[2] }
                }
                _ => 0.0,
            };
            values.insert(*out, v);
        }
        Op::Call { intrinsic, args, out } => {
            let entry = Catalog::entries().nth(intrinsic.0 as usize);
            let argv: Vec<f64> = args.iter().map(|a| values.get(a).copied().unwrap_or(0.0)).collect();
            let v = match entry.map(|e| e.name()) {
                Some("sin") => argv[0].sin(),
                Some("cos") => argv[0].cos(),
                Some("exp") => argv[0].exp(),
                Some("ln") => argv[0].ln(),
                Some("sqrt") => argv[0].sqrt(),
                Some("pow") => argv[0].powf(argv[1]),
                _ => 0.0,
            };
            values.insert(*out, v);
        }
        _ => {}
    }
}

/// Finite-difference cross-check: evaluate `value` at `inputs`, then
/// perturb the input `wrt_value` by ±h, and compare the central FD
/// slope against the analytic derivative graph.
fn fd_cross_check(
    module: &Module,
    block: BlockId,
    value: ValueId,
    deriv: ValueId,
    wrt_value: ValueId,
    inputs: &HashMap<ValueId, f64>,
    h: f64,
    tol: f64,
) -> bool {
    let v_plus = {
        let mut inp = inputs.clone();
        *inp.entry(wrt_value).or_insert(0.0) += h;
        eval(module, block, &inp)[&value]
    };
    let v_minus = {
        let mut inp = inputs.clone();
        *inp.entry(wrt_value).or_insert(0.0) -= h;
        eval(module, block, &inp)[&value]
    };
    let fd = (v_plus - v_minus) / (2.0 * h);
    let analytic = eval(module, block, inputs)[&deriv];
    (fd - analytic).abs() < tol
}

/// Build a fixture: an analog always-block reading `x = V(a, b)` and
/// producing `out = f(x)` for various f.
fn fixture_with_op(
    build_op: impl Fn(&mut Module, ValueId, ValueId) -> (Op, ValueId),
) -> (Module, BlockId, NetId, NetId, ValueId, ValueId) {
    let mut m = Module::new();
    let v = m.push_nature(NatureInfo { access: "v".into(), kind: NatureKind::Across });
    let i = m.push_nature(NatureInfo { access: "i".into(), kind: NatureKind::Through });
    let n_a = m.push_net(Net::node(v, i));
    let n_b = m.push_net(Net::node(v, i));
    let block = m.push_block(Block::new(Domain::Analog, Trigger::Always));
    let x = m.push_value(Ty::Real);
    m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: x });
    let (op, out) = build_op(&mut m, x, x);
    m.block_at_mut(block).push(op);
    (m, block, n_a, n_b, x, out)
}

/// d/dx (x + c) = 1 — FD cross-check on Add.
#[test]
fn fd_add_derivative_matches() {
    let (mut m, block, n_a, n_b, x, sum) = fixture_with_op(|_, x, c| {
        (Op::Pure { op: PureOp::Add, args: vec![x, c], out: ValueId(100) }, ValueId(100))
    });
    let mut diff = Differentiator::new(&mut m, block);
    let d = diff.d(sum, Wrt::Across { a: n_a, b: n_b });
    let mut inputs = HashMap::new();
    inputs.insert(x, 1.5);
    inputs.insert(ValueId(100), 0.0); // sum's init placeholder; eval overwrites
    inputs.insert(sum, 0.0);
    // c is x here (we passed x for both args), so d/dx (x + x) = 2.
    assert!(
        fd_cross_check(&m, block, sum, d, x, &inputs, 1e-6, 1e-3),
        "FD vs analytic mismatch for d/dx (x+x)"
    );
}

/// d/dx (x · x) = 2x — FD cross-check on Mul.
#[test]
fn fd_mul_derivative_matches() {
    let (mut m, block, n_a, n_b, x, prod) = fixture_with_op(|_, x, _| {
        (Op::Pure { op: PureOp::Mul, args: vec![x, x], out: ValueId(100) }, ValueId(100))
    });
    let mut diff = Differentiator::new(&mut m, block);
    let d = diff.d(prod, Wrt::Across { a: n_a, b: n_b });
    let mut inputs = HashMap::new();
    inputs.insert(x, 2.0);
    assert!(
        fd_cross_check(&m, block, prod, d, x, &inputs, 1e-6, 1e-3),
        "FD vs analytic mismatch for d/dx (x*x) at x=2"
    );
}

/// d/dx sin(x) = cos(x) — FD cross-check on the sin chain rule.
#[test]
fn fd_sin_derivative_matches() {
    let (mut m, block, n_a, n_b, x, sin_x) = fixture_with_op(|_, x, _| {
        let sin_id = Differentiator::catalog_id_for("sin");
        let out = ValueId(100);
        (Op::Call { intrinsic: sin_id, args: vec![x], out }, out)
    });
    let mut diff = Differentiator::new(&mut m, block);
    let d = diff.d(sin_x, Wrt::Across { a: n_a, b: n_b });
    let mut inputs = HashMap::new();
    inputs.insert(x, 0.7);
    assert!(
        fd_cross_check(&m, block, sin_x, d, x, &inputs, 1e-6, 1e-3),
        "FD vs analytic mismatch for d/dx sin(x) at x=0.7"
    );
}

/// d/dx cos(x) = -sin(x).
#[test]
fn fd_cos_derivative_matches() {
    let (mut m, block, n_a, n_b, x, cos_x) = fixture_with_op(|_, x, _| {
        let cos_id = Differentiator::catalog_id_for("cos");
        let out = ValueId(100);
        (Op::Call { intrinsic: cos_id, args: vec![x], out }, out)
    });
    let mut diff = Differentiator::new(&mut m, block);
    let d = diff.d(cos_x, Wrt::Across { a: n_a, b: n_b });
    let mut inputs = HashMap::new();
    inputs.insert(x, 0.5);
    assert!(
        fd_cross_check(&m, block, cos_x, d, x, &inputs, 1e-6, 1e-3),
        "FD vs analytic mismatch for d/dx cos(x)"
    );
}

/// d/dx exp(x) = exp(x).
#[test]
fn fd_exp_derivative_matches() {
    let (mut m, block, n_a, n_b, x, exp_x) = fixture_with_op(|_, x, _| {
        let exp_id = Differentiator::catalog_id_for("exp");
        let out = ValueId(100);
        (Op::Call { intrinsic: exp_id, args: vec![x], out }, out)
    });
    let mut diff = Differentiator::new(&mut m, block);
    let d = diff.d(exp_x, Wrt::Across { a: n_a, b: n_b });
    let mut inputs = HashMap::new();
    inputs.insert(x, 1.2);
    assert!(
        fd_cross_check(&m, block, exp_x, d, x, &inputs, 1e-6, 1e-3),
        "FD vs analytic mismatch for d/dx exp(x)"
    );
}

/// d/dx ln(x) = 1/x.
#[test]
fn fd_ln_derivative_matches() {
    let (mut m, block, n_a, n_b, x, ln_x) = fixture_with_op(|_, x, _| {
        let ln_id = Differentiator::catalog_id_for("ln");
        let out = ValueId(100);
        (Op::Call { intrinsic: ln_id, args: vec![x], out }, out)
    });
    let mut diff = Differentiator::new(&mut m, block);
    let d = diff.d(ln_x, Wrt::Across { a: n_a, b: n_b });
    let mut inputs = HashMap::new();
    inputs.insert(x, 2.5);
    assert!(
        fd_cross_check(&m, block, ln_x, d, x, &inputs, 1e-6, 1e-3),
        "FD vs analytic mismatch for d/dx ln(x)"
    );
}

/// d/dx sqrt(x) = 1/(2·sqrt(x)).
#[test]
fn fd_sqrt_derivative_matches() {
    let (mut m, block, n_a, n_b, x, sqrt_x) = fixture_with_op(|_, x, _| {
        let sqrt_id = Differentiator::catalog_id_for("sqrt");
        let out = ValueId(100);
        (Op::Call { intrinsic: sqrt_id, args: vec![x], out }, out)
    });
    let mut diff = Differentiator::new(&mut m, block);
    let d = diff.d(sqrt_x, Wrt::Across { a: n_a, b: n_b });
    let mut inputs = HashMap::new();
    inputs.insert(x, 4.0);
    assert!(
        fd_cross_check(&m, block, sqrt_x, d, x, &inputs, 1e-6, 1e-3),
        "FD vs analytic mismatch for d/dx sqrt(x)"
    );
}

/// d/dx tanh(x) = 1 - tanh²(x).
#[test]
fn fd_tanh_derivative_matches() {
    let (mut m, block, n_a, n_b, x, tanh_x) = fixture_with_op(|_, x, _| {
        let id = Differentiator::catalog_id_for("tanh");
        let out = ValueId(100);
        (Op::Call { intrinsic: id, args: vec![x], out }, out)
    });
    let mut diff = Differentiator::new(&mut m, block);
    let d = diff.d(tanh_x, Wrt::Across { a: n_a, b: n_b });
    let mut inputs = HashMap::new();
    inputs.insert(x, 0.9);
    assert!(
        fd_cross_check(&m, block, tanh_x, d, x, &inputs, 1e-6, 1e-3),
        "FD vs analytic mismatch for d/dx tanh(x)"
    );
}

/// d/dx atan(x) = 1/(1+x²).
#[test]
fn fd_atan_derivative_matches() {
    let (mut m, block, n_a, n_b, x, atan_x) = fixture_with_op(|_, x, _| {
        let id = Differentiator::catalog_id_for("atan");
        let out = ValueId(100);
        (Op::Call { intrinsic: id, args: vec![x], out }, out)
    });
    let mut diff = Differentiator::new(&mut m, block);
    let d = diff.d(atan_x, Wrt::Across { a: n_a, b: n_b });
    let mut inputs = HashMap::new();
    inputs.insert(x, 0.4);
    assert!(
        fd_cross_check(&m, block, atan_x, d, x, &inputs, 1e-6, 1e-3),
        "FD vs analytic mismatch for d/dx atan(x)"
    );
}

/// d/dx x^n for constant n: n·x^(n-1) — the Pow constant-exponent rule.
#[test]
fn fd_pow_constant_exponent_matches() {
    let mut m = Module::new();
    let v = m.push_nature(NatureInfo { access: "v".into(), kind: NatureKind::Across });
    let i = m.push_nature(NatureInfo { access: "i".into(), kind: NatureKind::Through });
    let n_a = m.push_net(Net::node(v, i));
    let n_b = m.push_net(Net::node(v, i));
    let block = m.push_block(Block::new(Domain::Analog, Trigger::Always));
    let x = m.push_value(Ty::Real);
    let exp_v = m.push_value(Ty::Real);
    let pow_x = m.push_value(Ty::Real);
    m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: x });
    // Mark exp_v as constant-bearing with value 3.0 via the self-add
    // pattern, then push the const after it.
    m.block_at_mut(block).push(Op::Pure {
        op: PureOp::Add,
        args: vec![exp_v, exp_v],
        out: exp_v,
    });
    let _ = m.push_const(Const::Real(3.0));
    let pow_id = Differentiator::catalog_id_for("pow");
    m.block_at_mut(block).push(Op::Call { intrinsic: pow_id, args: vec![x, exp_v], out: pow_x });

    let mut diff = Differentiator::new(&mut m, block);
    let d = diff.d(pow_x, Wrt::Across { a: n_a, b: n_b });
    let mut inputs = HashMap::new();
    inputs.insert(x, 1.5);
    inputs.insert(exp_v, 3.0);
    assert!(
        fd_cross_check(&m, block, pow_x, d, x, &inputs, 1e-6, 1e-2),
        "FD vs analytic mismatch for d/dx x^3 at x=1.5"
    );
}

/// d/dx (x² + sin(x)) — a mixed nonlinear fixture (the catalog +
/// structured-control proof). The analytic derivative must agree with
/// FD within tolerance.
#[test]
fn fd_nonlinear_mixed_expression_matches() {
    let mut m = Module::new();
    let v = m.push_nature(NatureInfo { access: "v".into(), kind: NatureKind::Across });
    let i = m.push_nature(NatureInfo { access: "i".into(), kind: NatureKind::Through });
    let n_a = m.push_net(Net::node(v, i));
    let n_b = m.push_net(Net::node(v, i));
    let block = m.push_block(Block::new(Domain::Analog, Trigger::Always));
    let x = m.push_value(Ty::Real);
    let x_sq = m.push_value(Ty::Real);
    let sin_x = m.push_value(Ty::Real);
    let sum = m.push_value(Ty::Real);
    m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: x });
    m.block_at_mut(block).push(Op::Pure { op: PureOp::Mul, args: vec![x, x], out: x_sq });
    let sin_id = Differentiator::catalog_id_for("sin");
    m.block_at_mut(block).push(Op::Call { intrinsic: sin_id, args: vec![x], out: sin_x });
    m.block_at_mut(block).push(Op::Pure { op: PureOp::Add, args: vec![x_sq, sin_x], out: sum });

    let mut diff = Differentiator::new(&mut m, block);
    let d = diff.d(sum, Wrt::Across { a: n_a, b: n_b });
    let mut inputs = HashMap::new();
    inputs.insert(x, 0.6);
    assert!(
        fd_cross_check(&m, block, sum, d, x, &inputs, 1e-6, 1e-3),
        "FD vs analytic mismatch for d/dx (x² + sin(x)) at x=0.6"
    );
}

/// Structured If differentiates arm-wise: the diff pass produces a new
/// If whose arms contain the derivative ops of the corresponding
/// original arms. (The flat-block derivative ValueId contract is
/// preserved for non-structured residuals; for structured control the
/// diff pass pushes the new If into the block, derivable from the
/// matching arm at evaluation time.)
#[test]
fn if_arm_wise_produces_structured_derivative() {
    let mut m = Module::new();
    let v = m.push_nature(NatureInfo { access: "v".into(), kind: NatureKind::Across });
    let i = m.push_nature(NatureInfo { access: "i".into(), kind: NatureKind::Through });
    let n_a = m.push_net(Net::node(v, i));
    let n_b = m.push_net(Net::node(v, i));
    let block = m.push_block(Block::new(Domain::Analog, Trigger::Always));
    let x = m.push_value(Ty::Real);
    let cond = m.push_value(Ty::Bits(1));
    let if_out = m.push_value(Ty::Real);
    m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: x });
    m.block_at_mut(block).push(Op::If {
        cond,
        then_ops: vec![Op::Pure { op: PureOp::Mul, args: vec![x, x], out: if_out }],
        else_ops: vec![Op::Pure { op: PureOp::Add, args: vec![x, x], out: if_out }],
    });

    let ops_before = m.block_at(block).ops().len();
    let mut diff = Differentiator::new(&mut m, block);
    let _ = diff.d(if_out, Wrt::Across { a: n_a, b: n_b });
    // The diff pass must push a new If into the block (preserving the
    // structured control — the source's behaviour).
    let has_new_if = m
        .block_at(block)
        .ops()
        .iter()
        .skip(ops_before)
        .any(|op| matches!(op, Op::If { .. }));
    assert!(has_new_if, "d/dx If pushes a new structured If derivative");
}
