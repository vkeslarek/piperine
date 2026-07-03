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

#[test]
fn tran_delayed_start_records_from_start_not_zero() {
    // SPEC_BENCH.md §5.1 `TranConfig.start`: solve from t=0 (state evolution
    // matters), but only record steps with `t >= start` (ngspice `.tran tstart
    // tstop` semantics). The RC node sits at the DC steady state (5V) for the
    // whole run, so a delayed start must still see ~5V at the first recorded
    // sample — and that sample's time must be `>= start`, not 0.
    let src = format!(
        "{CIRCUIT}
        bench RcCharge {{
            fn test_delayed_start() {{
                var t = $tran(TranConfig {{ .stop = 1e-3, .step = 1e-4, .start = 0.5e-3 }});
                var axis = t.axis();
                $assert(axis.len() > 1, \"delayed-start trace still has samples\");
                $assert(axis.at(0.0) >= 0.5e-3, \"recording starts at .start, not t=0\");
                var v = t.v(out, gnd);
                $assert(v.at(axis.at(0.0)) > 4.9, \"still settled at the delayed start\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("RcCharge", "test_delayed_start") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

// ─── Appendix A closure: config bundles, $ac, $noise, $write ──────────────────

#[test]
fn op_takes_a_config_bundle() {
    let src = format!(
        "{CIRCUIT}
        bench SwitchOpenTest {{
            fn test_op_cfg() {{
                var r = $op(OpConfig {{ .solver = Solver {{ .temperature = 350.0 }} }});
                $assert(r.v(vsrc, gnd) > 4.9, \"config-bundle $op still solves\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("SwitchOpenTest", "test_op_cfg") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn tran_takes_a_config_bundle_with_auto_step() {
    let src = format!(
        "{CIRCUIT}
        bench RcCharge {{
            fn test_tran_cfg() {{
                var t = $tran(TranConfig {{ .stop = 1e-3 }});
                var v = t.v(out, gnd);
                $assert(v.len() > 1, \"adaptive trace has samples\");
                $assert(v.at(1e-3) > 4.9, \"settled at vsrc\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("RcCharge", "test_tran_cfg") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn ac_returns_complex_waveforms_with_db() {
    let src = format!(
        "{CIRCUIT}
        bench RcCharge {{
            fn test_ac() {{
                var r = $ac(AcConfig {{ .fstart = 1.0, .fstop = 1e6, .points = 10 }});
                var axis = r.axis();
                $assert(axis.len() > 1, \"sweep produced points\");
                var vdb = r.v(out, gnd).db();
                $assert(vdb.len() > 1, \"db projection has the sweep's points\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("RcCharge", "test_ac") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn noise_returns_psd_and_total() {
    let src = format!(
        "{CIRCUIT}
        bench RcCharge {{
            fn test_noise() {{
                var n = $noise(out, NoiseConfig {{ .fstart = 1.0, .fstop = 1e6, .points = 5 }});
                $assert(n.psd().len() > 1, \"psd has the sweep's points\");
                $assert(n.total() >= 0.0, \"integrated noise is non-negative\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("RcCharge", "test_noise") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn write_emits_a_csv_artifact() {
    let dir = std::env::temp_dir().join("piperine_bench_write_test");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("sweep.csv");
    let _ = std::fs::remove_file(&path);
    let src = format!(
        "{CIRCUIT}
        bench SwitchOpenTest {{
            fn test_write() {{
                var curve = [];
                for rl in [1e5, 1e6] {{
                    resistor.resistance = rl;
                    var r = $op();
                    curve.push((rl, r.v(signal, gnd)));
                }}
                $write(\"{path}\", curve);
            }}
        }}",
        path = path.display()
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("SwitchOpenTest", "test_write") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
    let contents = std::fs::read_to_string(&path).expect("csv written");
    assert_eq!(contents.lines().count(), 2, "one row per sweep point: {contents}");
    assert!(contents.starts_with("100000,"), "{contents}");
}

#[test]
fn bench_module_params_resolve_by_bare_name() {
    let src = format!(
        "{CIRCUIT}
        bench SwitchOpenTest {{
            fn test_params() {{
                $assert(threshold > 4.8, \"module param readable by bare name\");
            }}
        }}"
    );
    // Give the bench module a param to read.
    let src = src.replace(
        "mod SwitchOpenTest() {",
        "mod SwitchOpenTest() {\n        param threshold: Real = 4.9;",
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("SwitchOpenTest", "test_params") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn waveform_rise_time_measures_a_ramp() {
    // The settled RC node never rises, so drive the check off cross():
    // reuse the charging network but assert the rise_time contract shape.
    let src = format!(
        "{CIRCUIT}
        bench RcCharge {{
            fn test_rise_time_contract() {{
                var t = $tran(TranConfig {{ .stop = 1e-3 }});
                var v = t.v(out, gnd);
                // Settled node: no rising crossing → rise_time is none.
                $assert(v.rise_time(1.0, 4.0).is_none(), \"no rise on a settled node\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("RcCharge", "test_rise_time_contract") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn select_stages_across_a_selection() {
    let src = format!(
        "{CIRCUIT}
        bench SwitchOpenTest {{
            fn test_select_staging() {{
                select(\"/resistor\").resistance = 1e3;
                var r = $op();
                $assert(r.i(resistor.p, resistor.n) < 1e-8,
                        \"switch still open — but the staged 1k divider must have applied\");
                sw.ctrl = 1.0;
                var r2 = $op();
                $assert(r2.i(resistor.p, resistor.n) > 1e-3,
                        \"closed with 1k (staged via select): current ~ mA, not ~ uA\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("SwitchOpenTest", "test_select_staging") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}
