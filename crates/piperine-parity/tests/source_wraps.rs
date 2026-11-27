//! A value `analog` reads that passes within one instant, run from MSL source on the Rust backend
//! (§15.2 rule 4: a zero-width solve at the instant reads it): the stdlib `idtmod` (Part VII
//! §22.2.9) wraps its integral through a `seat` pulse of one microstep (`tests/wraps/idtmod_ramp.msl`,
//! the sawtooth computed here in Rust across several wraps), and a zero-delay glitch into an `idt`
//! and an RC gains no area at any step (`tests/wraps/glitch.msl`), and a change a crossing of the
//! zero-width solve makes is read once more, at its own value (`tests/wraps/rounds.msl`).
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_parity::project::Project;
use piperine_parity::waveform::Waveform;
use piperine_solver::prelude::NodeIdentifier;
use piperine_solver::protocol::ZeroWidthCause;

#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use support::build;

/// One sawtooth of `wraps/idtmod_ramp.msl`: the closed form of an `idtmod` of a constant.
struct Sawtooth {
    /// The node that follows it.
    node: &'static str,
    /// The input of `idtmod`, in units per second.
    rate: f64,
    /// Its `ic`.
    initial: f64,
}

impl Sawtooth {
    /// The modulus and the offset of both: the output lies in `[OFFSET, OFFSET + MODULUS)`.
    const MODULUS: f64 = 1.0;
    const OFFSET: f64 = -0.5;
    /// The two sawtooths of the bench: over the top, and under the bottom from an `ic` outside the range.
    const BENCH: [Sawtooth; 2] =
        [Sawtooth { node: "up", rate: 0.3e6, initial: 0.0 }, Sawtooth { node: "down", rate: -0.4e6, initial: 2.3 }];

    /// §22.2.9: `ic` plus the integral, wrapped into `[offset, offset + modulus)`.
    fn wrapped(&self, at: f64) -> f64 {
        Self::OFFSET + (self.initial + self.rate * at - Self::OFFSET).rem_euclid(Self::MODULUS)
    }

    /// The period of the sawtooth.
    fn period(&self) -> f64 {
        Self::MODULUS / self.rate.abs()
    }

    /// The time of its first wrap: where the integral leaves the range from `start` (rule 2).
    fn first_wrap(&self) -> f64 {
        let start = self.wrapped(0.0);
        let edge = if self.rate > 0.0 { Self::OFFSET + Self::MODULUS } else { Self::OFFSET };
        (edge - start) / self.rate
    }

    /// The times to read it at: before its first wrap, just after each of the first three wraps
    /// and at a quarter, a half and three quarters of the period after each.
    fn readings(&self) -> Vec<f64> {
        let mut times = vec![0.25 * self.first_wrap(), 0.75 * self.first_wrap()];
        for k in 0..3 {
            let wrap = self.first_wrap() + f64::from(k) * self.period();
            for fraction in [0.003, 0.25, 0.5, 0.75] {
                times.push(wrap + fraction * self.period());
            }
        }
        times
    }
}

/// The bench of `wraps/idtmod_ramp.msl`.
struct IdtmodRamp;

impl IdtmodRamp {
    /// The node of each sawtooth over a transient to `stop` with the step cap `step`, as a waveform
    /// and as its value at every accepted step.
    fn outputs(stop: f64, step: f64) -> Vec<(Waveform, Vec<f64>)> {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/wraps/idtmod_ramp.msl");
        let built = build(&Project::new(&path), "IdtmodRamp");
        let (_, result) = built.run_with(stop, step);
        let accepted: Vec<_> = result.iter().collect();
        let times: Vec<f64> = accepted.iter().map(|point| point.time()).collect();
        Sawtooth::BENCH
            .iter()
            .map(|sawtooth| {
                let path = format!("IdtmodRamp.{}", sawtooth.node);
                let node = NodeIdentifier::Anonymous(built.ir.node_at(&path).expect("the node of a sawtooth").id().index());
                let values: Vec<f64> = accepted.iter().map(|point| point.get_node(&node).expect("its voltage")).collect();
                (Waveform::new(times.clone(), values.clone()), values)
            })
            .collect()
    }
}

