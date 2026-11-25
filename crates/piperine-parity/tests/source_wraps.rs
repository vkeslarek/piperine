//! A value `analog` reads that passes within one instant, run from MSL source on the Rust backend
//! (§15.2 rule 4: a zero-width solve at the instant reads it): the stdlib `idtmod` (Part VII
//! §22.2.9) wraps its integral through a `seat` pulse of one microstep (`tests/wraps/idtmod_ramp.msl`,
//! the sawtooth computed here in Rust across several wraps), and a zero-delay glitch into an `idt`
//! and an RC gains no area at any step (`tests/wraps/glitch.msl`).
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_parity::project::Project;
use piperine_parity::waveform::Waveform;
use piperine_solver::prelude::NodeIdentifier;
use piperine_solver::protocol::ZeroWidthCause;

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
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
