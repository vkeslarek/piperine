//! SC-04 enabler — transient full-state re-entry: an RC charge run 0→T,
//! captured at T, re-entered T→2T equals a single 0→2T run within the
//! solver's tolerance class. This is the PSS shooting seam
//! (`TransientSolver::with_initial_state`); on devices the captured step
//! carries the protocol's checkpoint (the digital engine's, the assembly's
//! histories) besides the solution that seeds the Newton history. Every
//! accepted step of a mixed run carries its digital net values and is a
//! state a transient re-enters from, the middle steps as the last. A re-entry
//! from a capture off the tick grid steps on from the tick its instant ran at:
//! a stop a tick or less later is reached once, and a stop inside that same
//! tick fails loud.

use piperine_device_fixtures::circuit;
use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::circuits::NodeMap;
use piperine_device_fixtures::mixed;
use piperine_device_fixtures::digital;
use piperine_abi::layout::{HIGH, LOW};
use piperine_solver::abi::{AnalogReference, InitialValue, ParamValue, Port};
use piperine_solver::prelude::{
    CircuitBuilder, CircuitInstance, Context, Reentry, TransientAnalysisOptions, TransientAnalysisResult, TransientStep,
};

/// τ = RC = 1 ms; T = 1 ms. Continuous 0→2T vs captured-at-T re-entry T→2T:
/// endpoints agree within the solver tolerance class (1e-9 + 1e-3·|v|), and
/// both sit near the analytic 5·(1−e^(−t/τ)).
#[test]
fn reentry_from_captured_state_matches_continuous_run() {
    let t_half = 1.0e-3;
    let read_out = |step: &piperine_solver::prelude::TransientStep, info: &NodeMap| {
        step.get_node(&info.id("out")).expect("v(out)")
    };

    // Reference: one continuous 0→2T run.
    let (mut circuit_a, info_a) = ResumedCircuit::build_rc();
    let opts_a = TransientAnalysisOptions::new(2.0 * t_half, t_half / 100.0);
    let ic_a = ResumedCircuit::discharged_ic(&info_a, &circuit_a);
    let mut solver_a = circuit_a.transient(opts_a, Context::default()).expect("tran A");
    solver_a.apply_initial_conditions(ic_a);
    let full = solver_a.solve().expect("solve A");
    let v_full_end = read_out(full.last().expect("A has steps"), &info_a);

    // Leg 1: 0→T, capture the final step.
    let (mut circuit_b, info_b) = ResumedCircuit::build_rc();
    let ic_b = ResumedCircuit::discharged_ic(&info_b, &circuit_b);
    let mut solver_b1 = circuit_b
        .transient(TransientAnalysisOptions::new(t_half, t_half / 100.0), Context::default())
        .expect("tran B1");
    solver_b1.apply_initial_conditions(ic_b);
    let leg1 = solver_b1.solve().expect("solve B1");
    let captured = leg1.last().expect("B1 has steps").clone();
    let v_captured = read_out(&captured, &info_b);
    let analytic_half = 5.0 * (1.0 - (-1.0_f64).exp());
    assert!(
        (v_captured - analytic_half).abs() < 5.0e-3,
        "capture point sanity: v(T)={v_captured} vs analytic {analytic_half}"
    );

    // Leg 2: re-enter from the captured state, T→2T on the same circuit.
    let opts_b2 = TransientAnalysisOptions::new(2.0 * t_half, t_half / 100.0).with_start(t_half);
    let mut solver_b2 = circuit_b.transient(opts_b2, Context::default()).expect("tran B2");
    solver_b2.with_initial_state(&captured);
    let leg2 = solver_b2.solve().expect("solve B2");
    let first_b2 = leg2.iter().next().expect("B2 has steps");
    assert!(
        (read_out(first_b2, &info_b) - v_captured).abs() < 1e-12,
        "re-entry starts exactly at the captured state"
    );
    let v_reentry_end = read_out(leg2.last().expect("B2 has steps"), &info_b);

    let tol = 1e-9 + 1e-3 * v_full_end.abs();
    assert!(
        (v_reentry_end - v_full_end).abs() <= tol,
        "re-entered end v={v_reentry_end} vs continuous v={v_full_end} (tol {tol})"
    );
    let analytic_end = 5.0 * (1.0 - (-2.0_f64).exp());
    assert!(
        (v_reentry_end - analytic_end).abs() < 1.0e-2,
        "analytic sanity: v(2T)={v_reentry_end} vs {analytic_end}"
    );
}

