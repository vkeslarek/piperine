//! The ramp, the PID and the setpoint register of the buck package run from MSL source (feature
//! `msl-digital`, T49; checks C132 and C137, DIG-14 AC 5 and 10). `Script` of
//! `tests/buck/control.msl` is a stimulus written in MSL (a sample counter and `if` chains) that
//! walks 200 samples of soft-start, a step of the measurement up and down to both rails, a lower
//! target, `run` dropped and `run` raised again; `RefRamp` and `Pid` of `examples/buck` run under
//! it on the 200 MHz oscillator. The expected values are the difference equations of
//! `examples/buck/SPEC.md` §6.2 to §6.4 (the PID) and §7.2 (the ramp) written here in plain
//! `i32` arithmetic, sample by sample, and compared with every value the solver committed, bit
//! for bit. `SetpointReg` (SPEC §7.1) takes the data written and saturates a large code.
#![cfg(unix)]

use piperine_parity::project::Project;

use piperine_ir_fixtures::digital;
use piperine_parity::mixed;
use piperine_ir_fixtures::mixed::TopDesign;

use mixed::Seen;
use piperine_parity::buck_fixture::BuckFixture;

/// The samples: one per rising edge of the oscillator.
const SAMPLES: i64 = 200;
/// One nanosecond in ticks.
const NS: i64 = 1_000_000;

/// Associated test helpers of `buck_control`, grouped on one fixture type.
struct BuckControlFixture;

impl BuckControlFixture {
    fn project() -> Project {
        BuckFixture::with_buck("control.msl")
    }

    /// The inputs of sample `s`: `(run, target, meas)`, as `Script` of `tests/buck/control.msl`
    /// computes them from its counter; the test asserts the two agree before it judges the loop.
    ///
    /// - `s < 8`: `run` low, the loop at rest.
    /// - `8 <= s < 170`: `run` high (soft-start from sample 8); `170 <= s < 180`: `run` dropped;
    ///   `s >= 180`: raised again (a second soft-start).
    /// - `target` is 150 codes, 100 from sample 120 (the reference ramps back down after it).
    /// - `meas` is 0 up to sample 39, a step to 20, then to 1023 (the output at full scale: the
    ///   error clamps at -256 and the output saturates low), then back to 0 (the error jumps up by
    ///   hundreds of codes: the derivative saturates the output high), then 100.
    fn script(s: i64) -> (bool, i64, i64) {
        let run = !(s < 8 || (170..180).contains(&s));
        let target = if s < 120 { 150 } else { 100 };
        let meas = match s {
            0..=39 => 0,
            40..=79 => 20,
            80..=109 => 1023,
            110..=139 => 0,
            _ => 100,
        };
        (run, target, meas)
    }

    /// Sample by sample, the SPEC's equations. Both bodies react to the same rising edge, so each
    /// reads the other's value from before the edge: the PID reads the reference the ramp committed
    /// at the previous sample (SPEC §9: a delay of exactly one sample).
    fn model() -> Vec<State> {
        let mut s = State { reference: 0, duty: 0, integ: 0, deriv: 0, e_prev: 0 };
        let mut states = Vec::new();
        for sample in 0..SAMPLES {
            let (run, target, meas) = BuckControlFixture::script(sample);
            let (target, meas) = (target as i32, meas as i32);
            let old_reference = s.reference;
            // SPEC §7.2: the reference moves one code per sample towards the target, and rests at 0
            // while `run` is low
            s.reference = if !run {
                0
            } else if old_reference < target {
                old_reference + 1
            } else if old_reference > target {
                old_reference - 1
            } else {
                old_reference
            };
            if !run {
                // SPEC §6.4: not running resets the integrator, the derivative state and the output
                s.integ = 0;
                s.deriv = 0;
                s.e_prev = 0;
                s.duty = 0;
            } else {
                let e = (old_reference - meas).clamp(E_MIN, E_MAX);
                let p = KP * e;
                let d = (s.deriv >> D_SHIFT) + KD * (e - s.e_prev);
                let u = (p + s.integ + d) >> Q_SHIFT;
                let windup = (u > DUTY_MAX && e > 0) || (u < 0 && e < 0);
                if !windup {
                    s.integ = (s.integ + KI * e).clamp(0, DUTY_MAX << Q_SHIFT);
                }
                s.deriv = d;
                s.e_prev = e;
                s.duty = u.clamp(0, DUTY_MAX);
            }
            states.push(s.clone());
        }
        states
    }

    fn observed(seen: &Seen) -> Vec<State> {
        let reference = BuckFixture::series(seen, "ControlBench.reference");
        let duty = BuckFixture::series(seen, "ControlBench.duty");
        let integ = BuckFixture::series(seen, "ControlBench.pid.integ");
        let deriv = BuckFixture::series(seen, "ControlBench.pid.deriv");
        let e_prev = BuckFixture::series(seen, "ControlBench.pid.e_prev");
        (0..SAMPLES)
            .map(|s| {
                // sample `s` is the rising edge `s + 1`; its commits apply at that instant
                let at = TopDesign::rising_edge(s + 1);
                State {
                    reference: BuckFixture::decode(BuckFixture::value_at(&reference, at), 2, false),
                    duty: BuckFixture::decode(BuckFixture::value_at(&duty, at), 2, false),
                    integ: BuckFixture::decode(BuckFixture::value_at(&integ, at), 4, true),
                    deriv: BuckFixture::decode(BuckFixture::value_at(&deriv, at), 4, true),
                    e_prev: BuckFixture::decode(BuckFixture::value_at(&e_prev, at), 2, true),
                }
            })
            .collect()
    }
}