#[test]
fn idtmod_wraps_its_integral_into_the_range_at_every_wrap() {
    // §22.2.9: each output is its sawtooth, wrap after wrap, over the top and under the bottom: the
    // seated integral (rule 3) continues from `base`, so the readings after the third wrap are as
    // exact as before the first (the input is a constant, which the integrator follows exactly;
    // a wrap lands on a tick, 1 fs from the crossing, which moves the seat by under 1e-9)
    let stop = 13e-6;
    for (sawtooth, (waveform, values)) in Sawtooth::BENCH.iter().zip(IdtmodRamp::outputs(stop, 0.05e-6)) {
        for time in sawtooth.readings() {
            let actual = waveform.at(time).unwrap_or_else(|| panic!("no point at {time} s"));
            let wanted = sawtooth.wrapped(time);
            assert!((actual - wanted).abs() < 1e-8, "idtmod {} at {time:e} s: {actual}, the sawtooth is {wanted}", sawtooth.node);
        }
        // rule 4: every solution lies in the range, the wrap included (to the seat's 1 fs)
        let (low, high) = values.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
        let (bottom, top) = (Sawtooth::OFFSET, Sawtooth::OFFSET + Sawtooth::MODULUS);
        assert!(low >= bottom - 1e-8 && high <= top + 1e-8, "idtmod {} stays in [{bottom}, {top}]: [{low}, {high}]", sawtooth.node);
    }
}

/// The bench of `wraps/glitch.msl`.
struct GlitchBench;

impl GlitchBench {
    /// The nodes the glitch drives through an integrand: the `idt` and the RC's capacitor.
    const NODES: [&'static str; 2] = ["integral", "charged"];

    /// Each node's value at every accepted step after the glitch, over a run to 3 us with the step
    /// cap `step`, and the zero-width solves the run made for a passed value.
    fn after_glitch(step: f64) -> (Vec<Vec<f64>>, u64) {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/wraps/glitch.msl");
        let built = build(&Project::new(&path), "Glitch");
        let (_, result) = built.run_with(3e-6, step);
        let solves = result.statistics().zero_width_for(ZeroWidthCause::PassedValue);
        let accepted: Vec<_> = result.iter().filter(|point| point.time() >= 1e-6).collect();
        let values = Self::NODES
            .iter()
            .map(|name| {
                let path = format!("Glitch.{name}");
                let node = NodeIdentifier::Anonymous(built.ir.node_at(&path).expect("a node of the bench").id().index());
                accepted.iter().map(|point| point.get_node(&node).expect("its voltage")).collect()
            })
            .collect();
        (values, solves)
    }
}

#[test]
fn a_zero_delay_glitch_gains_no_area_at_any_step() {
    // §15.2 rule 4: the glitch is read by one zero-width solve at 1 us, where no time passes. The
    // integral of 1 MV/s over no time and the charge 1 V puts on 1 nF through 1 kOhm over no time
    // are both 0, at a step cap of 50 ns and of 200 ns alike: a retake of the step to 1 us would
    // have integrated the high over the whole step (0.05 and 0.2 here)
    for step in [0.05e-6, 0.2e-6] {
        let (values, solves) = GlitchBench::after_glitch(step);
        assert_eq!(solves, 1, "one zero-width solve, at the glitch (step {step:e})");
        for (name, node) in GlitchBench::NODES.iter().zip(values) {
            assert!(!node.is_empty(), "{name} has steps after the glitch");
            let largest = node.iter().fold(0.0_f64, |most, v| most.max(v.abs()));
            assert!(largest < 1e-12, "{name} gains no area from the glitch at step {step:e}: {largest:e}");
        }
    }
}

/// The bench of `wraps/rounds.msl`.
struct RoundsBench;

impl RoundsBench {
    /// The time of wrap `k` (from 1) of its `idtmod`.
    fn wrap(k: u32) -> f64 {
        (f64::from(k) - 0.5) / 0.3e6
    }
}

#[test]
fn a_change_made_after_a_zero_width_solve_is_read_once_at_its_value() {
    // §15.2 rule 4 over two rounds, on the real sides: at each wrap round 1 counts `level` up and
    // pulses idtmod's `seat` (a zero-width solve for the value passed through); that solve seats the
    // phase, which makes the bench's second crossing, and round 2 counts `level` up once more (a
    // zero-width solve for the crossing's change). The solution at each wrap holds the count of both
    // rounds, every reading is counted under its own cause, and no value round 1 read counts again
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/wraps/rounds.msl");
    let built = build(&Project::new(&path), "Rounds");
    let (_, result) = built.run_with(9e-6, 0.05e-6);
    let statistics = result.statistics();
    let node = |name: &str| NodeIdentifier::Anonymous(built.ir.node_at(&format!("Rounds.{name}")).expect("a node").id().index());
    let (phase, count) = (node("phase"), node("count"));
    let points: Vec<(f64, f64, f64)> = result
        .iter()
        .map(|point| (point.time(), point.get_node(&phase).expect("v(phase)"), point.get_node(&count).expect("v(count)")))
        .collect();
    for k in 1..=3u32 {
        let wrap = RoundsBench::wrap(k);
        let at = points.iter().filter(|(time, ..)| (time - wrap).abs() < 1e-12).collect::<Vec<_>>();
        assert_eq!(at.len(), 1, "one solution at wrap {k} ({wrap:e} s): {at:?}");
        let (_, seated, counted) = *at[0];
        assert!((seated - (-0.5)).abs() < 1e-8, "wrap {k} seats the phase at the bottom: {seated}");
        assert_eq!(counted, 2.0 * f64::from(k), "the solution at wrap {k} reads both rounds' counts");
    }
    assert_eq!(statistics.zero_width_for(ZeroWidthCause::PassedValue), 3, "one per wrap, for the seat");
    assert_eq!(statistics.zero_width_for(ZeroWidthCause::CrossingChange), 3, "one per wrap, for round 2's count");
}

/// A bench of `wraps/` with cells on elements of an array net, run to 10 ns with a step cap of 0.5 ns.
struct ElementBench;

impl ElementBench {
    /// The time and each named node's value at every accepted step (the zero-width solve's included), and the
    /// zero-width solves the run made for a passed value.
    fn run(file: &str, top: &str, nodes: &[&str]) -> (Vec<(f64, Vec<f64>)>, u64) {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/wraps").join(file);
        let built = build(&Project::new(&path), top);
        let (_, result) = built.run_with(10e-9, 0.5e-9);
        let solves = result.statistics().zero_width_for(ZeroWidthCause::PassedValue);
        let ids: Vec<NodeIdentifier> = nodes
            .iter()
            .map(|name| NodeIdentifier::Anonymous(built.ir.node_at(&format!("{top}.{name}")).unwrap_or_else(|| panic!("no node `{name}`")).id().index()))
            .collect();
        let points = result.iter().map(|point| (point.time(), ids.iter().map(|id| point.get_node(id).expect("a node voltage")).collect())).collect();
        (points, solves)
    }