/// A host write made between the capture and the re-entry applies at the
/// re-entry: with the source set to 0 V at T the capacitor discharges from
/// the captured state, `v(2T) = v(T)·e^(−1)`.
#[test]
fn a_host_write_since_the_capture_applies_at_the_reentry() {
    let t_half = 1.0e-3;
    let (mut circuit, info) = ResumedCircuit::build_rc();
    let ic = ResumedCircuit::discharged_ic(&info, &circuit);
    let mut leg1 = circuit
        .transient(TransientAnalysisOptions::new(t_half, t_half / 100.0), Context::default())
        .expect("tran 1");
    leg1.apply_initial_conditions(ic);
    let captured = leg1.solve().expect("solve 1").last().expect("steps").clone();
    let v_captured = captured.get_node(&info.id("out")).expect("v(out)");

    circuit.set("v1", "dc", piperine_solver::abi::ParamValue::Real(0.0)).expect("host write");
    let opts = TransientAnalysisOptions::new(2.0 * t_half, t_half / 100.0).with_start(t_half);
    let mut leg2 = circuit.transient(opts, Context::default()).expect("tran 2");
    leg2.with_initial_state(&captured);
    let v_end = leg2.solve().expect("solve 2").last().expect("steps").get_node(&info.id("out")).expect("v(out)");
    let analytic = v_captured * (-1.0_f64).exp();
    assert!((v_end - analytic).abs() < 1.0e-2, "discharge from the captured state: v(2T)={v_end} vs {analytic}");
}

/// HYG-12 AC 1 (Q6 a): a transient that starts later and has no captured step solves the
/// operating point at its start time: a ramp source `v = 1e3·t` reads 2 V at t = 2 ms, and the
/// first recorded step is at 2 ms.
#[test]
fn a_late_start_solves_the_operating_point_at_its_start() {
    let start = 2.0e-3;
    let mut b = CircuitBuilder::new("ramp");
    let gnd = b.ground();
    let src = b.node("src");
    b.device(circuit::RampSource::module(), "ramp", &[], &[Port::Node(src.clone()), Port::Node(gnd.clone())]);
    CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &src, &gnd);
    let mut circuit = b.build().expect("build");
    let opts = TransientAnalysisOptions::new(start + 1.0e-3, 1.0e-4).with_start(start);
    let result = circuit.transient(opts, Context::default()).expect("tran").solve().expect("a late start solves");
    let first = result.iter().next().expect("steps");
    assert_eq!(first.time(), start, "the first recorded step is at the start time");
    let v = first.get_node(&CircuitBuilding::node_id(&src)).expect("v(src)");
    assert!((v - 2.0).abs() < 1.0e-6, "the operating point reads the time 2 ms: {v}");
    let v_end = result.last().expect("steps").get_node(&CircuitBuilding::node_id(&src)).expect("v(src)");
    assert!((v_end - 3.0).abs() < 1.0e-6, "the ramp continues from there: {v_end}");
}

/// A step the host built carries no state of the device instances, so a re-entry from it fails
/// loud (the second half of the former `a_late_start_needs_a_step_a_transient_recorded`).
#[test]
fn a_step_the_host_built_cannot_be_reentered() {
    let t_half = 1.0e-3;
    let (mut circuit, _info) = ResumedCircuit::build_rc();
    let opts = TransientAnalysisOptions::new(2.0 * t_half, t_half / 100.0).with_start(t_half);
    let foreign = TransientStep::new(t_half, std::collections::HashMap::new());
    let mut solver = circuit.transient(opts, Context::default()).expect("tran");
    solver.with_initial_state(&foreign);
    let err = solver.solve().expect_err("no device state on a step built by the host");
    assert!(err.to_string().contains("carries no state of the device instances"), "{err}");
}

