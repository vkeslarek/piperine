//! The compiled `RefRamp` and `Pid` on the solver under a scripted sequence of 200 samples
//! (feature `ir-rust`, RS-13 AC 7; check C91). A compiled `Script` body, run by the same 200 MHz
//! oscillator, holds `run`, `target` and `meas` steady around each rising edge and moves them
//! half a period before the next one (at each falling edge); it steps through soft-start, a step
//! of the measurement up and down to both rails, a lower target, `run` dropped and `run`
//! raised again. The expected values are the difference equations of `examples/buck/SPEC.md`
//! §6.2 to §6.4 (the PID) and §7.2 (the ramp) written here in plain `i32` arithmetic, sample by
//! sample, and compared with every value the solver committed, bit for bit.
#![cfg(unix)]

use piperine_ir::{BinaryOp, BodyId, CarrierRole, Constant, Event, EventSource, IrBuilder, Local, Real, UnaryOp, ValueType};

use piperine_ir_fixtures::digital;
use piperine_parity::mixed;

use digital::{Buck, Dx, ValueAliases};
use mixed::{BuiltDesign, Seen};
use piperine_ir_fixtures::mixed::TopDesign;

const BIT: ValueType = ValueType::Bool;
const F: &str = "script.msl";
/// The samples: one per rising edge of the oscillator.
const SAMPLES: i64 = 200;

/// Associated test helpers of `pid`, grouped on one fixture type.
struct PidFixture;

impl PidFixture {
    /// The inputs of sample `s`: `(run, target, meas)`. Both the `Script` body and the reference
    /// model read them from here, so the script is written once.
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

    /// `if s < t0 { v0 } else if s < t1 { v1 } ... else { last }`, over the local `s` (a plain
    /// `u8`), the values built by `value`.
    fn chain(x: &mut Dx<'_>, steps: &[(i128, i128)], last: i128, ty: &ValueType) -> piperine_ir::ExprId {
        let make = |x: &mut Dx<'_>, v: i128| if *ty == BIT { x.boolean(v != 0) } else { x.int(v, ty.clone()) };
        let mut tail = make(x, last);
        for (limit, value) in steps.iter().rev() {
            let s = x.local(0, ValueAliases::u(8));
            let at = x.int(*limit, ValueAliases::u(8));
            let below = x.bin(BinaryOp::Lt, s, at, BIT);
            let v = make(x, *value);
            tail = x.if_(below, v, tail, ty.clone());
        }
        tail
    }

    /// `Script`: the inputs of the loop, a sample ahead of each rising edge.
    ///
    /// ```text
    /// module Script(in clk: Bit, out run: Bit = false, out target: Bit<u10> = 150, out meas: Bit<u10> = 0) {
    ///     signal n: Bit<u8> = 0;
    /// }
    /// digital for Script {
    ///     on clk.fall() {
    ///         let s = n.known_or(0);
    ///         n <- s + 1;
    ///         run <- if s < 8 { false } else if s < 170 { true } else if s < 180 { false } else { true };
    ///         target <- if s < 120 { 150 } else { 100 };
    ///         meas <- if s < 40 { 0 } else if s < 80 { 20 } else if s < 110 { 1023 } else if s < 140 { 0 } else { 100 };
    ///     }
    /// }
    /// ```
    ///
    /// The operating point fires the `fall` at 0 (the clock starts low) and the oscillator's falling
    /// edge `m` is at `m` periods: the `fall` with `n = s` writes the inputs of sample `s`.
    fn script_body(b: &mut IrBuilder) -> BodyId {
        let body = b.body(Buck::spec("Script", F, 1));
        let (u8_, u10) = (ValueAliases::u(8), ValueAliases::u(10));
        b.carrier(body, Buck::carrier("clk", BIT, CarrierRole::In, None, F, 1, 20));
        b.carrier(body, Buck::carrier("run", BIT, CarrierRole::Out, Some(Constant::Bool(PidFixture::script(0).0)), F, 1, 30));
        b.carrier(body, Buck::carrier("target", u10.clone(), CarrierRole::Out, Some(Constant::Int(PidFixture::script(0).1 as i128)), F, 1, 40));
        b.carrier(body, Buck::carrier("meas", u10.clone(), CarrierRole::Out, Some(Constant::Int(PidFixture::script(0).2 as i128)), F, 1, 50));
        b.carrier(body, Buck::carrier("n", u8_.clone(), CarrierRole::Signal, Some(Constant::Int(0)), F, 2, 12));
        let mut x = Dx::new(b, F, 5, 12);
        let clk = x.carrier(0, BIT);
        let low = x.un(UnaryOp::Not, clk, BIT);
        let fall = b.event(body, Event { source: EventSource::Changed(0), guard: Some(low), origin: Buck::origin(F, 5, 8) });
        let mut x = Dx::new(b, F, 6, 17);
        let n = x.carrier(4, u8_.clone());
        let let_s = x.let_(0, n);
        x.at(7, 9);
        let s = x.local(0, u8_.clone());
        let one = x.int(1, u8_.clone());
        let next = x.bin(BinaryOp::Add, s, one, u8_.clone());
        let commit_n = x.commit(4, next, None);
        x.at(8, 16);
        let run = PidFixture::chain(&mut x, &[(8, 0), (170, 1), (180, 0)], 1, &BIT);
        let commit_run = x.commit(1, run, None);
        x.at(9, 19);
        let target = PidFixture::chain(&mut x, &[(120, 150)], 100, &u10);
        let commit_target = x.commit(2, target, None);
        x.at(10, 17);
        let meas = PidFixture::chain(&mut x, &[(40, 0), (80, 20), (110, 1023), (140, 0)], 100, &u10);
        let commit_meas = x.commit(3, meas, None);
        let locals = vec![Local { name: "s".into(), ty: u8_, mutable: false }];
        Buck::reaction(b, body, vec![fall], vec![let_s, commit_n, commit_run, commit_target, commit_meas], locals, F, 5);
        body
    }

