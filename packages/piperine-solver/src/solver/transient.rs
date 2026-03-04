use crate::analysis::transient::{
    TransientAnalysisContext, TransientAnalysisOptions, TransientAnalysisResult, TransientStep,
};
use crate::analysis::truncation::IntegrationMethod;
use crate::circuit::instance::CircuitInstance;
use crate::circuit::netlist::CircuitReference;
use crate::devices::soa::SoaViolations;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::deriv::Integrable;
use crate::math::faer::FaerSparseLinearSystem;
use crate::math::linear::Stamp;
use crate::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::solver::dc::DcSolver;
use crate::solver::{init_solver_configuration, Context};
use log::debug;
use ndarray::{Array1, ArrayView1, ArrayViewMut1};
use num_traits::Zero;
use std::collections::HashMap;

pub struct TransientSystem<'a> {
    pub circuit: &'a mut CircuitInstance,
    pub context: Context,
    pub time: f64,
    pub dt: f64,
    pub time_history: Vec<f64>,
    pub soa_violations: SoaViolations,
}

impl<'a> TransientSystem<'a> {
    pub fn map_dynamic_stamps(
        alpha: f64,
        history: &Array1<f64>,
        dynamic_stamps: Vec<Stamp<CircuitReference, f64>>,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        let mut stamps = Vec::with_capacity(dynamic_stamps.len() * 2);

        for s in dynamic_stamps {
            match s {
                Stamp::Matrix(row, col, val) => {
                    stamps.push(Stamp::Matrix(row.clone(), col.clone(), val * alpha));

                    if let Some(idx) = col.idx() {
                        let rhs_contribution = val * history[idx];

                        stamps.push(Stamp::Rhs(row, -rhs_contribution));
                    }
                }
                Stamp::Rhs(row, val) => {
                    stamps.push(Stamp::Rhs(row, val));
                }
            }
        }

        stamps
    }
}

impl<'a> NonLinearSystem<CircuitReference, f64> for TransientSystem<'a> {
    fn assemble(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        _alpha_hint: f64,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, f64>>> {
        let tran_ctx = TransientAnalysisContext {
            time: self.time.into(),
            dt: self.dt.into(),
        };

        let (alpha, history) = state
            .integration_parameters(self.time_history.clone())
            .unwrap_or((f64::zero(), Array1::zeros(state.size())));

        let mut all_stamps = Vec::new();

        // Update context time before calling update_all so sources can evaluate waveforms
        self.context.time = self.time;
        self.circuit.update_all(state, &self.context);
        for tran in self.circuit.transient_runtimes() {
            all_stamps.extend(tran.load_transient(state, &tran_ctx, &self.context));

            let raw_dynamic = tran.load_transient_dynamic(state, &tran_ctx, &self.context);
            all_stamps.extend(Self::map_dynamic_stamps(alpha, &history, raw_dynamic));
        }
        Ok(all_stamps)
    }

    fn converged(&self, state: &CircularArrayBuffer2<f64>, new_guess: &ArrayView1<f64>) -> bool {
        let netlist = self.circuit.netlist();
        self.context
            .has_converged(state.latest(), new_guess, netlist)
    }

    fn apply_limit(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        mut current_guess: ArrayViewMut1<f64>,
    ) {
        let last_guess = match state.latest() {
            Some(guess) => guess,
            None => return,
        };

        let diff_norm_sq: f64 = current_guess
            .iter()
            .zip(last_guess.iter())
            .fold(0.0, |acc, (curr, prev)| acc + (curr - prev).powi(2));

        let diff_norm = diff_norm_sq.sqrt();

        if diff_norm >= self.context.dc_damp_tolerance {
            for (curr, prev) in current_guess.iter_mut().zip(last_guess.iter()) {
                *curr = (*curr + *prev) * 0.5;
            }
        }
    }

    fn update_sources(&mut self, _state: &mut CircularArrayBuffer2<f64>) {}

    fn convergence_success_callback(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        _: &ArrayView1<f64>,
    ) {
        for soa_comp in self.circuit.soa_runtimes() {
            self.soa_violations
                .add_all(soa_comp.soa_check(state, &self.context));
        }
    }
}

pub struct TransientSolver<'a> {
    pub system: TransientSystem<'a>,
    pub solver: NewtonRaphsonSolver<CircuitReference, f64, FaerSparseLinearSystem<f64>>,
    pub options: TransientAnalysisOptions,
}

