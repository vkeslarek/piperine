//! T35 (VAMS-12/13) — `@device`/`@port` re-anchored on VAMS `(* ... *)`
//! attribute-instance syntax. `(* device = "plugin", type = "type_id" *)`
//! on a module binds exactly as PHDL's `@device(plugin = …, type = …)`
//! attribute did (design P2 AC1); `(* port = "name" [, kind = "…"] *)` on
//! a port mirrors PHDL's `@port(name = …, kind = …?)`. An attribute-instance
//! name that is neither of these two (nor seeded by the caller) fails loud
//! (`ElabErrorKind::UnknownAttrSchema`, E2022) — MD-24's "no silent
//! fallback" rule ported to VAMS (design P2 AC2).

use piperine_lang::{parse_and_elaborate, parse_and_elaborate_seeded, seed_device_port_schemas, SourceMap};

const DEVICE_SRC: &str = "
(* device = \"none\", type = \"host_resistor\" *)
module HostR (inout electrical p, inout electrical n);
    parameter real r = 1e3;
endmodule
";

const PORT_SRC: &str = "
module HostR(p, n);
    (* port = \"A0\", kind = \"digital\" *) inout electrical p;
    inout electrical n;
endmodule
";

const UNKNOWN_ATTR_SRC: &str = "
(* frobnicate = \"x\" *)
module Foo (inout electrical p, inout electrical n);
endmodule
";

const DEVICE_MISSING_TYPE_SRC: &str = "
(* device = \"none\" *)
module Foo (inout electrical p, inout electrical n);
endmodule
";

#[test]
fn device_attribute_lowers_into_the_device_schema_when_seeded() {
    let design = parse_and_elaborate_seeded(DEVICE_SRC, &SourceMap::dummy(), seed_device_port_schemas)
        .expect("elaborates with device/port schemas seeded");
    let m = design.module("HostR").expect("module HostR");
    let attr = m.attributes().iter().find(|a| a.schema() == "device").expect("device attribute present");
    assert_eq!(attr.field("plugin").unwrap().to_string(), "\"none\"");
    assert_eq!(attr.field("type").unwrap().to_string(), "\"host_resistor\"");
}

#[test]
fn port_attribute_lowers_into_the_port_schema_with_optional_kind() {
    let design = parse_and_elaborate_seeded(PORT_SRC, &SourceMap::dummy(), seed_device_port_schemas)
        .expect("elaborates with device/port schemas seeded");
    let m = design.module("HostR").expect("module HostR");
    let p = m.ports().iter().find(|p| p.name() == "p").expect("port p");
    let attr = p.attributes().iter().find(|a| a.schema() == "port").expect("port attribute present");
    assert_eq!(attr.field("name").unwrap().to_string(), "\"A0\"");
    assert_eq!(attr.field("kind").unwrap().to_string(), "\"digital\"");
    // The un-attributed sibling port carries no attributes at all.
    let n = m.ports().iter().find(|p| p.name() == "n").expect("port n");
    assert!(n.attributes().is_empty());
}

#[test]
fn device_attribute_without_seeding_fails_loud_as_unknown_schema() {
    let err = parse_and_elaborate(DEVICE_SRC, &SourceMap::dummy())
        .expect_err("must fail without seeded schemas — MD-24 no silent fallback");
    let rendered = format!("{err:?}");
    assert!(rendered.contains("E2022"), "{rendered}");
    assert!(rendered.contains("device"), "{rendered}");
}

#[test]
fn an_unregistered_attribute_name_fails_loud_as_unknown_schema() {
    let err = parse_and_elaborate_seeded(UNKNOWN_ATTR_SRC, &SourceMap::dummy(), seed_device_port_schemas)
        .expect_err("an attribute Piperine's schema registry never declared must fail loud");
    let rendered = format!("{err:?}");
    assert!(rendered.contains("E2022"), "{rendered}");
    assert!(rendered.contains("frobnicate"), "{rendered}");
}

#[test]
fn device_attribute_missing_its_required_type_field_fails_loud() {
    let err = parse_and_elaborate_seeded(DEVICE_MISSING_TYPE_SRC, &SourceMap::dummy(), seed_device_port_schemas)
        .expect_err("@device without a sibling `type` attribute must fail loud");
    let rendered = format!("{err:?}");
    assert!(rendered.contains("E2023"), "{rendered}");
    assert!(rendered.contains("type"), "{rendered}");
}