/// The state of the two bodies after one sample.
#[derive(Debug, Clone, PartialEq, Eq)]
struct State {
    reference: i32,
    duty: i32,
    integ: i32,
    deriv: i32,
    e_prev: i32,
}

/// SPEC §6.3 gains and limits (Q4 fixed point) and §6.4 clamps.
const KP: i32 = 64;
const KI: i32 = 8;
const KD: i32 = 1024;
const D_SHIFT: u32 = 2;
const Q_SHIFT: u32 = 4;
const E_MIN: i32 = -256;
const E_MAX: i32 = 255;
const DUTY_MAX: i32 = 2880;

#[test]
fn the_ramp_and_the_pid_follow_their_difference_equations() {
    let expected = BuckControlFixture::model();
    // the script reaches what it is for, so a loop that did nothing could not match: both rails,
    // the integrator's hold while saturated, the resets, the ramp both ways
    assert!(expected.iter().any(|s| s.duty == DUTY_MAX), "saturates high");
    assert!(expected.windows(2).any(|w| w[0].duty > 0 && w[1].duty == 0 && w[1].integ > 0), "saturates low with the integrator held");
    assert!(expected.windows(2).any(|w| w[1].duty == DUTY_MAX && w[1].integ == w[0].integ && w[1].integ > 0), "holds the integrator at the top");
    assert!(expected.iter().any(|s| s.integ == 0 && s.deriv == 0 && s.e_prev == 0 && s.reference == 0));
    assert!(expected.windows(2).any(|w| w[1].reference < w[0].reference && w[1].reference > 0), "the ramp goes down");
    assert!(expected.iter().any(|s| s.e_prev == E_MIN) && expected.iter().any(|s| s.e_prev == E_MAX || s.e_prev > 100));

    let seen = BuckFixture::build(&BuckControlFixture::project(), "ControlBench").run(1.0e-6, 1e-8);

    // the stimulus in MSL and the Rust reference compute the same `run`, `target` and `meas` from
    // the sample index: the value each carried when the rising edge of sample `s` read it
    let (run, target, meas) =
        (BuckFixture::series(&seen, "ControlBench.run"), BuckFixture::series(&seen, "ControlBench.target"), BuckFixture::series(&seen, "ControlBench.meas"));
    for s in 0..SAMPLES {
        let at = TopDesign::rising_edge(s + 1);
        let (want_run, want_target, want_meas) = BuckControlFixture::script(s);
        assert_eq!(BuckFixture::decode(BuckFixture::value_at(&run, at - 1), 1, false) != 0, want_run, "run at sample {s}");
        assert_eq!(BuckFixture::decode(BuckFixture::value_at(&target, at - 1), 2, false) as i64, want_target, "target at sample {s}");
        assert_eq!(BuckFixture::decode(BuckFixture::value_at(&meas, at - 1), 2, false) as i64, want_meas, "meas at sample {s}");
    }

    let observed = BuckControlFixture::observed(&seen);
    for (sample, (want, got)) in expected.iter().zip(&observed).enumerate() {
        assert_eq!(want, got, "sample {sample}");
    }
}

#[test]
fn the_setpoint_register_takes_known_data_and_saturates() {
    // `tests/buck/control.msl` `Writes`: `wr` rises at the rising edges 2, 5, 8 and 11 (the
    // 2nd, 5th, 8th and 11th of the oscillator, 1 + 3k of its counter), with the data 7, 100,
    // 200 and 200 again presented before
    let seen = BuckFixture::build(&BuckControlFixture::project(), "SetpointBench").run(70e-9, 1e-9);
    // SPEC §7.1: the register resets to 100; a write of 7 takes 7; 100 takes 100 (a change from
    // 7); 200 saturates to 168; a second 200 commits the held 168, which is no change
    let code = BuckFixture::transient(&seen, "SetpointBench.code");
    assert_eq!(
        code,
        vec![(TopDesign::rising_edge(2), vec![7]), (TopDesign::rising_edge(5), vec![100]), (TopDesign::rising_edge(8), vec![168])],
        "the register's code changes at the three first writes and at no other time"
    );
    // the fourth write rose `wr` at the 11th edge and committed the held 168: `wr` rose there, and the
    // register's commits are the three above and a fourth that changes nothing
    let wr = BuckFixture::transient(&seen, "SetpointBench.wr");
    assert_eq!(wr.iter().filter(|(_, b)| b[0] == 1).count(), 4, "wr rose four times: {wr:?}");
    let made = seen.commits("SetpointBench.reg");
    assert_eq!(made.len(), 4, "four commits: the fourth is the held value: {made:?}");
    assert!(made.iter().all(|t| *t % (5 * NS) == 5 * NS / 2), "each at a rising edge: {made:?}");
}
