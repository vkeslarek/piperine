//! LRM-COVERS: 6.4.2, 6.9.2
//! LRMW2-22 (`lrm-conformance-wave2`, T29): §6.4.2 paramset overloading —
//! several `paramset` declarations may share a name, and the simulator
//! picks the one whose declared parameters cover every override on the
//! instance and whose parameter values (defaults + overrides) fall within
//! its own declared ranges. This task implements exactly those two
//! selection rules (the LRM's own text: "All parameters overridden on the
//! instance shall be parameters of the paramset" and "The parameters of
//! the paramset, with overrides and defaults, shall be all within the
//! allowed ranges") — the further tie-break cascade (fewest un-overridden
//! parameters, most-ranged local parameters, fewest unconnected ports) is
//! out of this task's declared scope; a genuine tie after the first two
//! rules is reported as an ambiguity error rather than silently resolved.
//!
//! Fixtures adapt §6.4.2's own short-channel/long-channel `nch` example
//! (disjoint `l` ranges distinguishing the two paramsets sharing the name
//! `nch`), simplified to drop the third "mismatch" paramset (which needs
//! `$rdist_normal`, out of this task's scope) and the parenthesized
//! `paramset name (module);` form matching this crate's existing grammar
//! (same disclosed FORM adaptation `vams_paramset.rs` already documents).

use piperine_lang::pom::{ElabError, ElabErrorKind, Value};
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

/// §6.4.2's own example structure: two `nch` paramsets sharing a name,
/// distinguished by disjoint `l` ranges — "short-channel" `[0.25e-6:1e-6)`
/// and "long-channel" `[1e-6:inf)`.
const OVERLOADED_FIXTURE: &str = r#"
    module nmos3 (inout electrical d, inout electrical g, inout electrical s, inout electrical b);
        parameter real l = 1e-6;
        parameter real w = 1e-6;
    endmodule

    paramset nch (nmos3); // short-channel paramset
        parameter real l = 0.25e-6 from [0.25e-6:1e-6);
        parameter real w = 1e-6 from [0.2e-6:inf);
        .l = l;
        .w = w;
    endparamset

    paramset nch (nmos3); // long-channel paramset
        parameter real l = 1e-6 from [1e-6:inf);
        parameter real w = 1e-6 from [0.2e-6:inf);
        .l = l;
        .w = w;
    endparamset

    module top ();
        electrical d1, g, s0, b0;
        nch #(.l(__L__), .w(5e-6)) m1 (d1, g, s0, b0);
    endmodule
"#;

/// A `l` value only the short-channel range accepts (`0.5e-6` falls inside
/// `[0.25e-6:1e-6)` but outside `[1e-6:inf)`) selects the short-channel
/// paramset's own `l` default range/binding — asserted by the effective
/// `l` value actually landing on the instance.
#[test]
fn disjoint_range_selects_the_short_channel_paramset() {
    let src = OVERLOADED_FIXTURE.replace("__L__", "0.5e-6");
    let d = elab(&src);
    let top = d.module("top").expect("top elaborated");
    let m1 = top.instances.iter().find(|i| i.label.as_deref() == Some("m1")).expect("instance m1");
    assert_eq!(m1.module, "nmos3");
    let l = m1.params.iter().find(|(n, _)| n == "l").map(|(_, v)| v.clone());
    assert_eq!(l, Some(Value::Real(0.5e-6)), "l=0.5e-6 is only accepted by the short-channel paramset's range");
}

