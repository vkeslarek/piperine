//! Integer time of the ABI engine (feature `solver-abi-digital`, SAD-01 to
//! SAD-03; abi.md §6 C1): the tick count, the session resolution, the nearest
//! tick of a delay and the three ways a delay is not a time (E0502, E0435,
//! `+inf`).

use piperine_digital_solver::abi::tick::{Due, Resolution, Tick};
use piperine_digital_solver::error::DigitalError;

fn ticks(seconds_per_tick: f64) -> Resolution {
    Resolution { seconds_per_tick }
}

#[test]
fn the_default_tick_is_one_femtosecond() {
    assert_eq!(Resolution::default().seconds_per_tick, 1e-15);
}

#[test]
fn a_delay_is_due_at_the_nearest_tick() {
    // C1: a tick of 1 s makes the half-tick boundary exact.
    let r = ticks(1.0);
    let now = Tick(10);
    assert_eq!(r.due(now, 0.4), Ok(Due::NextMicrostep), "below half a tick rounds to no advance");
    assert_eq!(r.due(now, 0.5), Ok(Due::At(Tick(11))), "half a tick rounds up");
    assert_eq!(r.due(now, 0.6), Ok(Due::At(Tick(11))), "above half a tick");
    assert_eq!(r.due(now, 2.4), Ok(Due::At(Tick(12))));
    assert_eq!(r.due(now, 2.6), Ok(Due::At(Tick(13))));
    // the default resolution: 3 ns is 3_000_000 femtoseconds
    assert_eq!(Resolution::default().due(Tick(5), 3e-9), Ok(Due::At(Tick(5 + 3_000_000))));
}

#[test]
fn a_non_advancing_delay_is_the_next_microstep() {
    let r = ticks(1e-9);
    let now = Tick(7);
    assert_eq!(r.due(now, 0.0), Ok(Due::NextMicrostep));
    assert_eq!(r.due(now, -3e-9), Ok(Due::NextMicrostep));
    assert_eq!(r.due(now, f64::NEG_INFINITY), Ok(Due::NextMicrostep));
    assert_eq!(r.due(now, 0.25e-9), Ok(Due::NextMicrostep), "a quarter of a tick");
}

#[test]
fn a_time_outside_the_tick_range_is_e0502() {
    let r = Resolution::default();
    assert_eq!(r.tick_of(2.0e-15), Ok(Tick(2)));
    for seconds in [1e30, -1e30, f64::NAN, f64::INFINITY] {
        match r.tick_of(seconds) {
            Err(DigitalError::TickRange { code, .. }) => assert_eq!(code, "E0502"),
            other => panic!("{seconds}: expected E0502, got {other:?}"),
        }
    }
    // a due instant that overflows the count
    match r.due(Tick(i64::MAX - 1), 5e-15) {
        Err(DigitalError::TickRange { code, .. }) => assert_eq!(code, "E0502"),
        other => panic!("expected E0502, got {other:?}"),
    }
}

#[test]
fn a_nan_delay_is_e0435() {
    match Resolution::default().due(Tick(0), f64::NAN) {
        Err(DigitalError::NanDelay { code, .. }) => assert_eq!(code, "E0435"),
        other => panic!("expected E0435, got {other:?}"),
    }
}

#[test]
fn an_infinite_delay_never_comes_due() {
    assert_eq!(Resolution::default().due(Tick(0), f64::INFINITY), Ok(Due::Never));
}
