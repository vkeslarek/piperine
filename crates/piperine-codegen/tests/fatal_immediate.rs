//! `$fatal` fires immediately, without waiting for `accept_timestep`, while
//! `$error`/`$warning`/`$info` stay deferred (LRM §9.7.3, T27/LRMW2-27).
//!
//! `AnalogKernel::emit_diagnostics`'s own doc comment: that deferred list
//! only ever fires from `accept_timestep` (at most once per *accepted*
//! step). A `$fatal` guarded by a condition true only during an
//! intermediate, later-rejected Newton iteration would never be seen if it
//! stayed in that list — so `flatten/analog.rs` now routes `$fatal` into a
//! separate, guard-aware `fatal_flags`/`fatal_formats` pair
//! (`AnalogKernel::has_fatal`/`eval_fatal`/`emit_fatal_diagnostics`) that
//! `AnalogInstance::check_fatal` consults on every residual/Jacobian
//! evaluation (`eval_rhs_jac`), not just accepted ones.
//!
//! LRM-COVERS: 9.7.3

use std::collections::HashMap;

use piperine_codegen::resolve::{LoweredBody, Severity};
use piperine_codegen::{AnalogKernel, CircuitCompiler, CompiledModule, SimCtx};
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{CircularArrayBuffer2, DcAnalysisState};
use piperine_solver::prelude::{BufferSink, Context};
use std::rc::Rc;

