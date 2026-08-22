//! LRM-COVERS: 7.8.5, 7.6, 7.7.1
//! LRMW2-25 (`lrm-conformance-wave2`, T31): §7.8.5's predictable naming
//! scheme for auto-inserted connect-module instances
//! (`SigName__ModuleName__BottomDiscipline` merged /
//! `SigName__InstName__PortName` split), Table 7-2 direction-aware connect
//! module selection (`select_connect_module` now checks the actual
//! bridged port's direction against the candidate's own port direction,
//! complementary per Table 7-2, not discipline names alone), and
//! `connect_port_overrides` (§7.7.1 Syntax 7-6) actually changing which
//! connect module gets selected — previously parsed into `ConnectRule::
//! overrides` and never read (SPEC_DEVIATION.md §2.12).

use piperine_lang::pom::ElabError;
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

/// AC (Done-when #1, merged): the inserted instance's name is literally
/// `SigName__ModuleName__BottomDiscipline` — `n` (the net), `bridge` (the
/// connect module), `logic` (the discrete/"bottom" discipline at the
/// formal connection, i.e. `digital_leaf`'s own port `p`).
#[test]
fn merged_instance_name_follows_the_lrm_scheme() {
    const SRC: &str = r#"
        discipline logic;
            domain discrete;
        enddiscipline

        connectrules basic_rules;
            connect logic, electrical;
        endconnectrules

        connectmodule bridge (inout logic a, inout electrical b);
        endmodule

        module digital_leaf (inout logic p);
        endmodule

        module top ();
            electrical n;
            digital_leaf d1 (n);
            digital_leaf d2 (n);
        endmodule
    "#;
    let design = elab(SRC);
    let top_flat = design.flat_module("top").expect("top's flattened form");
    let bridge = top_flat.instances.iter().find(|i| i.module == "bridge").expect("one merged bridge inserted");
    assert_eq!(
        bridge.label.as_deref(),
        Some("n__bridge__logic"),
        "merged naming: SigName__ModuleName__BottomDiscipline, got {:?}",
        bridge.label
    );
}

/// AC (Done-when #1, split): `SigName__InstName__PortName` — one name per
/// port, naming the specific instance (`d1`/`d2`) and port (`p`) each
/// bridges, not a shared name.
#[test]
fn split_instance_names_follow_the_lrm_scheme() {
    const SRC: &str = r#"
        discipline logic;
            domain discrete;
        enddiscipline

        connectrules basic_rules;
            connect bridge split logic, electrical;
        endconnectrules

        connectmodule bridge (inout logic a, inout electrical b);
        endmodule

        module digital_leaf (inout logic p);
        endmodule

        module top ();
            electrical n;
            digital_leaf d1 (n);
            digital_leaf d2 (n);
        endmodule
    "#;
    let design = elab(SRC);
    let top_flat = design.flat_module("top").expect("top's flattened form");
    let mut names: Vec<&str> =
        top_flat.instances.iter().filter(|i| i.module == "bridge").filter_map(|i| i.label.as_deref()).collect();
    names.sort();
    assert_eq!(
        names,
        vec!["n__d1__p", "n__d2__p"],
        "split naming: SigName__InstName__PortName per port, got {names:?}"
    );
}

/// Two connect modules sharing the same discipline pair but declared with
/// *opposite* port directions (mirroring the LRM's own `elect_to_logic`/
/// `logic_to_elect` — Table 7-2's two non-`inout` rows): `d2a`'s `logic`
/// port is `input`, `a2d`'s `logic` port is `output`. Two topologies, each
/// needing the opposite digital-port direction, must each select the
/// *other* candidate — proving selection is genuinely direction-aware,
/// not just discipline-pair matching (AC Done-when #2).
const TWO_DIRECTIONAL_CANDIDATES: &str = r#"
    discipline logic;
        domain discrete;
    enddiscipline

    connectrules basic_rules;
        connect d2a logic, electrical;
        connect a2d logic, electrical;
    endconnectrules

    connectmodule d2a (input logic in, output electrical out);
    endmodule

    connectmodule a2d (output logic out, input electrical in);
    endmodule

    module digital_drv (output logic out);
    endmodule

    module digital_recv (input logic in);
    endmodule

    module top1 ();
        electrical n1;
        digital_drv d1 (n1);
    endmodule

    module top2 ();
        electrical n2;
        digital_recv d2 (n2);
    endmodule
"#;

