//! Digital events and logic values: `LogicValue`, the `DigitalNet`
//! handle, and the time-ordered `DigitalEvent` the scheduler queues.
//!
//! **T38 (LRMC-C7-12):** `DigitalEvent` now carries a §8.3 [`Region`]
//! (`super::regions`) — same-timestamp events order by `(time, region,
//! seq)` instead of `(time, seq)`. Every existing event source (the JIT
//! `pip_*`/`QueueSink::emit` runtime path, every hand-built fixture
//! across the workspace) stamps `Region::Active`, so this is a
//! behavior-preserving superset for all of them: `region` is always
//! equal within a same-source batch, so the new key component never
//! changes relative order there. `scheduler.rs` is what actually drains
//! by region now — see its own doc comment.
use std::cmp::Ordering;

use super::drivers::Strength;
use super::regions::Region;

// ---------------------------------------------------------------------------
// LogicValue
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LogicValue {
    Zero = 0,
    One = 1,
    X = 2,
    Z = 3,
}

impl LogicValue {
    /// Resolves two driving logic values onto the same net.
    pub fn resolve(a: LogicValue, b: LogicValue) -> LogicValue {
        match (a, b) {
            (LogicValue::Z, other) | (other, LogicValue::Z) => other,
            (LogicValue::Zero, LogicValue::Zero) => LogicValue::Zero,
            (LogicValue::One, LogicValue::One) => LogicValue::One,
            _ => LogicValue::X,
        }
    }
}

// ---------------------------------------------------------------------------
// DigitalNet / DigitalEvent
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DigitalNet(pub usize);

#[derive(Debug, Clone)]
pub struct DigitalEvent {
    pub time: f64,
    pub net: DigitalNet,
    pub value: LogicValue,
    pub source: usize,
    pub seq: u64,
    /// LRM §8.3 stratified region (T38/LRMC-C7-12) — `Active` for every
    /// event source that predates this field (the common case: an
    /// ordinary blocking-assign-derived write).
    pub region: Region,
    /// LRM §7.9 Table 7-2 drive strength (LRMW3-09, A.2.2.2) explicitly
    /// authored by the emitting model (e.g. a continuous assign carrying
    /// `assign (strong1, weak0) w = r;`) — `None` for every event source
    /// that predates this field (the overwhelming common case: an
    /// ordinary write with no authored strength), which
    /// [`super::scheduler::apply_driven_event`] falls back to
    /// [`super::drivers::default_strength_for`] for, exactly as before
    /// this field existed. Additive per this struct's own ABI-stability
    /// contract (`interface.rs`'s module doc) — a model compiled against
    /// an older version never needs to know this field exists.
    pub strength: Option<Strength>,
}

impl PartialEq for DigitalEvent {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time &&
        self.seq == other.seq &&
        self.net == other.net &&
        self.value == other.value &&
        self.source == other.source &&
        self.region == other.region &&
        self.strength == other.strength
    }
}

impl Eq for DigitalEvent {}

