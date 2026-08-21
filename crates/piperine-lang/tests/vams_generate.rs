//! LRM-COVERS: 6.6, 6.6.1, 6.6.2
//! VAMS T25 (VAMS-04) — `UnrollGenerate`: `for`/`if`/`case` generate
//! constructs (LRM ch. 6) unroll into a **derived** instance set, never
//! into `Design::modules` (MD-25 VAMS clause).

use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

const LEAF: &str = "
module leaf (inout electrical p, inout electrical n);
endmodule
";

#[test]
fn for_generate_is_absent_from_the_authored_module() {
    // MD-25: the generate block reads back as a generate block — the
    // authored `top` module has zero instances of its own (the generate
    // loop is the only source of instances here), even though its
    // *flattened* form has four.
    let src = format!(
        "{LEAF}\nmodule top (inout electrical a, inout electrical b);\n\
         genvar i;\n\
         for (i = 0; i < 4; i = i + 1) begin: g\n\
             leaf l1(a, b);\n\
         end\n\
         endmodule\n"
    );
    let d = elab(&src);
    let top = d.module("top").expect("top elaborated");
    assert!(top.instances().is_empty(), "generate-produced instances never land in Design::modules");
}

#[test]
fn for_generate_instances_reach_the_derived_flat_form() {
    let src = format!(
        "{LEAF}\nmodule top (inout electrical a, inout electrical b);\n\
         genvar i;\n\
         for (i = 0; i < 4; i = i + 1) begin: g\n\
             leaf l1(a, b);\n\
         end\n\
         endmodule\n"
    );
    let d = elab(&src);
    let flat = d.flat_module("top").expect("top has a flat form");
    // leaf is itself a leaf module, so its 4 generated instances survive
    // flattening as-is (no further inlining needed).
    assert_eq!(flat.instances().len(), 4, "all four loop iterations reached the derived flat form");
    let labels: Vec<&str> = flat.instances().iter().map(|i| i.name()).collect();
    for i in 0..4 {
        assert!(labels.contains(&format!("l1__{i}").as_str()), "mangled label for iteration {i}: {labels:?}");
    }
}

#[test]
fn if_generate_picks_the_taken_branch_only() {
    let src = format!(
        "{LEAF}\nmodule top (inout electrical a, inout electrical b);\n\
         if (1) begin\n\
             leaf yes(a, b);\n\
         end else begin\n\
             leaf no(a, b);\n\
         end\n\
         endmodule\n"
    );
    let d = elab(&src);
    let flat = d.flat_module("top").expect("top has a flat form");
    assert_eq!(flat.instances().len(), 1);
    assert_eq!(flat.instances()[0].name(), "yes");
}

#[test]
fn for_generate_body_sees_the_enclosing_module_s_own_params() {
    // `urc[N]`'s exact shape (headers/spice/urc.phdl): a generate loop's
    // sub-instance param override references two params the *enclosing*
    // module declares (`r`, `k`), combined with the loop's own genvar via
    // `pow` — this used to fail with `undefined name: r` because
    // `UnrollGenerate` built its own permanently-empty `ConstEnv`.
    let src = r#"
        module leaf (inout electrical p, inout electrical n);
            parameter real r = 1.0;
        endmodule

        module top (inout electrical a, inout electrical b);
            parameter real r = 100.0;
            parameter real k = 1.5;
            genvar i;
            for (i = 0; i < 3; i = i + 1) begin: g
                leaf #(.r(r * pow(k, i))) l1 (a, b);
            end
        endmodule
    "#;
    let d = elab(src);
    let flat = d.flat_module("top").expect("top has a flat form");
    assert_eq!(flat.instances().len(), 3);
    for (i, expected_k_pow) in [(0, 1.0), (1, 1.5), (2, 2.25)] {
        let inst = flat
            .instances()
            .iter()
            .find(|inst| inst.name() == format!("l1__{i}"))
            .unwrap_or_else(|| panic!("l1__{i} present"));
        let (_, value) = inst.params.iter().find(|(n, _)| n == "r").expect("r override");
        let expected = 100.0 * expected_k_pow;
        match value {
            piperine_lang::pom::Value::Real(got) => {
                assert!((got - expected).abs() < 1e-9, "iteration {i}: got {got}, expected {expected}")
            }
            other => panic!("expected Real, got {other:?}"),
        }
    }
}

#[test]
fn generate_produced_instance_colliding_with_an_authored_one_fails_loud() {
    // `if`-generate doesn't mangle its body's instance labels (no loop
    // index to fold in), so an authored `l1` and an `if`-generated `l1`
    // collide directly.
    let src = format!(
        "{LEAF}\nmodule top (inout electrical a, inout electrical b);\n\
         leaf l1(a, b);\n\
         if (1) begin\n\
             leaf l1(a, b);\n\
         end\n\
         endmodule\n"
    );
    let msg = elab_err(&src);
    assert!(msg.contains("l1"), "names the colliding instance: {msg}");
}