/// HYG-12 AC 2: the clock of `build_clocked_rc` (first edge `HALF` after its `START`) started at
/// `S` runs its `init` commits and `START` at `(S, 0)`: no instant before `S`, its first edge at
/// `S + HALF`.
#[test]
fn a_late_start_runs_no_instant_before_it() {
    let start = 3.0e-4;
    let (mut circuit, _c) = ClockedReentry::build_clocked_rc();
    let opts = TransientAnalysisOptions::new(start + STOP, 1.0e-5).with_start(start);
    let result = circuit.transient(opts, Context::default()).expect("tran").solve().expect("solve");
    assert_eq!(result.iter().next().expect("steps").time(), start, "the run starts at S");
    let mut edges = 0;
    for step in result.iter() {
        assert!(step.time() >= start, "a step at {:e} s precedes the start", step.time());
        let (clk, n) = ClockedReentry::expected_at(step.time() - start);
        assert_eq!(step.digital_named("code"), Some(&clk[..]), "code at {:e} s (S + {:e})", step.time(), step.time() - start);
        assert_eq!(mixed::MixedParts::count_of(step.digital_named("n").expect("n")), n, "edges counted by {:e} s", step.time());
        edges = edges.max(n);
    }
    assert!(edges >= 4, "the clock toggled from S + HALF on: {edges} edges");
}

/// HYG-12 AC 3: a start that is not finite, or is negative, fails before the operating point,
/// naming the value; the circuit still runs after.
#[test]
fn a_start_that_is_not_finite_fails_before_the_operating_point() {
    let (mut circuit, _c) = ClockedReentry::build_clocked_rc();
    for (start, shown) in [(f64::NAN, "NaN"), (f64::INFINITY, "inf"), (-1.0e-3, "-1e-3")] {
        let opts = TransientAnalysisOptions::new(STOP, 1.0e-5).with_start(start);
        let err = circuit.transient(opts, Context::default()).expect("tran").solve().expect_err("a bad start");
        let text = err.to_string();
        assert!(text.contains("start time") && text.contains(shown), "{text}");
    }
    let after = ClockedReentry::run_clocked(&mut circuit, None);
    assert!(after.len() > 20, "the circuit still runs: {} steps", after.len());
}

/// Re-entered at τ (tick fraction .92), a stop 1 fs and 0.5 fs later is
/// reached by one step that integrates, recorded once, on 1 − e^(−t/τ).
#[test]
fn an_off_grid_reentry_reaches_a_stop_within_the_next_tick_once() {
    let (mut circuit, out, captured) = ResumedCircuit::rc_captured_at_tau();
    let tau = captured.time();
    for stop in [tau + 1.0e-15, tau + 0.5e-15, tau + 2.0e-15] {
        let leg = ResumedCircuit::reenter(&mut circuit, &captured, stop).unwrap_or_else(|e| panic!("stop τ + {:e} s: {e}", stop - tau));
        let times: Vec<f64> = leg.iter().map(|s| s.time()).collect();
        assert_eq!(times.first(), Some(&tau), "the run starts at the capture");
        for pair in times.windows(2) {
            assert!(pair[1] > pair[0], "records at {:e} then {:e}: {times:?}", pair[0], pair[1]);
        }
        assert_eq!(times.iter().filter(|t| **t == stop).count(), 1, "one record at stop τ + {:e} s: {times:?}", stop - tau);
        assert_eq!(times.last(), Some(&stop), "the run ends at the stop: {times:?}");
        let v = leg.last().expect("steps").get_node(&CircuitBuilding::node_id(&out)).expect("v(out)");
        let analytic = 1.0 - (-stop / tau).exp();
        assert!((v - analytic).abs() < 1.0e-3, "v(out) at τ + {:e} s = {v} vs {analytic}", stop - tau);
    }
}

