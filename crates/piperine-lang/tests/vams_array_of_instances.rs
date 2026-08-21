//! LRM-COVERS: 6.2.2
//! LRMC-C5-08 (`.specs/features/lrm-conformance-closure/`, T30):
//! `name_of_instance [msb:lsb] (port_connections)` (§6.2.2, Annex A.4.1)
//! parses and elaborates into the specified range of instances, each with
//! correct per-element port connections — MD-25's own authored-vs-derived
//! split: `Design::modules` stays a single ranged `Instance`, expansion is
//! `FlattenHierarchy`'s job into `Design::flat_modules` only.
//!
//! **Fixture adaptation (disclosed):** the task's own worked example
//! (`buf8 [7:0] (out[7:0], in[7:0]);`) writes a `[7:0]` part-select
//! directly in the port-connection expression. Port-connection part-select
//! (`out[7:0]` as a *connection value*, distinct from `Instance::range`'s
//! own `[7:0]`) has no `expr_to_net_ref` support at all — a real,
//! separate FORM gap, out of LRMC-C5-08's own declared scope (`grammar/
//! item.rs:842-855`, `ast/item.rs`, `elab/lower/`, none of which cover
//! connection-expression parsing). These fixtures instead connect the
//! *whole* bus by bare name (`out`, not `out[7:0]`) — semantically
//! identical once `out` is itself declared with the matching width, and
//! FlattenHierarchy's own per-element slicing (matching each instance
//! index against the connected wire's declared width) produces the exact
//! same elaborated per-element connections either way.

use piperine_lang::pom::NetRef;
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

// Annex A.4.1's `module_instantiation ::= module_or_paramset_identifier
// module_instance {...}` requires both a module-type token and a
// separate instance-name token — the task's own shorthand `buf8 [7:0]
// (...)` names only the instance; the module type is `buf1` here,
// instantiated under the label `buf8` (matching the shorthand's own
// spelling as closely as a syntactically complete instantiation allows).
const FIXTURE: &str = r#"
    module buf1 (inout electrical out, inout electrical in);
        analog begin
            V(out, in) <+ 0.0;
        end
    endmodule

    module top ();
        electrical out [7:0];
        electrical in [7:0];
        buf1 buf8 [7:0] (out, in);
    endmodule
"#;

#[test]
fn array_of_instances_parses_and_elaborates_to_the_specified_range() {
    // Authored form (MD-25): a single ranged Instance, not 8.
    let d = elab(FIXTURE);
    let top = d.module("top").expect("top elaborated");
    assert_eq!(top.instances.len(), 1, "the authored POM keeps exactly one ranged instance");
    assert_eq!(top.instances[0].range, Some((7, 0)));

    // Derived form: FlattenHierarchy expands it into 8 leaf instances.
    let flat = d.flat_module("top").expect("top flattened");
    assert_eq!(flat.instances.len(), 8, "the derived flat form has 8 leaf instances");
    let labels: Vec<&str> = flat.instances.iter().map(|i| i.label.as_deref().unwrap()).collect();
    assert_eq!(labels, vec!["buf8[7]", "buf8[6]", "buf8[5]", "buf8[4]", "buf8[3]", "buf8[2]", "buf8[1]", "buf8[0]"]);
}

#[test]
fn each_expanded_instance_gets_the_matching_bit_of_each_connected_bus() {
    let d = elab(FIXTURE);
    let flat = d.flat_module("top").expect("top flattened");
    for (k, inst) in flat.instances.iter().enumerate() {
        let expected_bit = 7 - k as u64; // buf8[7] first, down to buf8[0]
        assert_eq!(
            inst.ports,
            vec![NetRef::indexed("out", expected_bit), NetRef::indexed("in", expected_bit)],
            "instance {} must connect to bit {expected_bit} of both out and in",
            inst.label.as_deref().unwrap()
        );
        assert_eq!(inst.module, "buf1", "every expanded instance targets the same module type");
    }
}

#[test]
fn a_shared_scalar_connection_is_unchanged_across_every_element() {
    let d = elab(
        r#"
        module buf1 (inout electrical out, inout electrical in, inout electrical vdd);
            analog begin
                V(out, in) <+ 0.0;
            end
        endmodule

        module top ();
            electrical out [3:0];
            electrical in [3:0];
            electrical vdd;
            buf1 buf4 [3:0] (out, in, vdd);
        endmodule
        "#,
    );
    let flat = d.flat_module("top").expect("top flattened");
    assert_eq!(flat.instances.len(), 4);
    for inst in &flat.instances {
        assert_eq!(inst.ports[2], NetRef::simple("vdd"), "the scalar vdd connection is shared unchanged");
    }
}

#[test]
fn width_mismatch_between_range_and_connected_bus_fails_loud() {
    let src = r#"
        module buf1 (inout electrical out, inout electrical in);
            analog begin
                V(out, in) <+ 0.0;
            end
        endmodule

        module top ();
            electrical out [2:0];
            electrical in [7:0];
            buf1 buf8 [7:0] (out, in);
        endmodule
    "#;
    let msg = elab_err(src);
    assert!(msg.contains("out"), "names the mismatched net: {msg}");
    assert!(msg.contains('3'), "names its actual width: {msg}");
    assert!(msg.contains('8'), "names the expected instance count: {msg}");
}
