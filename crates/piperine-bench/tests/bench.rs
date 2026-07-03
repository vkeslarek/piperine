//! End-to-end tests: parse → elaborate → `BenchRunner` (SPEC_BENCH.md §12.1,
//! adapted to milestone-1's supported grammar — no config bundles yet).

use piperine_bench::{BenchOutcome, BenchRunner};
use piperine_lang::SourceMap;

fn elab(src: &str) -> piperine_lang::Design {
    piperine_lang::parse_str(src)
        .expect("parse failed")
        .elaborate(&SourceMap::dummy())
        .expect("elaborate failed")
}

const CIRCUIT: &str = "
    discipline Electrical { potential v: Real; flow i: Real; }

    mod VoltageSource(inout p: Electrical, inout n: Electrical) {
        param voltage: Real = 0.0;
    }
    analog VoltageSource { V(p, n) <- voltage; }

    mod Resistor(inout p: Electrical, inout n: Electrical) {
        param resistance: Real = 1e3;
    }
    analog Resistor { I(p, n) <+ V(p, n) / resistance; }

    mod Switch(inout a: Electrical, inout b: Electrical) {
        param ctrl: Real = 0.0;
        param ron: Real = 1.0;
    }
    analog Switch { I(a, b) <+ ctrl * V(a, b) / ron; }

    mod SwitchOpenTest() {
        wire gnd : Electrical;
        wire signal : Electrical;
        wire vsrc : Electrical;
        sw       : Switch        (.a = signal, .b = gnd) { .ctrl = 0.0 };
        source   : VoltageSource (.p = vsrc, .n = gnd) { .voltage = 5.0 };
        resistor : Resistor      (.p = vsrc, .n = signal) { .resistance = 1e6 };
    }

    mod Capacitor(inout p: Electrical, inout n: Electrical) {
        param c: Real = 1e-9;
    }
    analog Capacitor { I(p, n) <+ c * ddt(V(p, n)); }

    mod RcCharge() {
        wire gnd : Electrical;
        wire vsrc : Electrical;
        wire out : Electrical;
        source : VoltageSource (.p = vsrc, .n = gnd) { .voltage = 5.0 };
        r1     : Resistor      (.p = vsrc, .n = out) { .resistance = 1e3 };
        c1     : Capacitor     (.p = out, .n = gnd) { .c = 1e-6 };
    }
";

#[test]
fn test_open_circuit_passes() {
    let src = format!(
        "{CIRCUIT}
        bench SwitchOpenTest {{
            fn test_open_circuit() {{
                var r = $op();
                $assert(r.v(vsrc, gnd) > 4.9, \"voltage source should be active\");
                $assert(r.i(resistor.p, resistor.n) < 1e-8, \"no current with the switch open\");
            }}
        }}"
    );
    let design = elab(&src);
    let outcome = BenchRunner::new(&design).run_entry("SwitchOpenTest", "test_open_circuit");
    match outcome {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn test_closed_circuit_stages_and_passes() {
    let src = format!(
        "{CIRCUIT}
        bench SwitchOpenTest {{
            fn test_closed_circuit() {{
                sw.ctrl = 1.0;
                var r = $op();
                $assert(r.i(resistor.p, resistor.n) > 4e-6, \"current should flow when closed\");
            }}
        }}"
    );
    let design = elab(&src);
    let outcome = BenchRunner::new(&design).run_entry("SwitchOpenTest", "test_closed_circuit");
    match outcome {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn failing_assert_reports_failed_with_message() {
    let src = format!(
        "{CIRCUIT}
        bench SwitchOpenTest {{
            fn test_bogus() {{
                var r = $op();
                $assert(r.v(vsrc, gnd) > 100.0, \"should never hold\");
            }}
        }}"
    );
    let design = elab(&src);
    let outcome = BenchRunner::new(&design).run_entry("SwitchOpenTest", "test_bogus");
    match outcome {
        BenchOutcome::Failed(msg) => assert!(msg.contains("should never hold")),
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[test]
fn staging_does_not_leak_between_entry_points() {
    let src = format!(
        "{CIRCUIT}
        bench SwitchOpenTest {{
            fn close_it() {{
                sw.ctrl = 1.0;
                var r = $op();
                $assert(r.i(resistor.p, resistor.n) > 4e-6, \"should be closed here\");
            }}
            fn check_still_open() {{
                var r = $op();
                $assert(r.i(resistor.p, resistor.n) < 1e-8, \"must still be open — no leaked staging\");
            }}
        }}"
    );
    let design = elab(&src);
    let runner = BenchRunner::new(&design);
    assert!(matches!(runner.run_entry("SwitchOpenTest", "close_it"), BenchOutcome::Passed));
    assert!(matches!(runner.run_entry("SwitchOpenTest", "check_still_open"), BenchOutcome::Passed));
}

#[test]
fn run_all_discovers_every_entry_point() {
    let design = elab(&format!(
        "{CIRCUIT}
        bench SwitchOpenTest {{
            fn test_open_circuit() {{
                var r = $op();
                $assert(r.v(vsrc, gnd) > 4.9, \"open\");
            }}
            fn test_closed_circuit() {{
                sw.ctrl = 1.0;
                var r = $op();
                $assert(r.i(resistor.p, resistor.n) > 4e-6, \"closed\");
            }}
        }}"
    ));
    let report = BenchRunner::new(&design).run_all();
    assert_eq!(report.results.len(), 2);
    assert!(report.all_passed(), "{:?}", report.results.iter().map(|r| &r.outcome).collect::<Vec<_>>());
}

#[test]
fn tune_loop_stages_across_iterations_of_a_for() {
    let src = format!(
        "{CIRCUIT}
        bench SwitchOpenTest {{
            fn dc_gain_vs_load() {{
                var last = 0.0;
                for rl in [1e5, 1e6, 1e7] {{
                    resistor.resistance = rl;
                    var r = $op();
                    last = r.v(signal, gnd);
                }}
                $assert(last > 4.9, \"largest divider ratio should pull vsignal near vsrc\");
            }}
        }}"
    );
    let design = elab(&src);
    let outcome = BenchRunner::new(&design).run_entry("SwitchOpenTest", "dc_gain_vs_load");
    match outcome {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn tran_traces_a_settled_rc_node_over_time() {
    // No pulsed source / `$ic` yet (SPEC_BENCH.md §11 — deferred), so the
    // transient's auto-computed initial condition is already the DC
    // steady state (a capacitor blocks no current at DC, so `out` sits at
    // 5V for the whole run) — this exercises `$tran`/`Trace`/`Waveform`
    // end-to-end, not a charging transient.
    let src = format!(
        "{CIRCUIT}
        bench RcCharge {{
            fn test_settled_trace() {{
                var t = $tran(5e-3, 1e-5);
                var v = t.v(out, gnd);
                $assert(v.len() > 1, \"the trace should have multiple samples\");
                $assert(v.at(0.0) > 4.9, \"already at DC steady state: out starts near 5V\");
                $assert(v.at(5e-3) > 4.9, \"and stays near 5V\");
                $assert(v.peak_to_peak() < 0.1, \"a settled node shouldn't move\");
                var axis = t.axis();
                $assert(axis.at(5e-3) > axis.at(0.0), \"the time axis should be increasing\");
            }}
        }}"
    );
    let design = elab(&src);
    let outcome = BenchRunner::new(&design).run_entry("RcCharge", "test_settled_trace");
    match outcome {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}
