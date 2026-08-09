//! Runtime operators: `delay`/`slew`/`transition`/`idt`/`idtmod` state,
//! updated once per *accepted* timestep — never inside a Newton iteration.
//! A rejected step must leave every operator untouched; that invariant is
//! the caller's responsibility (this type is only ever `accept`ed on
//! accepted timesteps), not something this type can enforce on its own.
//!
//! Extracted from `piperine-codegen`'s
//! `device/analog/operators.rs::Operator`, semantics unchanged. Codegen
//! keeps its own copy until `codegen-simplification` delegates to this one
//! (`.specs/features/dv-tester/design.md` §C1).

use std::collections::VecDeque;

/// A runtime-serviced analog operator.
pub enum Operator {
    /// `delay(x, t)` — a `(time, value)` history ring.
    Delay { delay: f64, history: VecDeque<(f64, f64)> },
    /// `slew(x, rise, fall)` — rate-limited follower.
    Slew { rise: f64, fall: f64, output: f64, time: f64 },
    /// `transition(x, td, rise, fall)` — the output walks linearly from the
    /// pre-change value to the latest input over rise/fall, starting `td`
    /// after the change. Single pending ramp (Verilog-AMS state =
    /// (start, target, t_change)): a new input mid-ramp re-anchors — start
    /// becomes the current output, t_change becomes now. Mutated only on
    /// accepted timesteps, so rejected steps leave the ramp untouched.
    Transition {
        delay: f64,
        rise: f64,
        fall: f64,
        start: f64,
        target: f64,
        t_change: f64,
        seeded: bool,
    },
    /// `idt`/`idtmod` — implicit-Euler accumulator (`value += dt·x`, wrapped
    /// into `[0, modulus)` when given). The in-step `dt·x` term is left to
    /// the consumer; `value` is the integral up to the last accepted step.
    Integrate { modulus: Option<f64>, value: f64, time: f64 },
}

impl Operator {
    /// A fresh `delay(x, td)` operator with empty history.
    pub fn delay(delay: f64) -> Self {
        Operator::Delay { delay, history: VecDeque::new() }
    }

    /// A fresh `slew(x, rise, fall)` operator, output starting at 0.
    pub fn slew(rise: f64, fall: f64) -> Self {
        Operator::Slew { rise, fall, output: 0.0, time: 0.0 }
    }

    /// A fresh `transition(x, td, rise, fall)` operator, unseeded.
    pub fn transition(delay: f64, rise: f64, fall: f64) -> Self {
        Operator::Transition { delay, rise, fall, start: 0.0, target: 0.0, t_change: 0.0, seeded: false }
    }

    /// A fresh `idt`/`idtmod` accumulator, `value` starting at `ic`.
    pub fn integrate(ic: f64, modulus: Option<f64>) -> Self {
        Operator::Integrate { modulus, value: ic, time: 0.0 }
    }

    /// Advance to `time` with the operator input `input`; returns the new
    /// output value. Call only on accepted timesteps.
    pub fn accept(&mut self, time: f64, input: f64) -> f64 {
        match self {
            Operator::Delay { delay, history } => {
                history.push_back((time, input));
                let target = time - *delay;
                // Drop history strictly older than the target, keeping one
                // sample at or before it for interpolation.
                while history.len() > 1 && history[1].0 <= target {
                    history.pop_front();
                }
                match history.front() {
                    Some(&(t0, v0)) => match history.get(1) {
                        Some(&(t1, v1)) if t1 > t0 && target >= t0 => {
                            let frac = ((target - t0) / (t1 - t0)).clamp(0.0, 1.0);
                            v0 + frac * (v1 - v0)
                        }
                        _ => v0,
                    },
                    None => input,
                }
            }
            Operator::Slew { rise, fall, output, time: last_time } => {
                let dt = (time - *last_time).max(0.0);
                let delta = input - *output;
                let limited = if delta >= 0.0 {
                    delta.min(*rise * dt)
                } else {
                    delta.max(-*fall * dt)
                };
                *output += limited;
                *last_time = time;
                *output
            }
            Operator::Transition { delay, rise, fall, start, target, t_change, seeded } => {
                if !*seeded {
                    // First observation (the DC op seed): settle at the input.
                    *seeded = true;
                    *start = input;
                    *target = input;
                    *t_change = time;
                    return input;
                }
                if input != *target {
                    *start = Self::transition_at(*start, *target, *t_change, *delay, *rise, *fall, time);
                    *target = input;
                    *t_change = time;
                }
                Self::transition_at(*start, *target, *t_change, *delay, *rise, *fall, time)
            }
            Operator::Integrate { modulus, value, time: last_time } => {
                let dt = (time - *last_time).max(0.0);
                *value += dt * input;
                if let Some(m) = *modulus
                    && m > 0.0
                {
                    *value -= m * (*value / m).floor();
                }
                *last_time = time;
                *value
            }
        }
    }

