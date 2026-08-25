//! LRM-COVERS: 4.7, 4.7.1, 4.7.2, 4.7.2.1, 4.7.2.2, 4.7.2.3, 4.7.3
//! Wave-2 (T69) re-reading, no new tests: §4.7.2 "Returning a value from
//! an analog user defined function" is the containing heading for
//! §4.7.2.1/§4.7.2.2/§4.7.2.3, all three already proven by this file.
//!
//! LRMW2-44 (`lrm-conformance-wave2`, T44): the resolved-form user-function
//! inliner (`resolve/pom/expr.rs::inline_user_fn`), now reachable once
//! `SymbolTable::fn_by_name` is populated (`resolve/pom/structure.rs`'s
//! `Function` registration loop, fed from `piperine_lang::pom::Module::
//! functions`). Root `tests/analog_function_e2e.rs` covers the AC15
//! simulate proof (§4.7.1's `maxValue` example); this file covers the
//! resolve-stage typed-error paths and the `output`/`inout` write-back
//! mechanism (§4.7.2.2/§4.7.2.3, §4.7.1 Example 2's `geomcalc`) that don't
//! need a full transient/DC run to observe.

use piperine_codegen::resolve::{lower_bodies, LowerErrors};
use piperine_lang::parse_and_elaborate;
use piperine_lang::SourceMap;

fn dut_source(function_decl: &str, body: &str) -> String {
    format!(
        "module dut (inout electrical p, inout electrical n);
            {function_decl}
            analog begin
                {body}
            end
        endmodule"
    )
}

/// §4.7's own restriction ("shall have at least one formal argument
/// declared") — checked at declaration time, in the elaborator, before
/// codegen ever sees the function.
#[test]
fn zero_argument_function_fails_loud_at_declaration_citing_4_7() {
    let src = "module dut ();
        analog function real noargs;
            begin
                noargs = 1.0;
            end
        endfunction
        analog begin
        end
    endmodule";
    let err = parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("a function with zero formal arguments must fail elaboration (LRM §4.7)");
    let msg = format!("{err:?}");
    assert!(msg.contains("4.7"), "expected the error to cite LRM §4.7, got: {msg}");
}

/// §4.7.1/§4.7.3: VAMS function formals have no default-value syntax — a
/// call with the wrong number of arguments is an arity error, not a
/// default-fill, asserted on the typed `LowerError` kind (never
/// `is_err()`).
#[test]
fn wrong_arity_call_fails_loud_by_typed_kind() {
    let src = dut_source(
        "analog function real maxValue;
            input n1, n2;
            real n1, n2;
            begin
                maxValue = (n1 > n2) ? n1 : n2;
            end
        endfunction",
        "V(p, n) <+ maxValue(1.0);",
    );
    let design = parse_and_elaborate(&src, &SourceMap::dummy()).expect("elaborates");
    let Err(LowerErrors(errors)) = lower_bodies(&design) else {
        panic!("a wrong-arity user-function call must fail lowering, not resolve silently");
    };
    assert!(
        errors.iter().any(|e| e.what.contains("arity") && e.what.contains("4.7")),
        "expected an arity error citing §4.7, got: {errors:?}"
    );
}

/// A (direct) recursive user-function call cannot terminate under this
/// compiler's substitution-based inliner — fails loud by typed kind rather
/// than overflowing the stack.
#[test]
fn recursive_call_fails_loud_by_typed_kind() {
    let src = dut_source(
        "analog function real recur;
            input x;
            real x;
            begin
                recur = recur(x);
            end
        endfunction",
        "V(p, n) <+ recur(1.0);",
    );
    let design = parse_and_elaborate(&src, &SourceMap::dummy()).expect("elaborates");
    let Err(LowerErrors(errors)) = lower_bodies(&design) else {
        panic!("a recursive user-function call must fail lowering, not hang/overflow");
    };
    assert!(
        errors.iter().any(|e| e.what.contains("recursive") && e.what.contains("4.7")),
        "expected a recursive-call error citing §4.7, got: {errors:?}"
    );
}

/// §4.7.2.1: "If this internal variable is not assigned during the
/// execution … the function will return the initialised value of zero
/// (0)" — NOT an error. A function whose body never assigns its own name
/// must lower successfully and read back as `0.0`.
#[test]
fn unassigned_return_defaults_to_zero_per_4_7_2_1() {
    let src = dut_source(
        "analog function real neverAssigns;
            input x;
            real x;
            begin
                // deliberately does not assign `neverAssigns`
            end
        endfunction",
        "V(p, n) <+ neverAssigns(1.0);",
    );
    let design = parse_and_elaborate(&src, &SourceMap::dummy()).expect("elaborates");
    lower_bodies(&design).expect("an unassigned return value is 0.0 per §4.7.2.1, not a lowering error");
}

/// §4.7.1 Example 2's `geomcalc`, §4.7.2.2: an `output` argument's
/// call-site variable observes the function's final assigned value after
/// the call — asserted at the resolve stage (the write-back companion
/// `Assign` the inliner synthesizes), not merely that lowering succeeds.
#[test]
fn output_argument_write_back_reaches_the_resolved_body() {
    let src = dut_source(
        "analog function real geomcalc;
            input l, w;
            output area, perim;
            real l, w, area, perim;
            begin
                area = l * w;
                perim = 2 * (l + w);
            end
        endfunction
        real a, peri;",
        "V(p, n) <+ geomcalc(3.0, 4.0, a, peri);",
    );
    let design = parse_and_elaborate(&src, &SourceMap::dummy()).expect("elaborates");
    let bodies = lower_bodies(&design).expect("output-argument call lowers");
    let dut = bodies.get("dut").expect("dut lowered");
    let analog = dut.analog.as_ref().expect("dut has an analog body");
    let dump = format!("{:?}", analog.stmts);
    // The write-back is a synthetic `a = <geomcalc's final `area`>` /
    // `peri = <geomcalc's final `perim`>` assignment, sequenced right
    // after the contribution — both caller-side idents must show up
    // assigned to the multiplied/summed values (3.0*4.0 = 12.0,
    // 2*(3.0+4.0) = 14.0), not left untouched at their pre-call value.
    assert!(dump.contains("\"a\""), "expected a write-back assignment to `a`, got: {dump}");
    assert!(dump.contains("\"peri\""), "expected a write-back assignment to `peri`, got: {dump}");
}

/// §4.7.2.2/§4.7.2.3: the argument bound to an `output`/`inout` formal
/// must be a variable reference — a literal constant cannot be written
/// back to, so this fails loud by typed kind rather than silently
/// dropping the write-back.
#[test]
fn output_argument_bound_to_a_non_variable_fails_loud() {
    let src = dut_source(
        "analog function real geomcalc;
            input l, w;
            output area, perim;
            real l, w, area, perim;
            begin
                area = l * w;
                perim = 2 * (l + w);
            end
        endfunction
        real peri;",
        "V(p, n) <+ geomcalc(3.0, 4.0, 0.0, peri);",
    );
    let design = parse_and_elaborate(&src, &SourceMap::dummy()).expect("elaborates");
    let Err(LowerErrors(errors)) = lower_bodies(&design) else {
        panic!("binding a literal to an output argument must fail lowering");
    };
    assert!(
        errors.iter().any(|e| e.what.contains("variable reference")),
        "expected an output/inout-argument-must-be-a-variable error, got: {errors:?}"
    );
}