    /// `v(p)` of a `Cell` whose gate is `gate`: 1 mA through 1 kOhm or 2 kOhm.
    fn cell(gate: bool) -> f64 {
        1e-3 * if gate { 1e3 } else { 2e3 }
    }

    /// Asserts that every point holds `expected(time)`, node by node, and that exactly one point is at 2 ns.
    fn assert_points(points: &[(f64, Vec<f64>)], expected: impl Fn(f64) -> Vec<f64>) {
        assert_eq!(points.iter().filter(|(time, _)| (time - 2e-9).abs() < 1e-15).count(), 1, "one solution at 2 ns, the zero-width solve's");
        for (time, values) in points {
            let wanted = expected(*time);
            assert!(values.iter().zip(&wanted).all(|(v, w)| (v - w).abs() < 1e-9), "at {time:e} s: {values:?}, wanted {wanted:?}");
        }
    }
}

#[test]
fn a_pulse_of_an_element_the_analog_side_reads_is_read_by_a_zero_width_solve() {
    // §15.2 rule 4 on an element of an array net (part II §5.1.8): `gates[1]` is high for one microstep at 2 ns. The
    // run passes the instant, and the one zero-width solve, the solution at 2 ns, reads the high element: `v(a)` is the
    // high-gate value there and the low-gate value at every other step
    let (points, solves) = ElementBench::run("element_pulse.msl", "ElementPulse", &["a"]);
    assert_eq!(solves, 1, "one zero-width solve, for the element's passed high");
    ElementBench::assert_points(&points, |time| vec![ElementBench::cell((time - 2e-9).abs() < 1e-15)]);
}

#[test]
fn an_element_that_changes_beside_a_scalar_pulse_is_read_at_its_value() {
    // §15.2 rule 4: at 2 ns `g` pulses and `gates[1]` rises to stay. The zero-width solve at 2 ns reads both high; after
    // it `v(a)` is back at the low-gate value and `v(b)` stays at the high-gate value; before it both are low-gate
    let (points, solves) = ElementBench::run("element_beside.msl", "ElementBeside", &["a", "b"]);
    assert_eq!(solves, 1, "one zero-width solve, for `g`'s passed high");
    ElementBench::assert_points(&points, |time| {
        let (at, after) = ((time - 2e-9).abs() < 1e-15, time > 2e-9 + 1e-15);
        vec![ElementBench::cell(at), ElementBench::cell(at || after)]
    });
}
