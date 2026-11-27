//! `DDT` terms (feature `solver-abi-analog`, SAN-03, T4): the solver-owned
//! TR-BDF2 companion. A `DDT` term is silent in an equilibrium solve (C3) and
//! when the step is not positive; in a stage it stamps the companion of
//! `TrBdf2::stage_coeffs` over its own history, with the trapezoidal stage's
//! previous-derivative correction (C4); an RC step driven by hand through
//! both stages of every step reads `1 − exp(−t/RC)`; a restore returns the
//! histories (C5, the `DDT` half).
//!
//! Numerically sensitive: the expected stamps are computed here from
//! `stage_coeffs` and `phase_coeffs`, as `C::load_transient` of
//! `piperine-solver/tests/helpers/devices.rs` reads `states.view(k)`.

use piperine_device_fixtures::analog::{RecordedCall, FixtureModules, VT};
use piperine_device_fixtures::assembly::Circuits;
use piperine_device_fixtures::assembly::{Solver, AssemblySolves, Tran};
use piperine_device_fixtures::examples::{K_B, Q};
use piperine_analog_solver::device::AnalogBinding::{Discrete, Ground};
use piperine_analog_solver::tolerances::Tolerances;
use piperine_analog_solver::device::{AnalogPlan, Assembly, JumpPosition, Stage};
use piperine_analog_solver::math::integration::{TrBdf2, TrBdf2Phase};
use piperine_analog_solver::math::iv::InitialValue;

const C: f64 = 1e-6;

#[test]
fn a_ddt_term_is_silent_in_an_equilibrium_solve() {
    let mut assembly = DdtCircuits::capacitor();
    let stamps = assembly.assemble_dc(&[0.8, 0.3], AssemblySolves::at_rest()).expect("assemble");
    let (a, b) = AssemblySolves::dense(&stamps, 2);
    assert_eq!(a, vec![vec![0.0; 2]; 2]);
    assert_eq!(b, vec![0.0; 2]);
    assert_eq!(AssemblySolves::matrix_positions(&stamps).len(), 4, "its positions stay in the pattern");

    // A step that is not positive adds nothing either, as `C::load_transient`.
    assembly.push_stage(&[0.5, 0.1], AssemblySolves::at_time(0.0)).expect("push");
    for h in [0.0, -1e-6] {
        let stage = Stage { phase: TrBdf2Phase::Trapezoidal, h, prev_h: 1e-6, jump: JumpPosition::None };
        let stamps = assembly.assemble_tran(&[0.8, 0.3], AssemblySolves::at_time(1e-6), stage).expect("assemble");
        let (a, b) = AssemblySolves::dense(&stamps, 2);
        assert_eq!((a, b), (vec![vec![0.0; 2]; 2], vec![0.0; 2]), "h = {h}");
    }
}

#[test]
fn a_ddt_term_stamps_the_capacitor_companion() {
    // Three accepted rows behind the iterate: lookbacks 3, 2, 1.
    let rows = [[0.10, 0.00], [0.25, 0.05], [0.45, 0.10]];
    let x = [0.70, 0.20];
    let q = |r: &[f64; 2]| C * (r[0] - r[1]);
    let (q3, q2, q1) = (q(&rows[0]), q(&rows[1]), q(&rows[2]));
    let q_now = C * (x[0] - x[1]);
    let h = 2e-6;
    for (phase, prev_h) in [
        (TrBdf2Phase::Trapezoidal, 0.0),
        (TrBdf2Phase::Trapezoidal, 1.5e-6),
        (TrBdf2Phase::Bdf2, 0.0),
        (TrBdf2Phase::Bdf2, 1.5e-6),
    ] {
        let mut assembly = DdtCircuits::capacitor();
        for r in &rows {
            assembly.push_stage(r, AssemblySolves::at_time(0.0)).expect("push");
        }
        let stamps = assembly.assemble_tran(&x, AssemblySolves::at_time(h), Stage { phase, h, prev_h, jump: JumpPosition::None }).expect("assemble");

        // C::load_transient, term for term.
        let (c0, c1, c2) = TrBdf2::stage_coeffs(phase, h, prev_h);
        let mut i_c = c0 * q_now + c1 * q1 + c2 * q2;
        if matches!(phase, TrBdf2Phase::Trapezoidal) && prev_h > 0.0 {
            let (d0, d1, d2) = TrBdf2::phase_coeffs(TrBdf2Phase::Bdf2, prev_h);
            i_c -= d0 * q1 + d1 * q2 + d2 * q3;
        }
        let rhs = c0 * q_now - i_c;
        let (a, b) = AssemblySolves::dense(&stamps, 2);
        let what = format!("{phase:?} prev_h = {prev_h}");
        let rel = 1e-12;
        AssemblySolves::close(a[0][0], c0 * C, rel, &format!("{what} (p, p)"));
        AssemblySolves::close(a[0][1], -c0 * C, rel, &format!("{what} (p, n)"));
        AssemblySolves::close(a[1][0], -c0 * C, rel, &format!("{what} (n, p)"));
        AssemblySolves::close(a[1][1], c0 * C, rel, &format!("{what} (n, n)"));
        AssemblySolves::close(b[0], rhs, rel, &format!("{what} rhs(p)"));
        AssemblySolves::close(b[1], -rhs, rel, &format!("{what} rhs(n)"));
    }
}

