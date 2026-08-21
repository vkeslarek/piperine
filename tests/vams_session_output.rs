//! LRM-COVERS: 9.2
//! LRMC-C7-07 (T43): `SessionBuilder::output(...)` — a host-configured
//! `$display`-family sink reaches every analysis run through the
//! resulting `Session`, end-to-end through the real host surface
//! (`piperine::Session`, not `CircuitCompiler` directly).

use std::rc::Rc;

use piperine::prelude::{BufferSink, SolverConfig};
use piperine::Session;
use piperine_lang::{parse_and_elaborate, SourceMap};

const RC_WITH_DISPLAY: &str = r#"
    module vsrc (inout electrical p, inout electrical n);
        parameter real dc = 1.0;
        analog begin
            V(p, n) <+ dc;
            $display("tick");
        end
    endmodule

    module resistor (inout electrical p, inout electrical n);
        parameter real r = 1.0e3;
        analog begin
            I(p, n) <+ V(p, n) / r;
        end
    endmodule

    module capacitor (inout electrical p, inout electrical n);
        parameter real c = 1.0e-6;
        analog begin
            I(p, n) <+ c * ddt(V(p, n));
        end
    endmodule

    module top ();
        electrical gnd, vin, vout;
        vsrc #(.dc(1.0)) v1 (vin, gnd);
        resistor #(.r(1.0e3)) r1 (vin, vout);
        capacitor #(.c(1.0e-6)) c1 (vout, gnd);
    endmodule
"#;

#[test]
fn session_builder_output_captures_display_through_a_real_session() {
    let design = parse_and_elaborate(RC_WITH_DISPLAY, &SourceMap::dummy()).expect("RC fixture elaborates");
    let sink = Rc::new(BufferSink::new());
    let mut session = Session::builder(&design, "top")
        .output(sink.clone())
        .compile()
        .expect("session compiles");

    let (r, c) = (1.0e3, 1.0e-6);
    let tau = r * c;
    let config = SolverConfig::default();
    let _trace = session
        .tran(5.0 * tau, Some(tau / 20.0), 0.0, &config, None, false, &[])
        .expect("the RC circuit converges");

    let tick_count = sink.stdout().matches("tick\n").count();
    assert!(
        tick_count > 0,
        "SessionBuilder::output's sink must capture $display output through a real Session, got: {:?}",
        sink.stdout()
    );
    assert_eq!(sink.stderr(), "", "no $fatal/$error path was exercised");
}