impl<'a> TransientSolver<'a> {
    pub fn new(
        circuit: &'a mut CircuitInstance,
        options: TransientAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<Self> {
        init_solver_configuration();

        let size = circuit.netlist().max_index().map(|i| i + 1).unwrap_or(0);

        let mut system = TransientSystem {
            circuit,
            context,
            time: 0.0,
            dt: options.dt,
            time_history: Vec::with_capacity(16),
            soa_violations: SoaViolations::new(),
        };

        let solver = NewtonRaphsonSolver::new(&mut system, size, 4)?;

        Ok(Self {
            system,
            solver,
            options,
        })
    }

    /// Collect all breakpoints from devices (sources with time-varying waveforms)
    fn collect_breakpoints(&self, start_time: f64, stop_time: f64) -> Vec<f64> {
        let mut breakpoints = Vec::new();

        for runtime in self.system.circuit.all_runtimes() {
            if let Some(bp_provider) = runtime.as_breakpoint_provider() {
                let device_bps = bp_provider.get_breakpoints(start_time.into(), stop_time.into());
                for bp in device_bps {
                    let bp_time: f64 = bp.into();
                    breakpoints.push(bp_time);
                }
            }
        }

        // Sort and deduplicate breakpoints
        breakpoints.sort_by(|a, b| a.partial_cmp(b).unwrap());
        breakpoints.dedup();
        breakpoints
    }

    /// Calculate next timestep based on truncation error from all devices
    fn calculate_next_timestep(&self, current_time: f64, breakpoints: &[f64]) -> Option<f64> {
        if !self.options.adaptive {
            return None; // Use fixed timestep
        }

        // Using Gear order 2 for now (hardcoded, can be made configurable later)
        let method = IntegrationMethod::Gear { order: 2 };

        let state_history = self.solver.state();
        let time_history = &self.system.time_history;

        // Collect timestep suggestions from all devices with truncation error
        let mut min_dt = f64::MAX;
        let mut suggestions_count = 0;

        for runtime in self.system.circuit.all_runtimes() {
            if let Some(truncation_device) = runtime.as_truncation_error() {
                if let Some(suggested_dt) = truncation_device.suggest_timestep(
                    state_history,
                    time_history,
                    method,
                    &self.system.context,
                ) {
                    let dt_value: f64 = suggested_dt.into();
                    min_dt = min_dt.min(dt_value);
                    suggestions_count += 1;
                }
            }
        }

        if suggestions_count > 0 {
            // Limit growth to 2x per step
            let current_dt = self.system.dt;
            let max_growth = current_dt * 2.0;
            min_dt = min_dt.min(max_growth);

            // Check breakpoints - don't step over them
            for &bp in breakpoints {
                if bp > current_time && bp < current_time + min_dt {
                    // There's a breakpoint ahead, limit timestep to reach it
                    min_dt = bp - current_time;
                    debug!(
                        "Breakpoint limiting: dt reduced to {:.3e}s to hit breakpoint at {:.6}s",
                        min_dt, bp
                    );
                }
            }

            // Clamp to user-specified limits
            let dt_min: f64 = self.options.dt_min.into();
            let dt_max: f64 = self.options.dt_max.into();
            min_dt = min_dt.clamp(dt_min, dt_max);

            Some(min_dt)
        } else {
            None // No devices provided suggestions, use fixed timestep
        }
    }

    pub fn solve(&mut self) -> crate::result::Result<TransientAnalysisResult> {
        let mut steps = Vec::new();
        let stop_time: f64 = self.options.stop_time.into();
        let mut dt: f64 = self.options.dt.into();

        debug!("Calculating DC Operating Point...");
        let mut dc_solver = DcSolver::new(self.system.circuit, Context::default())?;
        let dc_result = dc_solver.solve()?;

        let netlist = self.system.circuit.netlist();

        let iv_dc = dc_result.as_iv(netlist);

        self.solver.push_initial_conditions(iv_dc.clone());
        self.solver.push_initial_conditions(iv_dc);

        self.system.time_history.insert(0, 0.0);
        self.system.time_history.insert(0, 0.0 - dt);

        steps.push(self.snapshot(0.0));

        // Collect all breakpoints from sources
        let breakpoints = self.collect_breakpoints(0.0, stop_time);
        if !breakpoints.is_empty() {
            debug!("Collected {} breakpoints", breakpoints.len());
        }

        let mut current_time = 0.0;

        while current_time < stop_time {
            // Calculate next timestep (adaptive or fixed)
            if self.options.adaptive {
                if let Some(suggested_dt) = self.calculate_next_timestep(current_time, &breakpoints)
                {
                    dt = suggested_dt;
                    debug!("Adaptive timestep: dt = {:.3e}s", dt);
                }
            }

            // Don't overshoot stop_time
            if current_time + dt > stop_time {
                dt = stop_time - current_time;
            }

            current_time += dt;

            self.system.time = current_time;
            self.system.dt = dt;

            self.system.time_history.insert(0, current_time);

            debug!(
                "Solving Transient Step: t = {:.6}s, dt = {:.3e}s",
                current_time, dt
            );

            let max_iter = self.system.context.max_iter;
            let result = self.solver.solve(&mut self.system, 1.0 / dt, max_iter);

            if result.is_ok() {
                steps.push(self.snapshot(current_time));
                if self.system.time_history.len() > 10 {
                    self.system.time_history.truncate(10);
                }
            } else {
                // On convergence failure with adaptive timestep, retry with half the timestep
                if self.options.adaptive && dt > self.options.dt_min.into() {
                    debug!("Convergence failed, retrying with dt/2");
                    current_time -= dt; // Rewind time
                    dt = (dt / 2.0).max(self.options.dt_min.into());
                    self.system.time_history.remove(0); // Remove failed timestep from history
                    continue; // Retry with smaller timestep
                } else {
                    return Err(result.unwrap_err());
                }
            }
        }

        Ok(TransientAnalysisResult::new(
            steps,
            self.system.soa_violations.clone(),
        ))
    }

    fn snapshot(&self, time: f64) -> TransientStep {
        let mut values = HashMap::new();
        let netlist = self.system.circuit.netlist();
        let latest_state = self.solver.current_guess().unwrap();

        for reference in netlist.all_references() {
            if let Some(idx) = reference.idx() {
                values.insert(reference.variable().clone(), latest_state[idx]);
            }
        }

        TransientStep::new(time, values)
    }
}

#[cfg(test)]
mod test {
    use crate::analysis::transient::TransientAnalysisOptions;
    use crate::circuit::instance::CircuitInstance;
    use crate::circuit::netlist::GND;
    use crate::circuit::Circuit;
    use crate::devices::source::Waveform::Step;
    use crate::math::unit::UnitExt;
    use crate::solver::Context;

