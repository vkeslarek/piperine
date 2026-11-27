//! Evaluating analog functions (feature `solver-abi-analog`, SAN-06,
//! SAN-07, SAN-11, SAN-12, T2): a function whose read bytes did not change is
//! not called (F3, C11); setting a discrete slot makes its readers run again
//! (C21); a non-finite value or seeded partial fails the solve naming
//! instance, cell and origin (A4, C12); ambient inputs carry the solve's
//! coordinates (C20); partials against params are not seeded in a Newton
//! solve; a device fault reaches the caller with its code and the instance.

use piperine_device_fixtures::analog::{Call, Modules};
use piperine_device_fixtures::assembly::Circuits;
use piperine_analog_solver::device::AnalogBinding::{Discrete, Ground};
use piperine_analog_solver::device::{Ambient, AnalogPlan, Assembly};

#[test]
fn a_clean_function_is_not_called() {
    let m = Modules::all();
    let (net, n) = Circuits::netlist(1, &[]);
    let src = Circuits::tagged("i1", m.source, 101, vec![Discrete(0), Discrete(0), Circuits::at(&n[0]), Ground]);
    let res = Circuits::tagged("r1", m.counted, 102, vec![Discrete(1), Discrete(2), Circuits::at(&n[0]), Ground]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![src, res], &net).expect("bind"));
    assembly.set_discrete(1, &Circuits::real_bytes(1e-3));
    assembly.set_discrete(2, &Circuits::real_bytes(1e3));

    let mut x = vec![0.5];
    for _ in 0..10 {
        assembly.evaluate(&x, EvalCircuits::dc()).expect("evaluate");
    }
    assert_eq!(Call::recorded(101).len(), 1, "the source reads only its param: one call across ten iterations");
    assert_eq!(Call::recorded(102).len(), 1, "the resistor's pins did not move");

    x[0] = 0.75;
    assembly.evaluate(&x, EvalCircuits::dc()).expect("evaluate");
    assert_eq!(Call::recorded(101).len(), 1, "the source does not read the pin that moved");
    let r = Call::recorded(102);
    assert_eq!(r.len(), 2, "the resistor runs again when a pin it reads moves");
    assert_eq!(Circuits::real_of(&r[1].reads[0]), 0.75);
}

#[test]
fn setting_a_discrete_slot_makes_its_readers_dirty() {
    let m = Modules::all();
    let (net, n) = Circuits::netlist(1, &[]);
    let src = Circuits::tagged("i1", m.source, 201, vec![Discrete(0), Discrete(0), Circuits::at(&n[0]), Ground]);
    let res = Circuits::tagged("r1", m.counted, 202, vec![Discrete(1), Discrete(2), Circuits::at(&n[0]), Ground]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![src, res], &net).expect("bind"));
    assembly.set_discrete(0, &Circuits::real_bytes(1e-3));
    assembly.set_discrete(2, &Circuits::real_bytes(1e3));
    let x = [0.5];
    assembly.evaluate(&x, EvalCircuits::dc()).expect("evaluate");
    assert_eq!(Call::recorded(201).len(), 1);

    assembly.set_discrete(0, &Circuits::real_bytes(1e-3));
    assembly.evaluate(&x, EvalCircuits::dc()).expect("evaluate");
    assert_eq!(Call::recorded(201).len(), 1, "the same bytes are no change (P2)");

    assembly.set_discrete(0, &Circuits::real_bytes(2e-3));
    assembly.evaluate(&x, EvalCircuits::dc()).expect("evaluate");
    let s = Call::recorded(201);
    assert_eq!(s.len(), 2, "a reader of the slot runs again in the next solve");
    assert_eq!(Circuits::real_of(&s[1].reads[0]), 2e-3, "and reads the new bytes");
    assert_eq!(Call::recorded(202).len(), 1, "a function that does not read the slot stays clean");
}

