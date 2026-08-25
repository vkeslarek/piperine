//! LRM-COVERS: 6, 6.1, 6.2, 6.3, 6.3.3, 6.3.4, 6.5, 6.5.1, 6.5.2, 6.5.2.1, 6.5.2.2, 6.5.4, 6.5.5
//!
//! VAMS T22 (VAMS-04) — structural lowering: modules, ports (both
//! declaration-list and name-list styles), nets/wires with disciplines,
//! parameters, and hierarchical instantiation (named + ordered connections,
//! parameter overrides).
//!
//! MD-25 test: the elaborated instance→submodule→sub-instance tree is
//! walkable exactly as authored — no flattening, no synthesised structure.
//!
//! T69 (`lrm-conformance-wave2`) re-reading, no new tests: §6 "Hierarchical
//! structures" and §6.1 "Overview" are the containing narrative headings
//! for this whole file's subject, with no independent rule of their own.
//! `declaration_list_ports_lower_with_direction_and_discipline`/`name_
//! list_ports_lower_via_body_port_declarations` are §6.2's own module/port
//! declaration syntax (Syntax 6-1), §6.5's own "Ports" overview, §6.5.1's
//! port-definition syntax, §6.5.2's port declarations, and — via the
//! direction (`inout`) and discipline (`electrical`) assertions each test
//! makes — §6.5.2.2's port-direction and §6.5.2.1's port-type rules
//! specifically. `ordered_instance_connections_resolve_by_target_port_
//! position` is §6.5.4's ordered port-connection form;
//! `named_instance_connections_resolve_by_port_name_not_source_order` is
//! §6.5.5's named form. `instance_parameter_override_lowers_by_name`/`a_
//! module_forwards_its_own_param_to_a_sub_instance_override` are §6.3.3's
//! "module instance parameter value assignment by name" (§6.3 the
//! containing overview); `a_later_param_default_may_reference_an_earlier_
//! one_in_the_same_module` is §6.3.4's "Parameter dependence".

use piperine_lang::pom::Direction;
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

#[test]
fn declaration_list_ports_lower_with_direction_and_discipline() {
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
        endmodule
        "#,
    );
    let m = d.module("res").expect("res elaborated");
    assert_eq!(m.ports().len(), 2);
    let p = m.ports().iter().find(|p| p.name() == "p").expect("port p");
    assert_eq!(*p.direction(), Direction::Inout);
    assert_eq!(p.net_type().discipline_name(), "electrical");
}

#[test]
fn name_list_ports_lower_via_body_port_declarations() {
    let d = elab(
        r#"
        module res (p, n);
            inout electrical p, n;
        endmodule
        "#,
    );
    let m = d.module("res").expect("res elaborated");
    assert_eq!(m.ports().len(), 2);
    assert!(m.ports().iter().all(|p| *p.direction() == Direction::Inout));
    assert!(m.ports().iter().all(|p| p.net_type().discipline_name() == "electrical"));
}

#[test]
fn internal_wire_lowers_with_its_discipline() {
    let d = elab(
        r#"
        module divider (inout electrical p, inout electrical n);
            electrical mid;
        endmodule
        "#,
    );
    let m = d.module("divider").expect("divider elaborated");
    assert_eq!(m.wires().len(), 1);
    assert_eq!(m.wires()[0].name, "mid");
    assert_eq!(m.wires()[0].ty.discipline_name(), "electrical");
}

#[test]
fn parameter_declaration_lowers_with_default_value() {
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
            parameter real r = 1.0e3;
        endmodule
        "#,
    );
    let m = d.module("res").expect("res elaborated");
    assert_eq!(m.params().len(), 1);
    assert_eq!(m.params()[0].name, "r");
    assert_eq!(m.params()[0].default, Some(piperine_lang::pom::Value::Real(1.0e3)));
}

#[test]
fn ordered_instance_connections_resolve_by_target_port_position() {
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
        endmodule

        module divider (inout electrical top, inout electrical mid, inout electrical bot);
            res r1(top, mid);
            res r2(mid, bot);
        endmodule
        "#,
    );
    let m = d.module("divider").expect("divider elaborated");
    assert_eq!(m.instances().len(), 2, "MD-25: authored instance count, no flattening");

    let r1 = m.instances().iter().find(|i| i.name() == "r1").expect("r1");
    assert_eq!(r1.module, "res");
    assert_eq!(r1.ports.len(), 2);
    assert_eq!(r1.ports[0].net(), "top");
    assert_eq!(r1.ports[1].net(), "mid");

    let r2 = m.instances().iter().find(|i| i.name() == "r2").expect("r2");
    assert_eq!(r2.ports[0].net(), "mid");
    assert_eq!(r2.ports[1].net(), "bot");
}

