//! LRM-COVERS: 3.6.4
//!
//! LRM §3.6.4 "Ground declaration", Syntax 3-7: `ground [discipline] [range]
//! name;` designates a net as the global reference node. The parser only
//! ever accepted the bare `ground name;` spelling; `[discipline]`/`[range]`
//! were not implemented at all (SPEC_DEVIATION.md finding 1.4's grammar
//! half, re-verified reproducing at HEAD — `ground [3:0] bus;` and `ground
//! electrical gnd;` both failed to parse before this fix). The declaration
//! was also a pure no-op in the elaborator: `foo` in `ground foo;` never
//! became the POM's actual MNA reference, just a name the parser accepted
//! and threw away — `crates/piperine-codegen/tests/ground_declaration.rs`
//! is the "actually becomes the solver's reference node" simulate-level
//! proof; this file covers the grammar surface + the elaborated POM shape.

use piperine_lang::parse_and_elaborate;
use piperine_lang::SourceMap;

fn headers_source_map() -> SourceMap {
    let headers =
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/headers"));
    SourceMap::new(headers)
}

/// The LRM's own §3.6.4 worked example (`electrical gnd; ground gnd;`,
/// with `loadedsrc`'s ordinary declare-then-ground idiom) — bare form,
/// name already declared separately.
#[test]
fn bare_ground_of_an_already_declared_net_registers_it() {
    let src = "
        `include \"disciplines.vams\"
        module loadedsrc (inout electrical in, inout electrical out);
            electrical gnd;
            ground gnd;
            parameter real srcval = 5.0;
            analog V(out, gnd) <+ srcval;
        endmodule
    ";
    let design = parse_and_elaborate(src, &headers_source_map()).expect("elaborates");
    let m = design.modules().find(|m| m.name() == "loadedsrc").expect("module");
    assert_eq!(m.ground_names(), &["gnd".to_string()], "gnd must be registered as ground");
}

/// `ground electrical [3:0] bus;` — the range form (LRM Syntax 3-7's
/// `[range]`, paired with a discipline so Annex F has something to
/// resolve against — an undisciplined, unconnected net has no discipline
/// to infer by connectivity regardless of `ground`): declares AND grounds
/// a 4-wide bus in one statement.
#[test]
fn ranged_ground_declares_an_array_net() {
    let src = "
        `include \"disciplines.vams\"
        module m ();
            ground electrical [3:0] bus;
        endmodule
    ";
    let design = parse_and_elaborate(src, &headers_source_map()).expect("`ground electrical [3:0] bus;` parses");
    let m = design.modules().find(|m| m.name() == "m").expect("module");
    assert_eq!(m.ground_names(), &["bus".to_string()]);
    let wire = m.wires().iter().find(|w| w.name() == "bus").expect("bus wire declared");
    assert_eq!(wire.net_type().width(), 4, "[3:0] must declare a 4-wide net, got width {}", wire.net_type().width());
}

/// `ground electrical gnd;` — the combined discipline+declare form (LRM
/// Syntax 3-7's `[discipline_identifier]`): declares AND grounds `gnd` in
/// one statement, with no prior separate net declaration.
#[test]
fn ground_with_a_discipline_declares_and_grounds_in_one_statement() {
    let src = "
        `include \"disciplines.vams\"
        module m ();
            ground electrical gnd;
        endmodule
    ";
    let design = parse_and_elaborate(src, &headers_source_map()).expect("`ground electrical gnd;` parses");
    let m = design.modules().find(|m| m.name() == "m").expect("module");
    assert_eq!(m.ground_names(), &["gnd".to_string()]);
    let wire = m.wires().iter().find(|w| w.name() == "gnd").expect("gnd wire declared by the combined form");
    assert_eq!(wire.net_type().discipline_name(), "electrical");
}

/// Regression: `gnd`/`GND`/`vss`/`VSS`/`0` keep resolving to ground with
/// NO `ground` declaration at all — the crate-global name-list fallback
/// this task must not break.
#[test]
fn undeclared_gnd_still_elaborates_via_the_name_list_fallback() {
    let src = "
        `include \"disciplines.vams\"
        module m (inout electrical p, inout electrical gnd);
            analog V(p, gnd) <+ 1.0;
        endmodule
    ";
    let design = parse_and_elaborate(src, &headers_source_map()).expect("elaborates");
    let m = design.modules().find(|m| m.name() == "m").expect("module");
    assert!(
        m.ground_names().is_empty(),
        "no `ground` declaration was written — the POM registry must stay empty \
         (the fallback lives in the crate-global name list, not here)"
    );
}