/// A stop after the capture but inside the tick its instant ran at has no
/// step to reach it: the run fails loud instead of returning the capture alone.
#[test]
fn a_stop_inside_the_reentry_tick_fails_loud() {
    let (mut circuit, _out, captured) = ResumedCircuit::rc_captured_at_tau();
    let stop = captured.time() + 0.05e-15;
    assert!(stop > captured.time());
    let err = ResumedCircuit::reenter(&mut circuit, &captured, stop).expect_err("no step reaches a stop inside the tick");
    assert!(err.to_string().contains("no step reaches it"), "{err}");
}

// ---------------------------------------------------------------------------
// A mixed circuit: a 100 µs half-period clock drives a DAC into an RC (τ = 100
// µs) and a counter of its rising edges. Every step is a full circuit state.
// ---------------------------------------------------------------------------

const HALF: f64 = 1.0e-4;
const STOP: f64 = 1.0e-3;

#[test]
fn every_step_carries_the_digital_net_values() {
    let (mut circuit, _c) = ClockedReentry::build_clocked_rc();
    let result = ClockedReentry::run_clocked(&mut circuit, None);
    assert!(result.len() > 20, "{} steps", result.len());
    let mut highs = 0;
    for step in result.iter() {
        let (clk, n) = ClockedReentry::expected_at(step.time());
        let nets = step.digital_nets().unwrap_or_else(|| panic!("the step at {:e} s carries no digital values", step.time()));
        assert!(nets.names().iter().any(|name| name == "code"), "{:?}", nets.names());
        assert_eq!(step.digital_named("code"), Some(&clk[..]), "code at {:e} s", step.time());
        assert_eq!(mixed::MixedParts::count_of(step.digital_named("n").expect("n")), n, "edges counted by {:e} s", step.time());
        highs += usize::from(clk == HIGH);
    }
    assert!(highs > 0 && highs < result.len(), "the clock toggles over the run");
}

#[test]
fn reentry_from_any_captured_step_matches_the_continuous_run() {
    let (mut circuit, c) = ClockedReentry::build_clocked_rc();
    let full = ClockedReentry::run_clocked(&mut circuit, None);
    let last = full.last().expect("steps");
    let v_full = last.get_node(&CircuitBuilding::node_id(&c)).expect("v(c)");
    let n_full = mixed::MixedParts::count_of(last.digital_named("n").expect("n"));
    assert_eq!(n_full, 5, "rising edges at 1, 3, 5, 7 and 9 x 100 µs");

    // Middle steps, on both clock phases, and the first and the last.
    let picks: Vec<usize> = (0..full.len()).step_by(7).chain([full.len() - 1]).collect();
    assert!(picks.len() > 5);
    for &i in &picks {
        let captured = full.get(i).unwrap().clone();
        let leg = ClockedReentry::run_clocked(&mut circuit, Some(&captured));
        let first = leg.iter().next().expect("the re-entered run has steps");
        assert_eq!(first.time(), captured.time());
        assert_eq!(first.get_node(&CircuitBuilding::node_id(&c)), captured.get_node(&CircuitBuilding::node_id(&c)), "re-entry starts at the captured state");
        assert_eq!(first.digital_nets(), captured.digital_nets(), "and at its digital values");
        let end = leg.last().expect("steps");
        assert_eq!(end.time(), last.time());
        assert_eq!(end.digital_named("code"), last.digital_named("code"), "re-entered at step {i} ({:e} s)", captured.time());
        assert_eq!(mixed::MixedParts::count_of(end.digital_named("n").expect("n")), n_full, "re-entered at step {i} ({:e} s)", captured.time());
        let v = end.get_node(&CircuitBuilding::node_id(&c)).expect("v(c)");
        let tol = 1e-9 + 1e-3 * v_full.abs();
        assert!((v - v_full).abs() <= tol, "re-entered at step {i} ({:e} s): v(c) = {v} vs {v_full} (tol {tol})", captured.time());
    }
}