/// A resistive device whose analog body guards both `$fatal` and `$error`
/// on the SAME condition (`V(a,b) > 1.0`) — a fixture that would behave
/// identically for both severities if `$fatal` were still folded into the
/// same deferred list `$error` uses.
const GUARDED_FATAL_AND_ERROR: &str = "
module F (inout electrical a, inout electrical b);
    analog begin
        if (V(a, b) > 1.0)
            $fatal(\"overvoltage\");
        if (V(a, b) > 1.0)
            $error(\"also too high\");
        I(a, b) <+ V(a, b) / 1e3;
    end
endmodule
";

/// `GUARDED_FATAL_AND_ERROR`'s `F`, wrapped in a portless top module the
/// circuit builder can instantiate directly (mirrors
/// `checkpoint_limiter.rs`'s own `limiter_circuit()`/`Top` shape).
const GUARDED_FATAL_AND_ERROR_TOP: &str = "
module F (inout electrical a, inout electrical b);
    analog begin
        if (V(a, b) > 1.0)
            $fatal(\"overvoltage\");
        if (V(a, b) > 1.0)
            $error(\"also too high\");
        I(a, b) <+ V(a, b) / 1e3;
    end
endmodule

module Top (inout electrical x, inout electrical y);
    F f1 (x, y);
endmodule
";

fn compile() -> std::sync::Arc<AnalogKernel> {
    let design = parse_and_elaborate(GUARDED_FATAL_AND_ERROR, &piperine_lang::SourceMap::dummy())
        .expect("fixture elaborates");
    let bodies: HashMap<String, LoweredBody> =
        piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let compiled = CompiledModule::compile(&bodies["F"]).expect("compiles");
    compiled.analog().expect("has an analog body").clone()
}

/// `$fatal` no longer lands in the deferred `diagnostics()` list at all —
/// only the `$error` call site does, at `Severity::Error`. If `$fatal` were
/// still routed through the old shared path, this list would carry two
/// entries, one at `Severity::Fatal`.
#[test]
fn fatal_is_routed_out_of_the_deferred_diagnostics_batch() {
    let kernel = compile();
    assert!(kernel.has_fatal(), "the kernel has a genuine $fatal call site");

    let severities: Vec<Severity> = kernel.diagnostics().iter().map(|d| d.severity).collect();
    assert_eq!(
        severities,
        vec![Severity::Error],
        "only $error stays in the deferred (accept_timestep-only) diagnostics list; \
         $fatal must not appear there at all — got {severities:?}"
    );
}

/// spec AC discriminator: `eval_fatal` is a real, per-call runtime check of
/// the guard — two different terminal-voltage inputs produce two different
/// answers (an unconditionally-true or unconditionally-false flag would
/// fail this).
#[test]
fn eval_fatal_reflects_the_guard_for_the_given_voltages() {
    let kernel = compile();
    let params: Vec<f64> = vec![];
    let state = vec![0.0; kernel.num_state_slots()];
    let vars = vec![0.0; kernel.num_vars()];
    let sim = SimCtx::default();

    // V(a, b) = volts[0] - volts[1] (port order a, b).
    let over = kernel.eval_fatal(&[2.0, 0.0], &params, &state, &vars, &sim);
    let under = kernel.eval_fatal(&[0.5, 0.0], &params, &state, &vars, &sim);

    assert!(over, "V(a,b) = 2.0 > 1.0 must trip the $fatal guard");
    assert!(!under, "V(a,b) = 0.5 <= 1.0 must NOT trip the $fatal guard");
}

/// The literal Done-when scenario: `load_dc` is exactly one Newton
/// iteration's evaluation, called by the solver's stepper *before* any
/// accept/reject decision — `accept_timestep` is a wholly separate,
/// later call this test never makes. A hand-picked voltage guess (the
/// same `DcAnalysisState`-construction technique `checkpoint_limiter.rs`
/// uses to target exact intermediate states) proves `$fatal` reacts to
/// *this* guess, regardless of what the circuit would ever actually
/// converge/accept to — "an iteration that is later rejected still
/// terminates the run" needs nothing more than this: the fatal check
/// never waits to find out whether the iteration was accepted.
#[test]
fn load_dc_panics_on_a_fatal_guess_and_never_emits_the_deferred_error() {
    let design = parse_and_elaborate(GUARDED_FATAL_AND_ERROR_TOP, &piperine_lang::SourceMap::dummy())
        .expect("fixture elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");

    // Terminal order = port order (a, b); V(a, b) = volts[0] - volts[1].
    let build = || {
        let mut compiler = CircuitCompiler::new(&design, &bodies);
        compiler.build_circuit_mapped("Top").expect("CircuitCompiler builds a device tree").0
    };

    // A guess whose V(a,b) = 2.0 - 0.0 = 2.0 > 1.0 trips both guards —
    // `$fatal` must panic before `$error` ever gets a chance to emit.
    {
        let mut circuit = build();
        let dev = &mut circuit.all_devices_mut()[0];
        let mut buf = CircularArrayBuffer2::new(1, 2);
        buf.push(&ndarray::arr1(&[2.0, 0.0]).view());
        let dc_state = DcAnalysisState::new(&buf, &[], 1.0);

        let sink = Rc::new(BufferSink::new());
        let mut ctx = Context::default();
        ctx.io = sink.clone();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dev.load_dc(&dc_state, &ctx)));
        assert!(result.is_err(), "V(a,b) = 2.0 > 1.0 must panic inside load_dc itself");
    }

    // A guess whose V(a,b) = 0.2 - 0.0 = 0.2 does not trip either guard —
    // no panic, and (Done-when regression) nothing reaches the sink: this
    // one `load_dc` call never calls `accept_timestep`/`emit_diagnostics`,
    // so `$error`'s deferred message must not have leaked out either.
    {
        let mut circuit = build();
        let dev = &mut circuit.all_devices_mut()[0];
        let mut buf = CircularArrayBuffer2::new(1, 2);
        buf.push(&ndarray::arr1(&[0.2, 0.0]).view());
        let dc_state = DcAnalysisState::new(&buf, &[], 1.0);

        let sink = Rc::new(BufferSink::new());
        let mut ctx = Context::default();
        ctx.io = sink.clone();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dev.load_dc(&dc_state, &ctx)));
        assert!(result.is_ok(), "V(a,b) = 0.2 <= 1.0 must not panic");
        assert_eq!(
            sink.stdout(), "",
            "load_dc alone never reaches accept_timestep, so $error's deferred message must not emit"
        );
        assert_eq!(sink.stderr(), "", "no $fatal path was exercised for this guess");
    }
}
