//! Scope: an on-demand throughput benchmark of digital-only designs on the compiled backend
//! (`digital-performance` diagnosis), not a correctness test: no analog unknown anywhere. Each case
//! is MSL generated here, compiled once (not timed), then one transient is timed; the table gives
//! simulated clock cycles per second, the wall time per cycle, and that time shared over the device
//! calls of a cycle (reactions and discrete functions, counted from the design: see `Case`). `peak
//! MiB` is the process's peak resident memory so far: run one case (`DIGITAL_BENCH`) to read its own.
//! The last value of a probe is checked against the cycle count, so a case that silently stopped
//! early fails.
//!
//! Run it in release, by name (`test = false`, registered in `suite_hygiene.rs`):
//!
//! ```sh
//! cargo test --release -p piperine-api --test digital_bench -- --nocapture
//! ```
//!
//! `DIGITAL_BENCH=<case>` runs only the cases whose name starts with it; `DIGITAL_BENCH_CYCLES=<n>`
//! overrides every case's cycle count (for a long run under a profiler).
#![cfg(unix)]

use std::cell::RefCell;
use std::fmt::Write as _;
use std::rc::Rc;
use std::time::Instant;

use piperine_api::{Cell, Overrides, Project, Record, RecordSink, Session};
use piperine_transpile::Settings;

/// A sink that drops every record: the benchmark times the engine, not the host.
struct Dropped;

impl RecordSink for Dropped {
    fn record(&mut self, _: &Record) {}
}

/// The clock every case runs on: 100 MHz, a 10 ns period, both edges an instant.
const CLOCK: &str = "module Osc(out clk: Bit = false) {\n    f: f64 = 100M where f > 0.0;\n}\n\
digital for Osc {\n    on clk.changed() {\n        clk <- !clk after 0.5 / f;\n    }\n}\n\n";

/// A 32-bit counter of rising edges: the data source of the pipeline and the chain.
const SOURCE: &str = "module Src(in clk: Bit, out q: Bit<u32> = 0) {}\n\
digital for Src {\n    on clk.rise() {\n        q <- q + 1;\n    }\n}\n\n";

/// The combinational step of the pipeline and of the chain: a definition, so a discrete function.
const MIX: &str = "module Mix(in a: Bit<u32>, out y: Bit<u32>) {\n    y = (a ^ (a >> 3)) + 7;\n}\n\n";

/// One case: a design, its run length, and what one clock cycle costs in device calls. Every `on <event>`
/// head is a discrete function of its instance (it computes the event's bit), evaluated at both clock
/// edges, so a module clocked by `clk.rise()` costs two function calls and one reaction call per cycle.
struct Case {
    name: String,
    source: String,
    top: &'static str,
    /// Clock cycles simulated.
    cycles: u64,
    /// Reaction calls per cycle (rising and falling edge together; the oscillator's two included).
    reactions_per_cycle: u64,
    /// Discrete-function evaluations per cycle.
    functions_per_cycle: u64,
    /// The probe whose last value is checked, and the value it must hold after `cycles` cycles.
    probe: &'static str,
    /// `None` when the value is only printed (it must still be an integer).
    expect: Option<fn(u64) -> i128>,
}

impl Case {
    /// (1) A 64-bit counter at one clock.
    fn counter() -> Case {
        let source = format!(
            "{CLOCK}module Ctr(in clk: Bit, out q: Bit<u64> = 0) {{}}\n\
             digital for Ctr {{\n    on clk.rise() {{\n        q <- q + 1;\n    }}\n}}\n\n\
             module Top() {{\n    signal clk: Bit;\n    pub signal q: Bit<u64>;\n    let osc = Osc(clk);\n    let ctr = Ctr(clk, q);\n}}\n"
        );
        Case { name: "counter64".into(), source, top: "Top", cycles: 200_000, reactions_per_cycle: 3, functions_per_cycle: 4, probe: "q", expect: Some(|c| c as i128) }
    }

    /// (2) A four-state sequencer with an enum state, a tick counter and two decoded outputs
    /// (definitions).
    fn fsm() -> Case {
        let source = format!(
            "{CLOCK}#[derive(Repr, PartialEq)]\npub enum Phase {{\n    Idle,\n    Charge,\n    Hold,\n    Dump,\n}}\n\n\
             module Fsm(in clk: Bit, out charge: Bit, out dump: Bit, out phase: Bit<Phase> = Phase::Idle, out laps: Bit<u32> = 0) {{\n\
             \x20   signal ticks: Bit<u8> = 0;\n    charge = phase == Phase::Charge;\n    dump = phase == Phase::Dump;\n}}\n\
             digital for Fsm {{\n    on clk.rise() {{\n        let was = phase.known_or(Phase::Idle);\n        let t = ticks.known_or(0) + 1;\n\
             \x20       let next = match was {{\n            Phase::Idle => Phase::Charge,\n            Phase::Charge => if t == 20 {{ Phase::Hold }} else {{ Phase::Charge }},\n\
             \x20           Phase::Hold => if t == 10 {{ Phase::Dump }} else {{ Phase::Hold }},\n            Phase::Dump => if t == 20 {{ Phase::Charge }} else {{ Phase::Dump }},\n        }};\n\
             \x20       phase <- next;\n        ticks <- if next != was {{ 0 }} else {{ t }};\n        if was == Phase::Dump && next == Phase::Charge {{\n            laps <- laps + 1;\n        }}\n    }}\n}}\n\n\
             module Top() {{\n    signal clk: Bit;\n    signal charge: Bit;\n    signal dump: Bit;\n    signal phase: Bit<Phase>;\n    pub signal laps: Bit<u32>;\n\
             \x20   let osc = Osc(clk);\n    let fsm = Fsm(clk, charge, dump, phase, laps);\n}}\n"
        );
        // the decodes `charge` and `dump` run only when `phase` moves: 3 changes per 50 cycles, not counted
        Case { name: "fsm".into(), source, top: "Top", cycles: 200_000, reactions_per_cycle: 3, functions_per_cycle: 4, probe: "laps", expect: None }
    }