#[test]
fn a_reentry_from_every_recorded_step_continues_bit_for_bit() {
    let (mut circuit, _c) = ClockedReentry::build_clocked_rc();
    let full = ClockedReentry::run_clocked(&mut circuit, None);
    assert!(full.len() > 20, "{} steps", full.len());
    for i in 0..full.len() {
        let captured = full.get(i).expect("a recorded step").clone();
        let leg = ClockedReentry::run_clocked(&mut circuit, Some(&captured));
        BitComparison::assert_bit_for_bit(&full, &leg, i);
    }
}

/// The step at an edge of the clock is the first after a jump's delivery: its state holds the
/// restart (first-order stage, the jump estimate, the step size), and the run re-entered there takes it.
#[test]
fn a_reentry_after_a_jump_takes_the_same_restart() {
    let (mut circuit, _c) = ClockedReentry::build_clocked_rc();
    let full = ClockedReentry::run_clocked(&mut circuit, None);
    let edges: Vec<usize> = (0..full.len()).filter(|&i| i > 0 && (full.get(i).unwrap().time() / HALF).fract() == 0.0).collect();
    assert!(edges.len() >= 8, "the run records every edge of the clock: {edges:?}");
    for i in edges {
        let captured = full.get(i).expect("a recorded step").clone();
        let leg = ClockedReentry::run_clocked(&mut circuit, Some(&captured));
        BitComparison::assert_bit_for_bit(&full, &leg, i);
    }
}

/// An RC whose steps the controller shapes (no events): what a re-entry carries besides the
/// solution, the Newton history the predictor reads and the controller's memory, decides every
/// later time, so a re-entry that dropped either one leaves the grid.
#[test]
fn the_reentry_state_holds_the_newton_history_and_the_controller() {
    let t_half = 1.0e-3;
    let (mut circuit, info) = ResumedCircuit::build_rc();
    let run = |circuit: &mut CircuitInstance, from: Option<&TransientStep>| {
        let mut opts = TransientAnalysisOptions::new(2.0 * t_half, t_half / 100.0);
        if let Some(step) = from {
            opts = opts.with_start(step.time());
        }
        let ic = ResumedCircuit::discharged_ic(&info, circuit);
        let mut solver = circuit.transient(opts, Context::default()).expect("tran");
        match from {
            Some(step) => solver.with_initial_state(step),
            None => solver.apply_initial_conditions(ic),
        }
        solver.solve().expect("solve")
    };
    let full = run(&mut circuit, None);
    assert!(full.len() > 30, "{} steps", full.len());
    let sizes: std::collections::BTreeSet<u64> = full.iter().zip(full.iter().skip(1)).map(|(a, b)| (b.time() - a.time()).to_bits()).collect();
    assert!(sizes.len() > 5, "the controller changes the step size ({} distinct sizes)", sizes.len());
    for i in (1..full.len()).step_by(3) {
        let captured = full.get(i).expect("a recorded step").clone();
        let leg = run(&mut circuit, Some(&captured));
        BitComparison::assert_bit_for_bit(&full, &leg, i);
    }
}