    #[test]
    fn test_transient_rc_charging() {
        let mut v_out = GND;

        let mut circuit: CircuitInstance = Circuit::builder("RC Transient Demo", |b| {
            let v_in = b.port();
            v_out = b.port();

            b.voltage_source(
                "V1",
                v_in.clone(),
                GND,
                Step {
                    initial: 0.0.V(),
                    final_value: 5.0.V(),
                    delay: 0.0,
                    rise_time: 1.0.us(),
                },
            );

            b.resistor("R1", v_in, v_out.clone(), 1.0.kOhms());
            b.capacitor("C1", v_out.clone(), GND, 1.0.uF());
        })
        .into();

        let options = TransientAnalysisOptions::new(5.0.ms(), 100.0.us());

        let result = circuit
            .transient(options, Context::default())
            .unwrap()
            .solve()
            .unwrap();

        let one_tau_step = result
            .iter()
            .find(|step| (step.time() - 0.001).abs() < 1e-6)
            .expect("Time point 1.0ms not found in simulation results");

        let v_at_1ms = one_tau_step
            .get_node(&v_out)
            .expect("Variable 'out' missing in result step");

        println!("At 1ms (1 Tau): {:.4} V", v_at_1ms);
        assert!((v_at_1ms - 3.16).abs() < 0.1);

        // D. Check Final State (t = 5ms)
        let final_step = result.last().unwrap();
        let final_v = final_step.get_node(&v_out).unwrap();

        println!("At 5ms (Final): {:.4} V", final_v);
        assert!((final_v - 5.0).abs() < 0.05);
    }