impl PartialOrd for DigitalEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DigitalEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        let time_cmp = self.time.total_cmp(&other.time);
        if time_cmp != Ordering::Equal {
            return time_cmp;
        }
        let region_cmp = self.region.cmp(&other.region);
        if region_cmp != Ordering::Equal {
            return region_cmp;
        }
        self.seq.cmp(&other.seq)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digital::DigitalState;
    use crate::digital::interface::{DigitalPorts, EvalCtx, EventSink};
    use crate::core::element::{AnalogDevice, DigitalDevice, Element, ElementCapabilities, Introspect};
    use std::cmp::Reverse;

    struct MockInverter {
        // Set at construction so each mock is distinguishable in a failure
        // dump; these tests assert on the event queue, never on the id.
        #[allow(dead_code)]
        id: usize,
        input: DigitalNet,
        output: DigitalNet,
        delay: f64,
    }

    impl AnalogDevice for MockInverter {}

    impl DigitalDevice for MockInverter {
        fn boundary(&self) -> DigitalPorts<'_> {
            DigitalPorts {
                inputs: std::slice::from_ref(&self.input),
                outputs: std::slice::from_ref(&self.output),
            }
        }

        fn init(&mut self, _sink: &mut dyn EventSink) {}

        fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
            let out_val = match ctx.nets[self.input.0] {
                LogicValue::Zero => LogicValue::One,
                LogicValue::One => LogicValue::Zero,
                _ => LogicValue::X,
            };
            sink.emit(self.output, out_val, self.delay);
        }
    }

    impl Introspect for MockInverter {}

    impl Element for MockInverter {
        fn name(&self) -> &str { "mock_inverter" }
        fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::DIGITAL }
    }

    #[test]
    fn test_logic_resolution() {
        assert_eq!(LogicValue::resolve(LogicValue::Zero, LogicValue::One), LogicValue::X);
        assert_eq!(LogicValue::resolve(LogicValue::Z, LogicValue::One), LogicValue::One);
        assert_eq!(LogicValue::resolve(LogicValue::X, LogicValue::Zero), LogicValue::X);
        assert_eq!(LogicValue::resolve(LogicValue::One, LogicValue::One), LogicValue::One);
    }

    #[test]
    fn test_evaluate_until_stable_chain() {
        let mut state = DigitalState::new(4);

        state.nets[0] = LogicValue::One;

        state.schedule(DigitalEvent {
            time: 1.0,
            net: DigitalNet(0),
            value: LogicValue::Zero,
            source: 99,
            seq: 0, region: Region::Active, strength: None,
        });

        let mut devices: Vec<Box<dyn Element>> = vec![
            Box::new(MockInverter { id: 0, input: DigitalNet(0), output: DigitalNet(1), delay: 0.0 }),
            Box::new(MockInverter { id: 1, input: DigitalNet(1), output: DigitalNet(2), delay: 0.0 }),
            Box::new(MockInverter { id: 2, input: DigitalNet(2), output: DigitalNet(3), delay: 0.0 }),
        ];
        state.evaluate_until_stable(1.0, &mut devices, Default::default(), &[]).unwrap();

        assert_eq!(state.nets[0], LogicValue::Zero);
        assert_eq!(state.nets[1], LogicValue::One);
        assert_eq!(state.nets[2], LogicValue::Zero);
        assert_eq!(state.nets[3], LogicValue::One);
    }

    /// T60 (LRMC-C10-12, model half): a nonzero `#delay` produces an
    /// output event at exactly `input_time + delay` — not before, not at
    /// some other offset. `evaluate_until_stable` is the scheduler
    /// primitive every gate-primitive/UDP dispatch (`piperine-codegen`'s
    /// `device/prim.rs::GateElement`, which threads its own `#delay`
    /// straight into `EventSink::emit`'s `delay` argument) and every
    /// module-path delay ultimately runs on, so proving it here proves it
    /// for every caller without needing `piperine-codegen`'s own types
    /// (which this crate must not depend on).
    #[test]
    fn nonzero_gate_delay_produces_a_correctly_timed_event() {
        let mut state = DigitalState::new(2);
        let input = DigitalNet(0);
        let output = DigitalNet(1);
        let delay = 2.0e-6;
        let mut devices: Vec<Box<dyn Element>> =
            vec![Box::new(MockInverter { id: 0, input, output, delay })];

        let t_input = 1.0e-6;
        state.schedule(DigitalEvent {
            time: t_input,
            net: input,
            value: LogicValue::One,
            source: 0,
            seq: 0,
            region: Region::Active, strength: None,
        });
        state.evaluate_until_stable(t_input, &mut devices, Default::default(), &[]).unwrap();
        assert_eq!(state.nets[output.0], LogicValue::X, "output must not flip before the delay elapses");

        // Just short of the delay: still unchanged.
        let just_before = t_input + delay - 1.0e-9;
        state.evaluate_until_stable(just_before, &mut devices, Default::default(), &[]).unwrap();
        assert_eq!(state.nets[output.0], LogicValue::X, "output must not flip early");

        // Exactly at input_time + delay: the queued event drains and applies.
        state.evaluate_until_stable(t_input + delay, &mut devices, Default::default(), &[]).unwrap();
        assert_eq!(state.nets[output.0], LogicValue::Zero, "inverter of One flips exactly at input_time + delay");
    }

    #[test]
    fn test_checkpoint_rollback() {
        let mut state = DigitalState::new(1);
        state.nets[0] = LogicValue::Zero;

        state.checkpoint();

        state.nets[0] = LogicValue::One;
        state.schedule(DigitalEvent {
            time: 5.0,
            net: DigitalNet(0),
            value: LogicValue::Zero,
            source: 0,
            seq: 0, region: Region::Active, strength: None,
        });

        assert_eq!(state.nets[0], LogicValue::One);
        assert_eq!(state.peek_next_event_time(), 5.0);

        state.rollback();

        assert_eq!(state.nets[0], LogicValue::Zero);
        assert_eq!(state.peek_next_event_time(), f64::INFINITY);
    }

    #[test]
    fn test_event_ordering() {
        let mut state = DigitalState::new(1);

        state.schedule(DigitalEvent { time: 5.0, net: DigitalNet(0), value: LogicValue::One, source: 0, seq: 2, region: Region::Active, strength: None });
        state.schedule(DigitalEvent { time: 3.0, net: DigitalNet(0), value: LogicValue::Zero, source: 0, seq: 0, region: Region::Active, strength: None });
        state.schedule(DigitalEvent { time: 5.0, net: DigitalNet(0), value: LogicValue::Z, source: 0, seq: 1, region: Region::Active, strength: None });

        let mut extracted = Vec::new();
        while let Some(Reverse(e)) = state.event_queue.pop() {
            extracted.push((e.time, e.value));
        }

        assert_eq!(extracted, vec![
            (3.0, LogicValue::Zero),
            (5.0, LogicValue::Z),
            (5.0, LogicValue::One),
        ]);
    }

    /// T38 (LRMC-C7-12) — §8.3's canonical race, through the *production*
    /// path (`evaluate_until_stable`, not `RegionQueue` standalone): one
    /// process writing the same variable twice at the same timestamp, once
    /// blocking (`Active`) and once non-blocking (`Nba`), scheduled in a
    /// submission order that would give the wrong answer if region were
    /// ignored. Regardless of submission order the LRM's stratified regions
    /// make the `Nba` write the surviving one (`Region`'s derived `Ord` —
    /// `Active < Nba` — is the event queue's tie-break ahead of `seq`).
    ///
    /// **Both writes carry the same `source`.** A region race is two writes
    /// from ONE process (§8.5.3.3/4's `a = b;` vs `a <= b;`), which is a
    /// single driver updating twice — not two drivers. Two *distinct*
    /// sources on one net are a §7.9 driver contention resolved by Table
    /// 7-3, which is what `two_distinct_drivers_contend_per_table_7_3`
    /// below pins; conflating the two is what made this test read `X`
    /// after T54 gave the scheduler a real `DriverTable`.
    #[test]
    fn stratified_regions_resolve_the_canonical_active_nba_race_through_evaluate_until_stable() {
        for (active_seq, nba_seq) in [(0u64, 1u64), (1u64, 0u64)] {
            let mut state = DigitalState::new(1);
            state.schedule(DigitalEvent {
                time: 1.0,
                net: DigitalNet(0),
                value: LogicValue::Zero,
                source: 0,
                seq: active_seq,
                region: Region::Active, strength: None,
            });
            state.schedule(DigitalEvent {
                time: 1.0,
                net: DigitalNet(0),
                value: LogicValue::One,
                source: 0,
                seq: nba_seq,
                region: Region::Nba, strength: None,
            });
            state.evaluate_until_stable(1.0, &mut [], Default::default(), &[]).unwrap();
            assert_eq!(
                state.nets[0],
                LogicValue::One,
                "the Nba write must win regardless of submission order (active_seq={active_seq}, nba_seq={nba_seq})"
            );
        }
    }

    /// T54/LRMW2-46's half of the same boundary: two *distinct* sources
    /// driving one net at the same timestamp are drivers, not a region
    /// race — strong-0 against strong-1 resolves to `X` per Table 7-3, no
    /// matter which region each carries. This is the assertion that makes
    /// the same-`source` requirement above falsifiable rather than a
    /// stylistic choice.
    #[test]
    fn two_distinct_drivers_contend_per_table_7_3() {
        let mut state = DigitalState::new(1);
        state.schedule(DigitalEvent {
            time: 1.0,
            net: DigitalNet(0),
            value: LogicValue::Zero,
            source: 0,
            seq: 0,
            region: Region::Active, strength: None,
        });
        state.schedule(DigitalEvent {
            time: 1.0,
            net: DigitalNet(0),
            value: LogicValue::One,
            source: 1,
            seq: 1,
            region: Region::Nba, strength: None,
        });
        state.evaluate_until_stable(1.0, &mut [], Default::default(), &[]).unwrap();
        assert_eq!(
            state.nets[0],
            LogicValue::X,
            "two equal-strength opposing drivers contend to X (Table 7-3), \
             regardless of the region each write arrived in"
        );
    }

    /// T38: `Inactive` (`#0`-delay) is exercised through the production
    /// path too — the same process writing the same variable at the same
    /// timestamp in `Active` and `Inactive`; `Inactive` must land after
    /// `Active` per §8.3's region order (`Active < Inactive < Nba <
    /// Monitor`).
    #[test]
    fn inactive_region_lands_after_active_at_the_same_timestamp() {
        let mut state = DigitalState::new(1);
        state.schedule(DigitalEvent {
            time: 2.0,
            net: DigitalNet(0),
            value: LogicValue::One,
            source: 0,
            seq: 0,
            region: Region::Inactive, strength: None,
        });
        state.schedule(DigitalEvent {
            time: 2.0,
            net: DigitalNet(0),
            value: LogicValue::Zero,
            source: 0,
            seq: 1,
            region: Region::Active, strength: None,
        });
        state.evaluate_until_stable(2.0, &mut [], Default::default(), &[]).unwrap();
        assert_eq!(state.nets[0], LogicValue::One, "Inactive (#0) must land after Active at the same timestamp");
    }

    /// T38: `Monitor` sits last in the region order (`Active < Inactive <
    /// Nba < Monitor`) — a same-timestamp `Monitor` write from the same
    /// process overrides every earlier region's write to the same
    /// variable, through the production path. The Monitor value is `Zero`
    /// (not `X`) on purpose: an `X` here would also be the value a driver
    /// contention produces, so the assertion could not tell region order
    /// from resolution.
    #[test]
    fn monitor_region_lands_last_at_the_same_timestamp() {
        let mut state = DigitalState::new(1);
        state.schedule(DigitalEvent {
            time: 3.0,
            net: DigitalNet(0),
            value: LogicValue::Zero,
            source: 0,
            seq: 0,
            region: Region::Monitor, strength: None,
        });
        state.schedule(DigitalEvent {
            time: 3.0,
            net: DigitalNet(0),
            value: LogicValue::One,
            source: 0,
            seq: 1,
            region: Region::Nba, strength: None,
        });
        state.schedule(DigitalEvent {
            time: 3.0,
            net: DigitalNet(0),
            value: LogicValue::One,
            source: 0,
            seq: 2,
            region: Region::Active, strength: None,
        });
        state.evaluate_until_stable(3.0, &mut [], Default::default(), &[]).unwrap();
        assert_eq!(state.nets[0], LogicValue::Zero, "Monitor drains last, overriding Active/Nba at the same timestamp");
    }

    #[test]
    fn digital_state_carries_source_labels_or_anonymous_fallback() {
        let mut state = DigitalState::new(3);

        // No labels attached — defaults to d{idx}.
        assert_eq!(state.label_or_default(DigitalNet(0)), "d0");
        assert_eq!(state.label_or_default(DigitalNet(2)), "d2");

        // Attach a hierarchical source label and verify it survives a checkpoint
        // round-trip — used by result mappers and diagnostics.
        state.set_label(DigitalNet(1), "top.u1.clk");
        assert_eq!(state.label_or_default(DigitalNet(1)), "top.u1.clk");

        state.checkpoint();
        state.set_label(DigitalNet(1), "scratch");
        assert_eq!(state.label_or_default(DigitalNet(1)), "scratch");
        state.rollback();
        assert_eq!(state.label_or_default(DigitalNet(1)), "top.u1.clk");
    }
}