#[test]
fn named_instance_connections_resolve_by_port_name_not_source_order() {
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
        endmodule

        module divider (inout electrical top, inout electrical bot);
            res r1(.n(bot), .p(top));
        endmodule
        "#,
    );
    let m = d.module("divider").expect("divider elaborated");
    let r1 = m.instances().iter().find(|i| i.name() == "r1").expect("r1");
    // Regardless of the source's `.n(...)` / `.p(...)` order, ports[] follows
    // the target module's own declared port order (p, n).
    assert_eq!(r1.ports[0].net(), "top", "p resolves first, matching res's own port order");
    assert_eq!(r1.ports[1].net(), "bot");
}

#[test]
fn instance_parameter_override_lowers_by_name() {
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
            parameter real r = 1.0e3;
        endmodule

        module top (inout electrical a, inout electrical b);
            res #(.r(50.0)) r1 (a, b);
        endmodule
        "#,
    );
    let m = d.module("top").expect("top elaborated");
    let r1 = m.instances().iter().find(|i| i.name() == "r1").expect("r1");
    assert_eq!(r1.params, vec![("r".to_string(), piperine_lang::pom::Value::Real(50.0))]);
}

#[test]
fn hierarchy_is_navigable_and_never_flattened() {
    // MD-25 UNBREAKABLE: Design::modules mirrors authored structure exactly
    // — a two-level hierarchy stays two authored modules with one instance
    // each, never collapsed into one flat leaf.
    let d = elab(
        r#"
        module leaf (inout electrical p, inout electrical n);
        endmodule

        module mid_module (inout electrical a, inout electrical b);
            leaf l1(a, b);
        endmodule

        module top (inout electrical x, inout electrical y);
            mid_module m1(x, y);
        endmodule
        "#,
    );
    assert_eq!(d.module_count(), 3, "authored module count — no flattening at this stage");
    let top = d.module("top").expect("top elaborated");
    assert_eq!(top.instances().len(), 1);
    assert_eq!(top.instances()[0].module, "mid_module");
    let mid = d.module("mid_module").expect("mid_module elaborated");
    assert_eq!(mid.instances().len(), 1);
    assert_eq!(mid.instances()[0].module, "leaf");
}

#[test]
fn a_module_forwards_its_own_param_to_a_sub_instance_override() {
    // Reproduces `urc_seg`'s own shape (headers/spice/urc.phdl): a
    // mid-level module declares `parameter real r`, then forwards it into
    // a child instance's `#(.r(r))` override — the elaboration-constant
    // evaluator gap (T43) made `r` invisible to its own module's const-eval
    // context, failing with `undefined name: r`.
    let d = elab(
        r#"
        module leaf (inout electrical p, inout electrical n);
            parameter real r = 100.0;
        endmodule

        module mid (inout electrical p, inout electrical n);
            parameter real r = 200.0;
            leaf #(.r(r)) l1 (p, n);
        endmodule
        "#,
    );
    let m = d.module("mid").expect("mid elaborated");
    let l1 = m.instances().iter().find(|i| i.name() == "l1").expect("l1");
    assert_eq!(
        l1.params,
        vec![("r".to_string(), piperine_lang::pom::Value::Real(200.0))],
        "mid's own `r` param forwards into l1's override"
    );
}

#[test]
fn a_later_param_default_may_reference_an_earlier_one_in_the_same_module() {
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
            parameter real r = 100.0;
            parameter real r2 = r + 1.0;
        endmodule
        "#,
    );
    let m = d.module("res").expect("res elaborated");
    let r2 = m.params().iter().find(|p| p.name == "r2").expect("r2 param");
    assert_eq!(r2.default, Some(piperine_lang::pom::Value::Real(101.0)));
}

#[test]
fn a_param_referencing_itself_fails_loud_instead_of_panicking() {
    let src = r#"
        module res (inout electrical p, inout electrical n);
            parameter real r = r + 1.0;
        endmodule
    "#;
    let err = piperine_lang::parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .expect_err("a self-referential param default must fail elaboration, not panic");
    let msg = err.to_string();
    assert!(msg.contains('r'), "names the undefined self-reference: {msg}");
}

#[test]
fn a_net_array_range_bound_may_reference_the_module_s_own_param() {
    // `wire tap : Electrical[N + 1];`-equivalent (urc[N]'s own construct):
    // the array width bound references a param the module itself declares.
    let d = elab(
        r#"
        module ladder (inout electrical p, inout electrical n);
            parameter integer segs = 4;
            electrical tap [segs:0];
        endmodule
        "#,
    );
    let m = d.module("ladder").expect("ladder elaborated");
    let tap = m.wires().iter().find(|w| w.name == "tap").expect("tap wire");
    assert_eq!(tap.ty.width(), 5, "0:segs with segs=4 is a 5-wide array");
}

#[test]
fn analog_behavior_attaches_to_its_module() {
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
            analog begin
                I(p, n) <+ 1.0;
            end
        endmodule
        "#,
    );
    let m = d.module("res").expect("res elaborated");
    assert_eq!(m.behaviors().len(), 1);
    assert!(m.behaviors()[0].is_analog());
    assert_eq!(m.behaviors()[0].body().len(), 1, "the single contribution statement");
}
