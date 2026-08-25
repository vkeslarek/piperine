//! LRM-COVERS: 6.6, 6.6.1, 6.6.3, 6.9.4, 6.3.5, 9.19
//!
//! LRMW2-52 (`lrm-conformance-wave2`, T51): `elab/lower/generate.rs`'s
//! `unroll_items` used to recognize only `ModuleItem::Instantiation` and
//! nested generate constructs inside a generate body — every other item
//! kind (nets, `analog`/digital behavior blocks, ...) hit the `_ => {}`
//! fallthrough and vanished silently. This file covers the language/POM
//! layer: the LRM's own `rcline`/`adc` §6.6.1 worked examples elaborate
//! (spec AC16's "compile" half — the *simulated* half is root
//! `tests/generate_items_e2e.rs`, since `rcline`/`adc` themselves hit a
//! separate, pre-existing, disclosed codegen gap — see that file's own
//! doc comment), `genblk<n>` naming (§6.6.1) and explicit-label honoring,
//! `$param_given` (§6.3.5/§9.19) recognized in a generate condition, and
//! `defparam` applying to a generate-produced instance (§6.9.4).
//!
//! Both LRM examples are adapted for pre-existing, unrelated gaps this
//! task does not touch (none in `generate.rs`/`defparam.rs`, T51's own
//! file scope):
//! - a name-list-style port/net re-declaration must write the array range
//!   *after* the name (`electrical out [0:8];`), not before it (the LRM's
//!   own literal `electrical [0:bits-1] out;` — range-before-name — fails
//!   to parse today, `grammar/item.rs::parse_declarator` only accepts
//!   `name '[' range ']'`);
//! - a net declaration's array width must be a literal, not a parameter
//!   expression (`electrical n [0:N];` with `N` a `parameter` fails
//!   *elaboration* — `declarator_net_type`'s width const-eval runs against
//!   an env that does not yet see the enclosing module's own params at
//!   that call site — a separate, non-generate-specific gap);
//! - a parameter-dependent array index used as a `V(...)`/`I(...)`
//!   *contribution destination* (not a port connection, where it already
//!   works) fails the same way (`elab/lower/vams.rs::
//!   resolve_or_create_branch` evaluates the index against a hardcoded
//!   empty `ConstEnv`, unrelated to generate). `rcline`'s own
//!   `V(n2, n[N])` boundary connection hits this — replaced with the
//!   literal index matching `N`'s value.
//!
//! None of these three touch the actual generate-unrolling behavior under
//! test — `adc`'s `V(out[i])` is a *generated* statement whose `i` is
//! substituted to a literal *before* branch resolution ever runs
//! (`unroll_loop`), so it does not hit the third gap.

use piperine_lang::ast::{Expr, Stmt};
use piperine_lang::lexer::NumberLit;
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

/// §6.6.1's own `rcline` example, verbatim except the port/net
/// range-position adaptation noted above. AC16: compiles (elaborates) and
/// the generate loop's instances land in the derived flat form with the
/// LRM's own resistor/capacitor structure, `N` sections each.
const RCLINE: &str = r#"
    module resistor (inout electrical p, inout electrical n);
        parameter real r = 1e3;
        analog I(p, n) <+ V(p, n) / r;
    endmodule
    module capacitor (inout electrical p, inout electrical n);
        parameter real c = 1e-12;
        analog I(p, n) <+ c * ddt(V(p, n));
    endmodule
    module rcline (n1, n2);
        inout n1, n2;
        electrical n1, n2, gnd;
        ground gnd;
        parameter integer N = 4 from (0:inf);
        electrical n [0:4];
        parameter Cap = 1p, Res = 1k;
        localparam Csec = Cap/N, Rsec = Res/N;
        genvar i;
        // "generate" and "endgenerate" keywords are not required.
        for (i=0; i <N; i=i+1) begin
            resistor #(.r(Rsec)) R(n[i], n[i+1]);
            capacitor #(.c(Csec)) C(n[i+1], gnd);
        end
        analog begin
            V(n1, n[0]) <+ 0.0;
            V(n2, n[4]) <+ 0.0;
        end
    endmodule
"#;