#[test]
fn an_rc_step_reads_one_minus_exp() {
    let m = FixtureModules::all();
    let (net, n) = Circuits::netlist(2, &["v1.f"]);
    let (top, out, f) = (&n[0], &n[1], &n[2]);
    let (r, c) = (1.0e3, 1.0e-6);
    let tau = r * c;
    let v = Circuits::instance("v1", m.voltage_source, vec![Discrete(0), Circuits::at(top), Ground, Circuits::at(f)]);
    let r1 = Circuits::instance("r1", m.resistor, vec![Discrete(1), Circuits::at(top), Circuits::at(out)]);
    let c1 = Circuits::instance("c1", m.capacitor, vec![Discrete(2), Circuits::at(out), Ground]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![v, r1, c1], &net).expect("bind"));
    assembly.set_discrete(0, &Circuits::real_bytes(1.0));
    assembly.set_discrete(1, &Circuits::real_bytes(r));
    assembly.set_discrete(2, &Circuits::real_bytes(c));

    // The initial row: the source at 1 V, the capacitor uncharged.
    let x0 = [1.0, 0.0, 0.0];
    let first = Stage { phase: TrBdf2Phase::Trapezoidal, h: tau / 100.0, prev_h: 0.0, jump: JumpPosition::None };
    let mut solver = Solver::new(&mut Tran { assembly: &mut assembly, net: &net, ambient: AssemblySolves::at_time(0.0), stage: first }, AssemblySolves::size(&net), 4)
        .expect("dry run");
    let ivs = n.iter().zip(x0).map(|(r, value)| InitialValue { reference: r.clone(), value }).collect();
    solver.push_initial_conditions(ivs);
    assembly.push_stage(&x0, AssemblySolves::at_time(0.0)).expect("push the initial row");

    let (dt, steps) = (tau / 100.0, 100);
    let (mut t, mut prev_h) = (0.0, 0.0);
    let mut x = x0.to_vec();
    for _ in 0..steps {
        let gamma = TrBdf2::GAMMA * dt;
        let trap = Stage { phase: TrBdf2Phase::Trapezoidal, h: dt, prev_h, jump: JumpPosition::None };
        AssemblySolves::solve_stage(&mut solver, &mut assembly, &net, AssemblySolves::at_time(t + gamma), trap);
        let bdf2 = Stage { phase: TrBdf2Phase::Bdf2, h: dt, prev_h, jump: JumpPosition::None };
        x = AssemblySolves::solve_stage(&mut solver, &mut assembly, &net, AssemblySolves::at_time(t + dt), bdf2).to_vec();
        t += dt;
        prev_h = dt;
    }
    let got = x[out.idx().expect("out")];
    let analytic = 1.0 - (-1.0_f64).exp();
    assert!((t - tau).abs() < 1.0e-12, "stopped at t = {t}");
    assert!((got - analytic).abs() < 1.0e-3, "v(rc) = {got} vs 1 - 1/e = {analytic}");
}