#[test]
fn a_non_finite_value_names_instance_cell_and_origin() {
    let m = Modules::all();
    let (net, n) = Circuits::netlist(2, &[]);
    let run = |mode: f64| {
        let f = Circuits::instance("top.x1", m.faulty, vec![Discrete(0), Circuits::at(&n[0]), Circuits::at(&n[1])]);
        let mut assembly = Assembly::new(AnalogPlan::bind(vec![f], &net).expect("bind"));
        assembly.set_discrete(0, &Circuits::real_bytes(mode));
        assembly.evaluate(&[0.3, 0.1], EvalCircuits::dc()).map_err(|e| e.to_string())
    };
    assert!(run(0.0).is_ok());

    let err = run(1.0).expect_err("a NaN value");
    for part in ["top.x1", "term `i`", "fixture:faulty:T0", "NaN"] {
        assert!(err.contains(part), "`{err}` does not name {part}");
    }
    let err = run(2.0).expect_err("an infinite seeded partial");
    for part in ["top.x1", "partial 0 of term `i`", "fixture:faulty:T0", "inf"] {
        assert!(err.contains(part), "`{err}` does not name {part}");
    }
    assert!(run(3.0).is_ok(), "a partial against a param is not seeded, hence not read, in a Newton solve");
}

#[test]
fn a_device_fault_names_the_instance() {
    let m = Modules::all();
    let (net, n) = Circuits::netlist(2, &[]);
    let f = Circuits::instance("top.x2", m.faulty, vec![Discrete(0), Circuits::at(&n[0]), Circuits::at(&n[1])]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![f], &net).expect("bind"));
    assembly.set_discrete(0, &Circuits::real_bytes(4.0));
    let err = assembly.evaluate(&[0.3, 0.1], EvalCircuits::dc()).expect_err("a fault").to_string();
    for part in ["top.x2", "function `i`", "code 7", "mode four"] {
        assert!(err.contains(part), "`{err}` does not name {part}");
    }
}

#[test]
fn ambient_inputs_carry_the_solve_coordinates() {
    let m = Modules::all();
    let (net, n) = Circuits::netlist(1, &[]);
    let p = Circuits::tagged("p1", m.probe, 301, vec![Discrete(0), Circuits::at(&n[0])]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![p], &net).expect("bind"));
    let x = [0.2];

    let stepping = Ambient { time: 0.0, freq: 0.0, is_static: true, gmin: 1e-6, source_scale: 0.25 };
    let at_rest = Ambient { gmin: 1e-12, source_scale: 1.0, ..stepping };
    let tran = Ambient { time: 2.5e-6, freq: 0.0, is_static: false, gmin: 1e-12, source_scale: 1.0 };
    let sweep = Ambient { time: 0.0, freq: 1.0e3, is_static: false, gmin: 1e-12, source_scale: 1.0 };
    for ambient in [stepping, at_rest, tran, sweep] {
        assembly.evaluate(&x, ambient).expect("evaluate");
    }
    let c = Call::recorded(301);
    assert_eq!(c.len(), 4, "each new coordinate is a changed input");
    let read = |i: usize| {
        let r = &c[i].reads;
        (Circuits::real_of(&r[1]), Circuits::real_of(&r[2]), r[3].clone(), Circuits::real_of(&r[4]), Circuits::real_of(&r[5]))
    };
    assert_eq!(read(0), (0.0, 0.0, vec![1], 1e-6, 0.25), "TIME 0 and STATIC 1 in an equilibrium solve");
    assert_eq!(read(1), (0.0, 0.0, vec![1], 1e-12, 1.0), "GMIN and SOURCE_SCALE at rest");
    assert_eq!(read(2), (2.5e-6, 0.0, vec![0], 1e-12, 1.0), "TIME is the solve's time");
    assert_eq!(read(3), (0.0, 1.0e3, vec![0], 1e-12, 1.0), "FREQ is the sweep point's frequency");
}

#[test]
fn params_are_not_seeded_in_a_newton_solve() {
    let m = Modules::all();
    let (net, n) = Circuits::netlist(1, &[]);
    let src = Circuits::tagged("i1", m.source, 401, vec![Discrete(0), Discrete(1), Circuits::at(&n[0]), Ground]);
    let res = Circuits::tagged("r1", m.counted, 402, vec![Discrete(0), Discrete(2), Circuits::at(&n[0]), Ground]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![src, res], &net).expect("bind"));
    assembly.set_discrete(2, &Circuits::real_bytes(1e3));
    assembly.evaluate(&[0.5], EvalCircuits::dc()).expect("evaluate");
    assert_eq!(Call::recorded(401)[0].seed, vec![false], "d(i)/d(idc) is against a param");
    assert_eq!(Call::recorded(402)[0].seed, vec![true, true, false], "seeded against p and n, not against r");
}

/// The circuits the eval tests build.
pub struct EvalCircuits;

impl EvalCircuits {
    /// The dc helper `dc`.
    fn dc() -> Ambient {
        Ambient { time: 0.0, freq: 0.0, is_static: true, gmin: 1e-12, source_scale: 1.0 }
    }
}