#[test]
fn rcline_generate_loop_unrolls_resistor_capacitor_instances() {
    let mut d = elab(RCLINE);
    d.set_top("rcline");
    let flat = d.flat_module("rcline").expect("rcline flattens");
    let resistors = flat.instances.iter().filter(|i| i.module == "resistor").count();
    let capacitors = flat.instances.iter().filter(|i| i.module == "capacitor").count();
    assert_eq!(resistors, 4, "N=4: one resistor per generate iteration");
    assert_eq!(capacitors, 4, "N=4: one capacitor per generate iteration");
}

/// MD-25: `rcline`'s generate loop produces zero authored instances — the
/// POM shows exactly what was written (a `for` generate construct, no
/// instances), the derived instances live only in the flattened form.
#[test]
fn rcline_generate_produces_no_authored_instances() {
    let d = elab(RCLINE);
    let rcline = d.module("rcline").expect("rcline authored");
    assert_eq!(rcline.instances.len(), 0, "the generate loop's instances must not appear in Design::modules");
}

/// §6.6.1's own `adc` example, adapted only for the range-position
/// grammar limitation. AC16's "compile" half: elaborates cleanly and the
/// generate loop's `analog begin ... end` block is unrolled `bits` times.
const ADC: &str = r#"
    module adc (in, out);
        parameter bits=8, fullscale=1.0, dly=0.0, ttime=10n;
        input in;
        output out [0:7];
        electrical in;
        electrical out [0:7];
        real sample, thresh;
        genvar i;
        analog begin
            thresh = fullscale/2.0;
            sample = V(in);
        end
        generate
        for (i=bits-1; i>=0; i=i-1)
        analog begin
            V(out[i]) <+ transition(sample > thresh, dly, ttime);
            if (sample > thresh) sample = sample - thresh;
            sample = 2.0*sample;
        end
        endgenerate
    endmodule
"#;

fn int_lit(e: &Expr) -> Option<i64> {
    match e {
        Expr::Number { value: NumberLit::Integer(n), .. } => Some(*n),
        _ => None,
    }
}

/// The generated analog behavior's own `out[i]` target has `i` replaced
/// with the literal iteration value — §3.5 ("a genvar shall not be used
/// ... outside a loop generate scheme": once unrolled, the genvar has no
/// runtime existence). `bits=8` unrolls 8 times, one behavior per
/// iteration, each targeting a *different* literal index (7 down to 0,
/// the LRM's own descending loop) — the discrimination check a naive
/// "genvar substitution that always plugs in the same value" would fail.
#[test]
fn adc_generate_loop_substitutes_the_genvar_per_iteration() {
    let mut d = elab(ADC);
    d.set_top("adc");
    let flat = d.flat_module("adc").expect("adc flattens");
    // One authored analog behavior (`thresh`/`sample` init) plus 8
    // generated ones (one per `for` iteration).
    let generated: Vec<&piperine_lang::pom::Behavior> =
        flat.behaviors.iter().filter(|b| b.is_analog() && b.body.len() == 3).collect();
    assert_eq!(generated.len(), 8, "bits=8: one generated analog behavior per iteration");

    let mut indices: Vec<i64> = generated
        .iter()
        .filter_map(|b| {
            b.body.iter().find_map(|s| match s {
                Stmt::Contribution { target: Expr::AccessFunction { pos, .. }, .. } => match pos.as_ref() {
                    Expr::Index { index, .. } => int_lit(index),
                    _ => None,
                },
                _ => None,
            })
        })
        .collect();
    indices.sort_unstable();
    assert_eq!(
        indices,
        vec![0, 1, 2, 3, 4, 5, 6, 7],
        "each of the 8 iterations' `V(out[i])` target must carry a distinct literal index 0..7, \
         not the same value repeated or the unresolved genvar name"
    );
}

/// §6.6.1: an unnamed generate loop's block is implicitly named
/// `genblk<n>` — a net declared inside it lands in the flat module dotted
/// under that name.
#[test]
fn unnamed_generate_block_nets_are_scoped_genblk1() {
    let src = r#"
        module top ();
            electrical gnd;
            genvar i;
            for (i = 0; i < 2; i = i + 1) begin
                electrical w;
            end
        endmodule
    "#;
    let mut d = elab(src);
    d.set_top("top");
    let flat = d.flat_module("top").expect("top flattens");
    let names: Vec<&str> = flat.wires.iter().map(|w| w.name.as_str()).collect();
    assert!(names.contains(&"genblk1__0.w"), "expected `genblk1__0.w`, got {names:?}");
    assert!(names.contains(&"genblk1__1.w"), "expected `genblk1__1.w`, got {names:?}");
}