#[test]
fn a_restore_returns_histories_and_limit_state() {
    let x = [0.70, 0.20];
    let stage = Stage { phase: TrBdf2Phase::Trapezoidal, h: 2e-6, prev_h: 1e-6, jump: JumpPosition::None };
    let mut assembly = DdtCircuits::capacitor();
    assembly.push_stage(&[0.1, 0.0], AssemblySolves::at_time(0.0)).expect("push");
    assembly.push_stage(&[0.3, 0.1], AssemblySolves::at_time(1e-6)).expect("push");
    let before = assembly.assemble_tran(&x, AssemblySolves::at_time(3e-6), stage).expect("assemble");
    let snap = assembly.snapshot();

    // A rejected step: its two stages pushed rows the retry must not see.
    assembly.push_stage(&[0.9, 0.0], AssemblySolves::at_time(2e-6)).expect("push");
    assembly.push_stage(&[1.4, 0.2], AssemblySolves::at_time(3e-6)).expect("push");
    let moved = assembly.assemble_tran(&x, AssemblySolves::at_time(3e-6), stage).expect("assemble");
    assert_ne!(AssemblySolves::dense(&moved, 2).1, AssemblySolves::dense(&before, 2).1, "the pushed rows reach the companion");

    assembly.restore(&snap);
    let after = assembly.assemble_tran(&x, AssemblySolves::at_time(3e-6), stage).expect("assemble");
    assert_eq!(AssemblySolves::dense(&after, 2), AssemblySolves::dense(&before, 2), "the histories are those before the step");
    assembly.restore(&snap);
    let again = assembly.assemble_tran(&x, AssemblySolves::at_time(3e-6), stage).expect("assemble");
    assert_eq!(AssemblySolves::dense(&again, 2), AssemblySolves::dense(&before, 2), "restoring twice gives the same stamps");

    // The limit half: LAST and AGE come back to their values at the snapshot.
    let m = FixtureModules::all();
    let (net, n) = Circuits::netlist(2, &[]);
    let tr = Circuits::tagged("tr", m.tracker, 502, vec![Discrete(0), Circuits::at(&n[0]), Circuits::at(&n[1])]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![tr], &net).expect("bind"));
    assembly.accept(&[0.75, 0.25], AssemblySolves::at_rest()).expect("accept");
    let snap = assembly.snapshot();
    assembly.evaluate(&[1.0, 0.25], AssemblySolves::at_time(1e-9)).expect("evaluate");
    assembly.end_iteration();
    assembly.evaluate(&[1.25, 0.25], AssemblySolves::at_time(1e-9)).expect("evaluate");
    assembly.restore(&snap);
    assembly.evaluate(&[1.25, 0.25], AssemblySolves::at_time(2e-9)).expect("evaluate");
    let c = RecordedCall::recorded(502);
    let held = |c: &RecordedCall| (Circuits::real_of(&c.reads[2]), c.reads[3][0]);
    assert_eq!(held(&c[2]), (0.75, 2), "the rejected iteration advanced LAST");
    assert_eq!(held(&c[3]), (0.5, 1), "restored: LAST from the accepted solution, AGE 1");
}

#[test]
fn a_nonlinear_charge_through_a_limit_keeps_the_operating_point() {
    let (mut assembly, net, n) = DdtCircuits::charge_diode_circuit();
    let a = n[1].idx().expect("a");
    let x_op = AssemblySolves::solve_dc(&mut assembly, &net, &[1.0, 0.0, 0.0]).to_vec();
    let h = 1e-7;
    let first = Stage { phase: TrBdf2Phase::Trapezoidal, h, prev_h: 0.0, jump: JumpPosition::None };
    let mut solver = Solver::new(&mut Tran { assembly: &mut assembly, net: &net, ambient: AssemblySolves::at_time(0.0), stage: first }, AssemblySolves::size(&net), 4)
        .expect("dry run");
    let ivs: Vec<_> = n.iter().zip(&x_op).map(|(r, value)| InitialValue { reference: r.clone(), value: *value }).collect();
    solver.push_initial_conditions(ivs.clone());
    solver.push_initial_conditions(ivs);
    // As the transient's operating point: accepted, then its rows pushed as equilibrium rows.
    assembly.accept(&x_op, AssemblySolves::at_rest()).expect("accept the operating point");
    assembly.push_stage(&x_op, AssemblySolves::at_rest()).expect("push the operating point");
    assembly.push_stage(&x_op, AssemblySolves::at_rest()).expect("push the operating point");
    let tol = Tolerances::default();
    let (mut t, mut prev_h) = (0.0, 0.0);
    for step in 0..5 {
        let trap = Stage { phase: TrBdf2Phase::Trapezoidal, h, prev_h, jump: JumpPosition::None };
        AssemblySolves::solve_stage(&mut solver, &mut assembly, &net, AssemblySolves::at_time(t + TrBdf2::GAMMA * h), trap);
        let bdf2 = Stage { phase: TrBdf2Phase::Bdf2, h, prev_h, jump: JumpPosition::None };
        let x = AssemblySolves::solve_stage(&mut solver, &mut assembly, &net, AssemblySolves::at_time(t + h), bdf2);
        t += h;
        prev_h = h;
        let drift = (x[a] - x_op[a]).abs();
        assert!(drift <= tol.reltol * x_op[a].abs() + tol.vntol, "step {step}: v(a) = {} left the operating point {}", x[a], x_op[a]);
    }
}