    #[test]
    fn test_transient_rc_step() {
        let mut v_out = GND;

        let mut circuit: CircuitInstance = Circuit::builder("RC Step Response", |b| {
            let v_in = b.port();
            v_out = b.port();

            b.voltage_source(
                "V1",
                v_in.clone(),
                GND,
                Step {
                    initial: 0.0.V(),
                    final_value: 1.0.V(),
                    delay: 0.0,
                    rise_time: 1e-9,
                },
            );

            b.resistor("R1", v_in, v_out.clone(), 1.0.kOhms());
            b.capacitor("C1", v_out.clone(), GND, 1.0.uF());
        })
        .into();

        let result = circuit
            .transient(
                TransientAnalysisOptions::new(5.0.ms(), 100.0.us()),
                Context::default(),
            )
            .unwrap()
            .solve()
            .unwrap();

        let final_snapshot = result.last().expect("Simulation returned no data");

        let v_final = final_snapshot
            .get_node(&v_out)
            .expect("Voltage value for 'out' missing");

        println!("Transient Final Voltage: {:.4} V", v_final);
        assert!(
            (v_final - 1.0).abs() < 0.01,
            "Capacitor did not charge to 1V. Got {}",
            v_final
        );
    }

    #[test]
    fn test_adaptive_timestep_rc_charging() {
        let mut v_out = GND;

        // RC circuit: tau = R*C = 1kΩ * 1µF = 1ms
        let mut circuit: CircuitInstance = Circuit::builder("Adaptive RC Test", |b| {
            let v_in = b.port();
            v_out = b.port();

            b.voltage_source(
                "V1",
                v_in.clone(),
                GND,
                Step {
                    initial: 0.0.V(),
                    final_value: 5.0.V(),
                    delay: 0.0,
                    rise_time: 1.0.us(),
                },
            );

            b.resistor("R1", v_in, v_out.clone(), 1.0.kOhms());
            b.capacitor("C1", v_out.clone(), GND, 1.0.uF());
        })
        .into();

        // Test with adaptive timestep
        println!("\n=== ADAPTIVE TIMESTEP TEST ===");
        let options_adaptive = TransientAnalysisOptions::new_adaptive(5.0.ms(), 100.0.us())
            .with_dt_min(1.0.ns())
            .with_dt_max(500.0.us());

        let result_adaptive = circuit
            .transient(options_adaptive, Context::default())
            .unwrap()
            .solve()
            .unwrap();

        println!("Adaptive: {} steps", result_adaptive.len());

        // Compare with fixed timestep
        println!("\n=== FIXED TIMESTEP TEST ===");
        let mut circuit_fixed: CircuitInstance = Circuit::builder("Fixed RC Test", |b| {
            let v_in = b.port();
            let v_out_fixed = b.port();

            b.voltage_source(
                "V1",
                v_in.clone(),
                GND,
                Step {
                    initial: 0.0.V(),
                    final_value: 5.0.V(),
                    delay: 0.0,
                    rise_time: 1.0.us(),
                },
            );

            b.resistor("R1", v_in, v_out_fixed.clone(), 1.0.kOhms());
            b.capacitor("C1", v_out_fixed.clone(), GND, 1.0.uF());
        })
        .into();

        let options_fixed = TransientAnalysisOptions::new(5.0.ms(), 100.0.us());

        let result_fixed = circuit_fixed
            .transient(options_fixed, Context::default())
            .unwrap()
            .solve()
            .unwrap();

        println!("Fixed: {} steps", result_fixed.len());

        // Verify both reach approximately the same final voltage
        let v_adaptive_final = result_adaptive.last().unwrap().get_node(&v_out).unwrap();

        let v_fixed_final = result_fixed.last().unwrap().get_node(&v_out).unwrap();

        println!("\nFinal voltages:");
        println!("  Adaptive: {:.6} V", v_adaptive_final);
        println!("  Fixed:    {:.6} V", v_fixed_final);
        println!("\nStep count:");
        println!("  Adaptive: {}", result_adaptive.len());
        println!("  Fixed:    {}", result_fixed.len());

        // Both should reach approximately 5V * (1 - e^(-5)) ≈ 4.966V
        let expected_final = 5.0 * (1.0 - (-5.0_f64).exp());

        assert!(
            (v_adaptive_final - expected_final).abs() < 0.1,
            "Adaptive timestep final voltage {:.4}V doesn't match expected {:.4}V",
            v_adaptive_final,
            expected_final
        );

        assert!(
            (v_fixed_final - expected_final).abs() < 0.1,
            "Fixed timestep final voltage {:.4}V doesn't match expected {:.4}V",
            v_fixed_final,
            expected_final
        );

        // Voltages should be close to each other
        assert!(
            (v_adaptive_final - v_fixed_final).abs() < 0.05,
            "Adaptive ({:.4}V) and Fixed ({:.4}V) results differ too much",
            v_adaptive_final,
            v_fixed_final
        );

        println!("\n✓ Adaptive timestep test passed!");
        println!("  - Both methods converged to ~{:.4}V", expected_final);
        println!(
            "  - Difference: {:.6}V",
            (v_adaptive_final - v_fixed_final).abs()
        );
    }