/// Edge case 3 (§16.4): a re-entry whose stop is not the captured run's continues from the capture
/// and reaches the new stop; the old stop is its first time point. It is not required to equal a run
/// that never stopped.
#[test]
fn a_reentry_with_another_stop_lands_on_the_old_stop() {
    let (mut circuit, c) = ClockedReentry::build_clocked_rc();
    let old_stop = 4.5e-4;
    let mut leg1 = circuit.transient(TransientAnalysisOptions::new(old_stop, 1.0e-5), Context::default()).expect("tran 1");
    let captured = leg1.solve().expect("solve 1").last().expect("steps").clone();
    assert_eq!(captured.time(), old_stop, "the first run ends at its stop");

    let full = ClockedReentry::run_clocked(&mut circuit, None);
    let opts = TransientAnalysisOptions::new(STOP, 1.0e-5).with_start(old_stop);
    let mut leg2 = circuit.transient(opts, Context::default()).expect("tran 2");
    leg2.with_initial_state(&captured);
    let leg2 = leg2.solve().expect("solve 2");
    let times: Vec<f64> = leg2.iter().map(|s| s.time()).collect();
    assert_eq!(times.first(), Some(&old_stop), "the old stop is the first time point");
    assert_eq!(times.last(), Some(&STOP), "the run ends at the new stop");
    for pair in times.windows(2) {
        assert!(pair[1] > pair[0], "times increase: {times:?}");
    }
    let (end, want) = (leg2.last().unwrap(), full.last().unwrap());
    assert_eq!(end.digital_named("code"), want.digital_named("code"));
    let (v, v_full) = (end.get_node(&CircuitBuilding::node_id(&c)).expect("v(c)"), want.get_node(&CircuitBuilding::node_id(&c)).expect("v(c)"));
    assert!((v - v_full).abs() <= 1e-9 + 1e-3 * v_full.abs(), "v(c) = {v} vs {v_full} of the run that never stopped");
}

#[test]
fn reentry_last_step_attaches_state_to_the_last_step_only() {
    let (mut circuit, _c) = ClockedReentry::build_clocked_rc();
    let every = ClockedReentry::run_clocked_with(&mut circuit, Reentry::EveryStep);
    let last_only = ClockedReentry::run_clocked_with(&mut circuit, Reentry::LastStep);
    assert!(last_only.len() > 20, "{} steps", last_only.len());
    // The option changes the state kept, not the run.
    BitComparison::assert_bit_for_bit(&every, &last_only, 0);
    for i in 0..last_only.len() - 1 {
        let step = last_only.get(i).expect("a recorded step").clone();
        let err = ClockedReentry::reenter_clocked(&mut circuit, &step).err().unwrap_or_else(|| panic!("step {i} (t = {:e} s) carries a re-entry state under LastStep", step.time()));
        assert!(err.to_string().contains("carries no state of the device instances"), "step {i}: {err}");
    }
    let last = last_only.last().expect("steps").clone();
    let leg = ClockedReentry::reenter_clocked(&mut circuit, &last).expect("the last step is re-entered");
    assert_eq!(leg.iter().next().expect("steps").time(), last.time(), "the re-entry starts at the last step");
    assert_eq!(leg.last().expect("steps").time(), STOP + 1.0e-4);
}

#[test]
fn the_default_reentry_keeps_every_step_state() {
    assert_eq!(TransientAnalysisOptions::new(STOP, 1.0e-5).reentry, Reentry::EveryStep);
    let (mut circuit, _c) = ClockedReentry::build_clocked_rc();
    let result = ClockedReentry::run_clocked(&mut circuit, None);
    for i in 0..result.len() {
        let step = result.get(i).expect("a recorded step").clone();
        ClockedReentry::reenter_clocked(&mut circuit, &step).unwrap_or_else(|e| panic!("step {i} (t = {:e} s) has no re-entry state by default: {e}", step.time()));
    }
}

/// Edge case 4: a run asked to re-enter from a middle step of a `LastStep` run fails as a step no
/// transient recorded state for does today.
#[test]
fn a_middle_step_under_last_step_cannot_be_reentered() {
    let (mut circuit, _c) = ClockedReentry::build_clocked_rc();
    let result = ClockedReentry::run_clocked_with(&mut circuit, Reentry::LastStep);
    let middle = result.get(result.len() / 2).expect("a middle step").clone();
    let err = ClockedReentry::reenter_clocked(&mut circuit, &middle).expect_err("a middle step holds no state under LastStep");
    assert!(err.to_string().contains("carries no state of the device instances"), "{err}");
}

/// The RC circuit a transient resumes on: charged, captured and re-entered.
struct ResumedCircuit;