    fn design() -> BuiltDesign {
        let mut w = TopDesign::new();
        let osc = Buck::osc(&mut w.b);
        let script_body = PidFixture::script_body(&mut w.b);
        let ramp = Buck::ref_ramp(&mut w.b);
        let pid = Buck::pid(&mut w.b);
        let (u10, u12) = (ValueAliases::u(10), ValueAliases::u(12));
        w.place("osc", osc, vec![Constant::Real(Real::new(200e6))], &[("clk", BIT)]);
        w.place("script", script_body, vec![], &[("clk", BIT), ("run", BIT), ("target", u10.clone()), ("meas", u10.clone()), ("script.n", ValueAliases::u(8))]);
        // tick, run, target, reference, at_target
        w.place("ramp", ramp, vec![], &[("clk", BIT), ("run", BIT), ("target", u10.clone()), ("reference", u10.clone()), ("at_target", BIT)]);
        // tick, run, reference, meas, duty, then the signals integ, deriv, e_prev
        w.place(
            "pid",
            pid,
            vec![],
            &[
                ("clk", BIT),
                ("run", BIT),
                ("reference", u10),
                ("meas", ValueAliases::u(10)),
                ("duty", u12),
                ("pid.integ", ValueAliases::i(32)),
                ("pid.deriv", ValueAliases::i(32)),
                ("pid.e_prev", ValueAliases::i(16)),
            ],
        );
        BuiltDesign::new(w.finish())
    }

    /// Sample by sample, the SPEC's equations. Both bodies react to the same rising edge, so each
    /// reads the other's value from before the edge: the PID reads the reference the ramp committed
    /// at the previous sample (SPEC §9: a delay of exactly one sample).
    fn model() -> Vec<State> {
        let mut s = State { reference: 0, duty: 0, integ: 0, deriv: 0, e_prev: 0 };
        let mut states = Vec::new();
        for sample in 0..SAMPLES {
            let (run, target, meas) = PidFixture::script(sample);
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

    /// The value of the net's series after the instant `at`: the last change at or before it.
    fn value_at(series: &[(i64, Vec<u8>)], at: i64) -> &[u8] {
        &series.iter().rev().find(|(t, _)| *t <= at).expect("an initial value").1
    }

    /// A little-endian integer of `bytes` bytes, sign-extended when `signed`.
    fn decode(raw: &[u8], bytes: usize, signed: bool) -> i32 {
        let mut buf = [0u8; 4];
        buf[..bytes].copy_from_slice(&raw[..bytes]);
        let v = u32::from_le_bytes(buf);
        if signed && bytes < 4 && v & (1 << (bytes * 8 - 1)) != 0 { (v | (u32::MAX << (bytes * 8))) as i32 } else { v as i32 }
    }

    fn observed(seen: &Seen) -> Vec<State> {
        let reference = seen.series("reference");
        let duty = seen.series("duty");
        let integ = seen.series_of("Top.pid.integ");
        let deriv = seen.series_of("Top.pid.deriv");
        let e_prev = seen.series_of("Top.pid.e_prev");
        (0..SAMPLES)
            .map(|s| {
                // sample `s` is the rising edge `s + 1`; its commits apply at that instant
                let at = TopDesign::rising_edge(s + 1);
                State {
                    reference: PidFixture::decode(PidFixture::value_at(&reference, at), 2, false),
                    duty: PidFixture::decode(PidFixture::value_at(&duty, at), 2, false),
                    integ: PidFixture::decode(PidFixture::value_at(&integ, at), 4, true),
                    deriv: PidFixture::decode(PidFixture::value_at(&deriv, at), 4, true),
                    e_prev: PidFixture::decode(PidFixture::value_at(&e_prev, at), 2, true),
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
    let expected = PidFixture::model();
    // the script reaches what it is for, so a loop that did nothing could not match: both rails,
    // the integrator's hold while saturated, the resets, the ramp both ways
    assert!(expected.iter().any(|s| s.duty == DUTY_MAX), "saturates high");
    assert!(expected.windows(2).any(|w| w[0].duty > 0 && w[1].duty == 0 && w[1].integ > 0), "saturates low with the integrator held");
    assert!(expected.windows(2).any(|w| w[1].duty == DUTY_MAX && w[1].integ == w[0].integ && w[1].integ > 0), "holds the integrator at the top");
    assert!(expected.iter().any(|s| s.integ == 0 && s.deriv == 0 && s.e_prev == 0 && s.reference == 0));
    assert!(expected.windows(2).any(|w| w[1].reference < w[0].reference && w[1].reference > 0), "the ramp goes down");
    assert!(expected.iter().any(|s| s.e_prev == E_MIN) && expected.iter().any(|s| s.e_prev == E_MAX || s.e_prev > 100));

    let seen = PidFixture::design().run(1.0e-6, 1e-8);
    let observed = PidFixture::observed(&seen);
    for (sample, (want, got)) in expected.iter().zip(&observed).enumerate() {
        assert_eq!(want, got, "sample {sample}");
    }
}