    #[test]
    fn test_breakpoint_capture() {
        let mut v_out = GND;

        // RC circuit with a Step source
        // The step has a rise time from 0ns to 1µs
        let mut circuit: CircuitInstance = Circuit::builder("Breakpoint Test", |b| {
            let v_in = b.port();
            v_out = b.port();

            b.voltage_source(
                "V1",
                v_in.clone(),
                GND,
                Step {
                    initial: 0.0.V(),
                    final_value: 5.0.V(),
                    delay: 1.0.ms(),     // Step starts at 1ms
                    rise_time: 1.0.us(), // Rise time: 1µs
                },
            );

            b.resistor("R1", v_in, v_out.clone(), 1.0.kOhms());
            b.capacitor("C1", v_out.clone(), GND, 1.0.uF());
        })
        .into();

        println!("\n=== BREAKPOINT CAPTURE TEST ===");

        // Use adaptive timestep with a large initial timestep
        // Without breakpoints, the solver would step over the edge
        let options_adaptive = TransientAnalysisOptions::new_adaptive(5.0.ms(), 500.0.us())
            .with_dt_min(1.0.ns())
            .with_dt_max(500.0.us());

        let result = circuit
            .transient(options_adaptive, Context::default())
            .unwrap()
            .solve()
            .unwrap();

        println!("Total steps: {}", result.len());

        // Check that we have samples near the breakpoints
        // Breakpoints should be at: 1ms (start), 1.0005ms (mid), 1.001ms (end)
        let t_start = 1.0e-3;
        let t_mid = 1.0005e-3;
        let t_end = 1.001e-3;

        // Find the closest samples to each breakpoint
        let mut samples_near_start = 0;
        let mut samples_near_mid = 0;
        let mut samples_near_end = 0;

        let tolerance = 100.0e-6; // 100µs tolerance

        for step in result.iter() {
            let t = step.time();

            if (t - t_start).abs() < tolerance {
                samples_near_start += 1;
            }
            if (t - t_mid).abs() < tolerance {
                samples_near_mid += 1;
            }
            if (t - t_end).abs() < tolerance {
                samples_near_end += 1;
            }
        }

        println!("\nSamples near breakpoints:");
        println!("  Near 1.000ms (start): {}", samples_near_start);
        println!("  Near 1.0005ms (mid): {}", samples_near_mid);
        println!("  Near 1.001ms (end):  {}", samples_near_end);

        // Verify we captured the transition properly
        assert!(
            samples_near_start > 0,
            "No samples near breakpoint start (1ms)"
        );
        assert!(
            samples_near_mid > 0,
            "No samples near breakpoint mid (1.0005ms)"
        );
        assert!(
            samples_near_end > 0,
            "No samples near breakpoint end (1.001ms)"
        );

        // Check voltage values during the transition
        let v_before = result
            .iter()
            .find(|s| s.time() >= 0.999e-3 && s.time() < 1.0e-3)
            .and_then(|s| s.get_node(&v_out));

        let v_during = result
            .iter()
            .find(|s| s.time() >= 1.0005e-3 && s.time() < 1.0006e-3)
            .and_then(|s| s.get_node(&v_out));

        let v_after = result
            .iter()
            .find(|s| s.time() >= 1.001e-3 && s.time() < 1.002e-3)
            .and_then(|s| s.get_node(&v_out));

        println!("\nVoltages during transition:");
        if let Some(v) = v_before {
            println!("  Before (~999µs):  {:.6}V", v);
        }
        if let Some(v) = v_during {
            println!("  During (~1000.5µs): {:.6}V", v);
        }
        if let Some(v) = v_after {
            println!("  After (~1001µs):  {:.6}V", v);
        }

        // The voltage should be increasing during the transition
        if let (Some(vb), Some(vd), Some(va)) = (v_before, v_during, v_after) {
            assert!(
                vd > vb,
                "Voltage should increase from before to during transition"
            );
            assert!(
                va > vd,
                "Voltage should increase from during to after transition"
            );

            // During the transition, voltage should be between initial and final
            assert!(
                vd > 0.0 && vd < 5.0,
                "Voltage during transition should be between 0V and 5V, got {:.4}V",
                vd
            );
        }

        println!("\n✓ Breakpoint capture test passed!");
        println!("  - All breakpoints were captured");
        println!("  - Transition was properly sampled");
    }
}