    /// (3) `n` registered stages with one combinational definition each, fed by a counter.
    fn pipeline(n: usize, cycles: u64) -> Case {
        let mut source = format!(
            "{CLOCK}{SOURCE}module Stage(in clk: Bit, in d: Bit<u32>, out q: Bit<u32> = 0) {{\n    signal x: Bit<u32>;\n    x = (d ^ (d >> 3)) + 7;\n}}\n\
             digital for Stage {{\n    on clk.rise() {{\n        q <- x;\n    }}\n}}\n\nmodule Top() {{\n    signal clk: Bit;\n"
        );
        for i in 0..=n {
            let _ = writeln!(source, "    {}signal s{i}: Bit<u32>;", if i == 0 { "pub " } else { "" });
        }
        source.push_str("    let osc = Osc(clk);\n    let src = Src(clk, s0);\n");
        for i in 1..=n {
            let _ = writeln!(source, "    let st{i} = Stage(clk, s{}, s{i});", i - 1);
        }
        source.push_str("}\n");
        Case {
            name: format!("pipeline{n}"),
            source,
            top: "Top",
            cycles,
            reactions_per_cycle: 3 + n as u64,
            functions_per_cycle: 4 + 3 * n as u64,
            probe: "s0",
            expect: Some(|c| c as i128),
        }
    }

    /// (4) `n` independent toggle flops on one clock.
    fn flops(n: usize, cycles: u64) -> Case {
        let mut source = format!(
            "{CLOCK}module T(in clk: Bit, out q: Bit = false) {{}}\ndigital for T {{\n    on clk.rise() {{\n        q <- !q;\n    }}\n}}\n\n\
             {SOURCE}module Top() {{\n    signal clk: Bit;\n    pub signal n: Bit<u32>;\n    let osc = Osc(clk);\n    let src = Src(clk, n);\n"
        );
        for i in 0..n {
            let _ = writeln!(source, "    signal q{i}: Bit;\n    let t{i} = T(clk, q{i});");
        }
        source.push_str("}\n");
        Case { name: format!("flops{n}"), source, top: "Top", cycles, reactions_per_cycle: 3 + n as u64, functions_per_cycle: 4 + 2 * n as u64, probe: "n", expect: Some(|c| c as i128) }
    }

    /// (5) A combinational chain `d` definitions deep, re-evaluated at every clock by its counter.
    fn chain(d: usize, cycles: u64) -> Case {
        let mut source = format!("{CLOCK}{SOURCE}{MIX}module Top() {{\n    signal clk: Bit;\n");
        for i in 0..=d {
            let _ = writeln!(source, "    {}signal c{i}: Bit<u32>;", if i == 0 { "pub " } else { "" });
        }
        source.push_str("    let osc = Osc(clk);\n    let src = Src(clk, c0);\n");
        for i in 1..=d {
            let _ = writeln!(source, "    let m{i} = Mix(c{}, c{i});", i - 1);
        }
        source.push_str("}\n");
        Case { name: format!("chain{d}"), source, top: "Top", cycles, reactions_per_cycle: 3, functions_per_cycle: 4 + d as u64, probe: "c0", expect: Some(|c| c as i128) }
    }

    /// (6) The buck's DPWM alone, its source verbatim (its own `Osc`, set to 100 MHz like every case), with a
    /// fixed duty word.
    fn dpwm() -> Case {
        let dpwm = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/buck/src/dpwm.msl")).expect("the buck's dpwm.msl");
        let dpwm = dpwm.replace(
            "use crate::spec::{DEAD_COUNTS, N_PERIOD, SAMPLE_COUNT};",
            "const N_PERIOD: u9 = 400;\nconst DEAD_COUNTS: u9 = 4;\nconst SAMPLE_COUNT: u9 = 0;",
        );
        let source = format!(
            "{dpwm}\n{SOURCE}module Top() {{\n    signal clk: Bit;\n    signal run: Bit = true;\n    signal duty: Bit<u12> = 1000;\n    signal oc: Bit = false;\n    signal sink_oc: Bit = false;\n\
             \x20   signal hs: Bit;\n    signal ls: Bit;\n    signal sample: Bit;\n    signal cut: Bit;\n    pub signal n: Bit<u32>;\n\
             \x20   let osc = Osc(clk) {{ f: 100M }};\n    let pwm = Dpwm(clk, run, duty, oc, sink_oc, hs, ls, sample, cut);\n    let src = Src(clk, n);\n}}\n"
        );
        // reactions: Osc twice, Dpwm, its `Never` monitor, the counter; functions: the four edge events at
        // both edges (`sample` and `hs & ls` move a few times per 400 counts, not counted)
        Case { name: "dpwm".into(), source, top: "Top", cycles: 200_000, reactions_per_cycle: 5, functions_per_cycle: 8, probe: "n", expect: Some(|c| c as i128) }
    }