impl ResumedCircuit {
    /// `v(out) = 0` initial condition (discharged capacitor) so the runs show
    /// the charge transient instead of the DC steady state.
    fn discharged_ic(
        info: &NodeMap,
        _circuit: &CircuitInstance,
    ) -> Vec<InitialValue<piperine_solver::abi::AnalogReference, f64>> {
        let reference = info.get("out").clone();
        vec![InitialValue { reference, value: 0.0 }]
    }

    /// The RC charge circuit: 5 V source, 1 kOhm, 1 uF to ground.
    fn build_rc() -> (CircuitInstance, NodeMap) {
        let mut b = CircuitBuilder::new("rc");
        let gnd = b.ground();
        let top = b.node("top");
        let out = b.node("out");
        CircuitBuilding::add_voltage_source(&mut b, "v1", 5.0, &top, &gnd);
        CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &top, &out);
        CircuitBuilding::add_capacitor(&mut b, "c1", 1.0e-6, &out, &gnd);
        let circuit = b.build().expect("build");
        let mut map = NodeMap::new();
        map.insert("out", out);
        (circuit, map)
    }

    /// An RC with τ = 1 kOhm x 1.5915494309189535 nF, 0.92 of a 1 fs tick past
    /// tick 1591549430: a run stopped at τ ends there, off the grid, and its last
    /// step is the capture.
    fn rc_captured_at_tau() -> (CircuitInstance, AnalogReference, TransientStep) {
        let (r, c) = (1.0e3, 1.5915494309189535e-9);
        let tau: f64 = r * c;
        let fraction = (tau / 1.0e-15).fract();
        assert!((fraction - 0.92).abs() < 0.01, "τ lies 0.92 of a tick past the grid: {fraction}");
        let mut b = CircuitBuilder::new("rc");
        let gnd = b.ground();
        let top = b.node("top");
        let out = b.node("out");
        CircuitBuilding::add_voltage_source(&mut b, "v1", 1.0, &top, &gnd);
        CircuitBuilding::add_resistor(&mut b, "r1", r, &top, &out);
        CircuitBuilding::add_capacitor(&mut b, "c1", c, &out, &gnd);
        let mut circuit = b.build().expect("build");
        let mut leg = circuit.transient(TransientAnalysisOptions::new(tau, tau / 400.0), Context::default()).expect("tran");
        leg.apply_initial_conditions(vec![InitialValue { reference: out.clone(), value: 0.0 }]);
        let captured = leg.solve().expect("solve").last().expect("steps").clone();
        assert_eq!(captured.time(), tau, "the run stops at τ itself");
        (circuit, out, captured)
    }

    /// The reenter helper `reenter`.
    fn reenter(circuit: &mut CircuitInstance, captured: &TransientStep, stop: f64) -> piperine_solver::prelude::Result<TransientAnalysisResult> {
        let opts = TransientAnalysisOptions::new(stop, captured.time() / 400.0).with_start(captured.time());
        let mut solver = circuit.transient(opts, Context::default()).expect("tran");
        solver.with_initial_state(captured);
        solver.solve()
    }

    /// The net helper `net`.
    fn net(name: &str) -> Port {
        Port::Net(name.to_string())
    }
}

/// The clocked RC circuit a transient re-enters, and what it is expected to give.
struct ClockedReentry;