/// A `l` value only the long-channel range accepts (`2e-6` falls inside
/// `[1e-6:inf)` but outside `[0.25e-6:1e-6)`) selects the long-channel
/// paramset instead — the two instances above discriminate: an
/// implementation that always picked the first-registered paramset
/// (name-keyed `HashMap` overwrite, the pre-T29 behavior) would use the
/// short-channel paramset here too and this value would still be accepted
/// (0.2e-6:inf covers w, but l's short-channel range `[0.25e-6:1e-6)`
/// rejects 2e-6 outright) — so a wrong selection fails loud here, a
/// correct one elaborates.
#[test]
fn disjoint_range_selects_the_long_channel_paramset() {
    let src = OVERLOADED_FIXTURE.replace("__L__", "2e-6");
    let d = elab(&src);
    let top = d.module("top").expect("top elaborated");
    let m1 = top.instances.iter().find(|i| i.label.as_deref() == Some("m1")).expect("instance m1");
    let l = m1.params.iter().find(|(n, _)| n == "l").map(|(_, v)| v.clone());
    assert_eq!(l, Some(Value::Real(2e-6)), "l=2e-6 is only accepted by the long-channel paramset's range");
}

/// A `l` value outside *both* declared ranges (e.g. `1e-7`, below even the
/// short-channel floor of `0.25e-6`) matches no candidate — fails loud
/// citing §6.4.2, not a silent fallback to either paramset.
#[test]
fn value_outside_every_candidates_range_fails_loud() {
    let src = OVERLOADED_FIXTURE.replace("__L__", "1e-7");
    let report = parse_and_elaborate(&src, &SourceMap::dummy()).expect_err("no candidate range accepts l=1e-7");
    let err = report
        .downcast_ref::<ElabError>()
        .unwrap_or_else(|| panic!("elaboration failure is a typed ElabError, got: {report}"));
    match &err.kind {
        ElabErrorKind::Other(msg) => {
            assert!(msg.contains("§6.4.2"), "error cites the LRM clause: {msg}");
        }
        other => panic!("expected ElabErrorKind::Other citing §6.4.2, got {other:?}"),
    }
}

/// Two overlapping-range paramsets sharing a name: no `l` value is
/// distinguishing (both ranges cover it), so selection is genuinely
/// ambiguous — §6.4.2 requires a unique match; this task does not
/// implement the LRM's further tie-break cascade, so an ambiguous match
/// must fail loud rather than silently picking one (e.g. the first
/// registered).
#[test]
fn overlapping_ranges_fail_loud_as_ambiguous_not_silently_resolved() {
    let src = r#"
        module nmos3 (inout electrical d, inout electrical g, inout electrical s, inout electrical b);
            parameter real l = 1e-6;
        endmodule

        paramset nch (nmos3); // first: full range
            parameter real l = 1e-6 from [0.1e-6:10e-6);
            .l = l;
        endparamset

        paramset nch (nmos3); // second: fully overlapping range
            parameter real l = 1e-6 from [0.2e-6:9e-6);
            .l = l;
        endparamset

        module top ();
            electrical d1, g, s0, b0;
            nch #(.l(1e-6)) m1 (d1, g, s0, b0);
        endmodule
    "#;
    let report = parse_and_elaborate(src, &SourceMap::dummy()).expect_err("overlapping ranges must be ambiguous");
    let err = report
        .downcast_ref::<ElabError>()
        .unwrap_or_else(|| panic!("elaboration failure is a typed ElabError, got: {report}"));
    match &err.kind {
        ElabErrorKind::Other(msg) => {
            assert!(msg.contains("ambiguous"), "error names the ambiguity: {msg}");
            assert!(msg.contains("§6.4.2"), "error cites the LRM clause: {msg}");
        }
        other => panic!("expected ElabErrorKind::Other citing an ambiguous §6.4.2 match, got {other:?}"),
    }
}

