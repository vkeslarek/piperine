//! LRM-COVERS: 9.16
//! §9.16 (LRMC-C7-09, T49): `Session::simprobe` — the host surface half
//! of `$simprobe(inst_name, param_name[, expr])` (JIT dispatch inside a
//! `.vams` body is a documented, disclosed gap — see `Session::
//! simprobe`'s own doc comment).

use piperine::Session;
use piperine_lang::{parse_and_elaborate, SourceMap};

const SRC: &str = "
    module leaf (inout electrical p, inout electrical n);
        real probed_quantity;
        analog begin
            probed_quantity = 42.0;
            I(p, n) <+ V(p, n);
        end
    endmodule

    module top ();
        electrical vin, gnd;
        leaf l1 (vin, gnd);
    endmodule
";

#[test]
fn simprobe_returns_the_probed_quantity_during_a_run() {
    let design = parse_and_elaborate(SRC, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::compile(&design, "top").expect("session compiles");
    session.op(&Default::default(), None).expect("dc converges");

    let v = session.simprobe("l1", "probed_quantity", None).expect("resolves");
    assert!((v - 42.0).abs() < 1e-9, "got {v}");
}

#[test]
fn simprobe_falls_back_to_the_expression_when_unresolvable() {
    let design = parse_and_elaborate(SRC, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::compile(&design, "top").expect("session compiles");
    session.op(&Default::default(), None).expect("dc converges");

    let v = session.simprobe("no_such_instance", "no_such_param", Some(7.0)).expect("falls back");
    assert_eq!(v, 7.0);
}

#[test]
fn simprobe_fails_loud_when_unresolvable_and_no_fallback_given() {
    let design = parse_and_elaborate(SRC, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::compile(&design, "top").expect("session compiles");
    session.op(&Default::default(), None).expect("dc converges");

    let err = session.simprobe("no_such_instance", "no_such_param", None).unwrap_err();
    assert!(format!("{err}").contains("no_such_instance"));
}