    /// The transition output at `t`: `start` until `t_change + td`, a linear
    /// walk to `target` over rise (climbing) or fall (dropping) —
    /// instantaneous when that time is zero — then `target`.
    fn transition_at(start: f64, target: f64, t_change: f64, delay: f64, rise: f64, fall: f64, t: f64) -> f64 {
        let t0 = t_change + delay.max(0.0);
        if t <= t0 {
            return start;
        }
        let ramp = if target >= start { rise.max(0.0) } else { fall.max(0.0) };
        if ramp <= 0.0 || t >= t0 + ramp {
            return target;
        }
        start + (target - start) * (t - t0) / ramp
    }

    /// Ramp edges strictly inside `(from, end]` for breakpoint declaration.
    /// Only a transition with a pending ramp contributes; a settled one
    /// (start == target, or never seeded) has no future edges.
    pub fn pending_edges(&self, from: f64, end: f64, out: &mut Vec<f64>) {
        let Operator::Transition { delay, rise, fall, start, target, t_change, seeded: true } = self else {
            return;
        };
        if start == target {
            return;
        }
        let t0 = *t_change + delay.max(0.0);
        if t0 > from && t0 <= end {
            out.push(t0);
        }
        let ramp = if target >= start { rise.max(0.0) } else { fall.max(0.0) };
        let t1 = t0 + ramp;
        if ramp > 0.0 && t1 > from && t1 <= end {
            out.push(t1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transition(delay: f64, rise: f64, fall: f64) -> Operator {
        Operator::transition(delay, rise, fall)
    }

    #[test]
    fn transition_seeds_at_first_accept() {
        let mut op = transition(0.0, 1.0, 1.0);
        // The DC-op seed settles at the input — no ramp from 0.
        assert_eq!(op.accept(0.0, 2.5), 2.5);
        assert_eq!(op.accept(1.0, 2.5), 2.5, "settled while input holds");
    }

    #[test]
    fn transition_ramps_linearly_over_rise_and_fall() {
        let mut op = transition(0.0, 2.0, 3.0);
        assert_eq!(op.accept(0.0, 0.0), 0.0);
        // Step 0 → 1 observed at t=1: output still `start` at the edge...
        assert_eq!(op.accept(1.0, 1.0), 0.0);
        // ...then walks linearly over rise=2: 0.5/s slope.
        assert_eq!(op.accept(1.5, 1.0), 0.25);
        assert_eq!(op.accept(2.0, 1.0), 0.5);
        assert_eq!(op.accept(3.0, 1.0), 1.0, "ramp done at t_change + rise");
        assert_eq!(op.accept(4.0, 1.0), 1.0, "holds target afterwards");
        // Step back down at t=4: falls over fall=3 (slope 1/3).
        assert_eq!(op.accept(4.0, 0.0), 1.0);
        assert!((op.accept(5.0, 0.0) - 2.0 / 3.0).abs() < 1e-15);
        assert_eq!(op.accept(7.0, 0.0), 0.0, "fall done at t_change + fall");
    }

    #[test]
    fn transition_retargets_from_current_value_mid_ramp() {
        let mut op = transition(0.0, 2.0, 2.0);
        assert_eq!(op.accept(0.0, 0.0), 0.0);
        assert_eq!(op.accept(0.0, 1.0), 0.0);
        assert_eq!(op.accept(0.5, 1.0), 0.25);
        // New target mid-ramp: re-anchors at the current output (0.25).
        assert_eq!(op.accept(0.5, 2.0), 0.25);
        assert!((op.accept(1.0, 2.0) - 0.6875).abs() < 1e-15, "0.25 + 1.75·0.5/2");
        assert_eq!(op.accept(2.5, 2.0), 2.0);
    }

    #[test]
    fn transition_zero_rise_or_fall_steps_at_the_delay_edge() {
        let mut op = transition(0.5, 0.0, 0.0);
        assert_eq!(op.accept(0.0, 1.0), 1.0);
        assert_eq!(op.accept(1.0, 0.0), 1.0, "still start before t_change + td");
        assert_eq!(op.accept(1.4, 0.0), 1.0);
        let landed = op.accept(1.6, 0.0);
        assert_eq!(landed, 0.0, "zero fall: instantaneous past td");
        assert!(landed.is_finite(), "no divide-by-zero");
    }

    #[test]
    fn transition_pending_edges_while_ramping() {
        let mut op = transition(0.0, 2.0, 2.0);
        let mut out = Vec::new();
        op.pending_edges(0.0, 10.0, &mut out);
        assert!(out.is_empty(), "unseeded: no edges");

        op.accept(0.0, 0.0);
        op.accept(1.0, 1.0); // change at t=1, ramp over [1, 3]
        let mut out = Vec::new();
        op.pending_edges(0.5, 10.0, &mut out);
        assert_eq!(out, vec![1.0, 3.0], "ramp start + end");
        let mut out = Vec::new();
        op.pending_edges(1.0, 10.0, &mut out);
        assert_eq!(out, vec![3.0], "start edge already reached");

        op.accept(3.0, 1.0); // settled
        let mut out = Vec::new();
        op.pending_edges(3.0, 10.0, &mut out);
        assert!(out.is_empty(), "settled: no edges");
    }

    #[test]
    fn transition_zero_ramp_declares_only_the_start_edge() {
        let mut op = transition(0.5, 0.0, 0.0);
        op.accept(0.0, 0.0);
        op.accept(1.0, 1.0); // step at t0 = 1.5, instantaneous
        let mut out = Vec::new();
        op.pending_edges(1.0, 10.0, &mut out);
        assert_eq!(out, vec![1.5], "instantaneous step still lands a breakpoint");
    }

    // --- Delay: not covered by codegen's own tests (none existed there);
    // added here per DVT-02's rules for `delay`, spec.md P1 §2.

    #[test]
    fn delay_falls_back_to_input_on_empty_history() {
        let mut op = Operator::delay(1.0);
        assert_eq!(op.accept(0.0, 5.0), 5.0, "first sample: nothing to interpolate against");
    }

    #[test]
    fn delay_interpolates_linearly_with_clamped_fraction() {
        let mut op = Operator::delay(1.0);
        op.accept(0.0, 0.0);
        op.accept(1.0, 10.0);
        // target = t - td; at t=1.5, target=0.5, between samples (0,0) and (1,10).
        assert_eq!(op.accept(1.5, 20.0), 5.0, "linear interpolation at the midpoint");
    }

    #[test]
    fn delay_keeps_one_sample_at_or_before_the_target_and_drops_older() {
        let mut op = Operator::delay(1.0);
        op.accept(0.0, 0.0);
        op.accept(1.0, 10.0);
        op.accept(2.0, 20.0);
        // target = 3 - 1 = 2: at or exactly the newest sample.
        assert_eq!(op.accept(3.0, 30.0), 20.0);
    }

    // --- Slew

    #[test]
    fn slew_limits_rise_and_fall_asymmetrically() {
        let mut op = Operator::slew(1.0, 2.0);
        // Rising: limited to rise*dt = 1.0*1.0 = 1.0, input asks for 5.0.
        assert_eq!(op.accept(1.0, 5.0), 1.0);
        // Falling: limited to fall*dt = 2.0*1.0 = 2.0, input asks for -5.0 from 1.0.
        assert_eq!(op.accept(2.0, -5.0), -1.0);
    }

    #[test]
    fn slew_dt_floored_at_zero() {
        let mut op = Operator::slew(1.0, 1.0);
        assert_eq!(op.accept(1.0, 5.0), 1.0);
        // A time that goes backwards must not produce a negative dt (no
        // motion allowed, not a sign-flipped rate).
        assert_eq!(op.accept(0.5, 5.0), 1.0, "dt floored at 0: no backwards motion");
    }

    // --- Integrate (idt/idtmod)

    #[test]
    fn integrate_accumulates_implicit_euler() {
        let mut op = Operator::integrate(0.0, None);
        assert_eq!(op.accept(1.0, 2.0), 2.0, "dt=1 (from time=0) * x=2");
        assert_eq!(op.accept(2.0, 3.0), 5.0, "+ dt=1 * x=3");
    }

    #[test]
    fn integrate_wraps_into_modulus() {
        let mut op = Operator::integrate(0.0, Some(10.0));
        op.accept(1.0, 7.0); // value = 7
        let wrapped = op.accept(2.0, 7.0); // value = 14 -> wrapped into [0, 10)
        assert_eq!(wrapped, 4.0);
    }

    #[test]
    fn integrate_starts_at_initial_condition() {
        let mut op = Operator::integrate(3.0, None);
        assert_eq!(op.accept(0.0, 0.0), 3.0, "dt=0 at first accept: value stays at ic");
    }
}
