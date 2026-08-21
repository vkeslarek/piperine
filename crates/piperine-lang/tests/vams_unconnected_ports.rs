//! LRM-COVERS: 9.19
//! §9.19 (LRMC-C7-09, T50): a module instantiated with a port left
//! unconnected (`.p()` or a blank/missing positional slot) elaborates —
//! previously a hard elaboration error ("not yet supported"). The
//! `$port_connected()` distinguishing *effect* is proven at the codegen
//! level (`crates/piperine-codegen/tests/vams_port_connected.rs`, since
//! it needs a compiled kernel); this target proves the elaboration-level
//! claim in isolation.

use piperine_lang::{parse_and_elaborate, SourceMap};

const MYCLK: &str = "
    module myclk (inout electrical vout_q, inout electrical vout_qbar);
    endmodule
";

#[test]
fn blank_positional_slot_elaborates() {
    let src = format!("{MYCLK} module top(); electrical q; myclk c1 (q, ); endmodule");
    let design = parse_and_elaborate(&src, &SourceMap::dummy()).expect("blank positional slot elaborates");
    let top = design.module("top").unwrap();
    assert!(top.instances[0].ports[1].is_unconnected());
}

#[test]
fn fewer_positional_connections_than_declared_ports_elaborates() {
    let src = format!("{MYCLK} module top(); electrical q; myclk c1 (q); endmodule");
    let design = parse_and_elaborate(&src, &SourceMap::dummy()).expect("short positional list elaborates");
    let top = design.module("top").unwrap();
    assert_eq!(top.instances[0].ports.len(), 2);
    assert!(!top.instances[0].ports[0].is_unconnected());
    assert!(top.instances[0].ports[1].is_unconnected());
}

#[test]
fn named_connection_with_blank_parens_elaborates() {
    let src = format!("{MYCLK} module top(); electrical q; myclk c1 (.vout_q(q), .vout_qbar()); endmodule");
    let design = parse_and_elaborate(&src, &SourceMap::dummy()).expect("named blank connection elaborates");
    let top = design.module("top").unwrap();
    assert!(top.instances[0].ports[1].is_unconnected());
}

#[test]
fn named_connection_omitting_a_port_entirely_elaborates() {
    let src = format!("{MYCLK} module top(); electrical q; myclk c1 (.vout_q(q)); endmodule");
    let design = parse_and_elaborate(&src, &SourceMap::dummy()).expect("omitted named port elaborates");
    let top = design.module("top").unwrap();
    assert!(top.instances[0].ports[1].is_unconnected());
}

/// Two different unconnected ports must never resolve to the identical
/// sentinel net — that would electrically short two unrelated
/// unconnected ports together once compiled (a real bug caught during
/// this task's own development, not a hypothetical).
#[test]
fn distinct_unconnected_ports_get_distinct_sentinel_nets() {
    let src = format!("{MYCLK} module top(); electrical q1, q2; myclk c1 (q1); myclk c2 (q2); endmodule");
    let design = parse_and_elaborate(&src, &SourceMap::dummy()).expect("elaborates");
    let top = design.module("top").unwrap();
    let net1 = &top.instances[0].ports[1];
    let net2 = &top.instances[1].ports[1];
    assert!(net1.is_unconnected() && net2.is_unconnected());
    assert_ne!(net1.net(), net2.net(), "distinct unconnected ports must not share a net");
}