/// An explicit `begin : name` label is honored instead of `genblk<n>`
/// (§6.6.1's own `rcline2` example uses `begin : section`).
#[test]
fn explicit_generate_block_label_is_honored_over_genblk_naming() {
    let src = r#"
        module top ();
            electrical gnd;
            genvar i;
            for (i = 0; i < 2; i = i + 1) begin : section
                electrical w;
            end
        endmodule
    "#;
    let mut d = elab(src);
    d.set_top("top");
    let flat = d.flat_module("top").expect("top flattens");
    let names: Vec<&str> = flat.wires.iter().map(|w| w.name.as_str()).collect();
    assert!(names.contains(&"section__0.w"), "expected the explicit label `section`, got {names:?}");
    assert!(!names.iter().any(|n| n.contains("genblk")), "an explicit label must suppress genblk naming: {names:?}");
}

/// §6.3.5/§9.19: `$param_given` is recognized inside a generate `if`
/// condition (§6.3's own `nlres` worked example) instead of failing loud
/// as an unsupported const-eval expression — the un-overridden case (no
/// `#(...)` override anywhere) correctly evaluates `$param_given(coeff1)`
/// as `false`, taking the `else` arm. (T51 SPEC_DEVIATION, disclosed in
/// the commit body: `UnrollGenerate` runs once per module *declaration*,
/// not per instantiation, so a per-instantiation-overridden `true` case
/// is a follow-up architecture change, not implemented here.)
const NLRES: &str = r#"
    module resistor (inout electrical a, inout electrical b);
        parameter real r = 1e3;
        analog I(a, b) <+ V(a, b) / r;
    endmodule
    module nlres (inout electrical a, inout electrical b);
        parameter real res = 1k from (0:inf);
        parameter real coeff1 = 0.0;
        generate
        if ($param_given(coeff1) && coeff1 != 0.0)
            analog V(a, b) <+ res * (1.0 + coeff1 * I(a, b)) * I(a, b);
        else if (res == 0.0)
            analog V(a, b) <+ 0.0;
        else
            resistor #(.r(res)) R1(a, b);
        endgenerate
    endmodule
"#;

#[test]
fn param_given_in_a_generate_condition_does_not_fail_loud() {
    let mut d = elab(NLRES);
    d.set_top("nlres");
    let flat = d.flat_module("nlres").expect("nlres flattens");
    assert_eq!(flat.instances.len(), 1, "the un-overridden case must take the resistor branch");
    assert_eq!(flat.instances[0].module, "resistor");
}

/// §6.9.4 ("defparam statements shall be evaluated after generate
/// statement instantiation"): a `defparam` naming a generate-produced
/// instance now resolves and applies — before this task, `ApplyDefparams`
/// ran *before* `UnrollGenerate`, and `apply_one` only ever searched
/// `Design::modules`' authored instances, so the target could never be
/// found (LowerError "no instance labeled").
#[test]
fn defparam_applies_to_a_generate_produced_instance() {
    let src = r#"
        module leaf (inout electrical p, inout electrical n);
            parameter real r = 1.0;
        endmodule
        module top ();
            electrical a, b;
            genvar i;
            for (i = 0; i < 3; i = i + 1) begin
                leaf l (a, b);
            end
        endmodule
        module annotate ();
            defparam top.l__1.r = 42.0;
        endmodule
    "#;
    let mut d = elab(src);
    d.set_top("top");
    let flat = d.flat_module("top").expect("top flattens");
    let l0 = flat.instances.iter().find(|i| i.label.as_deref() == Some("l__0")).expect("l__0");
    let l1 = flat.instances.iter().find(|i| i.label.as_deref() == Some("l__1")).expect("l__1");
    let l2 = flat.instances.iter().find(|i| i.label.as_deref() == Some("l__2")).expect("l__2");
    assert!(l0.params.is_empty(), "l__0 must be untouched by the defparam");
    assert_eq!(
        l1.params.iter().find(|(n, _)| n == "r").map(|(_, v)| v.clone()),
        Some(piperine_lang::pom::Value::Real(42.0)),
        "l__1 (the defparam's own target) must get the overridden value"
    );
    assert!(l2.params.is_empty(), "l__2 must be untouched by the defparam");
}