#[test]
fn the_ddt_history_holds_the_charge_at_the_solution() {
    let m = FixtureModules::all();
    let stage = Stage { phase: TrBdf2Phase::Trapezoidal, h: 1e-7, prev_h: 0.0, jump: JumpPosition::None };
    let diode = || {
        let (net, n) = Circuits::netlist(1, &[]);
        let d1 = Circuits::instance("d1", m.charge_diode, vec![Discrete(0), Discrete(1), Discrete(2), Circuits::at(&n[0]), Ground, Discrete(3)]);
        let mut assembly = Assembly::new(AnalogPlan::bind(vec![d1], &net).expect("bind"));
        for (slot, value) in [(0, 1e-14), (1, 1.0), (2, 1e-6), (3, VT * Q / K_B)] {
            assembly.set_discrete(slot, &Circuits::real_bytes(value));
        }
        assembly
    };
    // The solution 0.8 V lies above the junction's critical voltage (about 0.73 V).
    let solution = [0.8];
    // Held: one evaluation from the seed leaves x̂ = vcrit short of x, and the row is pushed there.
    let mut held = diode();
    held.evaluate(&solution, AssemblySolves::at_time(0.0)).expect("evaluate");
    held.end_iteration();
    held.push_stage(&solution, AssemblySolves::at_time(0.0)).expect("push");
    // Reference: Newton iterated until the limit caught up, x̂ = x, and the row pushed there.
    let mut caught_up = diode();
    for _ in 0..4 {
        caught_up.evaluate(&solution, AssemblySolves::at_time(0.0)).expect("evaluate");
        caught_up.end_iteration();
    }
    caught_up.evaluate(&solution, AssemblySolves::at_time(0.0)).expect("evaluate");
    assert!(caught_up.limiting_active().is_empty(), "the reference limit caught up with the solution");
    caught_up.push_stage(&solution, AssemblySolves::at_time(0.0)).expect("push");
    // Both read the history at an iterate where no limit acts, so only the row differs.
    let x = [0.5];
    let (a, b) = (
        AssemblySolves::dense(&held.assemble_tran(&x, AssemblySolves::at_time(1e-7), stage).expect("assemble"), 1),
        AssemblySolves::dense(&caught_up.assemble_tran(&x, AssemblySolves::at_time(1e-7), stage).expect("assemble"), 1),
    );
    AssemblySolves::close(a.1[0], b.1[0], 1e-12, "the right side reads the history row: q(x), not its linearization");
}