/// §6.4.2's first selection rule: an override naming a parameter that only
/// one of the overloaded candidates declares excludes the other candidate
/// from the set, even without any range distinction at all.
#[test]
fn override_naming_only_one_candidates_parameter_selects_that_candidate() {
    let src = r#"
        module nmos3 (inout electrical d, inout electrical g, inout electrical s, inout electrical b);
            parameter real l = 1e-6;
            parameter integer mm = 0;
        endmodule

        paramset nch (nmos3); // no mm parameter
            parameter real l = 1e-6 from [0.2e-6:10e-6);
            .l = l;
        endparamset

        paramset nch (nmos3); // has mm parameter
            parameter real l = 1e-6 from [0.2e-6:10e-6);
            parameter integer mm = 0 from [0:1];
            .l = l;
            .mm = mm;
        endparamset

        module top ();
            electrical d1, g, s0, b0;
            nch #(.l(1e-6), .mm(1)) m1 (d1, g, s0, b0);
        endmodule
    "#;
    let d = elab(src);
    let top = d.module("top").expect("top elaborated");
    let m1 = top.instances.iter().find(|i| i.label.as_deref() == Some("m1")).expect("instance m1");
    let mm = m1.params.iter().find(|(n, _)| n == "mm").map(|(_, v)| v.clone());
    assert_eq!(mm, Some(Value::Int(1)), "only the mm-declaring candidate accepts the .mm override");
}

/// §6.9.2 ("Elaboration and paramsets"): "If a generate construct contains
/// an instantiation of an overloaded paramset, then the paramset selection
/// is performed after the generate construct has been evaluated. The
/// evaluation of the generate construct may influence the values ... of
/// the paramset instance, and hence the selection of matching paramset and
/// module." A `genvar`-driven `for`-generate loop overrides `l` with a
/// per-iteration, genvar-derived expression that lands inside the
/// short-channel range on even iterations and the long-channel range on
/// odd ones — each generated instance must independently resolve to the
/// paramset its own (post-generate-evaluation) `l` value falls in, not a
/// single selection frozen before the loop unrolled.
#[test]
fn generate_loop_selects_a_different_overloaded_paramset_per_iteration() {
    let src = r#"
        module nmos3 (inout electrical d, inout electrical g, inout electrical s, inout electrical b);
            parameter real l = 1e-6;
            parameter real w = 1e-6;
        endmodule

        paramset nch (nmos3); // short-channel paramset
            parameter real l = 0.25e-6 from [0.25e-6:1e-6);
            parameter real w = 1e-6 from [0.2e-6:inf);
            .l = l;
            .w = w;
        endparamset

        paramset nch (nmos3); // long-channel paramset
            parameter real l = 1e-6 from [1e-6:inf);
            parameter real w = 1e-6 from [0.2e-6:inf);
            .l = l;
            .w = w;
        endparamset

        module top ();
            electrical d1, g, s0, b0;
            genvar i;
            for (i = 0; i < 2; i = i + 1) begin: g
                nch #(.l(0.5e-6 + i * 0.75e-6), .w(5e-6)) m1 (d1, g, s0, b0);
            end
        endmodule
    "#;
    let d = elab(src);
    let top = d.module("top").expect("top elaborated");
    let flat = d.flat_module("top").expect("top has a flat form");
    let l_of = |name: &str| -> f64 {
        match flat
            .instances()
            .iter()
            .find(|i| i.name() == name)
            .unwrap_or_else(|| panic!("expected generated instance {name}"))
            .params
            .iter()
            .find(|(n, _)| n == "l")
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("expected an l override on {name}"))
        {
            Value::Real(v) => v,
            other => panic!("expected Real l override on {name}, got {other:?}"),
        }
    };
    // Iteration 0: l = 0.5e-6 (inside the short-channel range only).
    assert!((l_of("m1__0") - 0.5e-6).abs() < 1e-12, "iteration 0 resolves the short-channel paramset");
    // Iteration 1: l = 1.25e-6 (inside the long-channel range only).
    assert!((l_of("m1__1") - 1.25e-6).abs() < 1e-12, "iteration 1 resolves the long-channel paramset");
    // Both instances resolve to the same underlying module, per iteration's own selection.
    for inst in flat.instances() {
        assert_eq!(inst.module, "nmos3");
    }
    let _ = top; // (top authored, unflattened, kept alongside the flat proof)
}
