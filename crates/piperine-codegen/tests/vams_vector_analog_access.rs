//! LRM-COVERS: 5.5.2
//! LRMW3-02 (`lrm-conformance-wave3`, T7): analog terminal resolution
//! through a *constant* vector index — `V(bus[0], g)`/`I(bus[i], g) <+ …`
//! where `i` folds to an elaboration constant. Before this task,
//! `resolve/pom/expr.rs::ident_from_expr` only matched `Expr::Ident`, so
//! any indexed terminal silently failed lowering with an "unresolved net"
//! error; `terminal_name_of` widens this to fold a constant `Expr::Index`
//! to the same `"{base}[{i}]"` node name `structure.rs`'s element-node
//! registration loop already uses.
//!
//! Uses the same `AnalogKernel::eval_residual`/`eval_jacobian` technique as
//! `analog_loop_unroll.rs` — a real compiled-kernel evaluation, not a
//! shape-only elaboration check — with `AnalogKernel::terminal_name`/
//! `num_terminals` to locate each bus bit's own row without assuming a
//! fixed terminal order.

use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_codegen::SimCtx;
use piperine_lang::parse_and_elaborate;

/// Compile a ground-port module with an internal 4-bit bus (`bus[0:3]`,
/// the post-declarator vector form T4 already lands) whose analog body is
/// `analog_src`. `bus` is an internal wire, not a port — a vector *port*
/// does not yet expand into per-element nodes (`structure.rs`'s port loop
/// registers one node per port declaration, unlike its wire loop; that gap
/// is `piperine-codegen`'s pre-existing "urc codegen gaps"/T13 territory,
/// not this task's concern).
fn kernel(analog_src: &str) -> Result<AnalogKernel, String> {
    let src = format!(
        "module Dut(inout electrical g);
    electrical bus[0:3];
    {analog_src}
endmodule
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
        .map_err(|e| format!("elaborate failed: {e:?}"))?;
    let bodies = piperine_codegen::resolve::lower_bodies(&design).map_err(|e| e.to_string())?;
    AnalogKernel::compile(&bodies["Dut"]).map_err(|e| e.to_string())
}

/// Index of terminal named `name` in `k`'s own terminal order (ABI-27) —
/// never assumed positionally, always looked up.
fn terminal_index(k: &AnalogKernel, name: &str) -> usize {
    (0..k.num_terminals())
        .find(|&i| k.terminal_name(i) == name)
        .unwrap_or_else(|| panic!("terminal `{name}` not found among: {:?}", (0..k.num_terminals()).map(|i| k.terminal_name(i)).collect::<Vec<_>>()))
}

fn residual(k: &AnalogKernel, volts: &[f64]) -> Vec<f64> {
    let vars = vec![0.0; k.num_vars()];
    let state = vec![0.0; k.num_state_slots()];
    let mut out = vec![0.0; k.num_terminals()];
    k.eval_residual(volts, &[], &state, &vars, &SimCtx::default(), &mut out);
    out
}

fn jacobian(k: &AnalogKernel, volts: &[f64]) -> Vec<f64> {
    let n = k.num_terminals();
    let vars = vec![0.0; k.num_vars()];
    let state = vec![0.0; k.num_state_slots()];
    let mut out = vec![0.0; n * n];
    k.eval_jacobian(volts, &[], &state, &vars, &SimCtx::default(), &mut out);
    out
}

/// Done-when #1: `I(bus[0], g) <+ V(bus[0], g)/1e3;` compiles and stamps
/// *that bit's* node only — proven through a DC Jacobian read, not
/// elaboration alone: `d(residual[bus[0]])/d(V[bus[0]]) = 1/1000` and
/// every other bus bit's row is exactly zero (no cross-talk from the
/// widened index resolution wiring the wrong terminal).
#[test]
fn constant_index_stamps_only_that_bits_node() {
    // `bus[1..3]` get a zero-coefficient placeholder contribution purely
    // so they become kernel terminals at all (a node never referenced by
    // any contribution never appears in `terminals()`) — the assertion
    // below is that the *coefficient*, not merely the row's existence, is
    // untouched by `bus[0]`'s own contribution.
    let k = kernel(
        "analog begin
            I(bus[0], g) <+ V(bus[0], g) / 1e3;
            I(bus[1], g) <+ 0.0 * V(bus[1], g);
            I(bus[2], g) <+ 0.0 * V(bus[2], g);
            I(bus[3], g) <+ 0.0 * V(bus[3], g);
        end",
    )
    .expect("constant-indexed contribution must compile");
    let n = k.num_terminals();
    let mut volts = vec![0.0; n];
    let i_bus0 = terminal_index(&k, "bus[0]");
    volts[i_bus0] = 3.0;
    let jac = jacobian(&k, &volts);

    let g = jac[i_bus0 * n + i_bus0];
    assert!((g - 1e-3).abs() < 1e-9, "dI(bus[0])/dV(bus[0]) must be the 1/1e3 conductance, got {g}");

    for name in ["bus[1]", "bus[2]", "bus[3]"] {
        let i_other = terminal_index(&k, name);
        let row_self = jac[i_other * n + i_other];
        assert!(
            row_self.abs() < 1e-12,
            "contribution to bus[0] only must not stamp {name}'s own row, got {row_self}"
        );
        let cross = jac[i_other * n + i_bus0];
        assert!(
            cross.abs() < 1e-12,
            "contribution to bus[0] only must not stamp {name}'s row w.r.t. V(bus[0]), got {cross}"
        );
    }

    // The residual at bus[0]=3V must be exactly the stamped current
    // (3.0/1000 = 3mA), not zero (a silently-dropped/misrouted
    // contribution would read back as 0 here).
    let out = residual(&k, &volts);
    assert!((out[i_bus0] - 3e-3).abs() < 1e-9, "I(bus[0],g) residual must equal V(bus[0])/1e3 = 3mA, got {}", out[i_bus0]);
}

/// Done-when #2: `V(bus[3], bus[1])` resolves *both* terminals — a
/// current into `bus[2]` proportional to `V(bus[3]) - V(bus[1])` must
/// depend on exactly those two bits' voltages (Jacobian columns), and no
/// others.
#[test]
fn constant_index_resolves_both_terminals_of_a_read() {
    let k = kernel(
        "analog begin
            I(bus[2], g) <+ V(bus[3], bus[1]) / 1e3;
            I(bus[0], g) <+ 0.0 * V(bus[0], g);
        end",
    )
    .expect("two-terminal indexed read must compile");
    let n = k.num_terminals();
    let volts = vec![0.0; n];
    let jac = jacobian(&k, &volts);

    let i2 = terminal_index(&k, "bus[2]");
    let i1 = terminal_index(&k, "bus[1]");
    let i3 = terminal_index(&k, "bus[3]");
    let i0 = terminal_index(&k, "bus[0]");

    assert!((jac[i2 * n + i3] - 1e-3).abs() < 1e-9, "dI(bus[2])/dV(bus[3]) must be +1/1e3, got {}", jac[i2 * n + i3]);
    assert!((jac[i2 * n + i1] - -1e-3).abs() < 1e-9, "dI(bus[2])/dV(bus[1]) must be -1/1e3, got {}", jac[i2 * n + i1]);
    assert!(jac[i2 * n + i0].abs() < 1e-12, "dI(bus[2])/dV(bus[0]) must be 0 (bus[0] is not a read terminal here), got {}", jac[i2 * n + i0]);
    assert!(jac[i2 * n + i2].abs() < 1e-12, "dI(bus[2])/dV(bus[2]) must be 0 (bus[2] is the destination, not a read terminal), got {}", jac[i2 * n + i2]);
}

/// Done-when #3: a non-constant index still fails loud (T6's rule,
/// re-asserted here so widening the resolver to accept `Expr::Index`
/// cannot silently swallow a genuinely non-constant one, LRM §5.5.2's own
/// "the index ... shall be a constant expression"). `k` is a runtime
/// `integer` variable, never elaboration-constant, and this contribution
/// sits outside any `for`/`repeat` loop, so no unroll ever substitutes it.
///
/// **Updated by T8** (LRMW3-02): elaboration itself no longer rejects this
/// — `elab/lower/vams.rs::net_ref_for_branch` now tolerates *any*
/// non-foldable `Expr::Index` terminal at the elaboration-branch-list
/// stage, on the (correct, for T8's own genvar case) assumption that a
/// real per-iteration substitution happens later in codegen. Since this
/// contribution is *not* inside a `for` loop, that substitution never
/// happens, so the still-unresolved `bus[k]` terminal now fails loud one
/// stage later instead — `flatten::analog::AnalogFlattener::resolve_node`
/// (T7's own fail-loud fix), citing the identical §5.5.2 rule. The
/// guarantee this test pins ("never silently resolves to a node") holds
/// unchanged; only which layer reports it moved.
#[test]
fn non_constant_index_still_fails_loud() {
    let err = match kernel(
        "integer k;
    analog begin
        k = 2;
        I(bus[k], g) <+ V(bus[k], g) / 1e3;
    end",
    ) {
        Ok(_) => panic!("a runtime-variable vector index must never silently resolve to a node"),
        Err(e) => e,
    };
    assert!(
        err.contains("5.5.2"),
        "error must cite LRM §5.5.2 (the index-must-be-constant rule), got: {err}"
    );
    assert!(
        err.contains("does not resolve to a node"),
        "error must name the actual failure (the unresolved `bus[k]` terminal), got: {err}"
    );
}