#[test]
fn the_assembly_reports_each_ddt_state_after_a_step() {
    // The operating point's two equilibrium rows, then one step driven by hand: the trapezoidal
    // stage (restarted, prev_h = 0: backward Euler over γh) at x_g, the BDF2 stage at x_1. Each
    // state is the term's value at t_n, t_{n+γ}, t_{n+1} and its companion current at t_n (0: the
    // operating point is no stage) and t_{n+1}: c0·q_{n+1} + c1·q_{n+γ} + c2·q_n of
    // `phase_coeffs(Bdf2, h)` (abi.md §4.3), computed here.
    let (x_op, x_g, x_1) = ([0.50, 0.10], [0.62, 0.11], [0.80, 0.15]);
    let h = 2e-6;
    let q = |x: &[f64; 2]| C * (x[0] - x[1]);
    for (module, expected_abstol) in [(DdtCircuits::capacitor(), None), (DdtCircuits::tight_capacitor(), Some(2e-12))] {
        let mut assembly = module;
        assembly.assemble_dc(&x_op, AssemblySolves::at_rest()).expect("the equilibrium solve");
        assembly.push_stage(&x_op, AssemblySolves::at_rest()).expect("push the operating point");
        assembly.push_stage(&x_op, AssemblySolves::at_rest()).expect("push the operating point");
        let trap = Stage { phase: TrBdf2Phase::Trapezoidal, h, prev_h: 0.0, jump: JumpPosition::None };
        assembly.assemble_tran(&x_g, AssemblySolves::at_time(TrBdf2::GAMMA * h), trap).expect("trapezoidal stage");
        assembly.push_stage(&x_g, AssemblySolves::at_time(TrBdf2::GAMMA * h)).expect("push the trapezoidal stage");
        let bdf2 = Stage { phase: TrBdf2Phase::Bdf2, h, prev_h: 0.0, jump: JumpPosition::None };
        assembly.assemble_tran(&x_1, AssemblySolves::at_time(h), bdf2).expect("BDF2 stage");
        assembly.push_stage(&x_1, AssemblySolves::at_time(h)).expect("push the BDF2 stage");

        let states = assembly.ddt_states();
        assert_eq!(states.len(), 1, "one DDT term");
        let (c0, c1, c2) = TrBdf2::phase_coeffs(TrBdf2Phase::Bdf2, h);
        let i_end = c0 * q(&x_1) + c1 * q(&x_g) + c2 * q(&x_op);
        assert_eq!(states[0].q, [q(&x_op), q(&x_g), q(&x_1)], "the state at t_n, t_n+γ, t_n+1");
        assert_eq!(states[0].i, [0.0, i_end], "the current at t_n (the operating point) and t_n+1");
        assert_eq!(states[0].abstol, expected_abstol, "the term's own abstol, when it has one");
    }
}

/// The circuits the DDT tests build.
pub struct DdtCircuits;

impl DdtCircuits {
    /// A capacitor between unknowns 0 and 1.
    fn capacitor() -> Assembly {
        let m = FixtureModules::all();
        let (net, n) = Circuits::netlist(2, &[]);
        let c = Circuits::instance("c1", m.capacitor, vec![Discrete(0), Circuits::at(&n[0]), Circuits::at(&n[1])]);
        let mut assembly = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
        assembly.set_discrete(0, &Circuits::real_bytes(C));
        assembly
    }

    /// 1 V through 1 kOhm into a `ChargeDiode` (`tt` = 1 us, n·k·T/q = VT): a node `a` at about 0.63 V
    /// whose only dynamics is `ddt(tt·id)`, a nonlinear function of a limited voltage.
    fn charge_diode_circuit() -> (Assembly, piperine_analog_solver::netlist::Netlist, Vec<piperine_analog_solver::netlist::AnalogReference>) {
        let m = FixtureModules::all();
        let (net, n) = Circuits::netlist(2, &["v1.f"]);
        let (top, a, f) = (&n[0], &n[1], &n[2]);
        let v = Circuits::instance("v1", m.voltage_source, vec![Discrete(0), Circuits::at(top), Ground, Circuits::at(f)]);
        let r1 = Circuits::instance("r1", m.resistor, vec![Discrete(1), Circuits::at(top), Circuits::at(a)]);
        let d1 = Circuits::instance("d1", m.charge_diode, vec![Discrete(2), Discrete(3), Discrete(4), Circuits::at(a), Ground, Discrete(5)]);
        let mut assembly = Assembly::new(AnalogPlan::bind(vec![v, r1, d1], &net).expect("bind"));
        for (slot, value) in [(0, 1.0), (1, 1e3), (2, 1e-14), (3, 1.0), (4, 1e-6), (5, VT * Q / K_B)] {
            assembly.set_discrete(slot, &Circuits::real_bytes(value));
        }
        (assembly, net, n)
    }

    /// A capacitor with its own `abstol` of 2e-12 (pin 1) between unknowns 0 and 1.
    fn tight_capacitor() -> Assembly {
        let m = FixtureModules::all();
        let (net, n) = Circuits::netlist(2, &[]);
        let c = Circuits::instance("c1", m.tight_capacitor, vec![Discrete(0), Discrete(1), Circuits::at(&n[0]), Circuits::at(&n[1])]);
        let mut assembly = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
        assembly.set_discrete(0, &Circuits::real_bytes(C));
        assembly.set_discrete(1, &Circuits::real_bytes(2e-12));
        assembly
    }
}