#[test]
fn direction_aware_selection_picks_the_module_matching_an_output_digital_port() {
    let design = elab(TWO_DIRECTIONAL_CANDIDATES);
    let top1_flat = design.flat_module("top1").expect("top1's flattened form");
    // `digital_drv`'s `out` port is `output logic` -- Table 7-2 requires
    // the bridging module's own `logic` port to be `input` (complementary)
    // -- that's `d2a`, not `a2d`.
    assert!(
        top1_flat.instances.iter().any(|i| i.module == "d2a"),
        "an output digital port needs d2a (logic=input): {:?}",
        top1_flat.instances.iter().map(|i| &i.module).collect::<Vec<_>>()
    );
    assert!(
        !top1_flat.instances.iter().any(|i| i.module == "a2d"),
        "a2d (logic=output) does not fit an output digital port"
    );
}

#[test]
fn direction_aware_selection_picks_the_module_matching_an_input_digital_port() {
    let design = elab(TWO_DIRECTIONAL_CANDIDATES);
    let top2_flat = design.flat_module("top2").expect("top2's flattened form");
    // `digital_recv`'s `in` port is `input logic` -- Table 7-2 requires
    // the bridging module's own `logic` port to be `output`
    // (complementary) -- that's `a2d`, the *other* candidate from the
    // previous test, proving the selection genuinely depends on direction.
    assert!(
        top2_flat.instances.iter().any(|i| i.module == "a2d"),
        "an input digital port needs a2d (logic=output): {:?}",
        top2_flat.instances.iter().map(|i| &i.module).collect::<Vec<_>>()
    );
    assert!(
        !top2_flat.instances.iter().any(|i| i.module == "d2a"),
        "d2a (logic=input) does not fit an input digital port"
    );
}

/// Discriminating (Done-when #2's own parenthetical: "the old code picks
/// the wrong one"): with only `d2a` (`logic`=`input`) registered, an
/// `input`-digital-port topology needs the *opposite* (`logic`=`output`)
/// shape — pre-T31 code ignored direction entirely and would have
/// silently wired `d2a` in backwards (no error, wrong topology); T31's
/// direction check must instead fail loud rather than silently
/// mis-selecting.
#[test]
fn a_direction_mismatched_sole_candidate_fails_loud_instead_of_silently_wiring_backwards() {
    const SRC: &str = r#"
        discipline logic;
            domain discrete;
        enddiscipline

        connectrules basic_rules;
            connect d2a logic, electrical;
        endconnectrules

        connectmodule d2a (input logic in, output electrical out);
        endmodule

        module digital_recv (input logic in);
        endmodule

        module top ();
            electrical n;
            digital_recv d1 (n);
        endmodule
    "#;
    let report = parse_and_elaborate(SRC, &SourceMap::dummy())
        .expect_err("d2a's logic=input does not fit an input digital port (needs logic=output)");
    let err = report
        .downcast_ref::<ElabError>()
        .unwrap_or_else(|| panic!("elaboration failure is a typed ElabError, got: {report}"));
    use piperine_lang::pom::ElabErrorKind;
    assert!(
        matches!(err.kind, ElabErrorKind::ConnectModuleAmbiguous { .. }),
        "expected ConnectModuleAmbiguous (no direction-fit candidate), got {:?}",
        err.kind
    );
}

/// AC (Done-when #3): `connect_port_overrides` changes which discipline
/// pair a connect module bridges. `bridge2` is declared for
/// (`logic2`,`electrical2`) — a `connect bridge2 logic, electrical;` rule
/// (the `Both` override form, §7.7.1) reuses it for (`logic`,`electrical`)
/// instead. Without reading `overrides`, selection would look for a
/// `connectmodule` literally declaring `logic`/`electrical` ports and find
/// none (`bridge2`'s own ports are `logic2`/`electrical2`) — the pre-T31
/// bug.
#[test]
fn connect_port_overrides_changes_which_module_is_selected() {
    const SRC: &str = r#"
        discipline logic;
            domain discrete;
        enddiscipline
        discipline logic2;
            domain discrete;
        enddiscipline
        discipline electrical2;
            domain continuous;
        enddiscipline

        connectrules basic_rules;
            connect bridge2 logic, electrical;
        endconnectrules

        connectmodule bridge2 (inout logic2 a, inout electrical2 b);
        endmodule

        module digital_leaf (inout logic p);
        endmodule

        module top ();
            electrical n;
            digital_leaf d1 (n);
        endmodule
    "#;
    let design = elab(SRC);
    let top_flat = design.flat_module("top").expect("top's flattened form");
    assert!(
        top_flat.instances.iter().any(|i| i.module == "bridge2"),
        "connect_port_overrides must let bridge2 (native logic2/electrical2) bridge logic/electrical: {:?}",
        top_flat.instances.iter().map(|i| &i.module).collect::<Vec<_>>()
    );
}