impl ClockedReentry {
    /// The build clocked rc helper `build_clocked_rc`.
    fn build_clocked_rc() -> (CircuitInstance, AnalogReference) {
        let mut b = CircuitBuilder::new("clock -> dac -> rc");
        let gnd = b.ground();
        let o = b.node("o");
        let c = b.node("c");
        b.device(digital::Timer::module(), "clk", &[("half", ParamValue::Real(HALF))], &[ResumedCircuit::net("code")]);
        b.device(digital::Counter::module(), "cnt", &[], &[ResumedCircuit::net("code"), ResumedCircuit::net("n")]);
        b.device(mixed::BitDigitalToAnalog::module(), "dac", &[("vref", ParamValue::Real(1.0))], &[ResumedCircuit::net("code"), Port::Node(o.clone())]);
        CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &o, &c);
        CircuitBuilding::add_capacitor(&mut b, "c1", 1.0e-7, &c, &gnd);
        (b.build().expect("build"), c)
    }

    /// The run clocked helper `run_clocked`.
    fn run_clocked(circuit: &mut CircuitInstance, from: Option<&TransientStep>) -> TransientAnalysisResult {
        let mut opts = TransientAnalysisOptions::new(STOP, 1.0e-5);
        if let Some(step) = from {
            opts = opts.with_start(step.time());
        }
        let mut solver = circuit.transient(opts, Context::default()).expect("tran");
        if let Some(step) = from {
            solver.with_initial_state(step);
        }
        solver.solve().expect("solve")
    }

    /// The clock's value and the rising edges counted at a step's time, after the
    /// instant at that time ran (edges at every multiple of `HALF`, the first HIGH).
    fn expected_at(t: f64) -> ([u8; 1], u32) {
        let ticks = (t / 1.0e-15).round() as u64;
        let k = ticks / (HALF / 1.0e-15).round() as u64;
        let clk = if k % 2 == 1 { HIGH } else { LOW };
        (clk, k.div_ceil(2) as u32)
    }

    // ---------------------------------------------------------------------------
    // Exact re-entry (HYG-04): a run re-entered from any step it recorded, with the same options and
    // stop, takes every later step the continuous run took, bit for bit.
    // ---------------------------------------------------------------------------

    /// The run clocked with helper `run_clocked_with`.
    fn run_clocked_with(circuit: &mut CircuitInstance, reentry: Reentry) -> TransientAnalysisResult {
        let opts = TransientAnalysisOptions::new(STOP, 1.0e-5).with_reentry(reentry);
        circuit.transient(opts, Context::default()).expect("tran").solve().expect("solve")
    }

    /// The run `from` steps through the state `captured` carries, until `STOP`, or its failure.
    fn reenter_clocked(circuit: &mut CircuitInstance, captured: &TransientStep) -> piperine_solver::prelude::Result<TransientAnalysisResult> {
        let opts = TransientAnalysisOptions::new(STOP + 1.0e-4, 1.0e-5).with_start(captured.time());
        let mut solver = circuit.transient(opts, Context::default()).expect("tran");
        solver.with_initial_state(captured);
        solver.solve()
    }
}

/// Two runs equal bit for bit.
struct BitComparison;

impl BitComparison {
    /// Every accepted time and every unknown of `leg`, the run re-entered at step `from` of `full`,
    /// against `full` from that step on: `f64::to_bits` equal.
    fn assert_bit_for_bit(full: &TransientAnalysisResult, leg: &TransientAnalysisResult, from: usize) {
        assert_eq!(leg.len(), full.len() - from, "re-entered at step {from}: the same number of steps from there");
        for (k, (got, want)) in leg.iter().zip(full.iter().skip(from)).enumerate() {
            assert_eq!(got.time().to_bits(), want.time().to_bits(), "re-entered at step {from}, step +{k}: time {:e} vs {:e}", got.time(), want.time());
            assert_eq!(got.values().len(), want.values().len(), "re-entered at step {from}, step +{k}: the same unknowns");
            for (variable, want_value) in want.values() {
                let got_value = got.get(variable.clone()).unwrap_or_else(|| panic!("re-entered at step {from}, step +{k}: no {variable:?}"));
                assert_eq!(
                    got_value.to_bits(),
                    want_value.to_bits(),
                    "re-entered at step {from}, step +{k} (t = {:e}): {variable:?} = {got_value:e} vs {want_value:e}",
                    want.time()
                );
            }
        }
    }

    // ---------------------------------------------------------------------------
    // `Reentry::LastStep` (HYG-13): the state a run re-enters from is kept on the last step only.
    // ---------------------------------------------------------------------------
}
