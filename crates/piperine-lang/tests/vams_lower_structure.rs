//! VAMS T22 (VAMS-04) — structural lowering: modules, ports (both
//! declaration-list and name-list styles), nets/wires with disciplines,
//! parameters, and hierarchical instantiation (named + ordered connections,
//! parameter overrides).
//!
//! MD-25 test: the elaborated instance→submodule→sub-instance tree is
//! walkable exactly as authored — no flattening, no synthesised structure.

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
