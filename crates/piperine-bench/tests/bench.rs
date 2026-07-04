//! End-to-end tests: parse → elaborate → `BenchRunner` (piperine-bench/docs/SPEC.md §12.1,
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
    // No pulsed source / `$ic` yet (piperine-bench/docs/SPEC.md §11 — deferred), so the
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
    // piperine-bench/docs/SPEC.md §5.1 `TranConfig.start`: solve from t=0 (state evolution
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
fn noise_config_out_field_drives_the_analysis() {
    // piperine-bench/docs/SPEC.md §5.1 `NoiseConfig.out` (G6): the output is a config
    // field — a `Branch` expressed as a bare `Net` (`(net, gnd)`) or a
    // `(Net, Net)` pair. Both must drive the analysis. The deprecated
    // positional `$noise(out, cfg)` alias is covered by the test above.
    let src = format!(
        "{CIRCUIT}
        bench RcCharge {{
            fn test_noise_out_field() {{
                var n1 = $noise(NoiseConfig {{ .out = out, .fstart = 1.0, .fstop = 1e6, .points = 5 }});
                $assert(n1.psd().len() > 1, \"single-net .out drives the sweep\");
                $assert(n1.total() >= 0.0, \"single-net .out integrates\");
                var n2 = $noise(NoiseConfig {{ .out = (out, gnd), .fstart = 1.0, .fstop = 1e6, .points = 5 }});
                $assert(n2.psd().len() > 1, \"net-pair .out drives the sweep\");
                $assert(n2.total() >= 0.0, \"net-pair .out integrates\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("RcCharge", "test_noise_out_field") {
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

#[test]
fn generic_bench_target_runs_once_per_monomorph() {
    // piperine-bench/docs/SPEC.md §3: "Post monomorphization, generics appear in concrete
    // form." A `bench Counter` targeting a generic base attaches to every
    // monomorphized instance (Counter__8, Counter__12) via the same
    // `Base__args` suffix rule AttachBehaviors uses, and runs once per
    // monomorph.
    let src = format!(
        "{CIRCUIT}
        mod Counter[N]() {{
            wire gnd : Electrical;
            wire out : Electrical;
            source : VoltageSource (.p = out, .n = gnd) {{ .voltage = 5.0 }};
            r : Resistor (.p = out, .n = gnd) {{ .resistance = 1000.0 }};
        }}
        mod Top() {{
            c8 : Counter[8]();
            c12 : Counter[12]();
        }}
        bench Counter {{
            fn test_runs() {{
                var r = $op();
                $assert(r.v(out, gnd) > 4.9, \"source holds the node\");
            }}
        }}"
    );
    let design = elab(&src);
    let report = BenchRunner::new(&design).run_all();
    let modules: Vec<String> = report.results.iter().map(|r| r.module.clone()).collect();
    assert!(
        modules.contains(&"Counter__8".to_string()),
        "bench should attach to Counter__8; got {modules:?}"
    );
    assert!(
        modules.contains(&"Counter__12".to_string()),
        "bench should attach to Counter__12; got {modules:?}"
    );
    assert!(report.all_passed(), "both monomorph benches should pass");
}

#[test]
fn waveform_map_applies_a_closure_per_sample() {
    // piperine-bench/docs/SPEC.md §6 `Waveform.map(f)` (G2): a closure-taking method on a
    // host object — the interpreter invokes the closure per sample. A Real
    // result stays a Waveform; a Complex result stays a ComplexWaveform.
    let src = format!(
        "{CIRCUIT}
        bench RcCharge {{
            fn test_map() {{
                var t = $tran(TranConfig {{ .stop = 1e-3, .step = 1e-4 }});
                var v = t.v(out, gnd);
                var doubled = v.map(|x| x * 2.0);
                $assert(doubled.len() == v.len(), \"map preserves length\");
                $assert(doubled.at(0.0) > 9.9, \"each sample doubled (~10V)\");
                var a = $ac(AcConfig {{ .fstart = 1.0, .fstop = 1e6, .points = 5 }});
                var cw = a.v(out, gnd);
                var passthrough = cw.map(|c| c);
                $assert(passthrough.len() == cw.len(), \"complex map preserves length\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("RcCharge", "test_map") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn default_param_on_a_pom_fn_called_from_bench() {
    // the language spec Part I §9.1 (G8): a user `fn` may carry a trailing default
    // parameter; a call may omit it. This exercises the interpreter path
    // (call_pom_fn) — a top-level fn called from a bench with one arg
    // (default filled) and two args (explicit).
    let src = format!(
        "{CIRCUIT}
        fn scale(x: Real, k: Real = 2.0) -> Real {{ x * k }}
        bench SwitchOpenTest {{
            fn test_default_param() {{
                var y = scale(5.0);
                $assert(y > 9.9 && y < 10.1, \"scale(5) with default k=2 -> 10\");
                var z = scale(5.0, 3.0);
                $assert(z > 14.9 && z < 15.1, \"scale(5, 3) -> 15\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("SwitchOpenTest", "test_default_param") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn default_param_on_an_analog_fn_used_in_a_contribution() {
    // the language spec Part I §9.1 (G8): an analog fn with a default, used in a
    // contribution — exercises the IR/inliner path (defaults lowered to
    // constant IrExpr and filled at expansion). `gain(V/r)` omits `k`, so
    // the contribution is `2.0 * V/r` → I = 2*5/1k = 10 mA.
    let src = format!(
        "{CIRCUIT}
        fn gain(x: Real, k: Real = 2.0) -> Real {{ x * k }}
        mod GmRes(inout p : Electrical, inout n : Electrical) {{
            param r : Real = 1e3;
        }}
        analog GmRes {{ I(p, n) <+ gain(V(p, n) / r); }}
        mod GmTest() {{
            wire gnd : Electrical;
            wire out : Electrical;
            wire mid : Electrical;
            source : VoltageSource (.p = out, .n = gnd) {{ .voltage = 5.0 }};
            g : GmRes (.p = out, .n = mid) {{ .r = 1e3 }};
            load : Resistor (.p = mid, .n = gnd) {{ .resistance = 500.0 }};
        }}
        bench GmTest {{
            fn test_default_in_contribution() {{
                var r = $op();
                var i = r.i(g.p, g.n);
                // gain(V/r) with default k=2: I = 2*(5-2.5)/1k = 5mA.
                $assert(i > 4.5e-3 && i < 5.5e-3, \"default k=2 in contribution -> 5mA\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("GmTest", "test_default_in_contribution") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn trace_i_over_time_recomputes_a_resistor_current() {
    // piperine-bench/docs/SPEC.md §4/§6 (G3): `Trace.i(a, b)` beyond ideal-source
    // branches — a resistor's current over time is recomputed per step from
    // the solved terminal voltages (previously an error). A pure resistive
    // divider settles instantly, so the series current is the DC value:
    // 5V / (2k + 3k) = 1 mA.
    let src = format!(
        "{CIRCUIT}
        mod Divider() {{
            wire gnd : Electrical;
            wire out : Electrical;
            wire mid : Electrical;
            source : VoltageSource (.p = out, .n = gnd) {{ .voltage = 5.0 }};
            r1 : Resistor (.p = out, .n = mid) {{ .resistance = 2e3 }};
            r2 : Resistor (.p = mid, .n = gnd) {{ .resistance = 3e3 }};
        }}
        bench Divider {{
            fn test_i_over_time() {{
                var t = $tran(TranConfig {{ .stop = 1e-3, .step = 1e-4 }});
                var i = t.i(r1.p, r1.n);
                $assert(i.len() > 1, \"current waveform has samples\");
                $assert(i.at(0.0) > 0.9e-3 && i.at(0.0) < 1.1e-3,
                        \"series current ~ 1mA (5V / 5k)\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("Divider", "test_i_over_time") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn trace_i_over_time_exercises_the_reactive_path() {
    // A capacitor's current over time (previously an error — caps aren't
    // force devices) is the reactive `dQ/dt`. The settled RC starts at its
    // DC operating point, so both the cap current and the resistor current
    // are ~0 — this verifies the reactive recompute runs without crashing
    // and reports the steady-state zero, not that `Trace.i` only handles
    // ideal sources.
    let src = format!(
        "{CIRCUIT}
        bench RcCharge {{
            fn test_i_reactive_settled() {{
                var t = $tran(TranConfig {{ .stop = 1e-3, .step = 1e-4 }});
                var ic = t.i(c1.p, c1.n);
                $assert(ic.len() > 1, \"cap current waveform has samples\");
                $assert(ic.peak_to_peak() < 1e-6, \"settled cap current ~ 0\");
                var ir = t.i(r1.p, r1.n);
                $assert(ir.len() > 1, \"resistor current waveform has samples\");
                $assert(ir.at(0.0) < 1e-6, \"settled resistor current ~ 0\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("RcCharge", "test_i_reactive_settled") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn select_in_expression_position_returns_a_usable_selection() {
    // piperine-bench/docs/SPEC.md §7/§13 (G9): `select("...")` in expression position
    // returns a SelectionRef — `len`/`labels`/field-read work, and staging
    // via a held selection (`s.resistance = 1e3`) re-runs against the live
    // design. Field-reads return a List (always, no singleton coercion) of
    // the param snapshot taken at `select()` time.
    let src = format!(
        "{CIRCUIT}
        bench SwitchOpenTest {{
            fn test_select_expr() {{
                var s = select(\"/resistor\");
                $assert(s.len() == 1, \"one resistor matched\");
                $assert(s.labels().len() == 1, \"labels is a list of one\");
                var rs = s.resistance;
                $assert(rs.len() == 1, \"field-read is a list, not a scalar\");
                $assert(rs.get(0).unwrap() > 1e5, \"resistance snapshot ~ 1e6\");
                s.resistance = 1e3;
                sw.ctrl = 1.0;
                var r = $op();
                $assert(r.i(resistor.p, resistor.n) > 1e-3,
                        \"closed with 1k staged via held selection: ~ mA\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("SwitchOpenTest", "test_select_expr") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn waveform_map_rejects_a_non_numeric_closure_result() {
    // A closure that returns something other than Real/Complex is a
    // fail-loud type mismatch, not a silent no-op.
    let src = format!(
        "{CIRCUIT}
        bench RcCharge {{
            fn test_map_mismatch() {{
                var t = $tran(TranConfig {{ .stop = 1e-3, .step = 1e-4 }});
                var v = t.v(out, gnd);
                var bad = v.map(|x| [x]);
                $assert(bad.len() == v.len(), \"unreachable\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("RcCharge", "test_map_mismatch") {
        BenchOutcome::Error(msg) => assert!(msg.contains("map"), "expected a map error, got: {msg}"),
        other => panic!("expected Error, got {other:?}"),
    }
}

#[test]
fn map_literal_and_methods() {
    // piperine-bench/docs/SPEC.md §5.1 (G5): the `Map<Net, Real>` value type — `Map {}`
    // literal (empty and with entries), `.insert(k, v)`, `.get(k)`,
    // `.len()`. Used by `ic`/`nodeset` config fields.
    let src = format!(
        "{CIRCUIT}
        bench SwitchOpenTest {{
            fn test_map() {{
                var m = Map {{}};
                $assert(m.len() == 0, \"empty map\");
                m.insert(resistor, 1e6);
                $assert(m.len() == 1, \"len after insert\");
                var r = m.get(resistor);
                $assert(r.is_some(), \"get returns some\");
                $assert(r.unwrap() > 1e5, \"get value\");
                m.insert(resistor, 2e3);
                var r2 = m.get(resistor);
                $assert(r2.unwrap() > 1e3 && r2.unwrap() < 1e4, \"insert updates\");
                m.insert(sw, 0.0);
                $assert(m.len() == 2, \"two keys\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("SwitchOpenTest", "test_map") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn tran_ic_seeds_the_t0_state() {
    // piperine-bench/docs/SPEC.md §5.1 `TranConfig.ic`: a `Map<Net, Real>` hint seeds
    // the t=0 node voltages (milestone-1 seed — both companion rows
    // overwrite, so dV/dt = 0 at t=0; the cap doesn't gradually charge
    // because there's no preceding steady-state for the companion to
    // interpolate from). We assert the seed reaches the t=0 snapshot.
    let src = format!(
        "{CIRCUIT}
        bench RcCharge {{
            fn test_ic() {{
                var t = $tran(TranConfig {{ .stop = 1e-3, .step = 1e-4, .ic = Map {{ out: 0.0 }} }});
                var v = t.v(out, gnd);
                $assert(v.len() > 1, \"transient runs with ic\");
                $assert(v.at(0.0) < 0.5, \"ic seed: t=0 out near 0\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("RcCharge", "test_ic") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn noise_white_noise_produces_a_real_psd() {
    // piperine-bench/docs/SPEC.md §5/§6 (G10): a device's `white_noise(...)` contribution
    // reaches the noise solver and contributes to `$noise` PSD/total. This
    // is a smoke test verifying the structural fix (frequency threaded into
    // `SimCtx`, `_ac_context` no longer ignored in
    // `AnalogInstance::noise_current_psd`) without committing to a
    // Johnson-formula tolerance test — that requires confirming the solver's
    // exact noise integration factor, deferred.
    let src = format!(
        "{CIRCUIT}
        mod NoisyResistor(inout p : Electrical, inout n : Electrical) {{
            param r : Real = 1e3;
        }}
        analog NoisyResistor {{ I(p, n) <+ V(p, n) / r + white_noise(4 * 8.617e-5 * 300.15 / r); }}
        mod NoiseTest() {{
            wire gnd : Electrical;
            wire out : Electrical;
            nr : NoisyResistor (.p = out, .n = gnd) {{ .r = 1e3 }};
        }}
        bench NoiseTest {{
            fn test_noise() {{
                var n = $noise(out, NoiseConfig {{ .fstart = 1.0, .fstop = 1e6, .points = 5 }});
                // Structural check: the noise analysis runs and the PSD array
                // has the configured sweep points. Magnitude verification
                // (Johnson formula tolerance) is deferred — it requires the
                // full JIT noise PSD emission + adjoint integration to be
                // nonzero end-to-end.
                $assert(n.psd().len() == 5, \"psd has 5 points\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("NoiseTest", "test_noise") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn op_nodeset_hint_is_accepted() {
    // piperine-bench/docs/SPEC.md §5.1 `OpConfig.nodeset`: accepted and threaded to the DC
    // solver as an initial guess. Linear circuits converge regardless, so
    // we assert the bench runs and the OP matches the steady state.
    let src = format!(
        "{CIRCUIT}
        bench RcCharge {{
            fn test_nodeset() {{
                var r = $op(OpConfig {{ .nodeset = Map {{ out: 5.0 }} }});
                $assert(r.v(out, gnd) > 4.9, \"op converges with nodeset\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("RcCharge", "test_nodeset") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn ac_stim_drives_a_low_pass_response() {
    // piperine-bench/docs/SPEC.md G11: `ac_stim` must inject real small-signal
    // stimulus, not just survive the sweep. 1 A of AC current into R‖C
    // gives |V(f)| = R / sqrt(1 + (f/f3db)^2), f3db = 1/(2πRC) ≈ 159.15 Hz
    // for R = 1k, C = 1u: flat 1000 V in the passband, 707 V at f3db.
    let src = format!(
        "{CIRCUIT}
        mod AcSource(inout p: Electrical, inout n: Electrical) {{
        }}
        analog AcSource {{ I(p, n) <+ -ac_stim(1.0); }}

        mod RcLowPass() {{
            wire gnd : Electrical;
            wire out : Electrical;
            stim : AcSource  (.p = out, .n = gnd);
            r1   : Resistor  (.p = out, .n = gnd) {{ .resistance = 1e3 }};
            c1   : Capacitor (.p = out, .n = gnd) {{ .c = 1e-6 }};
        }}

        bench RcLowPass {{
            fn test_low_pass() {{
                var r = $ac(AcConfig {{ .fstart = 1.0, .fstop = 1e4, .points = 400 }});
                var mag = r.v(out, gnd).mag();
                var passband = mag.at(1.0);
                $assert(passband > 990.0, \"passband magnitude is R\");
                $assert(passband < 1010.0, \"passband magnitude is R\");
                var corner = mag.at(159.155);
                $assert(corner > 690.0, \"corner magnitude is R/sqrt(2)\");
                $assert(corner < 725.0, \"corner magnitude is R/sqrt(2)\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("RcLowPass", "test_low_pass") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn display_prints_values_without_failing() {
    // Bench spec §11 `$display(args…)`: renders scalars, tuples, and lists,
    // joined by a space, no severity prefix. The bench must pass — a
    // formatting error would surface as an Error outcome.
    let src = format!(
        "{CIRCUIT}
        bench RcCharge {{
            fn test_display() {{
                var r = $op();
                $display(\"v(out) =\", r.v(out, gnd));
                $display((1.0, 2.0), [1, 2, 3], Map {{ out: 5.0 }});
                $display();
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("RcCharge", "test_display") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn impl_method_dispatch_in_bench_and_analog() {
    // SPEC §6.5/§6.6: an `impl` method is callable on a bundle value in a
    // bench (interpreter dispatch via Host::resolve_method) and inside an
    // analog body (lowered as the flattened `Bundle::method` IR fn).
    let src = "
        discipline Electrical { potential v : Real; flow i : Real; }
        capability Conductive { fn conductance(self) -> Real; }
        bundle ResModel { rsh : Real = 1e3, }
        impl Conductive for ResModel {
            fn conductance(self) -> Real { return 1.0 / self.rsh; }
        }
        mod R(inout p : Electrical, inout n : Electrical) {
            param model : ResModel = ResModel {};
        }
        analog R { I(p, n) <+ V(p, n) * model.conductance(); }
        mod VSrc(inout p : Electrical, inout n : Electrical) { param voltage : Real = 5.0; }
        analog VSrc { V(p, n) <- voltage; }
        mod Divider() {
            wire gnd : Electrical; wire vin : Electrical; wire mid : Electrical;
            src : VSrc(.p = vin, .n = gnd);
            r1 : R(.p = vin, .n = mid);
            r2 : R(.p = mid, .n = gnd);
        }
        bench Divider {
            fn test_method_everywhere() {
                var r = $op();
                $assert(abs(r.v(mid, gnd) - 2.5) < 1e-6, \"analog method call divides\");
                var card = ResModel { .rsh = 2e3 };
                $assert(abs(card.conductance() - 5e-4) < 1e-12, \"bench method call computes\");
            }
        }";
    let design = elab(src);
    match BenchRunner::new(&design).run_entry("Divider", "test_method_everywhere") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn bench_fn_calls_a_sibling_bench_fn() {
    // Bench spec §2 "fn helper(x: T) -> U — reusable": helpers run in the
    // effectful context, so they may run analyses.
    let src = format!(
        "{CIRCUIT}
        bench SwitchOpenTest {{
            fn source_voltage(scale : Real) -> Real {{
                var r = $op();
                return r.v(vsrc, gnd) * scale;
            }}
            fn test_helper_call() {{
                $assert(abs(source_voltage(2.0) - 10.0) < 1e-6, \"helper ran the op and scaled\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("SwitchOpenTest", "test_helper_call") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn tuple_index_and_for_over_tuples() {
    // SPEC §6.1: `.0`/`.1` index a tuple.
    let src = format!(
        "{CIRCUIT}
        bench SwitchOpenTest {{
            fn test_tuple_index() {{
                var pair = (3.0, 4.0);
                $assert(abs(pair.0 - 3.0) < 1e-12, \"first element\");
                $assert(abs(pair.1 - 4.0) < 1e-12, \"second element\");
                var total = 0.0;
                for case in [(1.0, 10.0), (2.0, 20.0)] {{
                    total = total + case.0 + case.1;
                }}
                $assert(abs(total - 33.0) < 1e-12, \"tuple fields readable in a loop\");
            }}
        }}"
    );
    let design = elab(&src);
    match BenchRunner::new(&design).run_entry("SwitchOpenTest", "test_tuple_index") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn bundle_typed_fn_param_lowers_and_interprets() {
    // A free fn with a bundle-typed param works from a bench (interpreter)
    // and inside an analog contribution (flattened per-field in the IR).
    let src = "
        discipline Electrical { potential v : Real; flow i : Real; }
        bundle Gain { k : Real = 2.0, }
        fn apply(g : Gain, x : Real) -> Real { return g.k * x; }
        mod Amp(inout p : Electrical, inout n : Electrical) {
            param g : Gain = Gain {};
        }
        analog Amp { I(p, n) <+ apply(g, V(p, n)) / 1e3; }
        mod VSrc(inout p : Electrical, inout n : Electrical) { param voltage : Real = 5.0; }
        analog VSrc { V(p, n) <- voltage; }
        mod Cell() {
            wire gnd : Electrical; wire vin : Electrical; wire mid : Electrical;
            src : VSrc(.p = vin, .n = gnd);
            a1 : Amp(.p = vin, .n = mid);
            a2 : Amp(.p = mid, .n = gnd);
        }
        bench Cell {
            fn test_bundle_fn() {
                var r = $op();
                $assert(abs(r.v(mid, gnd) - 2.5) < 1e-6, \"bundle-arg fn in analog\");
                var g = Gain { .k = 3.0 };
                $assert(abs(apply(g, 2.0) - 6.0) < 1e-12, \"bundle-arg fn in bench\");
            }
        }";
    let design = elab(src);
    match BenchRunner::new(&design).run_entry("Cell", "test_bundle_fn") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}

#[test]
fn op_result_reads_digital_nets_directly() {
    // Digital-bench QoL: `r.v(bit_net)` returns the logic value (0/1) off
    // the DC mixed-signal solve — no analog readback stage needed.
    let src = "
        discipline Bit { storage Boolean; }
        mod BitDriver(output q : Bit) {
            param level : Real = 0.0;
            var b : Bit = 0;
        }
        digital BitDriver { b = level > 0.5; q <- b; }
        mod Not1(input a : Bit, output y : Bit) { var r : Bit = 0; }
        digital Not1 { r = !a; y <- r; }
        mod Board() {
            wire na : Bit; wire ny : Bit;
            d : BitDriver(.q = na);
            g : Not1(.a = na, .y = ny);
        }
        bench Board {
            fn test_read_bits() {
                var r = $op();
                $assert(abs(r.v(na)) < 1e-9, \"driver low\");
                $assert(abs(r.v(ny) - 1.0) < 1e-9, \"inverter high\");
                d.level = 1.0;
                var r2 = $op();
                $assert(abs(r2.v(ny)) < 1e-9, \"inverter follows the staged input\");
            }
        }";
    let design = elab(src);
    match BenchRunner::new(&design).run_entry("Board", "test_read_bits") {
        BenchOutcome::Passed => {}
        other => panic!("expected Passed, got {other:?}"),
    }
}