    /// Every case of the diagnosis, in the order its table lists them.
    fn all() -> Vec<Case> {
        vec![
            Case::counter(),
            Case::fsm(),
            Case::pipeline(10, 50_000),
            Case::pipeline(100, 5_000),
            Case::pipeline(1000, 500),
            Case::flops(100, 20_000),
            Case::flops(1000, 1_000),
            Case::flops(10_000, 20),
            Case::chain(10, 50_000),
            Case::chain(100, 5_000),
            Case::chain(1000, 500),
            Case::dpwm(),
        ]
    }
}

/// The process's peak resident memory in MiB (`VmHWM`), to see what a run keeps.
fn peak_rss_mib() -> f64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    status
        .lines()
        .find_map(|l| l.strip_prefix("VmHWM:"))
        .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<f64>().ok())
        .map_or(0.0, |kib| kib / 1024.0)
}

/// What one timed run measured.
struct Measured {
    compile_s: f64,
    run_s: f64,
    probe: i128,
}

fn settings() -> Settings {
    let cache = std::env::temp_dir().join("piperine-digital-bench-cache");
    std::fs::create_dir_all(&cache).expect("the compile cache");
    Settings { rustc: None, cache }
}

/// Compiles `case` (untimed by the run figure), runs `cycles` clock cycles and checks the probe.
fn measure(case: &Case, cycles: u64) -> Measured {
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, &case.source).expect("write");
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{}: {f}", case.name));
    // the stop is a quarter period past the last rising edge's fall, so `cycles` rising edges land
    let stop = cycles as f64 * 10e-9 + 2.5e-9;
    let overrides = Overrides {
        backend: Some("rust".into()),
        analysis: Some("transient".into()),
        top: Some(case.top.into()),
        stop: Some(stop),
        print_step: Some(stop),
        print: vec![case.probe.into()],
        ..Overrides::default()
    };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{}: {f}", case.name));
    let started = Instant::now();
    let mut session = Session::open(&project, &spec, &settings()).unwrap_or_else(|f| panic!("{}: {f}", case.name));
    let compile_s = started.elapsed().as_secs_f64();
    let started = Instant::now();
    let table = session.run(Rc::new(RefCell::new(Dropped))).unwrap_or_else(|f| panic!("{}: {f}", case.name));
    let run_s = started.elapsed().as_secs_f64();
    let last = table.rows.last().expect("a row");
    let got = match &last[1] {
        Cell::Int(v) => *v,
        other => panic!("{}: probe `{}` is not an integer: {other:?}", case.name, case.probe),
    };
    if let Some(expect) = case.expect {
        assert_eq!(got, expect(cycles), "{}: probe `{}` after {cycles} cycles", case.name, case.probe);
    }
    Measured { compile_s, run_s, probe: got }
}

#[test]
fn digital_throughput() {
    let only = std::env::var("DIGITAL_BENCH").ok();
    let cycles = std::env::var("DIGITAL_BENCH_CYCLES").ok().map(|c| c.parse::<u64>().expect("DIGITAL_BENCH_CYCLES is a count"));
    println!(
        "{:<13} {:>8} {:>10} {:>9} {:>10} {:>12} {:>9} {:>9} {:>9} {:>9}",
        "case", "cycles", "compile s", "run s", "cycles/s", "ns/cycle", "react/c", "fn/c", "ns/call", "peak MiB"
    );
    for case in Case::all() {
        if only.as_ref().is_some_and(|o| !case.name.starts_with(o.as_str())) {
            continue;
        }
        let cycles = cycles.unwrap_or(case.cycles);
        let m = measure(&case, cycles);
        // the wall of a cycle shared over every device call of the cycle, reactions and functions alike
        let calls = case.reactions_per_cycle + case.functions_per_cycle;
        println!(
            "{:<13} {:>8} {:>10.2} {:>9.3} {:>10.0} {:>12.0} {:>9} {:>9} {:>9.0} {:>9.0}   ({} = {})",
            case.name,
            cycles,
            m.compile_s,
            m.run_s,
            cycles as f64 / m.run_s,
            m.run_s * 1e9 / cycles as f64,
            case.reactions_per_cycle,
            case.functions_per_cycle,
            m.run_s * 1e9 / (cycles * calls) as f64,
            peak_rss_mib(),
            case.probe,
            m.probe,
        );
    }
}
